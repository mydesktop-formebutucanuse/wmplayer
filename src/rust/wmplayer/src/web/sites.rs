//! Which addresses are YouTube's and SoundCloud's, and what they point to.
//!
//! Only pages are recognised: a track's page is played, a list's (a
//! playlist, set, channel or user) is opened. Everything else is left to
//! the transport plugins, as before.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    YouTube,
    SoundCloud,
}

impl Site {
    pub fn name(self) -> &'static str {
        match self {
            Site::YouTube => "YouTube",
            Site::SoundCloud => "SoundCloud",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Track,
    List,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub site: Site,
    pub kind: Kind,
    /// For a track, the name of its file in the cache (without the
    /// extension). Made of letters, digits, '-' and '_' only.
    pub key: String,
}

struct Parts<'a> {
    host: String,
    path: &'a str,
    query: &'a str,
}

fn split(uri: &str) -> Option<Parts<'_>> {
    let rest = uri
        .strip_prefix("https://")
        .or_else(|| uri.strip_prefix("http://"))?;
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, rest) = rest.split_at(end);
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("").to_ascii_lowercase();
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    Some(Parts { host, path, query })
}

pub(crate) fn param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        (k == name && !v.is_empty()).then_some(v)
    })
}

fn is_video_id(s: &str) -> bool {
    s.len() == 11
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn youtube_track(id: &str) -> Page {
    Page {
        site: Site::YouTube,
        kind: Kind::Track,
        key: format!("youtube-{id}"),
    }
}

fn list(site: Site, what: &str) -> Page {
    Page {
        site,
        kind: Kind::List,
        key: format!("{}-list-{}", site.name().to_ascii_lowercase(), clean(what)),
    }
}

/// SoundCloud paths whose first part is not a user.
const SOUNDCLOUD_RESERVED: &[&str] = &[
    "charts", "discover", "feed", "imprint", "jobs", "logout", "messages",
    "mobile", "notifications", "pages", "people", "pro", "search", "settings",
    "signin", "stream", "terms-of-use", "upload", "you",
];

/// A user's lists on SoundCloud: soundcloud.com/USER/WHAT.
const SOUNDCLOUD_USER_LISTS: &[&str] = &[
    "albums", "likes", "popular-tracks", "reposts", "sets", "spotlight",
    "toptracks", "tracks",
];

/// What the page at `uri` is, if it is one of YouTube's or SoundCloud's.
pub fn classify(uri: &str) -> Option<Page> {
    let p = split(uri)?;
    let host = p.host.strip_prefix("www.").unwrap_or(&p.host);
    let host = host.strip_prefix("m.").unwrap_or(host);
    let segs: Vec<&str> = p.path.split('/').filter(|s| !s.is_empty()).collect();

    match host {
        "youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => {
            match *segs.first()? {
                "watch" => param(p.query, "v")
                    .filter(|v| is_video_id(v))
                    .map(youtube_track),
                "shorts" | "live" | "embed" | "v" => segs
                    .get(1)
                    .filter(|v| is_video_id(v))
                    .map(|v| youtube_track(v)),
                "playlist" => param(p.query, "list").map(|l| list(Site::YouTube, l)),
                s if s.starts_with('@') => Some(list(Site::YouTube, s)),
                "channel" | "c" | "user" => segs.get(1).map(|s| list(Site::YouTube, s)),
                _ => None,
            }
        }
        "youtu.be" => segs
            .first()
            .filter(|v| is_video_id(v))
            .map(|v| youtube_track(v)),
        "soundcloud.com" => {
            let user = *segs.first()?;
            if SOUNDCLOUD_RESERVED.contains(&user) {
                return None;
            }
            let track = |parts: &[&str]| Page {
                site: Site::SoundCloud,
                kind: Kind::Track,
                key: format!("soundcloud-{}", clean(&parts.join("-"))),
            };
            match segs.len() {
                1 => Some(list(Site::SoundCloud, user)),
                2 if SOUNDCLOUD_USER_LISTS.contains(&segs[1]) => {
                    Some(list(Site::SoundCloud, &segs.join("-")))
                }
                3 if segs[1] == "sets" => Some(list(Site::SoundCloud, &segs.join("-"))),
                // USER/TRACK, and USER/TRACK/s-TOKEN for a private link
                2 => Some(track(&segs)),
                3 if segs[2].starts_with("s-") => Some(track(&segs)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The address to list for a list page: a channel's videos rather than its
/// tabs.
pub fn list_target(uri: &str) -> String {
    if let Some(p) = split(uri) {
        let host = p.host.strip_prefix("www.").unwrap_or(&p.host);
        let host = host.strip_prefix("m.").unwrap_or(host);
        let segs: Vec<&str> = p.path.split('/').filter(|s| !s.is_empty()).collect();
        let channel_root = match segs.as_slice() {
            [s] => s.starts_with('@'),
            ["channel" | "c" | "user", _] => true,
            _ => false,
        };
        if host == "youtube.com" && channel_root {
            let base = uri.split(['?', '#']).next().unwrap_or(uri);
            return format!("{}/videos", base.trim_end_matches('/'));
        }
    }
    uri.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(uri: &str) -> Option<(Kind, String)> {
        classify(uri).map(|p| (p.kind, p.key))
    }

    #[test]
    fn youtube() {
        let t = Some((Kind::Track, "youtube-n61ULEU7CO0".to_owned()));
        assert_eq!(kind("https://www.youtube.com/watch?v=n61ULEU7CO0"), t);
        assert_eq!(kind("https://youtube.com/watch?feature=x&v=n61ULEU7CO0&t=3"), t);
        assert_eq!(kind("https://m.youtube.com/watch?v=n61ULEU7CO0"), t);
        assert_eq!(kind("https://music.youtube.com/watch?v=n61ULEU7CO0&list=RDAMVM"), t);
        assert_eq!(kind("https://youtu.be/n61ULEU7CO0?si=abc"), t);
        assert_eq!(kind("https://www.youtube.com/shorts/n61ULEU7CO0"), t);
        assert_eq!(kind("https://www.youtube.com/watch?v=short"), None);
        assert_eq!(
            classify("https://www.youtube.com/playlist?list=PL123").map(|p| p.kind),
            Some(Kind::List)
        );
        assert_eq!(classify("https://www.youtube.com/@LofiGirl").map(|p| p.kind), Some(Kind::List));
        assert_eq!(classify("https://www.youtube.com/results?search_query=x"), None);
        assert_eq!(classify("https://example.com/watch?v=n61ULEU7CO0"), None);
    }

    #[test]
    fn soundcloud() {
        assert_eq!(
            kind("https://soundcloud.com/some-user/some-track"),
            Some((Kind::Track, "soundcloud-some-user-some-track".to_owned()))
        );
        assert_eq!(
            kind("https://soundcloud.com/u/t/s-AbC12"),
            Some((Kind::Track, "soundcloud-u-t-s-AbC12".to_owned()))
        );
        assert_eq!(classify("https://soundcloud.com/u").map(|p| p.kind), Some(Kind::List));
        assert_eq!(classify("https://soundcloud.com/u/tracks").map(|p| p.kind), Some(Kind::List));
        assert_eq!(classify("https://soundcloud.com/u/sets/mix").map(|p| p.kind), Some(Kind::List));
        assert_eq!(classify("https://soundcloud.com/discover"), None);
        assert_eq!(classify("https://soundcloud.com/search/sounds?q=x"), None);
    }

    #[test]
    fn targets() {
        assert_eq!(
            list_target("https://www.youtube.com/@LofiGirl"),
            "https://www.youtube.com/@LofiGirl/videos"
        );
        assert_eq!(
            list_target("https://www.youtube.com/channel/UCx/"),
            "https://www.youtube.com/channel/UCx/videos"
        );
        assert_eq!(
            list_target("https://www.youtube.com/@LofiGirl/playlists"),
            "https://www.youtube.com/@LofiGirl/playlists"
        );
    }
}
