//! The YouTube and SoundCloud window, in libaudgui: search, open a
//! playlist, channel or uploader, and play or add what is chosen.
//!
//! libaudgui's web-browser.cc puts it in the Services menus and gives it
//! the playlist (wmplayer_gui_add()); everything runs in GTK's thread,
//! except yt-dlp, which runs in a thread of its own while the window
//! waits for it.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

use gtk::glib::{self, ControlFlow, Propagation};
use gtk::prelude::*;
use wmplayer::web::{self, Entry, EntryKind, Listing, Search};

/// How many results are asked for at a time.
const PAGE: usize = 50;

#[repr(C)]
struct GuiItem {
    url: *const c_char,
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
    length_ms: i64,
}

extern "C" {
    /// Adds `items` to the end of the active playlist, and plays the first
    /// of them if `play`. An item without a title is a list, which the core
    /// opens.
    fn wmplayer_gui_add(items: *const GuiItem, n_items: usize, play: c_int);
    fn wmplayer_gui_get_int(name: *const c_char) -> c_int;
    fn wmplayer_gui_set_int(name: *const c_char, value: c_int);
}

fn get_int(name: &str) -> i32 {
    let name = CString::new(name).unwrap();
    unsafe { wmplayer_gui_get_int(name.as_ptr()) }
}

fn set_int(name: &str, value: i32) {
    let name = CString::new(name).unwrap();
    unsafe { wmplayer_gui_set_int(name.as_ptr(), value) }
}

fn add_to_playlist(entries: &[Entry], play: bool) {
    let c = |s: &str| CString::new(s.replace('\0', " ")).unwrap_or_default();
    // Kept until wmplayer_gui_add() returns.
    let strings: Vec<_> = entries
        .iter()
        .map(|e| {
            let info = e.info();
            if e.kind == EntryKind::Track {
                (
                    c(&e.url),
                    Some(c(&e.title)),
                    info.artist.as_deref().map(c),
                    info.album.as_deref().map(c),
                    info.length_ms.unwrap_or(-1),
                )
            } else {
                (c(&e.url), None, None, None, -1)
            }
        })
        .collect();
    let p = |s: &Option<CString>| s.as_ref().map_or(ptr::null(), |s| s.as_ptr());
    let items: Vec<GuiItem> = strings
        .iter()
        .map(|(url, title, artist, album, length_ms)| GuiItem {
            url: url.as_ptr(),
            title: p(title),
            artist: p(artist),
            album: p(album),
            length_ms: *length_ms,
        })
        .collect();
    unsafe { wmplayer_gui_add(items.as_ptr(), items.len(), play as c_int) };
}

fn length_text(ms: Option<i64>) -> String {
    let Some(ms) = ms else { return String::new() };
    let s = (ms + 500) / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

#[derive(Clone)]
enum Source {
    Search(Search, String),
    List(String),
}

struct Page {
    title: String,
    source: Source,
    entries: Vec<Entry>,
    /// Asked for more and got fewer: there are no more.
    complete: bool,
}

struct Browser {
    window: gtk::Window,
    kind: gtk::ComboBoxText,
    query: gtk::Entry,
    back: gtk::Button,
    heading: gtk::Label,
    spinner: gtk::Spinner,
    store: gtk::ListStore,
    view: gtk::TreeView,
    play: gtk::Button,
    add: gtk::Button,
    open: gtk::Button,
    uploader: gtk::Button,
    more: gtk::Button,
    status: gtk::Label,
    pages: RefCell<Vec<Page>>,
    /// Counts the requests: an answer to an older one is dropped.
    request: RefCell<u64>,
    busy: RefCell<bool>,
}

thread_local! {
    static BROWSER: RefCell<Option<Rc<Browser>>> = const { RefCell::new(None) };
}

// Columns of the store.
const COL_MARK: u32 = 0;
const COL_TITLE: u32 = 1;
const COL_BY: u32 = 2;
const COL_LENGTH: u32 = 3;
const COL_INDEX: u32 = 4;

fn mark(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Track => "♪",
        EntryKind::Playlist => "☰",
        EntryKind::Channel => "@",
    }
}

impl Browser {
    fn new() -> Rc<Browser> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("YouTube and SoundCloud");
        window.set_role("web-browser");
        window.set_default_size(780, 540);
        window.set_border_width(6);

        let kind = gtk::ComboBoxText::new();
        for s in Search::ALL {
            kind.append_text(s.label());
        }
        let saved = get_int("search").clamp(0, Search::ALL.len() as i32 - 1);
        kind.set_active(Some(saved as u32));

        let query = gtk::Entry::new();
        query.set_placeholder_text(Some("Search, or paste the address of a page"));
        query.set_hexpand(true);
        let search = gtk::Button::with_label("Search");

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        top.pack_start(&kind, false, false, 0);
        top.pack_start(&query, true, true, 0);
        top.pack_start(&search, false, false, 0);

        let back = gtk::Button::with_label("◀ Back");
        let heading = gtk::Label::new(None);
        heading.set_xalign(0.0);
        heading.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let spinner = gtk::Spinner::new();
        let nav = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        nav.pack_start(&back, false, false, 0);
        nav.pack_start(&heading, true, true, 0);
        nav.pack_end(&spinner, false, false, 0);

        let store = gtk::ListStore::new(&[
            glib::Type::STRING,
            glib::Type::STRING,
            glib::Type::STRING,
            glib::Type::STRING,
            glib::Type::U32,
        ]);
        let view = gtk::TreeView::with_model(&store);
        view.selection().set_mode(gtk::SelectionMode::Multiple);
        view.set_enable_search(false);
        let column = |title: &str, col: u32, expand: bool, xalign: f32| {
            let cell = gtk::CellRendererText::new();
            cell.set_xalign(xalign);
            if expand {
                cell.set_property("ellipsize", gtk::pango::EllipsizeMode::End);
            }
            let c = gtk::TreeViewColumn::new();
            c.set_title(title);
            CellLayoutExt::pack_start(&c, &cell, true);
            CellLayoutExt::add_attribute(&c, &cell, "text", col as i32);
            c.set_expand(expand);
            c.set_resizable(expand);
            view.append_column(&c);
        };
        column("", COL_MARK, false, 0.5);
        column("Title", COL_TITLE, true, 0.0);
        column("By", COL_BY, true, 0.0);
        column("Length", COL_LENGTH, false, 1.0);
        let scrolled = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        scrolled.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled.set_shadow_type(gtk::ShadowType::In);
        scrolled.add(&view);

        let play = gtk::Button::with_label("Play");
        let add = gtk::Button::with_label("Add to playlist");
        let open = gtk::Button::with_label("Open");
        let uploader = gtk::Button::with_label("Uploader");
        let more = gtk::Button::with_label("More");
        let status = gtk::Label::new(None);
        status.set_xalign(1.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::Start);
        let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for b in [&play, &add, &open, &uploader, &more] {
            bottom.pack_start(b, false, false, 0);
        }
        bottom.pack_end(&status, true, true, 0);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 6);
        vbox.pack_start(&top, false, false, 0);
        vbox.pack_start(&nav, false, false, 0);
        vbox.pack_start(&scrolled, true, true, 0);
        vbox.pack_start(&bottom, false, false, 0);
        window.add(&vbox);

        let b = Rc::new(Browser {
            window,
            kind,
            query,
            back,
            heading,
            spinner,
            store,
            view,
            play,
            add,
            open,
            uploader,
            more,
            status,
            pages: RefCell::new(Vec::new()),
            request: RefCell::new(0),
            busy: RefCell::new(false),
        });

        let weak = Rc::downgrade(&b);
        let with = move |f: fn(&Rc<Browser>)| {
            let weak = weak.clone();
            move || {
                if let Some(b) = weak.upgrade() {
                    f(&b)
                }
            }
        };

        let f = with(Browser::search);
        search.connect_clicked(move |_| f());
        let f = with(Browser::search);
        b.query.connect_activate(move |_| f());
        let f = with(Browser::go_back);
        b.back.connect_clicked(move |_| f());
        let f = with(|b| b.chosen(true));
        b.play.connect_clicked(move |_| f());
        let f = with(|b| b.chosen(false));
        b.add.connect_clicked(move |_| f());
        let f = with(Browser::open_selected);
        b.open.connect_clicked(move |_| f());
        let f = with(Browser::open_uploader);
        b.uploader.connect_clicked(move |_| f());
        let f = with(Browser::load_more);
        b.more.connect_clicked(move |_| f());
        let f = with(|b| b.update_buttons());
        b.view.selection().connect_changed(move |_| f());
        let f = with(Browser::activated);
        b.view.connect_row_activated(move |_, _, _| f());
        b.kind.connect_changed(|k| {
            if let Some(i) = k.active() {
                set_int("search", i as i32);
            }
        });
        let f = with(Browser::go_back);
        b.window.connect_key_press_event(move |w, e| {
            use gtk::gdk::keys::constants as key;
            let k = e.keyval();
            let alt = e.state().contains(gtk::gdk::ModifierType::MOD1_MASK);
            if k == key::Escape {
                w.close();
                Propagation::Stop
            } else if alt && k == key::Left {
                f();
                Propagation::Stop
            } else {
                Propagation::Proceed
            }
        });
        b.window.connect_destroy(|_| {
            BROWSER.with(|w| w.borrow_mut().take());
        });

        b.window.show_all();
        b.spinner.hide();
        b.update_buttons();
        b.query.grab_focus();
        b
    }

    fn selected(&self) -> Vec<Entry> {
        let pages = self.pages.borrow();
        let Some(page) = pages.last() else { return Vec::new() };
        let (rows, model) = self.view.selection().selected_rows();
        rows.iter()
            .filter_map(|path| {
                let iter = model.iter(path)?;
                let i = model.value(&iter, COL_INDEX as i32).get::<u32>().ok()?;
                page.entries.get(i as usize).cloned()
            })
            .collect()
    }

    fn update_buttons(&self) {
        let sel = self.selected();
        let busy = *self.busy.borrow();
        self.play.set_sensitive(!sel.is_empty());
        self.add.set_sensitive(!sel.is_empty());
        self.open.set_sensitive(sel.len() == 1 && sel[0].kind != EntryKind::Track);
        self.uploader.set_sensitive(sel.len() == 1 && sel[0].by_list().is_some());
        let pages = self.pages.borrow();
        self.more.set_sensitive(!busy && pages.last().is_some_and(|p| !p.complete));
        self.back.set_sensitive(pages.len() > 1);
    }

    fn show_page(&self) {
        self.store.clear();
        let pages = self.pages.borrow();
        let Some(page) = pages.last() else {
            self.heading.set_text("");
            return;
        };
        self.heading.set_text(&page.title);
        for (i, e) in page.entries.iter().enumerate() {
            self.store.insert_with_values(
                None,
                &[
                    (COL_MARK, &mark(e.kind)),
                    (COL_TITLE, &e.title),
                    (COL_BY, &e.by.clone().unwrap_or_default()),
                    (COL_LENGTH, &length_text(e.length_ms)),
                    (COL_INDEX, &(i as u32)),
                ],
            );
        }
        self.status.set_text(&match page.entries.len() {
            0 => "Nothing found".to_owned(),
            1 => "1 result".to_owned(),
            n => format!("{n} results"),
        });
        drop(pages);
        self.update_buttons();
    }

    fn set_busy(&self, text: Option<&str>) {
        *self.busy.borrow_mut() = text.is_some();
        match text {
            Some(text) => {
                self.status.set_text(text);
                self.spinner.show();
                self.spinner.start();
            }
            None => {
                self.spinner.stop();
                self.spinner.hide();
            }
        }
        self.update_buttons();
    }

    /// Asks yt-dlp, in a thread, for entries of `source` from `from` on;
    /// then `done` gets them, unless another request came since.
    fn fetch(
        self: &Rc<Self>,
        source: Source,
        from: usize,
        done: impl FnOnce(&Browser, Result<Listing, String>) + 'static,
    ) {
        let request = {
            let mut r = self.request.borrow_mut();
            *r += 1;
            *r
        };
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| match &source {
                // A search starts over each time, with more results.
                Source::Search(what, query) => {
                    web::search(*what, query, from + PAGE).map(|mut l| {
                        l.entries.drain(..from.min(l.entries.len()));
                        l
                    })
                }
                Source::List(url) => web::list(url, from, PAGE),
            }))
            .unwrap_or_else(|_| Err("wmplayer: a bug in the search".to_owned()));
            let _ = tx.send(result);
        });
        let weak = Rc::downgrade(self);
        let mut done = Some(done);
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let result = match rx.try_recv() {
                Ok(r) => r,
                Err(TryRecvError::Empty) => return ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => Err("the search stopped".to_owned()),
            };
            if let Some(b) = weak.upgrade() {
                if *b.request.borrow() == request {
                    b.set_busy(None);
                    if let Some(done) = done.take() {
                        done(&b, result);
                    }
                    b.update_buttons();
                }
            }
            ControlFlow::Break
        });
    }

    fn search(self: &Rc<Self>) {
        let query = self.query.text().trim().to_owned();
        if query.is_empty() {
            return;
        }
        let what = Search::ALL[self.kind.active().unwrap_or(0) as usize];
        let page = web::classify(&query).is_some();
        self.set_busy(Some(if page {
            "Opening…"
        } else if what == Search::SoundCloudTracks {
            "Asking SoundCloud…"
        } else {
            "Asking YouTube…"
        }));
        let title = if page {
            query.clone()
        } else {
            format!("{}: “{query}”", what.label())
        };
        let source = Source::Search(what, query);
        self.fetch(source.clone(), 0, move |b, result| match result {
            Ok(listing) => {
                let complete = listing.entries.len() < PAGE;
                let title = if page { listing.title.unwrap_or(title) } else { title };
                *b.pages.borrow_mut() = vec![Page {
                    title,
                    source,
                    entries: listing.entries,
                    complete,
                }];
                b.show_page();
            }
            Err(e) => b.status.set_text(&e),
        });
    }

    fn open_list(self: &Rc<Self>, url: String, title: String) {
        self.set_busy(Some("Opening…"));
        let source = Source::List(url);
        self.fetch(source.clone(), 0, move |b, result| match result {
            Ok(listing) => {
                let complete = listing.entries.len() < PAGE;
                b.pages.borrow_mut().push(Page {
                    title: listing.title.unwrap_or(title),
                    source,
                    entries: listing.entries,
                    complete,
                });
                b.show_page();
            }
            Err(e) => b.status.set_text(&e),
        });
    }

    fn open_selected(self: &Rc<Self>) {
        if let [e] = self.selected().as_slice() {
            if e.kind != EntryKind::Track {
                self.open_list(e.url.clone(), e.title.clone());
            }
        }
    }

    fn open_uploader(self: &Rc<Self>) {
        if let [e] = self.selected().as_slice() {
            if let Some(url) = e.by_list() {
                self.open_list(url, e.by.clone().unwrap_or_default());
            }
        }
    }

    fn load_more(self: &Rc<Self>) {
        let Some((source, from)) = self
            .pages
            .borrow()
            .last()
            .map(|p| (p.source.clone(), p.entries.len()))
        else {
            return;
        };
        self.set_busy(Some("Asking for more…"));
        self.fetch(source, from, move |b, result| match result {
            Ok(listing) => {
                if let Some(page) = b.pages.borrow_mut().last_mut() {
                    page.complete = listing.entries.len() < PAGE;
                    page.entries.extend(listing.entries);
                }
                b.show_page();
            }
            Err(e) => b.status.set_text(&e),
        });
    }

    fn go_back(self: &Rc<Self>) {
        let mut pages = self.pages.borrow_mut();
        if pages.len() > 1 {
            pages.pop();
            drop(pages);
            // What was asked for on the page left behind is not wanted.
            *self.request.borrow_mut() += 1;
            self.set_busy(None);
            self.show_page();
        }
    }

    fn chosen(&self, play: bool) {
        let sel = self.selected();
        if sel.is_empty() {
            return;
        }
        add_to_playlist(&sel, play);
        let what = match sel.as_slice() {
            [e] => format!("“{}”", e.title),
            _ => format!("{} items", sel.len()),
        };
        self.status.set_text(&if play {
            format!("Playing {what}")
        } else {
            format!("Added {what}")
        });
    }

    fn activated(self: &Rc<Self>) {
        match self.selected().as_slice() {
            [e] if e.kind != EntryKind::Track => self.open_selected(),
            [] => {}
            _ => self.chosen(true),
        }
    }
}

/// Opens the window, or brings it to the front.
#[no_mangle]
pub extern "C" fn wmplayer_gtk_show_browser() {
    let _ = catch_unwind(|| {
        // libaudgui has set GTK up in this thread; gtk-rs is told so.
        if !gtk::is_initialized_main_thread() {
            unsafe { gtk::set_initialized() };
        }
        let open = BROWSER.with(|w| w.borrow().clone());
        match open {
            Some(b) => b.window.present(),
            None => {
                let b = Browser::new();
                BROWSER.with(|w| *w.borrow_mut() = Some(b));
            }
        }
    });
}

/// Closes the window, if it is open.
#[no_mangle]
pub extern "C" fn wmplayer_gtk_hide_browser() {
    let _ = catch_unwind(|| {
        let open = BROWSER.with(|w| w.borrow_mut().take());
        if let Some(b) = open {
            b.window.close();
        }
    });
}
