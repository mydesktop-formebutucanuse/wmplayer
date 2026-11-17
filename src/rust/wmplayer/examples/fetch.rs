//! cargo run --example fetch -- URL...: opens each track as the player
//! would, reads it to the end, and says how long that took.

use std::io::SeekFrom;
use std::time::Instant;

fn main() {
    for uri in std::env::args().skip(1) {
        let t = Instant::now();
        let mut s = match wmplayer::web::open(&uri) {
            Ok(s) => s,
            Err(e) => {
                println!("{uri}: {e}");
                continue;
            }
        };
        println!(
            "{uri}: open {:.2}s, {} {} size {:?}",
            t.elapsed().as_secs_f64(),
            s.ext(),
            s.mime(),
            s.size()
        );
        let mut buf = vec![0u8; 65536];
        // A seek near the end first, as a demuxer looking for an index does.
        if let Some(size) = s.size() {
            let t = Instant::now();
            s.seek(SeekFrom::Start(size.saturating_sub(100_000))).unwrap();
            let n = s.read(&mut buf).unwrap();
            println!("  read {n} near the end after {:.2}s", t.elapsed().as_secs_f64());
            s.seek(SeekFrom::Start(0)).unwrap();
        }
        let mut total = 0u64;
        loop {
            match s.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => total += n as u64,
                Err(e) => {
                    println!("  error: {e}");
                    break;
                }
            }
        }
        println!(
            "  read {total} bytes in {:.2}s, size now {:?}, eof {}",
            t.elapsed().as_secs_f64(),
            s.size(),
            s.eof()
        );
    }
}
