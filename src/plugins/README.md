# The plugins

These are plugins of Audacious (audacious-plugins 4.6.2, the directory of the
same name under its `src`), built by wmplayer so that it needs nothing of
Audacious installed. Each is under the terms at the head of its files
(`COPYING` is audacious-plugins's).

| Kind      | Plugins                                                        |
|-----------|----------------------------------------------------------------|
| Transport | `gio` (files), `neon` (HTTP and HTTPS)                         |
| Container | `asx`, `asx3`, `audpl`, `m3u`, `pls`, `xspf`                   |
| Input     | `ffaudio` (FFmpeg), `aac` (AAC as ADTS, which SoundCloud's tracks are written as), `mpg123`, `flac`, `vorbis`, `opus` |
| Output    | `pipewire`                                                     |
| General   | `playlist-manager`, `search-tool`, `playback-history` (in the menus of the Winamp-skin interface, `../skins`) |

Not here: the other outputs (ALSA, PulseAudio, JACK, …), the effects, the
visualisations, the GTK and Qt interfaces, MPRIS, notifications, the decoders
for tracker, chip and console music, CD audio, and everything else of
audacious-plugins.

## What wmplayer changes

* `meson.build`, `plugins-config.h`: the build inside wmplayer; all of these
  are built, and their libraries are required. The plugins' own `meson.build`
  files are left out.
* `mpg123/mpg123.cc`: `plugins-config.h` after its second `config.h`.
* The translations (`po-plugins`) have only the texts of these plugins and of
  the Winamp-skin interface.

## Adding one

Copy its directory from audacious-plugins, add it to the list in
`meson.build` with the libraries its own `meson.build` asks for, and add its
texts to `po-plugins` (`msggrep -N 'src/<name>/*'` on audacious-plugins's
`po` files gives them).
