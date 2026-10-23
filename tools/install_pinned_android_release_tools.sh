#!/usr/bin/env bash
set -euo pipefail

destination="${1:?usage: install_pinned_android_release_tools.sh DESTINATION}"
if [[ -e "$destination" ]]; then
  echo 'Refusing to install pinned Android tools over an existing path.' >&2
  exit 2
fi
command -v curl >/dev/null
command -v sha256sum >/dev/null
command -v unzip >/dev/null
command -v java >/dev/null

temporary=$(mktemp -d "${TMPDIR:-/tmp}/serea-android-tools.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
mkdir -m 700 "$destination"

build_tools_archive="$temporary/build-tools.zip"
platform_archive="$temporary/platform-35.zip"
curl --silent --show-error --fail --location --proto '=https' --tlsv1.2 --retry 2 --retry-all-errors \
  --max-time 240 'https://dl.google.com/android/repository/build-tools_r35_linux.zip' \
  --output "$build_tools_archive"
printf '%s  %s\n' \
  'bd3a4966912eb8b30ed0d00b0cda6b6543b949d5ffe00bea54c04c81e1561d88' \
  "$build_tools_archive" | sha256sum --check --status || {
    echo 'Pinned Android Build Tools archive SHA-256 mismatch.' >&2
    exit 1
  }

curl --silent --show-error --fail --location --proto '=https' --tlsv1.2 --retry 2 --retry-all-errors \
  --max-time 240 'https://dl.google.com/android/repository/platform-35_r02.zip' \
  --output "$platform_archive"
printf '%s  %s\n' \
  '0988cacad01b38a18a47bac14a0695f246bc76c1b06c0eeb8eb0dc825ab0c8e0' \
  "$platform_archive" | sha256sum --check --status || {
    echo 'Pinned Android Platform 35 archive SHA-256 mismatch.' >&2
    exit 1
  }

mkdir "$temporary/build" "$temporary/platform"
unzip -q "$build_tools_archive" -d "$temporary/build"
unzip -q "$platform_archive" -d "$temporary/platform"
test -s "$temporary/build/android-15/lib/apksigner.jar"
test -s "$temporary/build/android-15/aapt"
test -s "$temporary/platform/android-35/android.jar"
printf '%s  %s\n' \
  '00ef9948f843fe395d2440ae3ef41405b8040a6d5d46493bd1902ac0ee6deae7' \
  "$temporary/build/android-15/lib/apksigner.jar" \
  '393c4c87f675f9555c50921fb53be1f3893cacd29f2270c4863e2e3f99583c39' \
  "$temporary/build/android-15/aapt" \
  '4566663c3876e022b4fa4ced8c8697c4ab1688267f090114fd92d027b32e619b' \
  "$temporary/platform/android-35/android.jar" | sha256sum --check --status || {
    echo 'Pinned Android signing tool or platform artifact SHA-256 mismatch.' >&2
    exit 1
  }

mv "$temporary/build/android-15" "$destination/build-tools"
mv "$temporary/platform/android-35" "$destination/platform-35"
chmod -R a-w "$destination"
echo "Pinned Android release tools verified at $destination"
