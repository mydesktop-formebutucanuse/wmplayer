/*
 * wmweb.h
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

/* YouTube and SoundCloud, from wmplayer's Rust core (src/rust/core-ffi).
 * Strings the core returns are freed with wmplayer_free_string(). */

#ifndef LIBAUDCORE_WMWEB_H
#define LIBAUDCORE_WMWEB_H

#include <stddef.h>
#include <stdint.h>

extern "C" {

struct WmwebStream;

struct WmwebInfo
{
    char * url;
    char * title;
    char * artist;
    char * album;
    int64_t length_ms; /* -1 if not known */
};

struct WmwebList
{
    char * title;
    WmwebInfo * items;
    size_t n_items;
};

void wmplayer_free_string(char * s);

/* copies the directory <old> to <new> if <new> does not exist yet:
 * 1 copied, 0 nothing to do, -1 failed */
int wmplayer_migrate_config(const char * old_dir, const char * new_dir);

/* clears what an earlier run left in the cache */
void wmweb_init();

/* 0: not a page of a web site, 1: a track, 2: a list of them (a playlist,
 * a channel, a SoundCloud user or set) */
int wmweb_kind(const char * uri);

/* the track's audio; waits until its start has been downloaded */
WmwebStream * wmweb_open(const char * uri, char ** error);
int64_t wmweb_read(WmwebStream * s, void * buf, int64_t len); /* 0 end, -1 error */
int wmweb_seek(WmwebStream * s, int64_t offset, int whence);  /* 0 or -1 */
int64_t wmweb_tell(const WmwebStream * s);
/* -1 while not known; of AAC that is still downloading, about what it
 * will be */
int64_t wmweb_size(const WmwebStream * s);
int wmweb_eof(const WmwebStream * s);
const char * wmweb_mime(const WmwebStream * s); /* owned by the stream */
const char * wmweb_ext(const WmwebStream * s);  /* owned by the stream */
void wmweb_close(WmwebStream * s);

/* starts downloading the track, if it is not downloaded or downloading */
void wmweb_prefetch(const char * uri);

/* 1 and <info> (cleared with wmweb_info_clear()), or 0 and <error> */
int wmweb_info(const char * uri, WmwebInfo * info, char ** error);
void wmweb_info_clear(WmwebInfo * info);

/* the tracks of a list, up to <max>; or null and <error> */
WmwebList * wmweb_list(const char * uri, size_t max, char ** error);
void wmweb_list_free(WmwebList * list);

} // extern "C"

#endif /* LIBAUDCORE_WMWEB_H */
