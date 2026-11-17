//! AAC out of fragmented MP4, as ADTS.
//!
//! SoundCloud's HLS pieces are fragmented MP4. A player that can seek in
//! such a file looks at every fragment before it plays the first (FFmpeg
//! does), so the whole track would have to be there before it starts. The
//! AAC frames are taken out of the pieces instead and written one after
//! the other, each with an ADTS header: that plays as it comes.

/// What an ADTS header says about the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// The audio object type less one (1: AAC LC).
    profile: u8,
    /// The index of the sampling frequency.
    frequency: u8,
    channels: u8,
    /// The size of a sample when a fragment does not say (from trex).
    default_size: Option<u32>,
}

const FREQUENCIES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

fn be32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

/// The boxes in `data`: the type, where the box starts, and what is in it.
fn boxes(data: &[u8]) -> impl Iterator<Item = (&[u8], usize, &[u8])> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let size = be32(data, at)? as usize;
        let kind = data.get(at + 4..at + 8)?;
        let (header, size) = match size {
            // To the end of the file.
            0 => (8, data.len() - at),
            // A 64-bit size follows.
            1 => (16, u64::from_be_bytes(data.get(at + 8..at + 16)?.try_into().ok()?) as usize),
            n => (8, n),
        };
        if size < header {
            return None;
        }
        let body = data.get(at + header..at.checked_add(size)?)?;
        let start = at;
        at += size;
        Some((kind, start, body))
    })
}

fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).find(|(k, _, _)| k == kind).map(|(_, _, body)| body)
}

/// An MPEG-4 descriptor: its tag, what is in it, and what follows it.
fn descriptor(d: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *d.first()?;
    let mut size = 0usize;
    let mut at = 1;
    loop {
        let b = *d.get(at)?;
        at += 1;
        size = size << 7 | (b & 0x7f) as usize;
        if b & 0x80 == 0 || at > 4 {
            break;
        }
    }
    let end = (at + size).min(d.len());
    Some((tag, &d[at..end], &d[end..]))
}

struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn take(&mut self, n: usize) -> Option<u32> {
        let mut v = 0;
        for _ in 0..n {
            let byte = *self.data.get(self.at / 8)?;
            v = v << 1 | (byte >> (7 - self.at % 8) & 1) as u32;
            self.at += 1;
        }
        Some(v)
    }

    fn object_type(&mut self) -> Option<u32> {
        match self.take(5)? {
            31 => Some(32 + self.take(6)?),
            t => Some(t),
        }
    }

    fn frequency(&mut self) -> Option<u8> {
        match self.take(4)? {
            15 => {
                let hz = self.take(24)?;
                FREQUENCIES.iter().position(|&f| f == hz).map(|i| i as u8)
            }
            i if (i as usize) < FREQUENCIES.len() => Some(i as u8),
            _ => None,
        }
    }
}

/// From an AudioSpecificConfig.
fn audio_config(asc: &[u8]) -> Option<(u8, u8, u8)> {
    let mut bits = Bits { data: asc, at: 0 };
    let mut object = bits.object_type()?;
    let frequency = bits.frequency()?;
    let channels = bits.take(4)?;
    // SBR or PS said outright: the stream under it is what ADTS names, and
    // a decoder finds the rest in the frames.
    if object == 5 || object == 29 {
        bits.frequency()?;
        object = bits.object_type()?;
    }
    // ADTS can name Main, LC, SSR and LTP, and up to 7.1.
    ((1..=4).contains(&object) && (1..=7).contains(&channels))
        .then_some((object as u8 - 1, frequency, channels as u8))
}

/// From an esds box: the AudioSpecificConfig in it.
fn esds_config(esds: &[u8]) -> Option<(u8, u8, u8)> {
    // ES_Descriptor: ES_ID, flags, what the flags announce.
    let (tag, es, _) = descriptor(esds.get(4..)?)?;
    if tag != 3 {
        return None;
    }
    let flags = *es.get(2)?;
    let mut at = 3;
    if flags & 0x80 != 0 {
        at += 2;
    }
    if flags & 0x40 != 0 {
        at += 1 + *es.get(at)? as usize;
    }
    if flags & 0x20 != 0 {
        at += 2;
    }
    // DecoderConfigDescriptor: 13 bytes, then DecoderSpecificInfo.
    let mut rest = es.get(at..)?;
    loop {
        let (tag, body, after) = descriptor(rest)?;
        if tag == 4 {
            let mut inner = body.get(13..)?;
            loop {
                let (tag, asc, after) = descriptor(inner)?;
                if tag == 5 {
                    return audio_config(asc);
                }
                inner = after;
            }
        }
        rest = after;
    }
}

/// What the initialisation piece (the moov box) says, if what is in the
/// stream is AAC that ADTS can carry.
pub fn config(init: &[u8]) -> Option<Config> {
    let moov = find(init, b"moov")?;
    let stbl = [b"trak", b"mdia", b"minf", b"stbl"]
        .iter()
        .try_fold(moov, |data, kind| find(data, kind))?;
    // stsd: version and flags, the count, then the entries; mp4a: 28 bytes
    // of sample entry, then its boxes.
    let stsd = find(stbl, b"stsd")?;
    let mp4a = find(stsd.get(8..)?, b"mp4a")?;
    let (profile, frequency, channels) = esds_config(find(mp4a.get(28..)?, b"esds")?)?;
    // trex: version and flags, track, description, duration, size, flags.
    let default_size = find(moov, b"mvex")
        .and_then(|mvex| find(mvex, b"trex"))
        .and_then(|trex| be32(trex, 16))
        .filter(|&size| size > 0);
    Some(Config { profile, frequency, channels, default_size })
}

fn header(c: &Config, frame: usize) -> Result<[u8; 7], String> {
    let len = frame + 7;
    if len >= 1 << 13 {
        return Err("an AAC frame too big for ADTS".to_owned());
    }
    Ok([
        0xff,
        // MPEG-4, layer 0, no CRC.
        0xf1,
        c.profile << 6 | c.frequency << 2 | c.channels >> 2,
        (c.channels & 3) << 6 | (len >> 11) as u8,
        (len >> 3) as u8,
        ((len & 7) << 5) as u8 | 0x1f,
        0xfc,
    ])
}

/// Appends to `out` the frames of a media piece (moof and mdat boxes), each
/// with its ADTS header.
pub fn frames(c: &Config, piece: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    let bad = || "a fragment of the stream is not understood".to_owned();
    for (kind, moof_at, moof) in boxes(piece) {
        if kind != b"moof" {
            continue;
        }
        // One track: the audio.
        let traf = find(moof, b"traf").ok_or_else(bad)?;
        let tfhd = find(traf, b"tfhd").ok_or_else(bad)?;
        let flags = be32(tfhd, 0).ok_or_else(bad)? & 0xff_ffff;
        if flags & 0x1 != 0 {
            // The data is at a place in the whole file, which this is not.
            return Err(bad());
        }
        // After the track's number: the description, the duration, the
        // size and the flags of a sample, each if announced.
        let mut at = 8;
        let mut default_size = c.default_size;
        for (flag, is_size) in [(0x2, false), (0x8, false), (0x10, true)] {
            if flags & flag != 0 {
                if is_size {
                    default_size = Some(be32(tfhd, at).ok_or_else(bad)?);
                }
                at += 4;
            }
        }

        // Where a run without an offset goes on: after the run before it.
        let mut next = None;
        for (kind, _, trun) in boxes(traf) {
            if kind != b"trun" {
                continue;
            }
            let flags = be32(trun, 0).ok_or_else(bad)? & 0xff_ffff;
            let count = be32(trun, 4).ok_or_else(bad)? as usize;
            let mut at = 8;
            let mut pos = if flags & 0x1 != 0 {
                // From the start of the moof box.
                let offset = be32(trun, at).ok_or_else(bad)? as i32;
                at += 4;
                usize::try_from(moof_at as i64 + offset as i64).map_err(|_| bad())?
            } else {
                next.ok_or_else(bad)?
            };
            if flags & 0x4 != 0 {
                at += 4;
            }
            for _ in 0..count {
                // Of a sample: the duration, the size, the flags and the
                // composition offset, each if announced.
                let mut size = default_size;
                for (flag, is_size) in [(0x100, false), (0x200, true), (0x400, false), (0x800, false)] {
                    if flags & flag != 0 {
                        if is_size {
                            size = Some(be32(trun, at).ok_or_else(bad)?);
                        }
                        at += 4;
                    }
                }
                let size = size.ok_or_else(bad)? as usize;
                let frame = piece.get(pos..pos + size).ok_or_else(bad)?;
                out.extend_from_slice(&header(c, size)?);
                out.extend_from_slice(frame);
                pos += size;
            }
            next = Some(pos);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut b = (body.len() as u32 + 8).to_be_bytes().to_vec();
        b.extend_from_slice(kind);
        b.extend_from_slice(body);
        b
    }

    fn init() -> Vec<u8> {
        // AAC LC, 44100 Hz, stereo: 00010 0100 0010 000
        let asc = [0x12, 0x10];
        let mut dec = vec![0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, asc.len() as u8];
        dec.extend_from_slice(&asc);
        let mut es = vec![0, 1, 0, 4, 0x80, 0x80, 0x80, dec.len() as u8];
        es.extend_from_slice(&dec);
        let mut esds = vec![0, 0, 0, 0, 3, es.len() as u8];
        esds.extend_from_slice(&es);
        let mut mp4a = vec![0; 28];
        mp4a.extend(boxed(b"esds", &esds));
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(boxed(b"mp4a", &mp4a));
        let stbl = boxed(b"stbl", &boxed(b"stsd", &stsd));
        let trak = boxed(b"trak", &boxed(b"mdia", &boxed(b"minf", &stbl)));
        let mut moov = boxed(b"mvhd", &[0; 20]);
        moov.extend(trak);
        let mut file = boxed(b"ftyp", b"iso5");
        file.extend(boxed(b"moov", &moov));
        file
    }

    #[test]
    fn reads_the_configuration() {
        assert_eq!(
            config(&init()),
            Some(Config { profile: 1, frequency: 4, channels: 2, default_size: None })
        );
        assert_eq!(config(b"not an MP4 file"), None);
        // HE-AAC said outright: SBR, 22050 Hz, stereo, 44100 Hz, AAC LC.
        assert_eq!(audio_config(&[0x2b, 0x92, 0x08, 0x00]), Some((1, 7, 2)));
        // Opus is not for ADTS.
        assert_eq!(audio_config(&[0xf8, 0x10]), None);
    }

    #[test]
    fn takes_the_frames_out() {
        let c = config(&init()).unwrap();
        // Two samples, of 3 and 2 bytes, right after the moof box.
        let tfhd = boxed(b"tfhd", &[0, 2, 0, 0, 0, 0, 0, 1]);
        let mut trun = vec![0, 0, 2, 1, 0, 0, 0, 2];
        let moof_size = 8 + (8 + tfhd.len() + 8 + trun.len() + 12) as u32;
        trun.extend((moof_size + 8).to_be_bytes());
        trun.extend([0, 0, 0, 3, 0, 0, 0, 2]);
        let mut traf = tfhd;
        traf.extend(boxed(b"trun", &trun));
        let mut piece = boxed(b"moof", &boxed(b"traf", &traf));
        assert_eq!(piece.len() as u32, moof_size);
        piece.extend(boxed(b"mdat", b"abcde"));

        let mut out = Vec::new();
        frames(&c, &piece, &mut out).unwrap();
        // 0xfff1, profile 1, frequency 4, 2 channels, lengths 10 and 9.
        let mut want = vec![0xff, 0xf1, 0x50, 0x80, 0x01, 0x5f, 0xfc];
        want.extend(b"abc");
        want.extend([0xff, 0xf1, 0x50, 0x80, 0x01, 0x3f, 0xfc]);
        want.extend(b"de");
        assert_eq!(out, want);

        // A piece cut short is an error, not a panic.
        piece.truncate(piece.len() - 2);
        assert!(frames(&c, &piece, &mut Vec::new()).is_err());
    }
}
