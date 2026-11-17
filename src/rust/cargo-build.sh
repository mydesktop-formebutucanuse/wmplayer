#!/bin/sh
# Builds wmplayer's Rust code for meson: cargo-build.sh CARGO SRCDIR OUTDIR
# LIB..., where each LIB is the archive of one of the crates, to be put in
# OUTDIR.
set -e

cargo=$1 src=$2 out=$3
shift 3

packages=
for lib; do
    case ${lib##*/} in
        libwmplayer_core.a) packages="$packages -p wmplayer-core" ;;
        libwmplayer_gtk.a) packages="$packages -p wmplayer-gtk" ;;
        *) echo "cargo-build.sh: what is $lib?" >&2; exit 1 ;;
    esac
done

# shellcheck disable=SC2086
"$cargo" build --release --locked --manifest-path "$src/Cargo.toml" \
    --target-dir "$out/target" $packages

for lib; do
    built=$out/target/release/${lib##*/}
    # An unchanged archive keeps its time, and nothing is linked again.
    cmp -s "$built" "$lib" || cp "$built" "$lib"
done
