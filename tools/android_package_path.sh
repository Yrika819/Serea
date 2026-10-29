#!/usr/bin/env bash

# Print an installed package path (or empty output when absent). Package Manager
# can report absence as a non-zero `pm path` result; accept only its exact
# not-found messages. Transport and other query failures remain distinguishable.
android_package_path() {
  local package="$1" output status=0
  output="$(adb shell pm path "$package" 2>&1)" || status=$?
  output="${output//$'\r'/}"
  case "$output" in
    'Error: package not found'|"Error: package not found: $package")
      return 0
      ;;
    '')
      if [[ "$status" -eq 0 ]]; then return 0; fi
      ;;
    package:/*)
      if [[ "$status" -eq 0 ]]; then
        local line
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
      ;;
    *)
      ;;
  esac
  if [[ "$status" -eq 0 ]]; then
    printf 'ADB package path query returned unexpected output for %s: %s\n' \
      "$package" "$output" >&2
  else
    printf 'ADB package path query failed for %s (exit=%s): %s\n' \
      "$package" "$status" "$output" >&2
  fi
  return 2
}
