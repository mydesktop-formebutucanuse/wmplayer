//! Getting a file, or a range of it, with libcurl.

use std::thread::sleep;
use std::time::Duration;

use curl::easy::{Easy, List};

pub struct Response {
    pub code: u32,
    pub body: Vec<u8>,
    /// The size of the whole file, from Content-Range.
    pub total: Option<u64>,
}

pub struct Client {
    easy: Easy,
    headers: Vec<String>,
}

fn err(e: curl::Error) -> String {
    e.to_string()
}

impl Client {
    /// A client that sends `headers` (yt-dlp's, for the site) with each
    /// request. The connection is kept between requests.
    pub fn new(headers: &[(String, String)]) -> Result<Client, String> {
        let mut easy = Easy::new();
        easy.follow_location(true).map_err(err)?;
        easy.connect_timeout(Duration::from_secs(15)).map_err(err)?;
        // A stalled transfer fails, and is tried again.
        easy.low_speed_limit(1024).map_err(err)?;
        easy.low_speed_time(Duration::from_secs(20)).map_err(err)?;
        let headers = headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("range"))
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        Ok(Client { easy, headers })
    }

    fn get_once(&mut self, url: &str, range: Option<(u64, u64)>) -> Result<Response, String> {
        let mut list = List::new();
        for h in &self.headers {
            list.append(h).map_err(err)?;
        }
        if let Some((a, b)) = range {
            list.append(&format!("Range: bytes={a}-{b}")).map_err(err)?;
        }
        self.easy.http_headers(list).map_err(err)?;
        self.easy.url(url).map_err(err)?;

        let mut body = Vec::new();
        let mut total = None;
        {
            let mut t = self.easy.transfer();
            t.write_function(|d| {
                body.extend_from_slice(d);
                Ok(d.len())
            })
            .map_err(err)?;
            t.header_function(|h| {
                // Content-Range: bytes 0-2097151/398884282
                if let Ok(h) = std::str::from_utf8(h) {
                    if let Some((name, value)) = h.split_once(':') {
                        if name.trim().eq_ignore_ascii_case("content-range") {
                            total = value.rsplit('/').next().and_then(|t| t.trim().parse().ok());
                        }
                    }
                }
                true
            })
            .map_err(err)?;
            t.perform().map_err(err)?;
        }
        let code = self.easy.response_code().map_err(err)?;
        Ok(Response { code, body, total })
    }

    /// GET `url`, or the bytes `range` of it (both ends included). A new
    /// address of YouTube's answers 403 for a second or two, and a
    /// connection can fail: those are tried again.
    pub fn get(&mut self, url: &str, range: Option<(u64, u64)>) -> Result<Response, String> {
        let mut last = String::new();
        for attempt in 0..20 {
            if attempt > 0 {
                sleep(Duration::from_millis(500));
            }
            match self.get_once(url, range) {
                Ok(r) if r.code == 200 || r.code == 206 => return Ok(r),
                Ok(r) if r.code == 403 || r.code == 429 || r.code >= 500 => {
                    last = format!("HTTP {}", r.code)
                }
                Ok(r) => return Err(format!("HTTP {}", r.code)),
                Err(e) => {
                    last = e;
                    if attempt >= 3 {
                        break;
                    }
                }
            }
        }
        Err(last)
    }
}
