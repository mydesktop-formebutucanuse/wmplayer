//! YouTube and SoundCloud, built into wmplayer: their pages are played and
//! opened like files and folders, and searched from a window of their own.

mod adts;
mod cache;
mod download;
mod hls;
mod http;
pub mod sites;
pub mod ytdlp;

pub use download::{open, prefetch, Stream};
pub use sites::{classify, Kind, Page, Site};
pub use ytdlp::{info, list, search, Entry, EntryKind, Info, Listing, Search};

/// For the start of the program: clears the cache of what is old or was
/// left half downloaded.
pub fn init() {
    cache::prune();
}
