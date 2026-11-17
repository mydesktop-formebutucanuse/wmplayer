/*
 * vfs_web.cc
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

#include "vfs_web.h"

#include <limits.h>
#include <string.h>

#include "audstrings.h"
#include "hook.h"
#include "i18n.h"
#include "runtime.h"
#include "tuple.h"
#include "wmweb.h"

WebTransport web_transport;

/* how many tracks of a playlist or channel are added at most */
#define WEB_LIST_MAX 1000

static String take_error(char * error)
{
    String s(error ? error : _("Unknown error"));
    wmplayer_free_string(error);
    return s;
}

class WebFile : public VFSImpl
{
public:
    explicit WebFile(WmwebStream * stream) : m_stream(stream) {}
    ~WebFile() { wmweb_close(m_stream); }

    int64_t fread(void * ptr, int64_t size, int64_t nmemb)
    {
        if (size <= 0 || nmemb <= 0)
            return 0;

        int64_t want = size * nmemb, got = 0;
        while (got < want)
        {
            int64_t n = wmweb_read(m_stream, (char *)ptr + got, want - got);
            if (n <= 0)
                break;
            got += n;
        }

        return got / size;
    }

    int fseek(int64_t offset, VFSSeekType whence)
    {
        if (whence != VFS_SEEK_SET && whence != VFS_SEEK_CUR &&
            whence != VFS_SEEK_END)
            return -1;

        return wmweb_seek(m_stream, offset, (int)whence);
    }

    int64_t ftell() { return wmweb_tell(m_stream); }
    int64_t fsize() { return wmweb_size(m_stream); }
    bool feof() { return wmweb_eof(m_stream); }

    int64_t fwrite(const void *, int64_t, int64_t) { return 0; }
    int ftruncate(int64_t) { return -1; }
    int fflush() { return 0; }

    String get_metadata(const char * field)
    {
        if (!strcmp(field, "content-type"))
            return String(wmweb_mime(m_stream));
        /* for probe.cc: the page's address has no extension */
        if (!strcmp(field, "file-extension"))
            return String(wmweb_ext(m_stream));

        return String();
    }

private:
    WmwebStream * m_stream;
};

VFSImpl * WebTransport::fopen(const char * filename, const char * mode,
                              String & error)
{
    if (mode[0] != 'r' || strchr(mode, '+'))
    {
        error = String(_("Web pages can only be read"));
        return nullptr;
    }

    char * err = nullptr;
    WmwebStream * stream = wmweb_open(filename, &err);
    if (!stream)
    {
        error = take_error(err);
        AUDERR("%s: %s\n", filename, (const char *)error);
        return nullptr;
    }

    return new WebFile(stream);
}

VFSFileTest WebTransport::test_file(const char * filename, VFSFileTest test,
                                   String & error)
{
    return VFSFileTest(test & (VFS_IS_REGULAR | VFS_EXISTS));
}

static bool is_http(const char * filename)
{
    return !strncmp(filename, "https://", 8) || !strncmp(filename, "http://", 7);
}

bool web_is_track(const char * filename)
{
    return is_http(filename) && wmweb_kind(filename) == 1;
}

bool web_is_list(const char * filename)
{
    return is_http(filename) && wmweb_kind(filename) == 2;
}

static Tuple make_tuple(const char * filename, const WmwebInfo & info)
{
    Tuple tuple;
    tuple.set_filename(filename);

    if (info.title)
        tuple.set_str(Tuple::Title, info.title);
    if (info.artist)
        tuple.set_str(Tuple::Artist, info.artist);
    if (info.album)
        tuple.set_str(Tuple::Album, info.album);
    if (info.length_ms >= 0)
        tuple.set_int(Tuple::Length, aud::min(info.length_ms, (int64_t)INT_MAX));

    tuple.set_state(Tuple::Valid);
    return tuple;
}

bool web_read_tuple(const char * filename, Tuple & tuple, String & error)
{
    WmwebInfo info{};
    char * err = nullptr;

    if (!wmweb_info(filename, &info, &err))
    {
        error = take_error(err);
        return false;
    }

    tuple = make_tuple(filename, info);
    wmweb_info_clear(&info);
    return true;
}

bool web_read_list(const char * filename, Index<PlaylistAddItem> & items,
                   String & title, String & error)
{
    char * err = nullptr;
    WmwebList * list = wmweb_list(filename, WEB_LIST_MAX, &err);

    if (!list)
    {
        error = take_error(err);
        return false;
    }

    title = String(list->title);

    for (size_t i = 0; i < list->n_items; i++)
    {
        const WmwebInfo & info = list->items[i];
        if (info.url)
            items.append(String(info.url), make_tuple(info.url, info));
    }

    wmweb_list_free(list);
    return true;
}

/* Starts downloading the track that comes next, so that it plays at once. */
static void prefetch_next(void *, void *)
{
    auto playlist = Playlist::playing_playlist();
    if (!playlist.exists())
        return;

    int next = -1;
    if (playlist.n_queued())
        next = playlist.queue_get_entry(0);
    else if (!aud_get_bool("shuffle") && !aud_get_bool("no_playlist_advance"))
    {
        next = playlist.get_position() + 1;
        if (next >= playlist.n_entries())
            next = aud_get_bool("repeat") ? 0 : -1;
    }

    if (next < 0)
        return;

    String filename = playlist.entry_filename(next);
    if (filename && web_is_track(filename))
        wmweb_prefetch(filename);
}

void web_init()
{
    /* what is left half downloaded is removed; another instance may be
     * downloading, though */
    if (aud_get_instance() == 1)
        wmweb_init();

    hook_associate("playback ready", prefetch_next, nullptr);
}

void web_cleanup() { hook_dissociate("playback ready", prefetch_next); }
