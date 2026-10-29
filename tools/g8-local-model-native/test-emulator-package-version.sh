#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
VERIFY="$SCRIPT_DIR/verify-emulator-package.sh"
ROOT_DIR="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/serea-emulator-version-test.XXXXXX")"
trap 'rm -rf -- "$TEMP_ROOT"' EXIT

emulator_root="$TEMP_ROOT/sdk/emulator"
mkdir -p "$emulator_root"
cat > "$emulator_root/source.properties" <<'PROPERTIES'
Pkg.Path=emulator
Pkg.Revision=37.2.13
PROPERTIES
cat > "$emulator_root/emulator" <<'EMULATOR'
#!/usr/bin/env bash
[[ "${1:-}" == -version ]] || exit 2
printf '%s\n' "${FAKE_EMULATOR_VERSION_LINE:-Android emulator version 37.2.13.0 (build_id 16740729) (CL:N/A)}"
if [[ -n "${FAKE_EMULATOR_EXTRA_LINE:-}" ]]; then
  printf '%s\n' "$FAKE_EMULATOR_EXTRA_LINE"
fi
exit "${FAKE_EMULATOR_EXIT:-0}"
EMULATOR
chmod +x "$emulator_root/emulator"

expected_revision=37.2.13
expected_version=37.2.13.0
expected_build_id=16740729

bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/pass.out"
grep -Fxq 'emulator_pkg_path=emulator' "$TEMP_ROOT/pass.out"
grep -Fxq 'emulator_pkg_revision=37.2.13' "$TEMP_ROOT/pass.out"
grep -Fxq 'emulator_version=37.2.13.0' "$TEMP_ROOT/pass.out"
grep -Fxq 'emulator_build_id=16740729' "$TEMP_ROOT/pass.out"
grep -Fxq 'emulator_version_verification=PASS' "$TEMP_ROOT/pass.out"

if FAKE_EMULATOR_VERSION_LINE='Android emulator version 37.3.6.0 (build_id 16696583) (CL:N/A)' \
    bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/preview.out"; then
    echo 'emulator_version_contract=FAIL accepted preview channel emulator' >&2
    exit 1
fi
grep -Fxq 'emulator_version_verification=FAIL reason=SDK emulator version/build differs from pin' "$TEMP_ROOT/preview.out"

if FAKE_EMULATOR_VERSION_LINE='Android emulator version 37.2.13.0 (build_id 16740000) (CL:N/A)' \
    bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/build-id.out"; then
    echo 'emulator_version_contract=FAIL accepted different build id' >&2
    exit 1
fi
grep -Fxq 'emulator_version_verification=FAIL reason=SDK emulator version/build differs from pin' "$TEMP_ROOT/build-id.out"

sed -i 's/Pkg.Revision=37.2.13/Pkg.Revision=37.3.6/' "$emulator_root/source.properties"
if bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/revision.out"; then
    echo 'emulator_version_contract=FAIL accepted mismatched package revision' >&2
    exit 1
fi
grep -Fxq 'emulator_version_verification=FAIL reason=SDK package revision differs from pin' "$TEMP_ROOT/revision.out"

printf 'Pkg.Path=emulator\nPkg.Revision=37.2.13\n' > "$emulator_root/source.properties"
if FAKE_EMULATOR_EXIT=9 \
    bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/command-failure.out"; then
    echo 'emulator_version_contract=FAIL accepted nonzero emulator version command' >&2
    exit 1
fi
grep -Fxq 'emulator_version_verification=FAIL reason=SDK emulator version command failed' "$TEMP_ROOT/command-failure.out"

if FAKE_EMULATOR_EXTRA_LINE="$emulator_root/private-diagnostic" \
    bash "$VERIFY" "$emulator_root" "$expected_revision" "$expected_version" "$expected_build_id" \
    > "$TEMP_ROOT/sanitized.out"; then
    :
else
    echo 'emulator_version_contract=FAIL valid version with extra output was rejected' >&2
    exit 1
fi
if grep -Fq "$emulator_root" "$TEMP_ROOT/sanitized.out"; then
    echo 'emulator_version_contract=FAIL leaked filesystem path' >&2
    exit 1
fi

workflow="$ROOT_DIR/.github/workflows/g8-local-native-runtime-focused.yml"
grep -Fq 'sdkmanager --channel=0' "$workflow"
grep -Fq 'verify-emulator-package.sh' "$workflow"
grep -Fq '"$ANDROID_HOME/emulator" 37.2.13 37.2.13.0 16740729' "$workflow"
if grep -Fq "grep -F '37.3.6'" "$workflow"; then
    echo 'emulator_version_contract=FAIL stale preview-version assertion remains in workflow' >&2
    exit 1
fi

echo 'emulator_version_contract=PASS stable_package_revision=PINNED semantic_version=PINNED build_id=PINNED preview_and_drift=REJECTED paths=SANITIZED'
