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
  subdir('skins')
endif


if get_option('dbus')
  subdir('dbus')
endif


subdir('audacious')
