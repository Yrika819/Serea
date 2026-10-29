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
unresolved_names=()
malformed_unresolved=false
while IFS= read -r dependency_line; do
    [[ "$dependency_line" == *'not found'* ]] || continue
    read -r -a dependency_fields <<<"$dependency_line"
    if [[ ${#dependency_fields[@]} -ne 4 ||
        "${dependency_fields[1]:-}" != '=>' ||
        "${dependency_fields[2]:-}" != 'not' ||
        "${dependency_fields[3]:-}" != 'found' ||
        ! "${dependency_fields[0]:-}" =~ ^lib[A-Za-z0-9_+-]+\.so(\.[0-9]+)*$ ]]; then
        malformed_unresolved=true
        continue
    fi
    unresolved_names+=("${dependency_fields[0]}")
done <<<"$dependencies"
if [[ "$malformed_unresolved" == true ]]; then
    # Malformed unresolved records fail closed, but their contents are never rendered.
    fail 'QEMU dependency output contains malformed unresolved entries unresolved_details=withheld'
fi
unresolved_libraries="$(printf '%s\n' "${unresolved_names[@]}" | sed '/^$/d' | sort -u)"
if [[ -n "$unresolved_libraries" ]]; then
    unresolved_count="$(wc -l <<<"$unresolved_libraries" | tr -d '[:space:]')"
    unresolved_csv="$(paste -sd, <<<"$unresolved_libraries")"
    # Emit only validated shared-library basenames. Never forward ldd output, which can
    # contain paths, addresses, or other host-specific diagnostic data.
    fail "QEMU has unresolved shared library dependencies unresolved_count=$unresolved_count unresolved_libraries=$unresolved_csv"
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
