#!/bin/sh
# Install wmplayer with emerge: puts the live ebuild
# media-sound/wmplayer-9999 into a local overlay, accepts its (empty)
# keywords, turns off what this machine does not use in Audacious's plugins
# (package.use) and emerges it. Run as root, again whenever the ebuild or
# package.use changes:
#   sudo sh gentoo/install.sh [OVERLAY]   default overlay: mydesktop
# Code updates need no rerun: the ebuild builds the latest commit on GitHub,
#   sudo emerge --oneshot media-sound/wmplayer
set -eu

here=$(cd "$(dirname "$0")" && pwd)
pkg=media-sound/wmplayer
ebuild=wmplayer-9999.ebuild
overlay=${1:-mydesktop}
keywords=/etc/portage/package.accept_keywords/$overlay
use=/etc/portage/package.use

if [ "$(id -u)" -ne 0 ]; then
	echo "Run as root: sudo sh $0 $*" >&2
	exit 1
fi

repo=$(portageq get_repo_path / "$overlay" 2>/dev/null) || repo=
if [ -z "$repo" ] || [ ! -d "$repo" ]; then
	echo "No overlay named $overlay in /etc/portage/repos.conf." >&2
	exit 1
fi

install -D -m 644 "$here/$pkg/$ebuild" "$repo/$pkg/$ebuild"
echo "installed $repo/$pkg/$ebuild"

# A live ebuild has no keywords.
[ -f /etc/portage/package.accept_keywords ] && keywords=/etc/portage/package.accept_keywords
if ! grep -qs "^$pkg[[:space:]]" "$keywords"; then
	mkdir -p "$(dirname "$keywords")"
	echo "$pkg **" >>"$keywords"
	echo "added '$pkg **' to $keywords"
fi

# USE flags: a file of wmplayer's own, read after the one named audacious.
if [ -d "$use" ]; then
	install -m 644 "$here/package.use" "$use/wmplayer"
	echo "installed $use/wmplayer"
elif ! grep -qs "^media-plugins/audacious-plugins .*-alsa" "$use"; then
	grep -v '^#' "$here/package.use" >>"$use"
	echo "added the USE flags of $here/package.use to $use"
fi

# The plugins are built again only if their USE flags changed.
emerge --oneshot --changed-use media-plugins/audacious-plugins
emerge "$pkg"
