//! Tracks, read while they download.
//!
//! A track is downloaded into the cache once, by one thread, however many
//! times it is opened meanwhile (Audacious's core opens a file to find its
//! decoder, then again to play it). A reader waits for the part it wants;
//! the download goes on when nobody reads, so that a track played again, or
//! the next one, is there already.
//!
//! A file served over plain HTTPS (YouTube's) is fetched a range at a time:
//! YouTube sends a whole file of more than 10 MB or so slowly (30 KB/s),
//! and each range at full speed. The range that a reader waits for, after a
//! seek, comes next. An HLS stream (SoundCloud's) is fetched a piece at a
//! time, in order; its size is known at the end. Its AAC is taken out of the
//! fragmented MP4 it comes in (adts.rs), so that it plays from the first
//! piece on.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use super::adts;
use super::cache;
use super::hls::{self, Playlist};
use super::http::Client;
use super::sites::{self, Kind};
use super::ytdlp;

const CHUNK: u64 = 2 << 20;

/// A download bigger than this stops when nobody has read it for a while:
/// a long mix that was skipped is not fetched to the end.
const KEEP_GOING_UP_TO: u64 = 48 << 20;
const UNREAD_FOR: Duration = Duration::from_secs(15);

#[derive(Default)]
struct State {
    /// Where the data is: the .part file, then the finished one.
    path: PathBuf,
    ext: String,
    hls: bool,
    /// Ranges: known after the first piece. HLS: known when done.
    size: Option<u64>,
    /// Ranges: which CHUNKs are there.
    chunks: Vec<bool>,
    /// HLS: how much there is, from the start.
    have: u64,
    /// HLS, as ADTS: about what the size will be, from how long the pieces
    /// that are there play. A decoder of plain AAC seeks by the size alone,
    /// and takes a file without one for a radio station.
    estimate: Option<u64>,
    /// Something can be read.
    ready: bool,
    done: bool,
    error: Option<String>,
    /// Where a reader waits.
    want: Option<u64>,
    readers: usize,
    unread_since: Option<Instant>,
}

impl State {
    /// How many bytes from `pos` on are there to be read.
    fn avail(&self, pos: u64) -> u64 {
        if self.done || self.hls {
            let end = if self.done { self.size.unwrap_or(0) } else { self.have };
            return end.saturating_sub(pos);
        }
        let Some(size) = self.size else { return 0 };
        let mut c = (pos / CHUNK) as usize;
        let mut end = pos;
        while c < self.chunks.len() && self.chunks[c] {
            c += 1;
            end = (c as u64 * CHUNK).min(size);
        }
        end.saturating_sub(pos)
    }

    fn at_end(&self, pos: u64) -> bool {
        match self.size {
            Some(size) => pos >= size,
            None => self.done,
        }
    }

    /// The next CHUNK to fetch: the one a reader waits for, or the first
    /// missing one after `from`, or the first missing one.
    fn next_chunk(&mut self, from: usize) -> Option<usize> {
        let missing = |s: &State, from: usize| (from..s.chunks.len()).find(|&c| !s.chunks[c]);
        if let Some(w) = self.want.take() {
            if let Some(c) = missing(self, (w / CHUNK) as usize) {
                return Some(c);
            }
        }
        missing(self, from).or_else(|| missing(self, 0))
    }

    fn abandoned(&self) -> bool {
        self.readers == 0
            && self.size.is_some_and(|s| s > KEEP_GOING_UP_TO)
            && self.unread_since.is_some_and(|t| t.elapsed() > UNREAD_FOR)
    }
}

struct Download {
    key: String,
    state: Mutex<State>,
    changed: Condvar,
}

impl Download {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The downloads that are going on, by key. Taken before a download's own
/// lock, never after.
fn active() -> MutexGuard<'static, HashMap<String, Arc<Download>>> {
    static ACTIVE: OnceLock<Mutex<HashMap<String, Arc<Download>>>> = OnceLock::new();
    ACTIVE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn start(map: &mut HashMap<String, Arc<Download>>, key: &str, uri: &str) -> Arc<Download> {
    if let Some(d) = map.get(key) {
        return d.clone();
    }
    let d = Arc::new(Download {
        key: key.to_owned(),
        state: Mutex::new(State {
            unread_since: Some(Instant::now()),
            ..State::default()
        }),
        changed: Condvar::new(),
    });
    map.insert(key.to_owned(), d.clone());
    let (dl, uri) = (d.clone(), uri.to_owned());
    let spawned = thread::Builder::new()
        .name("wmplayer web".to_owned())
        .spawn(move || {
            let mut result = run(&dl, &uri);
            // YouTube now and then hands out an address that only ever
            // answers 403: another one is asked for, once.
            let refused = matches!(&result, Err(e) if e.starts_with("HTTP "));
            if refused && !dl.lock().ready {
                ytdlp::forget(&uri);
                result = run(&dl, &uri);
            }
            if result.is_err() {
                ytdlp::forget(&uri);
            }
            finish(&dl, result);
        });
    if let Err(e) = spawned {
        map.remove(key);
        d.lock().error = Some(e.to_string());
    }
    d
}

fn finish(d: &Download, result: Result<(), String>) {
    let mut map = active();
    let mut s = d.lock();
    match result {
        Ok(()) => {
            let done = cache::path(&d.key, &s.ext);
            match fs::rename(&s.path, &done) {
                Ok(()) => s.path = done,
                Err(e) => s.error = Some(format!("{}: {e}", done.display())),
            }
            s.done = s.error.is_none();
            s.ready = true;
        }
        Err(e) => {
            let _ = fs::remove_file(&s.path);
            s.error = Some(e);
        }
    }
    map.remove(&d.key);
    d.changed.notify_all();
}

fn run(d: &Download, uri: &str) -> Result<(), String> {
    let r = ytdlp::resolve(uri)?;
    if r.live {
        return Err("live streams cannot be played".to_owned());
    }
    let hls = r.format.protocol.starts_with("m3u8");
    if !hls && !r.format.protocol.starts_with("http") {
        return Err(format!("{} streams cannot be played", r.format.protocol));
    }
    let ext: String = r
        .format
        .ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let mut client = Client::new(&r.format.headers)?;
    if hls {
        fetch_hls(d, &mut client, &ext, &r.format.url)
    } else {
        let file = create(d, &ext, false)?;
        fetch_ranges(d, &mut client, &file, &r.format.url, r.format.filesize)
    }
}

/// The .part file of the download, whose data is of the kind `ext`.
fn create(d: &Download, ext: &str, hls: bool) -> Result<File, String> {
    let part = cache::part_path(&d.key, ext);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&part)
        .map_err(|e| format!("{}: {e}", part.display()))?;
    let mut s = d.lock();
    s.path = part;
    s.ext = ext.to_owned();
    s.hls = hls;
    Ok(file)
}

fn write_at(file: &File, data: &[u8], at: u64) -> Result<(), String> {
    file.write_all_at(data, at).map_err(|e| e.to_string())
}

fn fetch_ranges(
    d: &Download,
    client: &mut Client,
    file: &File,
    url: &str,
    filesize: Option<u64>,
) -> Result<(), String> {
    let first = client.get(url, Some((0, CHUNK - 1)))?;
    let whole = first.code == 200;
    let size = match (whole, first.total, filesize) {
        (true, _, _) => first.body.len() as u64,
        (false, Some(t), _) | (false, None, Some(t)) => t,
        (false, None, None) => return Err("the size of the file is not known".to_owned()),
    };
    if !whole && first.body.len() as u64 != size.min(CHUNK) {
        return Err("the server sent less than asked for".to_owned());
    }
    file.set_len(size).map_err(|e| e.to_string())?;
    write_at(file, &first.body, 0)?;
    {
        let mut s = d.lock();
        s.size = Some(size);
        s.chunks = vec![whole; size.div_ceil(CHUNK).max(1) as usize];
        s.chunks[0] = true;
        s.ready = true;
        d.changed.notify_all();
    }

    let mut next = 1;
    loop {
        let c = {
            let mut s = d.lock();
            if s.abandoned() {
                return Err("nobody listened".to_owned());
            }
            match s.next_chunk(next) {
                Some(c) => c,
                None => return Ok(()),
            }
        };
        let (a, b) = (c as u64 * CHUNK, ((c as u64 + 1) * CHUNK).min(size) - 1);
        let r = client.get(url, Some((a, b)))?;
        if r.body.len() as u64 != b - a + 1 {
            return Err("the server sent less than asked for".to_owned());
        }
        write_at(file, &r.body, a)?;
        let mut s = d.lock();
        s.chunks[c] = true;
        d.changed.notify_all();
        next = c + 1;
    }
}

fn fetch_hls(d: &Download, client: &mut Client, ext: &str, url: &str) -> Result<(), String> {
    let mut url = url.to_owned();
    let (init, parts) = loop {
        let text = client.get(&url, None)?.body;
        match hls::parse(&url, &String::from_utf8_lossy(&text))? {
            Playlist::Master(variant) if variant != url => url = variant,
            Playlist::Master(_) => return Err("the HLS playlist points to itself".to_owned()),
            Playlist::Media { live: true, .. } => {
                return Err("live streams cannot be played".to_owned())
            }
            Playlist::Media { init, parts, .. } => break (init, parts),
        }
    };
    let init = match init {
        Some(init) => client.get(&init, None)?.body,
        None => Vec::new(),
    };
    let total: u64 = parts.iter().map(|p| p.ms).sum();
    let mut parts = parts.into_iter();
    let (first, mut played) = match parts.next() {
        Some(part) => (client.get(&part.url, None)?.body, part.ms),
        None => (Vec::new(), 0),
    };

    // AAC in fragmented MP4 is written as ADTS, if the first piece is
    // understood; anything else as it comes, the pieces one after the other.
    let mut data = Vec::new();
    let aac = adts::config(&init)
        .filter(|c| adts::frames(c, &first, &mut data).is_ok() && !data.is_empty());
    if aac.is_none() {
        data = init;
        data.extend_from_slice(&first);
    }
    drop(first);
    let file = create(d, if aac.is_some() { "aac" } else { ext }, true)?;

    let mut have = 0;
    loop {
        write_at(&file, &data, have)?;
        have += data.len() as u64;
        {
            let mut s = d.lock();
            s.have = have;
            if aac.is_some() && played > 0 {
                s.estimate = Some(have.saturating_mul(total) / played);
            }
            s.ready = true;
            d.changed.notify_all();
        }
        let Some(part) = parts.next() else { break };
        let body = client.get(&part.url, None)?.body;
        played += part.ms;
        match &aac {
            Some(c) => {
                data.clear();
                adts::frames(c, &body, &mut data)?;
            }
            None => data = body,
        }
    }
    d.lock().size = Some(have);
    Ok(())
}

enum Source {
    File(File, u64),
    Download(Arc<Download>, Option<File>),
}

/// A track, open for reading.
pub struct Stream {
    source: Source,
    pos: u64,
    ext: String,
}

fn track_key(uri: &str) -> Result<String, String> {
    match sites::classify(uri) {
        Some(p) if p.kind == Kind::Track => Ok(p.key),
        _ => Err("not a track of YouTube or SoundCloud".to_owned()),
    }
}

/// Opens the track at `uri`: from the cache, or as it downloads. Returns
/// once there is something to read, which takes some seconds for a track
/// that yt-dlp has to look up first.
pub fn open(uri: &str) -> Result<Stream, String> {
    let key = track_key(uri)?;
    let d = {
        let mut map = active();
        if !map.contains_key(&key) {
            if let Some((path, ext)) = cache::find(&key) {
                drop(map);
                let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                let size = file.metadata().map_err(|e| e.to_string())?.len();
                cache::touch(&path);
                return Ok(Stream {
                    source: Source::File(file, size),
                    pos: 0,
                    ext,
                });
            }
        }
        let d = start(&mut map, &key, uri);
        d.lock().readers += 1;
        d
    };
    let mut s = d.lock();
    while !s.ready && s.error.is_none() {
        s = d.changed.wait(s).unwrap_or_else(|e| e.into_inner());
    }
    if let Some(e) = s.error.clone() {
        s.readers -= 1;
        return Err(e);
    }
    let ext = s.ext.clone();
    drop(s);
    Ok(Stream {
        source: Source::Download(d, None),
        pos: 0,
        ext,
    })
}

/// Starts downloading the track at `uri`, unless it is there already.
pub fn prefetch(uri: &str) {
    let Ok(key) = track_key(uri) else { return };
    let mut map = active();
    if !map.contains_key(&key) && cache::find(&key).is_none() {
        start(&mut map, &key, uri);
    }
}

impl Stream {
    pub fn ext(&self) -> &str {
        &self.ext
    }

    pub fn mime(&self) -> &'static str {
        match self.ext.as_str() {
            "webm" | "weba" => "audio/webm",
            "m4a" | "mp4" | "m4s" => "audio/mp4",
            "mp3" => "audio/mpeg",
            "ogg" | "opus" | "oga" => "audio/ogg",
            "aac" => "audio/aac",
            "flac" => "audio/flac",
            "wav" => "audio/wav",
            _ => "application/octet-stream",
        }
    }

    /// Reads at most buf.len() bytes, waiting for them if they are still
    /// to come. 0 is the end.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        if buf.is_empty() {
            return Ok(0);
        }
        let pos = self.pos;
        let n = match &mut self.source {
            Source::File(file, size) => {
                if pos >= *size {
                    return Ok(0);
                }
                file.read_at(buf, pos).map_err(|e| e.to_string())?
            }
            Source::Download(d, file) => {
                let mut s = d.lock();
                let avail = loop {
                    if let Some(e) = &s.error {
                        return Err(e.clone());
                    }
                    let avail = s.avail(pos);
                    if avail > 0 {
                        break avail;
                    }
                    if s.at_end(pos) {
                        return Ok(0);
                    }
                    s.want = Some(pos);
                    s = d.changed.wait(s).unwrap_or_else(|e| e.into_inner());
                };
                // Opened here, with the lock held: the .part file is
                // renamed when it is finished.
                if file.is_none() {
                    *file = Some(File::open(&s.path).map_err(|e| format!("{}: {e}", s.path.display()))?);
                }
                drop(s);
                let len = buf.len().min(avail.min(usize::MAX as u64) as usize);
                file.as_ref().unwrap().read_at(&mut buf[..len], pos).map_err(|e| e.to_string())?
            }
        };
        self.pos += n as u64;
        Ok(n)
    }

    pub fn seek(&mut self, to: SeekFrom) -> Result<u64, String> {
        let pos = match to {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::Current(o) => self.pos.checked_add_signed(o),
            SeekFrom::End(o) => self
                .size()
                .ok_or("the size is not known yet")?
                .checked_add_signed(o),
        }
        .ok_or("seek out of range")?;
        self.pos = pos;
        Ok(pos)
    }

    pub fn tell(&self) -> u64 {
        self.pos
    }

    /// While an HLS stream downloads: None, or about what it will be if
    /// the stream is written as ADTS.
    pub fn size(&self) -> Option<u64> {
        match &self.source {
            Source::File(_, size) => Some(*size),
            Source::Download(d, _) => {
                let s = d.lock();
                s.size.or(s.estimate)
            }
        }
    }

    pub fn eof(&self) -> bool {
        match &self.source {
            Source::File(_, size) => self.pos >= *size,
            Source::Download(d, _) => {
                let s = d.lock();
                s.at_end(self.pos) && s.avail(self.pos) == 0
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Source::Download(d, _) = &self.source {
            let mut s = d.lock();
            s.readers -= 1;
            if s.readers == 0 {
                s.unread_since = Some(Instant::now());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks() {
        let mut s = State {
            size: Some(5 * CHUNK + 10),
            chunks: vec![true, true, false, true, false, false],
            ..State::default()
        };
        assert_eq!(s.avail(0), 2 * CHUNK);
        assert_eq!(s.avail(CHUNK + 5), CHUNK - 5);
        assert_eq!(s.avail(2 * CHUNK), 0);
        assert_eq!(s.avail(3 * CHUNK), CHUNK);
        assert_eq!(s.next_chunk(1), Some(2));
        s.want = Some(4 * CHUNK + 1);
        assert_eq!(s.next_chunk(1), Some(4));
        assert_eq!(s.next_chunk(5), Some(5));
        s.chunks = vec![true; 6];
        assert_eq!(s.next_chunk(0), None);
        assert_eq!(s.avail(5 * CHUNK), 10);
        assert!(s.at_end(5 * CHUNK + 10));
    }
}
