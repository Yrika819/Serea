#!/usr/bin/env bash

# Print installed package paths (or empty output when absence is independently
# confirmed). A non-zero `pm path` result is ambiguous, so verify absence with a
# second healthy Package Manager query and keep transport/other failures closed.
android_package_path() {
  local package="$1" output status=0 list_output list_status=0 line
  output="$(adb shell pm path "$package" 2>&1)" || status=$?
  output="${output//$'\r'/}"
  if [[ "$status" -eq 0 && "$output" == package:* ]]; then
    while IFS= read -r line; do
      if [[ ! "$line" =~ ^package:/[^[:space:]]+$ ]]; then
        printf 'ADB package path query returned malformed output for %s: %s\n' \
          "$package" "$output" >&2
        return 2
      fi
    done <<< "$output"
    printf '%s' "$output"
    return 0
  fi

  # AOSP Android 8.0 and Android 15 both return exit 1 with no output from
  # `pm path` for a missing package. Verify that absence through a second,
  # healthy Package Manager query before allowing the emulator stage to proceed.
  if [[ ( "$status" -eq 0 || "$status" -eq 1 ) &&
        ( -z "$output" || "$output" == 'Error: package not found' || "$output" == "Error: package not found: $package" ) ]]; then
    list_output="$(adb shell pm list packages -u "$package" 2>&1)" || list_status=$?
    list_output="${list_output//$'\r'/}"
    if [[ "$list_status" -eq 0 && -z "$list_output" ]]; then
      return 0
    fi
    if [[ "$list_status" -eq 0 && "$list_output" == "package:$package" ]]; then
      printf 'Package Manager lists %s but `pm path` returned no usable path.\n' "$package" >&2
      return 2
    fi
    printf 'Could not independently verify that %s is absent (pm list packages exit=%s): %s\n' \
      "$package" "$list_status" "$list_output" >&2
    return 2
  fi

  if [[ "$status" -eq 0 ]]; then
    printf 'ADB package path query returned unexpected output for %s: %s\n' \
      "$package" "$output" >&2
  else
    printf 'ADB package path query failed for %s (exit=%s): %s\n' \
      "$package" "$status" "$output" >&2
  fi
  return 2
}
