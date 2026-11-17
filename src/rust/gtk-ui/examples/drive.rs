//! Drives the YouTube and SoundCloud window by itself, to see that it
//! works: searches, opens a playlist, goes to an uploader, plays and adds;
//! what would go to the playlist is printed. With a directory as argument,
//! pictures of the window are saved there.
//!
//!     cargo run --release -p wmplayer-gtk --example drive -- /tmp/pictures

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_int, CStr};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib::{self, ControlFlow};
use gtk::prelude::*;

#[repr(C)]
pub struct GuiItem {
    url: *const c_char,
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
    length_ms: i64,
}

fn text(p: *const c_char) -> String {
    if p.is_null() {
        "-".to_owned()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

#[no_mangle]
pub extern "C" fn wmplayer_gui_add(items: *const GuiItem, n_items: usize, play: c_int) {
    for item in unsafe { std::slice::from_raw_parts(items, n_items) } {
        println!(
            "  {} {} | {} | {} | {} | {} ms",
            if play != 0 { "play" } else { "add" },
            text(item.url),
            text(item.title),
            text(item.artist),
            text(item.album),
            item.length_ms
        );
    }
}

#[no_mangle]
pub extern "C" fn wmplayer_gui_get_int(_name: *const c_char) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn wmplayer_gui_set_int(_name: *const c_char, _value: c_int) {}

fn widgets(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
    out.push(w.clone());
    if let Some(c) = w.downcast_ref::<gtk::Container>() {
        for child in c.children() {
            widgets(&child, out);
        }
    }
}

struct Ui {
    window: gtk::Window,
    kind: gtk::ComboBoxText,
    query: gtk::Entry,
    view: gtk::TreeView,
    status: gtk::Label,
    heading: gtk::Label,
    buttons: Vec<gtk::Button>,
}

impl Ui {
    fn find() -> Ui {
        let window = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.role().as_deref() == Some("web-browser"))
            .expect("the window");
        let mut all = Vec::new();
        widgets(window.upcast_ref(), &mut all);
        let labels: Vec<gtk::Label> = all.iter().cloned()
            .filter_map(|w| w.downcast().ok())
            .filter(|l: &gtk::Label| !l.parent().is_some_and(|p| p.is::<gtk::Button>()))
            .collect();
        Ui {
            kind: all.iter().cloned().find_map(|w| w.downcast().ok()).expect("the combo box"),
            query: all.iter().cloned().find_map(|w| w.downcast().ok()).expect("the entry"),
            view: all.iter().cloned().find_map(|w| w.downcast().ok()).expect("the list"),
            heading: labels[0].clone(),
            status: labels[labels.len() - 1].clone(),
            buttons: all.iter().cloned().filter_map(|w| w.downcast().ok()).collect(),
            window,
        }
    }

    fn click(&self, label: &str) {
        let b = self
            .buttons
            .iter()
            .find(|b| b.label().is_some_and(|l| l.contains(label)))
            .unwrap_or_else(|| panic!("no button {label}"));
        println!("[{label}]{}", if b.is_sensitive() { "" } else { " (insensitive!)" });
        b.clicked();
    }

    fn rows(&self) -> i32 {
        self.view.model().map_or(0, |m| m.iter_n_children(None))
    }

    fn select(&self, row: i32) {
        let sel = self.view.selection();
        sel.unselect_all();
        sel.select_path(&gtk::TreePath::from_indicesv(&[row]));
    }

    fn print(&self) {
        println!("“{}”: {} rows; {}", self.heading.text(), self.rows(), self.status.text());
        let Some(model) = self.view.model() else { return };
        for row in 0..self.rows().min(4) {
            let iter = model.iter_nth_child(None, row).unwrap();
            let col = |c: i32| model.value(&iter, c).get::<String>().unwrap_or_default();
            println!("  {} {} | {} | {}", col(0), col(1), col(2), col(3));
        }
    }

    fn picture(&self, dir: &Option<PathBuf>, name: &str) {
        let (Some(dir), Some(w)) = (dir, self.window.window()) else { return };
        if let Some(p) = w.pixbuf(0, 0, w.width(), w.height()) {
            let _ = p.savev(dir.join(format!("{name}.png")), "png", &[]);
        }
    }

    fn search(&self, kind: u32, query: &str) {
        println!("search {kind}: {query}");
        self.kind.set_active(Some(kind));
        self.query.set_text(query);
        self.query.emit_activate();
    }
}

fn main() {
    let dir = std::env::args().nth(1).map(PathBuf::from);
    gtk::init().expect("GTK");
    wmplayer_gtk::wmplayer_gtk_show_browser();
    let ui = Rc::new(Ui::find());
    ui.window.connect_destroy(|_| gtk::main_quit());

    // Each step runs once the one before it has its answer (the status
    // line no longer ends in "…").
    type Step = Box<dyn Fn(&Ui, &Option<PathBuf>)>;
    let steps: Vec<Step> = vec![
        Box::new(|ui, _| ui.search(0, "lofi hip hop")),
        Box::new(|ui, dir| {
            ui.print();
            ui.picture(dir, "1-videos");
            ui.select(0);
            ui.click("Play");
            ui.select(1);
            ui.click("Add to playlist");
            ui.click("More");
        }),
        Box::new(|ui, _| {
            ui.print();
            ui.select(0);
            ui.click("Uploader");
        }),
        Box::new(|ui, dir| {
            ui.print();
            ui.picture(dir, "2-uploader");
            ui.click("Back");
            ui.search(1, "lofi hip hop");
        }),
        Box::new(|ui, _| {
            ui.print();
            ui.select(0);
            ui.click("Add to playlist");
            ui.click("Open");
        }),
        Box::new(|ui, dir| {
            ui.print();
            ui.picture(dir, "3-playlist");
            ui.search(2, "lofi girl");
        }),
        Box::new(|ui, _| {
            ui.print();
            ui.search(3, "flickermood");
        }),
        Box::new(|ui, dir| {
            ui.print();
            ui.picture(dir, "4-soundcloud");
            ui.select(0);
            ui.click("Uploader");
        }),
        Box::new(|ui, _| {
            ui.print();
            ui.search(0, "https://www.youtube.com/watch?v=jNQXAC9IVRw");
        }),
        Box::new(|ui, _| {
            ui.print();
            ui.window.close();
        }),
    ];

    let next = Cell::new(0);
    let since = RefCell::new(Instant::now());
    let d = dir.clone();
    glib::timeout_add_local(Duration::from_millis(300), move || {
        let waiting = ui.status.text().ends_with('…');
        if waiting && since.borrow().elapsed() < Duration::from_secs(90) {
            return ControlFlow::Continue;
        }
        if waiting {
            println!("no answer in 90 s");
            gtk::main_quit();
            return ControlFlow::Break;
        }
        let Some(step) = steps.get(next.get()) else { return ControlFlow::Break };
        step(&ui, &d);
        next.set(next.get() + 1);
        *since.borrow_mut() = Instant::now();
        ControlFlow::Continue
    });
    gtk::main();
}
