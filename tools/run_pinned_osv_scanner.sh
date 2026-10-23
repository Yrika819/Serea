#!/usr/bin/env bash
set -euo pipefail

readonly expected_image='ghcr.io/google/osv-scanner-action@sha256:fc633b975f339ddfde55c747f5bf4dde71605ccda1073fd535760ba38e4e7065'
readonly output_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-osv-output"
readonly source_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-osv-source"
readonly docker_context="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-osv-context"
readonly dockerfile="$docker_context/Dockerfile"
readonly local_image="serea-osv-scanner:${GITHUB_RUN_ID:-local}"
cleanup() {
  chmod -R u+w "$source_root" 2>/dev/null || true
  rm -rf "$source_root" "$docker_context" "$output_root"
  docker image rm "$local_image" >/dev/null 2>&1 || true
}
trap cleanup EXIT

if [[ "${SEREA_OSV_SCANNER_IMAGE:-$expected_image}" != "$expected_image" ]]; then
  echo 'OSV scanner image does not match the reviewed immutable digest.' >&2
  exit 1
fi
command -v docker >/dev/null
command -v python3 >/dev/null
ca_bundle="${SSL_CERT_FILE:-/etc/ssl/certs/ca-certificates.crt}"
test -r "$ca_bundle"
mkdir -p "$output_root"
chmod 777 "$output_root"
rm -f "$output_root/osv-android-dependencies.json"
rm -rf "$source_root"
mkdir -m 700 "$source_root"
python3 tools/android_beta_release.py copy-android-scan-tree \
  --source "$PWD/android" --destination "$source_root/android"
chmod -R a+rX "$source_root/android"
rm -rf "$docker_context"
mkdir -m 700 "$docker_context"

cat > "$dockerfile" <<EOF
FROM $expected_image AS pinned
FROM pinned
COPY --from=pinned --chown=65532:65532 /root/osv-scanner /usr/local/bin/osv-scanner
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/osv-scanner"]
EOF

# The local image contains only the scanner binary copied from the digest-pinned image.
# It runs without root, Linux capabilities, a writable source tree, or repository secrets.
docker build --pull --quiet --file "$dockerfile" --tag "$local_image" "$docker_context" >/dev/null
docker run --rm \
  --network bridge \
  --read-only \
  --tmpfs /tmp:rw,nosuid,nodev,noexec,size=128m \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --pids-limit 64 \
  --memory 1g \
  --cpus 2 \
  --user 65532:65532 \
  --env HOME=/tmp \
  --env SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
  --mount "type=bind,src=$source_root/android,dst=/scan,readonly" \
  --mount "type=bind,src=$ca_bundle,dst=/etc/ssl/certs/ca-certificates.crt,readonly" \
  --mount "type=bind,src=$output_root,dst=/out" \
  --workdir /tmp \
  "$local_image" \
  --format=json --output=/out/osv-android-dependencies.json --recursive /scan

test -s "$output_root/osv-android-dependencies.json"
python3 - "$output_root/osv-android-dependencies.json" <<'PY'
import json
import sys
from pathlib import Path

try:
    json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError):
    raise SystemExit("OSV scanner output is missing or invalid JSON")
PY
cp "$output_root/osv-android-dependencies.json" ./osv-android-dependencies.json
