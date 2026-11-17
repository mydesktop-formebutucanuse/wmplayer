//! HLS playlists: the pieces of a track, in order. SoundCloud serves its
//! tracks this way only; the pieces, put one after the other, are the
//! track's file (fragmented MP4, Ogg or MP3).

/// A piece of the stream.
#[derive(Debug, PartialEq, Eq)]
pub struct Part {
    pub url: String,
    /// How long it plays, in milliseconds; 0 if the playlist does not say.
    pub ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Playlist {
    /// Several versions of the stream; the address of the first.
    Master(String),
    Media {
        /// The initialisation piece (of fragmented MP4), if there is one.
        init: Option<String>,
        parts: Vec<Part>,
        /// Still being added to: a live stream.
        live: bool,
    },
}

/// `rel` as an address, relative to `base`.
pub fn join(base: &str, rel: &str) -> String {
    if rel.contains("://") {
        return rel.to_owned();
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    if let Some(rest) = rel.strip_prefix("//") {
        return format!("{}{rest}", &base[..scheme_end]);
    }
    if rel.starts_with('/') {
        let host_end = base[scheme_end..]
            .find(['/', '?'])
            .map_or(base.len(), |i| scheme_end + i);
        return format!("{}{rel}", &base[..host_end]);
    }
    let path = base.split(['?', '#']).next().unwrap_or(base);
    let dir = path.rfind('/').filter(|&i| i >= scheme_end).map_or(path, |i| &path[..=i]);
    format!("{dir}{rel}")
}

fn attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (_, attrs) = line.split_once(':')?;
    let start = attrs.find(&format!("{name}="))? + name.len() + 1;
    let rest = &attrs[start..];
    match rest.strip_prefix('"') {
        Some(q) => q.split('"').next(),
        None => rest.split(',').next(),
    }
}

pub fn parse(base: &str, text: &str) -> Result<Playlist, String> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() != Some("#EXTM3U") {
        return Err("not an HLS playlist".to_owned());
    }
    let mut init = None;
    let mut parts = Vec::new();
    let mut ms = 0;
    let mut ended = false;
    let mut variant = false;
    for line in lines {
        if let Some(tag) = line.strip_prefix('#') {
            if tag.starts_with("EXT-X-STREAM-INF") {
                variant = true;
            } else if let Some(length) = tag.strip_prefix("EXTINF:") {
                let seconds = length.split(',').next().and_then(|s| s.trim().parse::<f64>().ok());
                ms = (seconds.unwrap_or(0.0).max(0.0) * 1000.0).round() as u64;
            } else if tag.starts_with("EXT-X-MAP") {
                let uri = attribute(line, "URI").ok_or("EXT-X-MAP without URI")?;
                init = Some(join(base, uri));
            } else if tag.starts_with("EXT-X-KEY") {
                if attribute(line, "METHOD") != Some("NONE") {
                    return Err("the stream is encrypted".to_owned());
                }
            } else if tag.starts_with("EXT-X-BYTERANGE") {
                return Err("HLS byte ranges are not supported".to_owned());
            } else if tag == "EXT-X-ENDLIST" {
                ended = true;
            }
        } else if variant {
            return Ok(Playlist::Master(join(base, line)));
        } else {
            parts.push(Part { url: join(base, line), ms: std::mem::take(&mut ms) });
        }
    }
    Ok(Playlist::Media { init, parts, live: !ended })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins() {
        let base = "https://h.example/a/b/playlist.m3u8?x=1";
        assert_eq!(join(base, "data0.m4s?y"), "https://h.example/a/b/data0.m4s?y");
        assert_eq!(join(base, "/c/d.ts"), "https://h.example/c/d.ts");
        assert_eq!(join(base, "//o.example/e"), "https://o.example/e");
        assert_eq!(join(base, "https://p.example/f"), "https://p.example/f");
    }

    #[test]
    fn soundcloud() {
        let text = "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:10\n\
            #EXT-X-PLAYLIST-TYPE:VOD\n#EXT-X-MAP:URI=\"https://s/init.mp4?p=1\"\n\
            #EXTINF:10.0,\nhttps://s/data000.m4s?p=1\n#EXTINF:9.9,\ndata001.m4s\n#EXT-X-ENDLIST\n";
        assert_eq!(
            parse("https://s/playlist.m3u8", text),
            Ok(Playlist::Media {
                init: Some("https://s/init.mp4?p=1".to_owned()),
                parts: vec![
                    Part { url: "https://s/data000.m4s?p=1".to_owned(), ms: 10000 },
                    Part { url: "https://s/data001.m4s".to_owned(), ms: 9900 }
                ],
                live: false
            })
        );
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\nlow/index.m3u8\n";
        assert_eq!(
            parse("https://s/master.m3u8", master),
            Ok(Playlist::Master("https://s/low/index.m3u8".to_owned()))
        );
        let key = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\na.ts\n";
        assert!(parse("https://s/x.m3u8", key).is_err());
    }
}
