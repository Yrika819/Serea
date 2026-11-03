#!/usr/bin/env bash

# Print an installed package path (or empty output when absent). Return 2 when
# the ADB query itself fails so callers cannot treat a transport error as absence.
android_package_path() {
  local package="$1" output
  if ! output="$(adb shell pm path "$package" 2>/dev/null)"; then
    return 2
  fi
  output="${output//$'\r'/}"
  printf '%s' "$output"
}
