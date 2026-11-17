# wmplayer

**wmplayer** is [Audacious](https://audacious-media-player.org) with YouTube
and SoundCloud built in. It is a fork: the player, its plugins and its skins
are Audacious's, and what follows below is Audacious's own README.

What wmplayer adds:

* **Services → YouTube and SoundCloud …** (in the main menu and in the
  playlist's Add menu): search videos, playlists, channels or SoundCloud
  tracks, open a playlist, channel or uploader, and play or add what is chosen.
* The address of a page plays like a file: a video or track
  (`wmplayer https://www.youtube.com/watch?v=…`, *Add URL*, drag and drop),
  and a playlist, channel or SoundCloud user or set, whose tracks are added.
* Tracks are downloaded as they play (`~/.cache/wmplayer/web`, kept for a week
  after they were last played), so seeking works, and the next track of the
  playlist is fetched ahead.

How it is put together:

* The new code is Rust, in `src/rust`: `wmplayer` (the sites, yt-dlp, the
  downloads), `core-ffi` (its C interface, linked into libaudcore) and
  `gtk-ui` (the window, linked into libaudgui). The C++ side is
  `src/libaudcore/vfs_web.cc` and `src/libaudgui/web-browser.cc`.
* It needs `yt-dlp` at run time, and `cargo` and libcurl to build.
* wmplayer installs next to Audacious and uses its plugins
  (`LIBDIR/audacious`), data (`DATADIR/audacious`) and translations. Its own
  libaudcore, libaudgui and libaudtag are in `LIBDIR/wmplayer`. `audtool`
  controls it (the D-Bus name is Audacious's, so only one of the two runs at a
  time).
* Settings are in `~/.config/wmplayer`; the first start copies
  `~/.config/audacious`.
* Not yet: live streams, encrypted HLS, the Qt interface.

Build and install:

    meson setup build && ninja -C build && sudo ninja -C build install

or, on Gentoo, `sudo sh gentoo/install.sh` (a live ebuild in a local overlay).

---

# Audacious

## Overview

**Audacious** is a lightweight, open-source audio player focused on high audio
quality and low resource usage. It supports a wide range of audio formats and
provides a flexible plugin system for extending functionality.

## Features

* Fast and lightweight with low system resource usage
* Runs on Linux, BSD derivatives, macOS and Windows
* Wide support for audio formats
* Plugin-based architecture
* Multiple user interfaces:
  * GTK interface
  * Qt interface
  * Winamp interface, including support for Winamp 2 skins
* Advanced playlist management
* Built-in equalizer, audio effects and visualizations
* Support for ReplayGain and gapless playback
* Controllable also via MPRIS and with `audtool`

## Installation

### Prebuilt Packages

Most Linux distributions provide Audacious in their respective package
repositories. For Windows we offer an installer. On macOS, Audacious can be
installed using [Homebrew](https://brew.sh).

There are also Flatpak and Snap packages of Audacious, however we do neither
provide nor support them. So if you have an issue with these packages, make sure
it also affects ordinary installation methods before reporting it to us.

See also the [download instructions](https://audacious-media-player.org/download)
on our website.

### Building from Source

#### Requirements

* Meson
* GCC or Clang
* pkg-config
* Required development libraries (GTK/Qt, GLib, audio backends, codecs, etc.)

Reading our build pipeline [installation script](/.github/actions/install-dependencies/install-dependencies.sh)
may help with the package names when using Ubuntu or macOS.

#### Build Steps

After downloading the source code, you need to create a build directory (e.g.
`meson setup build`). To view a list of all supported build options, you can use
the command `meson configure build`. Alternatively inspect the file `meson_options.txt`.
Logs can be found in `meson-logs` within your build directory.

Example:

```bash
meson setup build      # pass custom build options with -D option=value
meson compile -C build # start building
meson install -C build # install the built software
```

Consult the official Meson documentation for more details, e.g.
[Running Meson](https://mesonbuild.com/Running-Meson.html) and
[Configuring a build directory](https://mesonbuild.com/Configuring-a-build-directory.html).

## Plugins

Audacious uses a modular plugin system to provide most of its functionality.
Many plugins are included in the [audacious-plugins](https://github.com/audacious-media-player/audacious-plugins)
package, officially provided and supported by the Audacious developers.
But there are also third-party plugins available, install and use them at your
own risk though.

Plugins can be enabled or disabled at runtime within the Audacious settings window.
When using the GTK or Qt interface, several plugins (e.g. album cover, lyrics,
file browser, visualizations) can be docked into the main window.

### Plugin Types

* **General plugins** – provide additional features (e.g. lyrics viewer, last.fm, playlist manager)
* **Effect plugins** – apply audio processing (e.g. echo, equalizer, speed and pitch)
* **Visualization plugins** – display visual effects (e.g. spectrum analyzer, VU meter)
* **Input plugins** – decode audio formats (e.g. MP3, FLAC, Ogg Vorbis, Opus)
* **Output plugins** – handle audio playback devices (e.g. ALSA, PipeWire, PulseAudio)
* **Playlist plugins** – offer support for various playlist formats (e.g. M3U, ASX, Cuesheets)
* **Transport plugins** – allow reading from streams or file shares (e.g. HTTPS, FTP, SMB)

## Supported Formats

Supported audio formats depend on enabled plugins but typically include:

* MP3
* FLAC
* Ogg Vorbis
* Opus
* AAC / M4A
* WAV
* WavPack
* Ad-lib chiptunes
* Audio CD
* MIDI
* Nintendo DS (2SF)
* PlayStation Audio (PSF)
* Tracker formats (MOD, XM, etc.)
* … and many more (through FFmpeg)

## Troubleshooting

* Read [Common Problems](https://audacious-media-player.org/problems) on our website
* Make sure to use the latest Audacious version
* Enable verbose output (with `audacious -V` on the command line)
* Check our issue tracker and forum for similar reports (also closed ones)

## Contributing

Contributions are welcome!

### How to Contribute

* Submit pull requests for fixes or feature requests
* Support and help others in our forum
* Report issues in our bug tracker
* Translate Audacious into your native language
* Recommend and advertise Audacious to your friends

Before submitting large changes, consider opening a forum thread first to
discuss your ideas. Also remember that the developers of Audacious are
volunteers and can only spend a limited amount of their free time on the project.

## License

Audacious is licensed under the BSD-2-Clause license.
Please note that many of the plugins distributed with Audacious are under
different licenses. See the [COPYING](/COPYING) file and the respective
plugin source code for more details.

## Links

* Homepage: https://audacious-media-player.org
* Source Code Repository: https://github.com/audacious-media-player
* Issue Tracker: https://github.com/audacious-media-player/audacious/issues
* Forum: https://github.com/orgs/audacious-media-player/discussions
* Translations: https://app.transifex.com/audacious
