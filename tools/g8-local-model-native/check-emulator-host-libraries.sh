#!/usr/bin/env bash
set -euo pipefail

fail() {
    echo "emulator_host_preflight=FAIL reason=$1" >&2
    exit 1
}

if [[ $# -ne 1 ]]; then
    fail 'usage: check-emulator-host-libraries.sh QEMU_BINARY'
fi

qemu_binary="$1"
if [[ ! -f "$qemu_binary" || ! -x "$qemu_binary" || -L "$qemu_binary" ]]; then
    fail 'QEMU binary must be a regular executable installed by the Android SDK'
fi

package_state="$(dpkg-query -W -f='${Status} ${Architecture}' libpulse0:amd64 2>/dev/null)" || \
    fail 'Ubuntu libpulse0:amd64 package is not installed'
if [[ "$package_state" != 'install ok installed amd64' ]]; then
    fail 'Ubuntu libpulse0:amd64 package is not installed'
fi

dependencies="$(ldd "$qemu_binary" 2>&1)" || fail 'ldd could not inspect the QEMU binary'
if grep -Fq 'not found' <<<"$dependencies"; then
    fail 'QEMU has unresolved shared library dependencies'
fi

pulse_path="$(awk '$1 == "libpulse.so.0" && $2 == "=>" && $3 ~ /^\// { print $3; exit }' \
    <<<"$dependencies")"
[[ -n "$pulse_path" && -r "$pulse_path" ]] || fail 'QEMU cannot resolve readable libpulse.so.0'
pulse_real_path="$(readlink -f -- "$pulse_path")"
[[ -n "$pulse_real_path" && -f "$pulse_real_path" && -r "$pulse_real_path" ]] || \
    fail 'resolved libpulse target is not a readable regular file'
pulse_owner="$(dpkg-query -S "$pulse_real_path" 2>/dev/null)" || \
    fail 'resolved libpulse target is not owned by an installed package'
[[ "$pulse_owner" == libpulse0:amd64:* ]] || \
    fail 'resolved libpulse target is not provided by libpulse0:amd64'

echo 'emulator_host_preflight=PASS qemu=regular_sdk_binary unresolved_libraries=0 libpulse=package_owned'
