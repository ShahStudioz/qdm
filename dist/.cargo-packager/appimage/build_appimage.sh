#!/usr/bin/env bash

# Copyright 2019-2023 Tauri Programme within The Commons Conservancy
# Copyright 2023-2023 CrabNebula Ltd.
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

set -euxo pipefail

export ARCH=x86_64

mkdir -p "qdm.AppDir"
cp -r ../appimage_deb/data/* "qdm.AppDir"

cd "qdm.AppDir"
mkdir -p "usr/bin"
mkdir -p "usr/lib"
mkdir -p "usr/lib64"

# Copy libs. Follow symlinks in case `/usr/lib64` is a symlink to `/usr/lib`
find -L /usr/lib* -name libayatana-appindicator3.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libayatana-ido3-0.4.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libdbusmenu-glib.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libdbusmenu-gtk3.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libxdo.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libssl.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true
find -L /usr/lib* -name libcrypto.so* -exec mkdir -p "$(dirname '{}')" \; -exec cp --parents '{}' "." \; || true

# Copy bins.

# We need AppRun to be installed as qdm.AppDir/AppRun.
# Otherwise the linuxdeploy scripts will default to symlinking our main bin instead and will crash on trying to launch.
cp "/home/shah/.cache/.cargo-packager/AppImage/AppRun-${ARCH}" AppRun

cp "usr/share/icons/hicolor/1024x1024/apps/qdm.png" .DirIcon
ln -sf "usr/share/icons/hicolor/1024x1024/apps/qdm.png" "qdm.png"

ln -sf "usr/share/applications/qdm.desktop" "qdm.desktop"

cd ..

# modify the linux deploy appimage ELF header so that binfmt no longer identifies it as an appimage
# and so appimagelauncher doesn't inject itself and the binary runs directly
dd if=/dev/zero bs=1 count=3 seek=8 conv=notrunc of="/home/shah/.cache/.cargo-packager/AppImage/linuxdeploy-x86_64.AppImage"

OUTPUT="/home/shah/Desktop/Projects/Rust/qdm/dist/qdm_1.0.2_x86_64.AppImage" "/home/shah/.cache/.cargo-packager/AppImage/linuxdeploy-x86_64.AppImage" --appimage-extract-and-run --appdir "qdm.AppDir" --plugin gtk  --output appimage
