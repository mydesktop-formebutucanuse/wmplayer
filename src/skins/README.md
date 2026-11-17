# The Winamp-skin interface

This is the `skins` plugin of Audacious (audacious-plugins 4.6.2, `src/skins`;
`src/ui-common` holds the three menu files it shares with the GTK interface),
built by wmplayer as `General/skins.so`. The skins themselves are in
`src/skins-data`, the translations in `po-plugins`, both from
audacious-plugins too.

The code is under the GNU General Public License, version 3 (see the head of
each file); it is a module of its own, as it is in Audacious.

## What wmplayer changes

The skin's images are kept in the X server, so a redraw copies from a pixmap
to the window inside the server (XCopyArea or XRender, on the GPU where the
server draws with it) instead of uploading the pixels each time:

* `surface.h`, `surface.cc`: `ServerSurface`, a copy of an image surface in the
  server, scaled in advance for the scale it is drawn at, and
  `surface_draw_rect()`. The copy is made when it is first drawn from and made
  anew when the display, the window scale or the scale factor has changed. If
  the window is not an X11 window, the image is drawn from as before.
* `skin.h`, `skin.cc`: `skin_draw_pixbuf()` draws from the copies
  (`Skin::server_pixmaps`).
* `textbox.h`, `textbox.cc`: the rendered text is kept in the server too, so a
  step of the scrolling title is a copy.
* `skins_cfg.h`, `skins_cfg.cc`: `server_pixmaps` in the `[skins]` section of
  the configuration (default `TRUE`); `FALSE` draws from the images as
  Audacious does.
* `plugin.cc`: a skin of Audacious's that the settings name (they are carried
  over from Audacious) and that is no longer there is taken from wmplayer's
  skins; the user's own skins are in `~/.local/share/wmplayer/Skins`.
* `meson.build`, `skins-config.h`: the build inside wmplayer.

The visualiser is drawn from an image as before: its pixels change with every
frame.

## Bringing in a newer Audacious

Copy `src/skins/*.cc`, `src/skins/*.h` and `src/ui-common/menu-ops*` of the new
audacious-plugins over these, and apply the changes listed above again
(`git diff` of this directory against the import shows them).
