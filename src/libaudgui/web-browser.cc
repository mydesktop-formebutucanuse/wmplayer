/*
 * web-browser.cc
 * Copyright 2026 wmplayer developers
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 *    this list of conditions, and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 *    this list of conditions, and the following disclaimer in the documentation
 *    provided with the distribution.
 *
 * This software is provided "as is" and without any warranty, express or
 * implied. In no event shall the authors be liable for any damages arising from
 * the use of this software.
 */

/* The YouTube and SoundCloud window. The window itself is wmplayer's Rust
 * code (src/rust/gtk-ui); here it is put in the Services menus and given
 * the playlist and the settings. */

#include <limits.h>
#include <stddef.h>
#include <stdint.h>

#include <libaudcore/audstrings.h>
#include <libaudcore/interface.h>
#include <libaudcore/playlist.h>
#include <libaudcore/runtime.h>
#include <libaudcore/tuple.h>

#include "internal.h"
#include "libaudgui.h"

#define CFG_SECTION "wmplayer-web"

extern "C" {

struct WmplayerGuiItem
{
    const char * url;
    const char * title; /* null: a list, which the core opens */
    const char * artist;
    const char * album;
    int64_t length_ms; /* -1 if not known */
};

/* from the Rust code */
void wmplayer_gtk_show_browser ();
void wmplayer_gtk_hide_browser ();

/* for the Rust code */
void wmplayer_gui_add (const WmplayerGuiItem * items, size_t n_items, int play);
int wmplayer_gui_get_int (const char * name);
void wmplayer_gui_set_int (const char * name, int value);

} // extern "C"

void wmplayer_gui_add (const WmplayerGuiItem * items, size_t n_items, int play)
{
    Index<PlaylistAddItem> add;

    for (size_t i = 0; i < n_items; i ++)
    {
        const WmplayerGuiItem & item = items[i];
        if (! item.url)
            continue;

        Tuple tuple;

        if (item.title)
        {
            tuple.set_filename (item.url);
            tuple.set_str (Tuple::Title, item.title);
            if (item.artist)
                tuple.set_str (Tuple::Artist, item.artist);
            if (item.album)
                tuple.set_str (Tuple::Album, item.album);
            if (item.length_ms >= 0)
                tuple.set_int (Tuple::Length, aud::min (item.length_ms, (int64_t) INT_MAX));

            tuple.set_state (Tuple::Valid);
        }

        add.append (String (item.url), std::move (tuple));
    }

    if (add.len ())
        Playlist::active_playlist ().insert_items (-1, std::move (add), play);
}

int wmplayer_gui_get_int (const char * name)
{
    return aud_get_int (CFG_SECTION, name);
}

void wmplayer_gui_set_int (const char * name, int value)
{
    aud_set_int (CFG_SECTION, name, value);
}

static void show_browser ()
{
    wmplayer_gtk_show_browser ();
}

static const char menu_label[] = "YouTube and SoundCloud …";
static const AudMenuID menu_ids[] = {AudMenuID::Main, AudMenuID::PlaylistAdd};

void web_browser_init ()
{
    for (AudMenuID id : menu_ids)
        audgui_plugin_menu_add (id, show_browser, menu_label, "applications-internet");
}

void web_browser_cleanup ()
{
    wmplayer_gtk_hide_browser ();

    for (AudMenuID id : menu_ids)
        audgui_plugin_menu_remove (id, show_browser);
}
