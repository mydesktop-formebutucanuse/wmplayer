# Copyright 2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

EAPI=8

RUST_MIN_VER="1.80.0"
inherit cargo git-r3 meson xdg

DESCRIPTION="Audacious with YouTube and SoundCloud built in"
HOMEPAGE="https://github.com/mydesktop-formebutucanuse/wmplayer"
EGIT_REPO_URI="https://github.com/mydesktop-formebutucanuse/wmplayer.git"

# The plugins (src/plugins, src/skins) are under their own terms, the skin
# Glare under CC-BY-SA.
LICENSE="BSD-2 CC-BY-SA-4.0 GPL-2+ GPL-3+ ISC"
SLOT="0"

# From the second group on: what the plugins (src/plugins) decode, fetch and
# play with.
DEPEND="
	>=dev-libs/glib-2.36:2
	net-misc/curl
	virtual/freedesktop-icon-theme
	x11-libs/cairo
	x11-libs/gdk-pixbuf:2
	>=x11-libs/gtk+-3.24:3[X]
	x11-libs/pango

	dev-libs/libxml2:2=
	media-libs/faad2
	>=media-libs/flac-1.2.1-r1:=
	>=media-libs/libogg-1.1.3
	>=media-libs/libvorbis-1.2.0
	>=media-libs/opusfile-0.4
	>=media-sound/mpg123-base-1.12
	>=media-video/ffmpeg-4.2.3:=
	>=media-video/pipewire-0.3.33:=
	>=net-libs/neon-0.27:=
"
# wmplayer has its own plugins, skins, translations and audtool, and needs
# nothing of Audacious. The two cannot be installed together: both have
# /usr/bin/audtool, and both answer to the same name on D-Bus.
RDEPEND="${DEPEND}
	net-misc/yt-dlp
	!media-sound/audacious
	!media-plugins/audacious-plugins
"
BDEPEND="
	>=dev-util/gdbus-codegen-2.80.5-r1
	sys-devel/gettext
	virtual/pkgconfig
"

src_unpack() {
	git-r3_src_unpack
	# cargo.eclass no longer changes into the source itself (gentoo commit
	# 0ed513f46179); the Rust code is in src/rust.
	pushd "${S}/src/rust" >/dev/null || die
	cargo_live_src_unpack
	popd >/dev/null || die
}

src_prepare() {
	default
	# This ebuild takes Audacious and its plugins off the machine. Source
	# from before wmplayer had plugins of its own (an older branch) would be
	# left with none.
	[[ -f src/plugins/meson.build ]] ||
		die "this source has no plugins of its own (src/plugins); it needs the ebuild that came with it"
}

src_configure() {
	local emesonargs=(
		-Ddbus=true
		-Dgtk=true
		-Dgtk2=false
		-Dqt=false
		-Dqt5=false
		-Dlibarchive=false
		-Dbuildstamp="Gentoo ${P}"
	)
	meson_src_configure
}

src_compile() {
	# meson runs cargo (src/rust/cargo-build.sh)
	cargo_env meson_src_compile
}

src_install() {
	meson_src_install
	einstalldocs
}
