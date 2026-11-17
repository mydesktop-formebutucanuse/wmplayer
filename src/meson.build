src_inc = include_directories('.')


subdir('rust')
subdir('libguess')
subdir('libaudcore')
subdir('libaudtag')


if get_option('qt')
  subdir('libaudqt')
endif


if get_option('gtk')
  subdir('libaudgui')
endif


subdir('plugins')


if get_option('gtk')
  subdir('skins')
  subdir('skins-data')
endif


if get_option('dbus')
  subdir('dbus')
  subdir('audtool')
endif


subdir('audacious')
