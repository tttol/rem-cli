#!/bin/sh

set -eu

binary=$1
shift

binary_dir=$(dirname "$binary")
app_dir="$binary_dir/rem.app"
app_binary="$app_dir/Contents/MacOS/rem"

mkdir -p "$app_dir/Contents/MacOS"
cp native/Info.plist "$app_dir/Contents/Info.plist"
cp "$binary" "$app_binary"
codesign --force --sign - --identifier link.about-tttol.rem-cli "$app_dir"

exec "$app_binary" "$@"
