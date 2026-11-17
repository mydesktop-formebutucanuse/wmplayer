# The plugins wmplayer plays with: those of audacious-plugins 4.6.2 that are
# needed to play files and network streams through PipeWire, and the three
# tools the Winamp-skin interface has in its menus. Each is under the terms
# in its own source files (COPYING here). The interface itself is in ../skins.

plugins_args = ['-include', join_paths(meson.current_source_dir(), 'plugins-config.h')]

# for the one that asks for "config.h" by name
plugins_inc = [src_inc, include_directories('../..')]

gio_dep = dependency('gio-2.0', version: glib_req, required: true)
xml_dep = dependency('libxml-2.0', required: true)
neon_dep = dependency('neon', version: '>= 0.27', required: true)
libavcodec_dep = dependency('libavcodec', version: '>= 58.54.100', required: true)
libavformat_dep = dependency('libavformat', version: '>= 58.29.100', required: true)
libavutil_dep = dependency('libavutil', version: '>= 56.31.100', required: true)
faad_dep = dependency('faad2', required: false)
if not faad_dep.found()
  faad_dep = cxx.find_library('faad', has_headers: 'neaacdec.h', required: true)
endif
mpg123_dep = dependency('libmpg123', version: '>= 1.12', required: true)
flac_dep = dependency('flac', version: '>= 1.2.1', required: true)
ogg_dep = dependency('ogg', version: '>= 1.0', required: true)
vorbis_dep = dependency('vorbis', version: '>= 1.0', required: true)
vorbisenc_dep = dependency('vorbisenc', version: '>= 1.0', required: true)
vorbisfile_dep = dependency('vorbisfile', version: '>= 1.0', required: true)
opusfile_dep = dependency('opusfile', version: '>= 0.4', required: true)
pipewire_dep = dependency('libpipewire-0.3', version: '>= 0.3.33', required: true)
spa_dep = dependency('libspa-0.2', version: '>= 0.2', required: true)

# kind, name, sources, dependencies, whether it reads tags with libaudtag
plugins = [
  ['Transport', 'gio', ['gio/gio.cc'], [gio_dep], false],
  ['Transport', 'neon', ['neon/neon.cc', 'neon/cert_verification.cc'],
   [neon_dep, glib_dep], false],

  ['Container', 'asx', ['asx/asx.cc'], [], false],
  ['Container', 'asx3', ['asx3/asx3.cc'], [xml_dep], false],
  ['Container', 'audpl', ['audpl/audpl.cc'], [], false],
  ['Container', 'm3u', ['m3u/m3u.cc'], [], false],
  ['Container', 'pls', ['pls/pls.cc'], [], false],
  ['Container', 'xspf', ['xspf/xspf.cc'], [xml_dep, glib_dep], false],

  ['Input', 'ffaudio', ['ffaudio/ffaudio-core.cc', 'ffaudio/ffaudio-io.cc'],
   [libavcodec_dep, libavformat_dep, libavutil_dep], true],
  ['Input', 'aac-raw', ['aac/aac.cc'], [faad_dep], true],
  ['Input', 'madplug', ['mpg123/mpg123.cc'], [mpg123_dep], true],
  ['Input', 'flacng',
   ['flac/plugin.cc', 'flac/tools.cc', 'flac/seekable_stream_callbacks.cc',
    'flac/metadata.cc'],
   [flac_dep], false],
  ['Input', 'vorbis',
   ['vorbis/vcupdate.cc', 'vorbis/vcedit.cc', 'vorbis/vorbis.cc'],
   [ogg_dep, vorbis_dep, vorbisenc_dep, vorbisfile_dep, glib_dep], false],
  ['Input', 'opus', ['opus/opus.cc'], [glib_dep, opusfile_dep], false],

  ['Output', 'pipewire', ['pipewire/pipewire.cc'], [pipewire_dep, spa_dep],
   false],
]

foreach p : plugins
  shared_module(p[1],
    p[2],
    include_directories: plugins_inc,
    cpp_args: plugins_args,
    dependencies: p[3],
    link_with: p[4] ? [libaudcore_lib, libaudtag_lib] : [libaudcore_lib],
    name_prefix: '',
    install: true,
    install_dir: join_paths(install_plugindir, p[0]),
    install_rpath: install_rpath
  )
endforeach

if get_option('gtk')
  gtk_plugins = [
    ['playlist-manager', ['playlist-manager/playlist-manager.cc']],
    ['search-tool',
     ['search-tool/library.cc', 'search-tool/search-model.cc',
      'search-tool/search-tool.cc']],
    ['playback-history',
     ['playback-history/history-entry.cc',
      'playback-history/playback-history.cc']],
  ]

  foreach p : gtk_plugins
    shared_module(p[0],
      p[1],
      include_directories: plugins_inc,
      cpp_args: plugins_args,
      dependencies: [gtk_dep],
      link_with: [libaudcore_lib, libaudgui_lib],
      name_prefix: '',
      install: true,
      install_dir: join_paths(install_plugindir, 'General'),
      install_rpath: install_rpath
    )
  endforeach
endif
