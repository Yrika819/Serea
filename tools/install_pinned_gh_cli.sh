#!/usr/bin/env bash
set -euo pipefail

destination="${1:?usage: install_pinned_gh_cli.sh DESTINATION}"
if [[ -e "$destination" ]]; then
  echo 'Refusing to install GitHub CLI over an existing path.' >&2
  exit 2
fi
temporary=$(mktemp -d "${TMPDIR:-/tmp}/serea-gh-cli.XXXXXX")
trap 'rm -rf "$temporary"' EXIT
mkdir -m 700 "$destination"
archive="$temporary/gh.tar.gz"
checksums="$temporary/checksums.txt"
curl --silent --show-error --fail --location --proto '=https' --tlsv1.2 --retry 2 --retry-all-errors \
  --max-time 180 'https://github.com/cli/cli/releases/download/v2.103.0/gh_2.103.0_linux_amd64.tar.gz' \
  --output "$archive"
printf '%s  %s\n' \
  'fdd768eb70f6409f80fcfa9f3a1c0adf0fc98d827ab3ce0bed2ac22762c5d786' \
  "$archive" | sha256sum --check --status || {
    echo 'Pinned GitHub CLI archive SHA-256 mismatch.' >&2
    exit 1
  }
curl --silent --show-error --fail --location --proto '=https' --tlsv1.2 --retry 2 --retry-all-errors \
  --max-time 60 'https://github.com/cli/cli/releases/download/v2.103.0/gh_2.103.0_checksums.txt' \
  --output "$checksums"
printf '%s  %s\n' \
  '075bb954e2b113d0d2ac788d318b516280cb87103e601e788c34f3557acb15df' \
  "$checksums" | sha256sum --check --status || {
    echo 'Pinned GitHub CLI checksum manifest SHA-256 mismatch.' >&2
    exit 1
  }
grep -E "^fdd768eb70f6409f80fcfa9f3a1c0adf0fc98d827ab3ce0bed2ac22762c5d786[[:space:]]+gh_2\.103\.0_linux_amd64\.tar\.gz$" "$checksums" >/dev/null
tar -xzf "$archive" -C "$temporary"
install -m 0755 "$temporary/gh_2.103.0_linux_amd64/bin/gh" "$destination/gh"
"$destination/gh" version | grep -F 'gh version 2.103.0 '
"$destination/gh" attestation verify --help | grep -F -- '--source-digest' >/dev/null
"$destination/gh" attestation verify --help | grep -F -- '--deny-self-hosted-runners' >/dev/null
echo "$destination/gh" >> "${GITHUB_PATH:?GITHUB_PATH is required}"
