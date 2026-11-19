#!/bin/bash
# Build Work Tracker and pack WorkTracker-<version>-x86_64.AppImage.
#
# Runs inside the Rocky Linux 8 image started by the release workflow.
# The published 0.3.7 file was linked on Ubuntu 22.04 and required
# GLIBC_2.35, GLIBC_2.34, GLIBC_2.33, GLIBC_2.32, and weak 2.29/2.30.
# On a glibc 2.28 host the loader looks those up in /lib64/libc.so.6 and
# /lib64/libm.so.6, does not find them, and never starts the program.
# Rocky 8 is glibc 2.28, so the compiler records symbol versions this host
# already has. A newer glibc can still run that binary. Ubuntu 22.04 cannot
# be the link environment: its headers bind libm to GLIBC_2.35.
#
# The same floor applies to zenity. Import and Export run zenity from PATH,
# and the AppImage AppRun puts its own usr/bin first. linuxdeploy copies
# zenity from this image, so the dialog program is a 2.28 binary too.
set -euo pipefail

if [ -z "${GITHUB_REF_NAME:-}" ]; then
  echo "GITHUB_REF_NAME is required" >&2
  exit 1
fi
tag="${GITHUB_REF_NAME#v}"

dnf install -y --setopt=install_weak_deps=False \
  gcc \
  pkgconf-pkg-config \
  libX11-devel \
  libXcursor-devel \
  libXrandr-devel \
  libXi-devel \
  libxkbcommon-devel \
  mesa-libGL-devel \
  wayland-devel \
  gtk3-devel \
  gdk-pixbuf2-devel \
  librsvg2-devel \
  gobject-introspection-devel \
  binutils \
  file \
  curl \
  ca-certificates \
  zenity

if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

cd /src
cargo test --bin weekly_work_tracker
cargo build --release --bin weekly_work_tracker

# Fail the release here, before the AppImage is published. GLIBC_2.3 and
# GLIBC_2.27 are older than this host and are allowed. GLIBC_2.29 and above
# are the versions that made 0.3.7 refuse to start.
if objdump -T target/release/weekly_work_tracker | grep -E 'GLIBC_2\.(29|[3-9][0-9])'; then
  echo "worktracker requires a glibc newer than 2.28" >&2
  exit 1
fi
if objdump -T /usr/bin/zenity | grep -E 'GLIBC_2\.(29|[3-9][0-9])'; then
  echo "zenity requires a glibc newer than 2.28" >&2
  exit 1
fi

appdir="/src/AppDir"
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin"
install -m 755 target/release/weekly_work_tracker "$appdir/usr/bin/worktracker"
printf '%s\n' \
  '[Desktop Entry]' \
  'Type=Application' \
  'Name=Work Tracker' \
  'Exec=worktracker' \
  'Icon=worktracker' \
  'Categories=Office;' \
  'Terminal=false' \
  > /tmp/worktracker.desktop
# linuxdeploy keeps this file as AppRun.wrapped and writes the real AppRun,
# which sources the GTK plugin hook and then execs this file. The hook points
# zenity at the schemas and modules inside the image. usr/bin is first on PATH
# so Import and Export find that zenity.
printf '%s\n' \
  '#!/bin/sh' \
  'set -e' \
  'here=$(dirname "$(readlink -f "$0")")' \
  'export PATH="$here/usr/bin:$PATH"' \
  'if [ -d "$here/apprun-hooks" ]; then' \
  '  for hook in "$here"/apprun-hooks/*.sh; do' \
  '    [ -f "$hook" ] && . "$hook"' \
  '  done' \
  'fi' \
  'exec "$here/usr/bin/worktracker" "$@"' \
  > /tmp/AppRun
chmod +x /tmp/AppRun

tools=/tmp/appimage-tools
mkdir -p "$tools"
# Pinned on purpose. The continuous linuxdeploy asset is rebuilt without a
# version, and the GTK plugin publishes no release file: its README is this
# shell script. A floating URL can change the packager between two tags.
# Rocky 8's curl does not accept --retry-all-errors.
curl -fsSL --retry 5 --retry-delay 2 \
  -o "$tools/linuxdeploy" \
  https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage
curl -fsSL --retry 5 --retry-delay 2 \
  -o "$tools/linuxdeploy-plugin-gtk.sh" \
  https://github.com/linuxdeploy/linuxdeploy-plugin-gtk/raw/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh
chmod +x "$tools/linuxdeploy" "$tools/linuxdeploy-plugin-gtk.sh"

# APPIMAGE_EXTRACT_AND_RUN: the container has no FUSE mount, so the packager
# extracts itself instead of mounting. NO_STRIP: a second strip pass can break
# the Rust binary cargo already stripped.
export APPIMAGE_EXTRACT_AND_RUN=1
export ARCH=x86_64
export VERSION="$tag"
export LINUXDEPLOY_OUTPUT_VERSION="$tag"
export NO_STRIP=1
export DEPLOY_GTK_VERSION=3
export PATH="$tools:$PATH"
"$tools/linuxdeploy" --appdir "$appdir" \
  --executable "$appdir/usr/bin/worktracker" \
  --executable /usr/bin/zenity \
  --desktop-file /tmp/worktracker.desktop \
  --icon-file packaging/icon/worktracker.png \
  --custom-apprun /tmp/AppRun \
  --plugin gtk \
  --output appimage

shopt -s nullglob
files=()
for f in ./*.AppImage; do
  case "$(basename "$f")" in
    linuxdeploy*|appimagetool*) continue ;;
  esac
  files+=("$f")
done
test "${#files[@]}" -eq 1
mv "${files[0]}" "WorkTracker-${tag}-x86_64.AppImage"
chmod +x "WorkTracker-${tag}-x86_64.AppImage"
rm -rf "$appdir"
