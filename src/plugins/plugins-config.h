/*
 * plugins-config.h
 *
 * What the configuration of audacious-plugins gives its plugins, for the
 * build inside wmplayer.  Included after wmplayer's config.h, each time that
 * is included.
 */

/* The plugins have their texts in a gettext domain of their own
 * (po-plugins). */
#undef PACKAGE
#define PACKAGE "wmplayer-plugins"
