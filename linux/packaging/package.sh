#!/usr/bin/env bash
# Build the Linux packages from a release binary:
#
#   linux/packaging/package.sh <path/to/madhyam> <out-dir> [file-version]
#
# Writes <out-dir>/Madhyam-v<version>-linux-amd64.deb and, when linuxdeploy is available
# (LINUXDEPLOY=/path/to/linuxdeploy-x86_64.AppImage, plus linuxdeploy-plugin-gtk.sh next to
# it), <out-dir>/Madhyam-v<version>-linux-x86_64.AppImage.
# Name and version come from branding/brand.ini and VERSION, like every other platform.
# Needs: dpkg-deb, dpkg-shlibdeps (dpkg-dev), rsvg-convert (librsvg2-bin).
set -euo pipefail

BIN=$(readlink -f "$1")
OUT=$(mkdir -p "$2" && readlink -f "$2")
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
VERSION=$(tr -d '[:space:]' < "$ROOT/VERSION")
FILE_VERSION=${3:-$VERSION}
NAME=$(sed -n 's/^name=//p' "$ROOT/branding/brand.ini" | tr -d '\r')
APP_ID=dev.madhyam.Madhyam
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# ------------------------------------------------------------------ shared file tree (/usr)
STAGE="$WORK/stage"
install -Dm755 "$BIN" "$STAGE/usr/bin/madhyam"
sed "s/@NAME@/$NAME/g" "$HERE/$APP_ID.desktop.in" > "$WORK/$APP_ID.desktop"
install -Dm644 "$WORK/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
sed -e "s/@NAME@/$NAME/g" -e "s/@VERSION@/$VERSION/g" "$HERE/$APP_ID.metainfo.xml.in" > "$WORK/$APP_ID.metainfo.xml"
install -Dm644 "$WORK/$APP_ID.metainfo.xml" "$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 "$ROOT/branding/icon.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/$APP_ID.svg"
for s in 48 64 128 256 512; do
  mkdir -p "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps"
  rsvg-convert -w "$s" -h "$s" "$ROOT/branding/icon.svg" -o "$STAGE/usr/share/icons/hicolor/${s}x${s}/apps/$APP_ID.png"
done
install -Dm644 "$ROOT/LICENSE" "$STAGE/usr/share/doc/madhyam/copyright"

# ------------------------------------------------------------------ .deb
DEB="$WORK/deb"
cp -a "$STAGE" "$DEB"
mkdir -p "$DEB/DEBIAN" "$WORK/shlib/debian"
# dpkg-shlibdeps works out the library packages the binary links against (GTK 4, libmpv…).
printf 'Source: madhyam\n\nPackage: madhyam\nArchitecture: any\n' > "$WORK/shlib/debian/control"
DEPENDS=$(cd "$WORK/shlib" && dpkg-shlibdeps -O "$DEB/usr/bin/madhyam" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
SIZE=$(du -sk "$DEB/usr" | cut -f1)
cat > "$DEB/DEBIAN/control" <<EOF
Package: madhyam
Version: $FILE_VERSION
Architecture: amd64
Maintainer: Diwas Khadka <diwaskhadka@users.noreply.github.com>
Installed-Size: $SIZE
Depends: $DEPENDS
Recommends: yt-dlp, gnome-keyring | kwalletmanager | keepassxc
Section: video
Priority: optional
Homepage: https://github.com/diwaskhadka/Madhyam
Description: Free IPTV player for M3U playlists and Xtream Codes
 $NAME plays M3U / M3U8 playlists and Xtream Codes accounts, with a TV guide
 (EPG), movies, series, favorites and continue-watching. Free TV (public
 channels from iptv-org) comes preinstalled. It is a player only: it does not
 sell subscriptions, channels or playlists.
EOF
dpkg-deb --root-owner-group --build "$DEB" "$OUT/$NAME-v$FILE_VERSION-linux-amd64.deb" >/dev/null
echo "built $OUT/$NAME-v$FILE_VERSION-linux-amd64.deb (Depends: $DEPENDS)"

# ------------------------------------------------------------------ AppImage
if [ -n "${LINUXDEPLOY:-}" ]; then
  APPDIR="$WORK/AppDir"
  cp -a "$STAGE" "$APPDIR"
  export DEPLOY_GTK_VERSION=4 APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1
  export LDAI_OUTPUT="$OUT/$NAME-v$FILE_VERSION-linux-x86_64.AppImage"
  (cd "$WORK" && "$LINUXDEPLOY" --appdir "$APPDIR" --plugin gtk \
      --desktop-file "$APPDIR/usr/share/applications/$APP_ID.desktop" \
      --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png" \
      --output appimage)
  echo "built $LDAI_OUTPUT"
fi
