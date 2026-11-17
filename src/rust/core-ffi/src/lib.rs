//! The C side of the wmplayer crate, for libaudcore (see
//! src/libaudcore/wmplayer-rs.h, which declares the same).
//!
//! Strings that come back are the caller's, and are freed with
//! wmplayer_free_string(); infos and lists with their own functions.

use std::ffi::{c_char, c_int, CStr, CString};
use std::io::SeekFrom;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr;

use wmplayer::web::{self, Kind};

fn arg<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

fn c_string(s: &str) -> *mut c_char {
    CString::new(s.replace('\0', " ")).map_or(ptr::null_mut(), CString::into_raw)
}

fn c_opt(s: &Option<String>) -> *mut c_char {
    s.as_deref().map_or(ptr::null_mut(), c_string)
}

fn set_error(error: *mut *mut c_char, msg: &str) {
    if !error.is_null() {
        unsafe { *error = c_string(msg) };
    }
}

/// Runs `f`, and a panic in it is an error rather than the end of the
/// player.
fn guard<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(fallback)
}

#[no_mangle]
pub extern "C" fn wmplayer_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Copies the settings directory `old` to `new` unless `new` is there.
/// 1 if it did, 0 if not, -1 if it failed.
#[no_mangle]
pub extern "C" fn wmplayer_migrate_config(old: *const c_char, new: *const c_char) -> c_int {
    let (Some(old), Some(new)) = (arg(old), arg(new)) else { return -1 };
    guard(-1, || match wmplayer::config::migrate(Path::new(old), Path::new(new)) {
        Ok(copied) => copied as c_int,
        Err(_) => -1,
    })
}

#[no_mangle]
pub extern "C" fn wmweb_init() {
    guard((), web::init)
}

/// 0: not a page of YouTube or SoundCloud. 1: a track's. 2: a list's.
#[no_mangle]
pub extern "C" fn wmweb_kind(uri: *const c_char) -> c_int {
    match arg(uri).and_then(web::classify) {
        None => 0,
        Some(p) if p.kind == Kind::Track => 1,
        Some(_) => 2,
    }
}

pub struct WmwebStream {
    stream: web::Stream,
    mime: CString,
    ext: CString,
}

#[no_mangle]
pub extern "C" fn wmweb_open(uri: *const c_char, error: *mut *mut c_char) -> *mut WmwebStream {
    let Some(uri) = arg(uri) else {
        set_error(error, "no address");
        return ptr::null_mut();
    };
    let opened = guard(Err("wmplayer: a bug in opening".to_owned()), || web::open(uri));
    match opened {
        Ok(stream) => Box::into_raw(Box::new(WmwebStream {
            mime: CString::new(stream.mime()).unwrap_or_default(),
            ext: CString::new(stream.ext()).unwrap_or_default(),
            stream,
        })),
        Err(e) => {
            set_error(error, &e);
            ptr::null_mut()
        }
    }
}

/// Bytes read, 0 at the end, -1 on an error.
#[no_mangle]
pub extern "C" fn wmweb_read(s: *mut WmwebStream, buf: *mut u8, len: i64) -> i64 {
    if s.is_null() || buf.is_null() || len <= 0 {
        return 0;
    }
    let s = unsafe { &mut *s };
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, len as usize) };
    guard(-1, || match s.stream.read(buf) {
        Ok(n) => n as i64,
        Err(_) => -1,
    })
}

/// `whence` as in fseek(). 0, or -1 on an error.
#[no_mangle]
pub extern "C" fn wmweb_seek(s: *mut WmwebStream, offset: i64, whence: c_int) -> c_int {
    if s.is_null() {
        return -1;
    }
    let s = unsafe { &mut *s };
    let to = match whence {
        0 if offset >= 0 => SeekFrom::Start(offset as u64),
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -1,
    };
    if s.stream.seek(to).is_ok() {
        0
    } else {
        -1
    }
}

#[no_mangle]
pub extern "C" fn wmweb_tell(s: *const WmwebStream) -> i64 {
    if s.is_null() {
        return -1;
    }
    unsafe { &*s }.stream.tell() as i64
}

/// -1 while it is not known.
#[no_mangle]
pub extern "C" fn wmweb_size(s: *const WmwebStream) -> i64 {
    if s.is_null() {
        return -1;
    }
    unsafe { &*s }.stream.size().map_or(-1, |n| n as i64)
}

#[no_mangle]
pub extern "C" fn wmweb_eof(s: *const WmwebStream) -> c_int {
    (s.is_null() || unsafe { &*s }.stream.eof()) as c_int
}

#[no_mangle]
pub extern "C" fn wmweb_mime(s: *const WmwebStream) -> *const c_char {
    if s.is_null() {
        return ptr::null();
    }
    unsafe { &*s }.mime.as_ptr()
}

#[no_mangle]
pub extern "C" fn wmweb_ext(s: *const WmwebStream) -> *const c_char {
    if s.is_null() {
        return ptr::null();
    }
    unsafe { &*s }.ext.as_ptr()
}

#[no_mangle]
pub extern "C" fn wmweb_close(s: *mut WmwebStream) {
    if !s.is_null() {
        drop(unsafe { Box::from_raw(s) });
    }
}

/// Starts downloading the track, if it is not in the cache.
#[no_mangle]
pub extern "C" fn wmweb_prefetch(uri: *const c_char) {
    if let Some(uri) = arg(uri) {
        guard((), || web::prefetch(uri))
    }
}

#[repr(C)]
pub struct WmwebInfo {
    pub url: *mut c_char,
    pub title: *mut c_char,
    pub artist: *mut c_char,
    pub album: *mut c_char,
    /// -1 if not known.
    pub length_ms: i64,
}

impl WmwebInfo {
    fn new(info: &web::Info) -> WmwebInfo {
        WmwebInfo {
            url: c_string(&info.url),
            title: c_opt(&info.title),
            artist: c_opt(&info.artist),
            album: c_opt(&info.album),
            length_ms: info.length_ms.unwrap_or(-1),
        }
    }

    fn clear(&mut self) {
        for p in [&mut self.url, &mut self.title, &mut self.artist, &mut self.album] {
            wmplayer_free_string(*p);
            *p = ptr::null_mut();
        }
    }
}

/// Fills `info` for the track at `uri` (yt-dlp looks it up). 1, or 0 and
/// `error`.
#[no_mangle]
pub extern "C" fn wmweb_info(
    uri: *const c_char,
    info: *mut WmwebInfo,
    error: *mut *mut c_char,
) -> c_int {
    let (Some(uri), false) = (arg(uri), info.is_null()) else { return 0 };
    match guard(Err("wmplayer: a bug in looking up".to_owned()), || web::info(uri)) {
        Ok(i) => {
            unsafe { *info = WmwebInfo::new(&i) };
            1
        }
        Err(e) => {
            set_error(error, &e);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn wmweb_info_clear(info: *mut WmwebInfo) {
    if !info.is_null() {
        unsafe { &mut *info }.clear();
    }
}

#[repr(C)]
pub struct WmwebList {
    pub title: *mut c_char,
    pub items: *mut WmwebInfo,
    pub n_items: usize,
}

/// The tracks of the list at `uri`, up to `max`; or null and `error`.
#[no_mangle]
pub extern "C" fn wmweb_list(uri: *const c_char, max: usize, error: *mut *mut c_char) -> *mut WmwebList {
    let Some(uri) = arg(uri) else { return ptr::null_mut() };
    match guard(Err("wmplayer: a bug in listing".to_owned()), || web::list(uri, 0, max)) {
        Ok(listing) => {
            let items: Box<[WmwebInfo]> = listing
                .entries
                .iter()
                .filter(|e| e.kind == web::EntryKind::Track)
                .map(|e| WmwebInfo::new(&e.info()))
                .collect();
            let n_items = items.len();
            Box::into_raw(Box::new(WmwebList {
                title: c_opt(&listing.title),
                items: Box::into_raw(items) as *mut WmwebInfo,
                n_items,
            }))
        }
        Err(e) => {
            set_error(error, &e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn wmweb_list_free(list: *mut WmwebList) {
    if list.is_null() {
        return;
    }
    let list = unsafe { Box::from_raw(list) };
    wmplayer_free_string(list.title);
    let items = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(list.items, list.n_items)) };
    for mut item in items.into_vec() {
        item.clear();
    }
}
