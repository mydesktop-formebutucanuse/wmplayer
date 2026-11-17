//! What yt-dlp says: search results, what is in a list, and a track's
//! title and the address of its audio.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::sites::{self, Kind, Site};

/// Opus or AAC over plain HTTPS first: those are read a range at a time.
/// SoundCloud has HLS only.
const FORMAT: &str =
    "bestaudio[protocol=https][acodec=opus]/bestaudio[protocol=https]/bestaudio/best";

/// How long an address of the audio is used again. YouTube's work for
/// about six hours.
const RESOLVED_FOR: Duration = Duration::from_secs(30 * 60);

/// yt-dlp runs at most this many times at once for tracks (a playlist that
/// is being scanned would start one for each).
const AT_ONCE: usize = 3;

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub url: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub length_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Format {
    pub url: String,
    pub protocol: String,
    pub ext: String,
    pub headers: Vec<(String, String)>,
    pub filesize: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Resolved {
    pub info: Info,
    pub format: Format,
    pub live: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Track,
    Playlist,
    Channel,
}

/// A row of search results or of a list.
#[derive(Clone, Debug)]
pub struct Entry {
    pub kind: EntryKind,
    pub url: String,
    pub title: String,
    /// The channel or uploader.
    pub by: Option<String>,
    pub by_url: Option<String>,
    pub length_ms: Option<i64>,
    pub site: Option<Site>,
}

impl Entry {
    pub fn info(&self) -> Info {
        Info {
            url: self.url.clone(),
            title: Some(self.title.clone()),
            artist: self.by.clone(),
            album: self.site.map(|s| s.name().to_owned()),
            length_ms: self.length_ms,
        }
    }

    /// The address of the uploader's tracks or the channel's videos.
    pub fn by_list(&self) -> Option<String> {
        let url = self.by_url.as_deref()?;
        match self.site {
            Some(Site::SoundCloud) => Some(format!("{}/tracks", url.trim_end_matches('/'))),
            _ => Some(sites::list_target(url)),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub title: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Search {
    YouTubeVideos,
    YouTubePlaylists,
    YouTubeChannels,
    SoundCloudTracks,
}

impl Search {
    pub const ALL: [Search; 4] = [
        Search::YouTubeVideos,
        Search::YouTubePlaylists,
        Search::YouTubeChannels,
        Search::SoundCloudTracks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Search::YouTubeVideos => "YouTube: videos",
            Search::YouTubePlaylists => "YouTube: playlists",
            Search::YouTubeChannels => "YouTube: channels",
            Search::SoundCloudTracks => "SoundCloud: tracks",
        }
    }
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn length_ms(v: &Value) -> Option<i64> {
    let d = v.get("duration")?.as_f64()?;
    (d > 0.0).then(|| (d * 1000.0).round() as i64)
}

/// Runs yt-dlp and reads the JSON it prints.
fn run(args: &[&str]) -> Result<Value, String> {
    let out = Command::new("yt-dlp")
        .arg("--no-warnings")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("yt-dlp: {e}"))?;
    // A list prints its JSON even when some of it failed.
    if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) {
        if v.is_object() {
            return Ok(v);
        }
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let line = err
        .lines()
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .or_else(|| err.lines().rev().find(|l| !l.trim().is_empty()))
        .unwrap_or("yt-dlp printed nothing");
    Err(line.trim_start_matches("ERROR:").trim().to_owned())
}

struct Slots {
    used: Mutex<usize>,
    freed: Condvar,
}

struct Slot(&'static Slots);

impl Slot {
    fn take() -> Slot {
        static SLOTS: Slots = Slots {
            used: Mutex::new(0),
            freed: Condvar::new(),
        };
        let mut used = SLOTS.used.lock().unwrap();
        while *used >= AT_ONCE {
            used = SLOTS.freed.wait(used).unwrap();
        }
        *used += 1;
        Slot(&SLOTS)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        *self.0.used.lock().unwrap() -= 1;
        self.0.freed.notify_one();
    }
}

fn resolve_now(uri: &str) -> Result<Resolved, String> {
    let _slot = Slot::take();
    let v = run(&["-J", "--no-playlist", "-f", FORMAT, "--", uri])?;
    let site = sites::classify(uri).map(|p| p.site);

    // A music video can have the song's own title and artist.
    let track = text(&v, "track");
    let artist = track.as_ref().and_then(|_| text(&v, "artist"));
    let info = Info {
        url: uri.to_owned(),
        title: track.or_else(|| text(&v, "title")),
        artist: artist
            .or_else(|| text(&v, "channel"))
            .or_else(|| text(&v, "uploader")),
        album: text(&v, "album").or_else(|| site.map(|s| s.name().to_owned())),
        length_ms: length_ms(&v),
    };

    let live = v.get("is_live").and_then(Value::as_bool).unwrap_or(false)
        || text(&v, "live_status").as_deref() == Some("is_live");

    let headers = v
        .get("http_headers")
        .and_then(Value::as_object)
        .map(|h| {
            h.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let format = Format {
        url: text(&v, "url").ok_or("yt-dlp found no audio")?,
        protocol: text(&v, "protocol").unwrap_or_else(|| "https".to_owned()),
        ext: text(&v, "ext").unwrap_or_else(|| "bin".to_owned()),
        headers,
        filesize: v.get("filesize").and_then(Value::as_u64),
    };

    Ok(Resolved { info, format, live })
}

type Cached = Arc<Mutex<Option<(Instant, Arc<Resolved>)>>>;

fn cache() -> &'static Mutex<HashMap<String, Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The title and the audio of the track at `uri`. What yt-dlp said is used
/// again for a while, so that the playlist's scan, a prefetch and playback
/// ask once between them.
pub fn resolve(uri: &str) -> Result<Arc<Resolved>, String> {
    let slot = {
        let mut map = cache().lock().unwrap();
        map.retain(|_, c| match c.try_lock() {
            Ok(c) => c.as_ref().is_some_and(|(t, _)| t.elapsed() < RESOLVED_FOR),
            Err(_) => true,
        });
        map.entry(uri.to_owned()).or_default().clone()
    };
    // Held while yt-dlp runs: who asks for the same track waits for it.
    let mut slot = slot.lock().unwrap();
    if let Some((t, r)) = slot.as_ref() {
        if t.elapsed() < RESOLVED_FOR {
            return Ok(r.clone());
        }
    }
    let r = Arc::new(resolve_now(uri)?);
    *slot = Some((Instant::now(), r.clone()));
    Ok(r)
}

/// Forgets what yt-dlp said about `uri`: its address has stopped working.
pub fn forget(uri: &str) {
    cache().lock().unwrap().remove(uri);
}

/// The title, artist and length of the track at `uri`.
pub fn info(uri: &str) -> Result<Info, String> {
    match sites::classify(uri) {
        Some(p) if p.kind == Kind::Track => Ok(resolve(uri)?.info.clone()),
        _ => Err("not a track".to_owned()),
    }
}

fn entry(v: &Value) -> Option<Entry> {
    let ie = text(v, "ie_key").unwrap_or_default();
    let title = text(v, "title")?;
    // Private and deleted videos stay in playlists, and do not play.
    if title.starts_with("[Private") || title.starts_with("[Deleted") {
        return None;
    }
    // Live streams cannot be played (yet), and are not offered.
    if matches!(text(v, "live_status").as_deref(), Some("is_live" | "is_upcoming")) {
        return None;
    }
    // SoundCloud's url is the API's; webpage_url is the track's page.
    let url = text(v, "webpage_url")
        .filter(|u| sites::classify(u).is_some())
        .or_else(|| text(v, "url"))
        .or_else(|| {
            (ie == "Youtube")
                .then(|| text(v, "id"))
                .flatten()
                .map(|id| format!("https://www.youtube.com/watch?v={id}"))
        })?;
    let page = sites::classify(&url)?;
    let kind = match page.kind {
        Kind::Track => EntryKind::Track,
        Kind::List if ie.ends_with("User") => EntryKind::Channel,
        Kind::List if url.contains("list=") || url.contains("/sets/") => EntryKind::Playlist,
        Kind::List if page.site == Site::YouTube => EntryKind::Channel,
        Kind::List => EntryKind::Playlist,
    };
    Some(Entry {
        kind,
        url,
        title,
        by: text(v, "channel").or_else(|| text(v, "uploader")),
        by_url: text(v, "channel_url").or_else(|| text(v, "uploader_url")),
        length_ms: length_ms(v),
        site: Some(page.site),
    })
}

fn listing(v: &Value) -> Listing {
    let mut entries: Vec<Entry> = v
        .get("entries")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(entry).collect())
        .unwrap_or_default();
    // A channel's page names the channel once, not with each video.
    if let Some(by) = text(v, "channel").or_else(|| text(v, "uploader")) {
        let by_url = text(v, "channel_url").or_else(|| text(v, "uploader_url"));
        for e in entries.iter_mut().filter(|e| e.kind == EntryKind::Track && e.by.is_none()) {
            e.by = Some(by.clone());
            if e.by_url.is_none() {
                e.by_url = by_url.clone();
            }
        }
    }
    Listing { title: text(v, "title"), entries }
}

/// `count` entries of the list at `uri`, from the `from`th on (0 is the
/// first).
pub fn list(uri: &str, from: usize, count: usize) -> Result<Listing, String> {
    let target = sites::list_target(uri);
    let range = format!("{}:{}", from + 1, from + count);
    Ok(listing(&run(&[
        "-J", "--flat-playlist", "-I", &range, "--", &target,
    ])?))
}

fn query_escape(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// What YouTube or SoundCloud finds for `query`. An address of a page is
/// what is on that page instead.
pub fn search(what: Search, query: &str, count: usize) -> Result<Listing, String> {
    let query = query.trim();
    if let Some(page) = sites::classify(query) {
        return match page.kind {
            Kind::List => list(query, 0, count),
            Kind::Track => {
                let info = info(query)?;
                Ok(Listing {
                    title: info.title.clone(),
                    entries: vec![Entry {
                        kind: EntryKind::Track,
                        url: info.url,
                        title: info.title.unwrap_or_else(|| query.to_owned()),
                        by: info.artist,
                        by_url: None,
                        length_ms: info.length_ms,
                        site: Some(page.site),
                    }],
                })
            }
        };
    }
    // The filters are YouTube's own, from its search page.
    let results = |filter: &str| {
        format!(
            "https://www.youtube.com/results?search_query={}&sp={filter}",
            query_escape(query)
        )
    };
    let target = match what {
        Search::YouTubeVideos => format!("ytsearch{count}:{query}"),
        Search::YouTubePlaylists => results("EgIQAw%253D%253D"),
        Search::YouTubeChannels => results("EgIQAg%253D%253D"),
        Search::SoundCloudTracks => format!("scsearch{count}:{query}"),
    };
    let range = format!("1:{count}");
    Ok(listing(&run(&[
        "-J", "--flat-playlist", "-I", &range, "--", &target,
    ])?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries() {
        let v: Value = serde_json::from_str(
            r#"{"title": "lofi", "entries": [
                {"ie_key": "Youtube", "id": "rFZHOHl-L8A",
                 "url": "https://www.youtube.com/watch?v=rFZHOHl-L8A",
                 "title": "radio", "channel": "Lofi Girl",
                 "channel_url": "https://www.youtube.com/channel/UCSJ", "duration": null},
                {"ie_key": "YoutubeTab", "id": "PL1",
                 "url": "https://www.youtube.com/playlist?list=PL1", "title": "a list"},
                {"ie_key": "YoutubeTab", "id": "UC1",
                 "url": "https://www.youtube.com/channel/UC1", "title": "a channel"},
                {"ie_key": "Soundcloud", "id": "1",
                 "url": "https://api.soundcloud.com/tracks/soundcloud%3Atracks%3A1",
                 "webpage_url": "https://soundcloud.com/u/t", "title": "t",
                 "uploader": "U", "uploader_url": "https://soundcloud.com/u", "duration": 244.108},
                {"ie_key": "Youtube", "id": "aaaaaaaaaaa", "title": "[Private video]"}
            ]}"#,
        )
        .unwrap();
        let l = listing(&v);
        assert_eq!(l.title.as_deref(), Some("lofi"));
        let kinds: Vec<_> = l.entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [EntryKind::Track, EntryKind::Playlist, EntryKind::Channel, EntryKind::Track]
        );
        assert_eq!(l.entries[3].url, "https://soundcloud.com/u/t");
        assert_eq!(l.entries[3].length_ms, Some(244108));
        assert_eq!(l.entries[3].by_list().as_deref(), Some("https://soundcloud.com/u/tracks"));
        assert_eq!(
            l.entries[0].by_list().as_deref(),
            Some("https://www.youtube.com/channel/UCSJ/videos")
        );
    }

    #[test]
    fn escape() {
        assert_eq!(query_escape("lo fi/한"), "lo+fi%2F%ED%95%9C");
    }
}
