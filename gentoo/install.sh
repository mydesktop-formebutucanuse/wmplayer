#!/bin/sh
# Install wmplayer with emerge: puts the live ebuild
# media-sound/wmplayer-9999 into a local overlay, accepts its (empty)
# keywords and emerges it. wmplayer takes the place of Audacious, which
# cannot stay installed beside it: media-sound/audacious and
# media-plugins/audacious-plugins are taken out of the world file, so that
# emerge removes them once wmplayer is in, and the settings in /etc/portage
# that were for them alone are removed. Run as root, again whenever the
# ebuild changes:
#   sudo sh gentoo/install.sh [OVERLAY]   default overlay: mydesktop
# Code updates need no rerun: the ebuild builds the latest commit on GitHub,
#   sudo emerge --oneshot media-sound/wmplayer
set -eu

here=$(cd "$(dirname "$0")" && pwd)
pkg=media-sound/wmplayer
ebuild=wmplayer-9999.ebuild
overlay=${1:-mydesktop}
keywords=/etc/portage/package.accept_keywords/$overlay
replaced="media-sound/audacious media-plugins/audacious-plugins"

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

# Audacious is no longer wanted for itself; emerge removes it and its
# plugins after wmplayer, which blocks both, is installed. Until then they
# stay, so a build that fails leaves the machine as it was.
deselected=
for old in $replaced; do
	if portageq has_version / "$old"; then
		emerge --deselect "$old"
		deselected="$deselected $old"
	fi
done

if ! emerge "$pkg"; then
	# Audacious is still installed, and wanted again.
	for old in $deselected; do
		emerge --noreplace "$old"
	done
	echo "$pkg was not installed; Audacious is left as it was." >&2
	exit 1
fi

# What was set for Audacious alone: a file of that name, and the one an
# earlier version of this script installed for its plugins' USE flags.
for dir in /etc/portage/package.use /etc/portage/package.accept_keywords; do
	for name in audacious wmplayer; do
		f=$dir/$name
		[ -f "$f" ] || continue
		if grep -v '^[[:space:]]*#' "$f" | grep -v '^[[:space:]]*$' |
			grep -qvE '^[<>=~]*media-(sound/audacious|plugins/audacious-plugins)([[:space:]:-]|$)'; then
			echo "left $f alone: it has settings for other packages"
		else
			rm "$f"
			echo "removed $f"
		fi
	done
done

for old in $replaced; do
	if portageq has_version / "$old"; then
		echo "$old is still installed; remove it with: emerge --ask --depclean $old" >&2
	fi
done
