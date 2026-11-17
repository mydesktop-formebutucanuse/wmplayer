# Copyright 2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

EAPI=8

RUST_MIN_VER="1.80.0"
inherit cargo git-r3 meson xdg

DESCRIPTION="Audacious with YouTube and SoundCloud built in"
HOMEPAGE="https://github.com/mydesktop-formebutucanuse/wmplayer"
EGIT_REPO_URI="https://github.com/mydesktop-formebutucanuse/wmplayer.git"
# master follows Audacious; wmplayer is on its own branch.
EGIT_BRANCH="wmplayer"

LICENSE="BSD-2"
SLOT="0"

DEPEND="
	>=dev-libs/glib-2.36:2
	net-misc/curl
	virtual/freedesktop-icon-theme
	x11-libs/cairo
	x11-libs/gdk-pixbuf:2
	>=x11-libs/gtk+-3.24:3
	x11-libs/pango
"
# wmplayer runs Audacious's plugins, with Audacious's skins and translations,
# from where Audacious has them; its own libaudcore, libaudgui and libaudtag
# are in /usr/lib*/wmplayer. The plugin API is that of Audacious 4.6.
RDEPEND="${DEPEND}
	=media-sound/audacious-4.6*[gtk]
	=media-plugins/audacious-plugins-4.6*[ffmpeg,gtk]
	net-misc/yt-dlp
"
BDEPEND="
	>=dev-util/gdbus-codegen-2.80.5-r1
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
