#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
CHECKER="$SCRIPT_DIR/check-emulator-host-libraries.sh"
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/serea-emulator-libs-test.XXXXXX")"
trap 'rm -rf -- "$TEMP_ROOT"' EXIT

sdk_emulator_root="$TEMP_ROOT/sdk/emulator"
qemu_directory="$sdk_emulator_root/qemu/linux-x86_64"
mkdir -p "$TEMP_ROOT/bin" "$TEMP_ROOT/lib" "$sdk_emulator_root/lib64/qt/lib" "$qemu_directory"
qemu="$qemu_directory/qemu-system-x86_64"
pulse="$TEMP_ROOT/lib/libpulse.so.0"
export SEREA_TEST_EXPECTED_LIBRARY_PATH="$sdk_emulator_root/lib64:$sdk_emulator_root/lib64/qt/lib:$qemu_directory"
printf '#!/usr/bin/env bash\nexit 0\n' > "$qemu"
printf 'fixture library\n' > "$pulse"
chmod +x "$qemu"

cat > "$TEMP_ROOT/bin/ldd" <<'EOF'
#!/usr/bin/env bash
if [[ "${LD_LIBRARY_PATH:-}" != "$SEREA_TEST_EXPECTED_LIBRARY_PATH" ]]; then
  printf 'libQt6CoreAndroidEmu.so.6 => not found\n'
  exit 0
fi
case "${SEREA_TEST_LDD_MODE:-ok}" in
  ok)
    printf 'libpulse.so.0 => %s (0x0000000000000000)\n' "$SEREA_TEST_PULSE_PATH"
    printf 'libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x0000000000000000)\n'
    ;;
  missing)
    printf 'libpulse.so.0 => not found\n'
    ;;
  multiple-missing)
    printf 'libpulse.so.0 => not found\n'
    printf 'libprivate-host.so.7 => not found\n'
    printf 'host/path/should-not-appear 0xabcdef private-runtime-data\n'
    ;;
  malformed)
    printf 'libpulse.so.0 => %s (0x0000000000000000)\n' "$SEREA_TEST_PULSE_PATH"
    printf 'host/private/path => not found injected-private-value\n'
    ;;
  non-so)
    printf 'libpulse.so.0 => %s (0x0000000000000000)\n' "$SEREA_TEST_PULSE_PATH"
    printf 'helper-tool => not found\n'
    ;;
  no-pulse)
    printf 'libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x0000000000000000)\n'
    ;;
  *) exit 3 ;;
esac
EOF
cat > "$TEMP_ROOT/bin/dpkg-query" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" == '-W' ]]; then
  case "${SEREA_TEST_PACKAGE_MODE:-ok}" in
    ok) printf 'install ok installed amd64\n' ;;
    missing) printf 'deinstall ok config-files amd64\n' ;;
    *) exit 3 ;;
  esac
elif [[ "$1" == '-S' ]]; then
  printf 'libpulse0:amd64: %s\n' "$2"
else
  exit 3
fi
EOF
chmod +x "$TEMP_ROOT/bin/ldd" "$TEMP_ROOT/bin/dpkg-query"

if ! PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_PULSE_PATH="$pulse" \
    "$CHECKER" "$qemu" | grep -Fxq \
    'emulator_host_preflight=PASS qemu=regular_sdk_binary unresolved_libraries=0 libpulse=package_owned'; then
    echo 'emulator_host_library_contract=FAIL valid dependency set was rejected' >&2
    exit 1
fi

for mode in missing no-pulse; do
    if PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_LDD_MODE="$mode" \
        SEREA_TEST_PULSE_PATH="$pulse" "$CHECKER" "$qemu" >/dev/null 2>&1; then
        echo "emulator_host_library_contract=FAIL accepted ldd mode $mode" >&2
        exit 1
    fi
done

diagnostic_output="$TEMP_ROOT/missing-dependencies.out"
if PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_LDD_MODE=multiple-missing \
    SEREA_TEST_PULSE_PATH="$pulse" "$CHECKER" "$qemu" \
    >"$diagnostic_output" 2>&1; then
    echo 'emulator_host_library_contract=FAIL accepted unresolved dependency set' >&2
    exit 1
fi
grep -Fxq \
    'emulator_host_preflight=FAIL reason=QEMU has unresolved shared library dependencies unresolved_count=2 unresolved_libraries=libprivate-host.so.7,libpulse.so.0' \
    "$diagnostic_output"
if grep -Eq 'host/path|abcdef|private-runtime-data|=> not found' "$diagnostic_output"; then
    echo 'emulator_host_library_contract=FAIL leaked raw ldd diagnostic data' >&2
    exit 1
fi

for mode in malformed non-so; do
    diagnostic_output="$TEMP_ROOT/$mode-dependencies.out"
    if PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_LDD_MODE="$mode" \
        SEREA_TEST_PULSE_PATH="$pulse" "$CHECKER" "$qemu" \
        >"$diagnostic_output" 2>&1; then
        echo "emulator_host_library_contract=FAIL accepted $mode unresolved entry" >&2
        exit 1
    fi
    grep -Fxq \
        'emulator_host_preflight=FAIL reason=QEMU dependency output contains malformed unresolved entries unresolved_details=withheld' \
        "$diagnostic_output"
    if grep -Eq 'host/private/path|injected-private-value|helper-tool|=> not found|0x0000000000000000' \
        "$diagnostic_output"; then
        echo "emulator_host_library_contract=FAIL leaked $mode unresolved entry" >&2
        exit 1
    fi
done

if PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_PACKAGE_MODE=missing \
    SEREA_TEST_PULSE_PATH="$pulse" "$CHECKER" "$qemu" >/dev/null 2>&1; then
    echo 'emulator_host_library_contract=FAIL accepted missing Ubuntu package' >&2
    exit 1
fi

ln -s "$qemu" "$TEMP_ROOT/qemu-symlink"
if PATH="$TEMP_ROOT/bin:$PATH" SEREA_TEST_PULSE_PATH="$pulse" \
    "$CHECKER" "$TEMP_ROOT/qemu-symlink" >/dev/null 2>&1; then
    echo 'emulator_host_library_contract=FAIL accepted a symlink alias for the QEMU binary' >&2
    exit 1
fi

echo 'emulator_host_library_contract=PASS resolved_shared_libraries=REQUIRED unresolved_names_sanitized=REQUIRED libpulse_package=REQUIRED symlink_alias=REJECTED'
