/*
 * skins-config.h
 *
 * What the configuration of audacious-plugins gives the plugin, for the
 * build inside wmplayer.  Included after wmplayer's config.h.
 */

#ifndef SKINS_CONFIG_H
#define SKINS_CONFIG_H

/* The texts of the interface are translated in audacious-plugins. */
#undef PACKAGE
#define PACKAGE "audacious-plugins"

/* The menu entries of these plugins of Audacious; an entry does nothing if
 * its plugin is not installed. */
#define HAVE_PLAYBACK_HISTORY 1
#define HAVE_PLAYLIST_MANAGER 1
#define HAVE_SEARCH_TOOL 1

#endif
