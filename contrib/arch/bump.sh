#!/usr/bin/env bash
# Bump contrib/arch/PKGBUILD to a released version and refresh its checksum.
#
# Usage: ./contrib/arch/bump.sh X.Y.Z
#
# The tag vX.Y.Z must already exist on GitHub, because the checksum is computed
# from the tag's source archive.
set -euo pipefail

ver="${1:?usage: bump.sh X.Y.Z (without leading v)}"
ver="${ver#v}"

if [[ ! "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: invalid version '$ver' (expected X.Y.Z)" >&2
    exit 1
fi

cd "$(dirname "$0")"

pkgname="$(sed -n 's/^pkgname=//p' PKGBUILD)"
sed -i -E "s/^pkgver=.*/pkgver=${ver}/; s/^pkgrel=.*/pkgrel=1/" PKGBUILD

if command -v updpkgsums >/dev/null 2>&1; then
    updpkgsums
    # makepkg leaves the downloaded source archive behind.
    rm -f "${pkgname}-${ver}.tar.gz"
else
    # No Arch tooling available (e.g. on the Ubuntu CI runner): fetch the
    # source archive ourselves and rewrite the checksum line.
    if ! command -v curl >/dev/null 2>&1; then
        echo "Error: neither updpkgsums nor curl is available" >&2
        exit 1
    fi
    url="$(sed -n 's/^url="\(.*\)"$/\1/p' PKGBUILD)"
    if [ -z "$url" ]; then
        echo "Error: could not find url= in PKGBUILD" >&2
        exit 1
    fi
    src="$url/archive/refs/tags/v${ver}.tar.gz"
    echo "Downloading $src" >&2
    sum="$(curl -fsSL "$src" | sha256sum | awk '{print $1}')"
    if [ -z "$sum" ]; then
        echo "Error: could not compute checksum for $src" >&2
        exit 1
    fi

    # Replace the whole sha256sums=( ... ) block, however it is formatted.
    awk -v sum="$sum" '
        /^sha256sums=\(.*\)$/ {
            print "sha256sums=(\x27" sum "\x27)"
            next
        }
        /^sha256sums=\(/ {
            found = 1
            next
        }
        found && /^\)/ {
            found = 0
            print "sha256sums=(\x27" sum "\x27)"
            next
        }
        found { next }
        { print }
    ' PKGBUILD > PKGBUILD.tmp
    mv PKGBUILD.tmp PKGBUILD
fi

git --no-pager diff -- PKGBUILD
