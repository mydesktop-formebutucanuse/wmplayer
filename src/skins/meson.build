# The Winamp-skin interface: the "skins" plugin of audacious-plugins 4.6.2
# (GPL), changed to keep the skin's images in the X server (see README.md
# here). The skins themselves are in ../skins-data.

skins_sources = [
  'actions.cc',
  'button.cc',
  'dock.cc',
  'drag-handle.cc',
  'eq-graph.cc',
  'eq-slider.cc',
  'equalizer.cc',
  'hslider.cc',
  'main.cc',
  'menu-ops.cc',
  'menurow.cc',
  'menus.cc',
  'monostereo.cc',
  'number.cc',
  'playlistwin.cc',
  'playlist-slider.cc',
  'playlist-widget.cc',
  'playstatus.cc',
  'plugin.cc',
  'plugin-window.cc',
  'search-select.cc',
  'skin.cc',
  'skin-ini.cc',
  'skins_cfg.cc',
  'skins_util.cc',
  'skinselector.cc',
  'surface.cc',
  'svis.cc',
  'textbox.cc',
  'view.cc',
  'vis-callbacks.cc',
  'vis.cc',
  'widget.cc',
  'window.cc'
]


shared_module('skins',
  skins_sources,
  include_directories: [src_inc],
  # after config.h: what audacious-plugins has in its own config.h
  cpp_args: ['-include', join_paths(meson.current_source_dir(), 'skins-config.h')],
  dependencies: [gtk_dep, cxx.find_library('m', required: false)],
  link_with: [libaudcore_lib, libaudgui_lib],
  name_prefix: '',
  install: true,
  install_dir: join_paths(install_plugindir, 'General'),
  install_rpath: install_rpath
)
