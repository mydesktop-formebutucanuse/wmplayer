# wmplayer's own code, in Rust: the YouTube and SoundCloud core, for
# libaudcore, and the window to look through them, for libaudgui. cargo
# builds a static library of each.

cargo = find_program('cargo')

rust_outputs = ['libwmplayer_core.a']
if get_option('gtk')
  rust_outputs += ['libwmplayer_gtk.a']
endif

rust_libs = custom_target('wmplayer-rust',
  output: rust_outputs,
  command: [
    files('cargo-build.sh'), cargo,
    meson.current_source_dir(), meson.current_build_dir(), '@OUTPUT@'
  ],
  console: true,
  build_always_stale: true
)

# What Rust's standard library needs, and libcurl for the downloads.
rust_sys_deps = [
  dependency('libcurl'),
  thread_dep,
  cxx.find_library('dl', required: false),
  cxx.find_library('m', required: false),
  cxx.find_library('rt', required: false),
  cxx.find_library('util', required: false)
]

# The archives' own symbols stay inside the library they are linked into.
wmplayer_core_dep = declare_dependency(
  link_with: rust_libs[0],
  link_args: ['-Wl,--exclude-libs,libwmplayer_core.a'],
  dependencies: rust_sys_deps
)

if get_option('gtk')
  wmplayer_gtk_dep = declare_dependency(
    link_with: rust_libs[1],
    link_args: ['-Wl,--exclude-libs,libwmplayer_gtk.a'],
    dependencies: rust_sys_deps
  )
endif
