/*
 * skins-config.h
 *
 * What the configuration of audacious-plugins gives the plugin, for the
 * build inside wmplayer.  Included after wmplayer's config.h.
 */

#ifndef SKINS_CONFIG_H
#define SKINS_CONFIG_H

/* the gettext domain of the plugins */
#include "../plugins/plugins-config.h"

/* The menu entries of these plugins (src/plugins). */
#define HAVE_PLAYBACK_HISTORY 1
#define HAVE_PLAYLIST_MANAGER 1
#define HAVE_SEARCH_TOOL 1

#endif
