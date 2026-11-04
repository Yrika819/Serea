#!/usr/bin/env bash
set -euo pipefail

emulator_root_arg="${1:?pass the Android SDK emulator install directory}"
expected_package_revision="${2:?pass the pinned Pkg.Revision}"
expected_version="${3:?pass the pinned emulator version}"
expected_build_id="${4:?pass the pinned emulator build id}"

fail() {
    local reason="$1"
    printf 'emulator_version_verification=FAIL reason=%s\n' "$reason"
    exit 1
}

[[ -d "$emulator_root_arg" ]] || fail 'SDK emulator directory missing'
emulator_root="$(cd -- "$emulator_root_arg" && pwd -P)" || fail 'SDK emulator directory unresolved'
properties="$emulator_root/source.properties"
emulator_binary="$emulator_root/emulator"
[[ -f "$properties" && ! -L "$properties" ]] || fail 'SDK package metadata missing or symlinked'
[[ -f "$emulator_binary" && -x "$emulator_binary" && ! -L "$emulator_binary" ]] || \
    fail 'SDK emulator executable missing or symlinked'

read_property() {
    local key="$1"
    awk -F= -v wanted="$key" '
        $1 == wanted { count++; value = substr($0, index($0, "=") + 1) }
        END { if (count != 1) exit 1; print value }
    ' "$properties"
}

package_path="$(read_property 'Pkg.Path')" || fail 'SDK package path is ambiguous'
package_revision="$(read_property 'Pkg.Revision')" || fail 'SDK package revision is ambiguous'
[[ "$package_path" == emulator ]] || fail 'unexpected SDK package path'
if [[ "$package_revision" != "$expected_package_revision" ]]; then
    printf 'emulator_pkg_revision=%s\n' "$package_revision"
    fail 'SDK package revision differs from pin'
fi

version_output="$("$emulator_binary" -version 2>&1)" || fail 'SDK emulator version command failed'
version_line="${version_output%%$'\n'*}"
expected_prefix="Android emulator version ${expected_version} (build_id ${expected_build_id})"
if [[ "$version_line" != "$expected_prefix"* ]]; then
    if [[ "$version_line" =~ ^Android\ emulator\ version\ ([0-9]+\.[0-9]+\.[0-9]+(\.[0-9]+)?)\ \(build_id\ ([0-9]+)\) ]]; then
        printf 'emulator_version=%s\n' "${BASH_REMATCH[1]}"
        printf 'emulator_build_id=%s\n' "${BASH_REMATCH[3]}"
    fi
    fail 'SDK emulator version/build differs from pin'
fi

printf 'emulator_pkg_path=emulator\n'
printf 'emulator_pkg_revision=%s\n' "$package_revision"
printf 'emulator_version=%s\n' "$expected_version"
printf 'emulator_build_id=%s\n' "$expected_build_id"
printf 'emulator_version_verification=PASS\n'
