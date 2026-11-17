/*
 * vfs_web.h
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

/* The pages of YouTube and SoundCloud as files: a track's page plays its
 * audio, and a playlist's or channel's page adds its tracks. The work is
 * done by wmplayer's Rust core (wmweb.h). */

#ifndef LIBAUDCORE_VFS_WEB_H
#define LIBAUDCORE_VFS_WEB_H

#include "index.h"
#include "playlist.h"
#include "plugin.h"

class WebTransport : public TransportPlugin
{
public:
    constexpr WebTransport() : TransportPlugin(PluginInfo(), nullptr) {}

    VFSImpl * fopen(const char * filename, const char * mode, String & error);
    VFSFileTest test_file(const char * filename, VFSFileTest test,
                          String & error);
};

extern WebTransport web_transport;

/* the page of a track, or of a list of tracks */
bool web_is_track(const char * filename);
bool web_is_list(const char * filename);

/* what the site says about the track */
bool web_read_tuple(const char * filename, Tuple & tuple, String & error);

/* the tracks of a list, with what the site says about each */
bool web_read_list(const char * filename, Index<PlaylistAddItem> & items,
                   String & title, String & error);

void web_init();
void web_cleanup();

#endif /* LIBAUDCORE_VFS_WEB_H */
