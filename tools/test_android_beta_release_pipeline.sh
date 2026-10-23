#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-beta-release-e2e.XXXXXX")
cleanup() {
  chmod -R u+w "$test_root" 2>/dev/null || true
  rm -rf "$test_root"
}
trap cleanup EXIT
tools="$test_root/android-tools"
mkdir -m 700 "$test_root/outputs"
bash "$repo_root/tools/install_pinned_android_release_tools.sh" "$tools"
aapt="$tools/build-tools/aapt"
apksigner="$tools/build-tools/apksigner"
android_jar="$tools/platform-35/android.jar"

make_unsigned_apk() {
  local package_name="$1" version="$2" apk="$3"
  local manifest_dir="$test_root/manifest-$package_name-$version"
  local manifest="$manifest_dir/AndroidManifest.xml"
  mkdir -p "$manifest_dir"
  cat > "$manifest" <<EOF
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="$package_name" android:versionCode="$version" android:versionName="test-$version">
    <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="35" />
    <application android:label="Serea test fixture" />
</manifest>
EOF
  "$aapt" package -f -M "$manifest" -I "$android_jar" -F "$apk"
}

make_test_key() {
  local store="$1" alias="$2" password_name="$3"
  keytool -genkeypair -noprompt -alias "$alias" -keystore "$store" -storetype PKCS12 \
    -keyalg RSA -keysize 2048 -validity 30 \
    -dname 'CN=Serea disposable Beta signing test' \
    -storepass:env "$password_name" -keypass:env "$password_name" >/dev/null 2>&1
  keytool -list -v -keystore "$store" -storetype PKCS12 -alias "$alias" \
    -storepass:env "$password_name" 2>/dev/null \
    | awk -F ': ' '/SHA256:/{print $2; exit}' | tr -d ':[:space:]' | tr '[:lower:]' '[:upper:]'
}

source_sha='b73e20e8f086e2b2770f78189963b8d13d93df0a'
tree_sha='3af7188cfae302d6f946fd9a1876d8661805dbb2'
run_id=123456789
version_code=101
unsigned="$test_root/unsigned.apk"
record="$test_root/release.json"
signed="$test_root/signed.apk"
make_unsigned_apk app.serea.android.beta "$version_code" "$unsigned"

key1="$test_root/test-owner-one.p12"
export TEST_SIGNING_PASSWORD="$(openssl rand -hex 32)"
cert1=$(make_test_key "$key1" serea-beta-test-one TEST_SIGNING_PASSWORD)
python3 "$repo_root/tools/android_beta_release.py" record-unsigned \
  --apk "$unsigned" --record "$record" --source-sha "$source_sha" --tree-sha "$tree_sha" \
  --run-id "$run_id" --version-code "$version_code" --aapt "$aapt" --apksigner "$apksigner"
python3 "$repo_root/tools/android_beta_release.py" verify-unsigned \
  --apk "$unsigned" --record "$record" --source-sha "$source_sha" --tree-sha "$tree_sha" \
  --run-id "$run_id" --expected-sha256 "$(sha256sum "$unsigned" | awk '{print $1}')" \
  --version-code "$version_code" --aapt "$aapt" --apksigner "$apksigner"

export SEREA_BETA_SIGNING_STORE_PASSWORD="$TEST_SIGNING_PASSWORD"
export SEREA_BETA_SIGNING_KEY_PASSWORD="$TEST_SIGNING_PASSWORD"
export SEREA_BETA_SIGNING_KEY_ALIAS=serea-beta-test-one
python3 "$repo_root/tools/android_beta_release.py" sign \
  --apk "$unsigned" --record "$record" --keystore "$key1" --output "$signed" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" \
  --expected-sha256 "$(sha256sum "$unsigned" | awk '{print $1}')" --expected-cert "$cert1" \
  --version-code "$version_code" --aapt "$aapt" --apksigner "$apksigner"
signed_sha=$(sha256sum "$signed" | awk '{print $1}')
python3 "$repo_root/tools/android_beta_release.py" verify-signed \
  --apk "$signed" --expected-sha256 "$signed_sha" --expected-cert "$cert1" \
  --version-code "$version_code" --aapt "$aapt" --apksigner "$apksigner"

next_unsigned="$test_root/unsigned-v102.apk"
next_record="$test_root/release-v102.json"
next_signed="$test_root/signed-v102.apk"
make_unsigned_apk app.serea.android.beta 102 "$next_unsigned"
python3 "$repo_root/tools/android_beta_release.py" record-unsigned \
  --apk "$next_unsigned" --record "$next_record" --source-sha "$source_sha" --tree-sha "$tree_sha" \
  --run-id "$run_id" --version-code 102 --aapt "$aapt" --apksigner "$apksigner" >/dev/null
python3 "$repo_root/tools/android_beta_release.py" sign \
  --apk "$next_unsigned" --record "$next_record" --keystore "$key1" --output "$next_signed" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" \
  --expected-sha256 "$(sha256sum "$next_unsigned" | awk '{print $1}')" --expected-cert "$cert1" \
  --version-code 102 --aapt "$aapt" --apksigner "$apksigner" >/dev/null
python3 "$repo_root/tools/android_beta_release.py" verify-signed \
  --apk "$next_signed" --expected-sha256 "$(sha256sum "$next_signed" | awk '{print $1}')" \
  --expected-cert "$cert1" --version-code 102 --aapt "$aapt" --apksigner "$apksigner" >/dev/null

if python3 "$repo_root/tools/android_beta_release.py" sign \
  --apk "$unsigned" --record "$record" --keystore "$key1" --output "$test_root/wrong-version.apk" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" \
  --expected-sha256 "$(sha256sum "$unsigned" | awk '{print $1}')" --expected-cert "$cert1" \
  --version-code 102 --aapt "$aapt" --apksigner "$apksigner"; then
  echo 'Signer accepted a versionCode that did not match the bound unsigned APK metadata.' >&2
  exit 1
fi

wrong_package="$test_root/wrong-package.apk"
make_unsigned_apk app.serea.android.demo "$version_code" "$wrong_package"
if python3 "$repo_root/tools/android_beta_release.py" record-unsigned \
  --apk "$wrong_package" --record "$test_root/wrong-package.json" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" \
  --version-code "$version_code" --aapt "$aapt" --apksigner "$apksigner"; then
  echo 'Build recorder accepted the Demo package as Personal Beta.' >&2
  exit 1
fi

key2="$test_root/test-owner-two.p12"
export TEST_SIGNING_PASSWORD_TWO="$(openssl rand -hex 32)"
cert2=$(make_test_key "$key2" serea-beta-test-two TEST_SIGNING_PASSWORD_TWO)
if [[ "$cert1" == "$cert2" ]]; then
  echo 'Disposable test certificates unexpectedly match.' >&2
  exit 1
fi
export SEREA_BETA_SIGNING_STORE_PASSWORD="$TEST_SIGNING_PASSWORD_TWO"
export SEREA_BETA_SIGNING_KEY_PASSWORD="$TEST_SIGNING_PASSWORD_TWO"
export SEREA_BETA_SIGNING_KEY_ALIAS=serea-beta-test-two
if python3 "$repo_root/tools/android_beta_release.py" sign \
  --apk "$next_unsigned" --record "$next_record" --keystore "$key2" --output "$test_root/wrong-signer.apk" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" \
  --expected-sha256 "$(sha256sum "$next_unsigned" | awk '{print $1}')" --expected-cert "$cert1" \
  --version-code 102 --aapt "$aapt" --apksigner "$apksigner"; then
  echo 'Signer accepted a different signing key than the pinned Owner certificate.' >&2
  exit 1
fi

export TEST_TRANSPORT_PASSWORD="$(openssl rand -hex 32)"
openssl req -new -x509 -newkey rsa:3072 -days 30 -nodes \
  -keyout "$test_root/transport-key-plain.pem" -out "$test_root/transport-cert.pem" \
  -subj '/CN=Serea disposable Beta transport test' >/dev/null 2>&1
openssl pkcs8 -topk8 -in "$test_root/transport-key-plain.pem" -out "$test_root/transport-key.pem" \
  -passout env:TEST_TRANSPORT_PASSWORD >/dev/null 2>&1
rm -f "$test_root/transport-key-plain.pem"
transport_cert_sha=$(openssl x509 -in "$test_root/transport-cert.pem" -noout -fingerprint -sha256 \
  | sed 's/.*=//' | tr -d ':' | tr '[:lower:]' '[:upper:]')
ciphertext="$test_root/signed.apk.cms"
python3 "$repo_root/tools/android_beta_release.py" encrypt \
  --apk "$signed" --certificate "$test_root/transport-cert.pem" \
  --expected-cert "$transport_cert_sha" --source-sha "$source_sha" --tree-sha "$tree_sha" \
  --run-id "$run_id" --run-attempt 1 --version-code "$version_code" \
  --signing-certificate-sha256 "$cert1" --output "$ciphertext"
export SEREA_BETA_TRANSPORT_KEY_PASSWORD="$TEST_TRANSPORT_PASSWORD"
decrypted="$test_root/decrypted.apk"
python3 "$repo_root/tools/android_beta_release.py" decrypt \
  --ciphertext "$ciphertext" --certificate "$test_root/transport-cert.pem" \
  --private-key "$test_root/transport-key.pem" --expected-cert "$transport_cert_sha" \
  --expected-sha256 "$signed_sha" --expected-signing-certificate-sha256 "$cert1" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" --run-attempt 1 \
  --version-code "$version_code" --output "$decrypted"
cmp --silent "$signed" "$decrypted"

randomized_ciphertext="$test_root/signed-randomized.apk.cms"
python3 "$repo_root/tools/android_beta_release.py" encrypt \
  --apk "$signed" --certificate "$test_root/transport-cert.pem" \
  --expected-cert "$transport_cert_sha" --source-sha "$source_sha" --tree-sha "$tree_sha" \
  --run-id "$run_id" --run-attempt 1 --version-code "$version_code" \
  --signing-certificate-sha256 "$cert1" --output "$randomized_ciphertext" >/dev/null
if cmp --silent "$ciphertext" "$randomized_ciphertext"; then
  echo 'CMS AES-GCM encryption unexpectedly reused the same ciphertext for a new encryption.' >&2
  exit 1
fi

tampered_ciphertext="$test_root/tampered.apk.cms"
python3 - "$ciphertext" "$tampered_ciphertext" <<'PY'
from pathlib import Path
import sys
data = bytearray(Path(sys.argv[1]).read_bytes())
data[len(data) // 2] ^= 1
Path(sys.argv[2]).write_bytes(data)
PY
if python3 "$repo_root/tools/android_beta_release.py" decrypt \
  --ciphertext "$tampered_ciphertext" --certificate "$test_root/transport-cert.pem" \
  --private-key "$test_root/transport-key.pem" --expected-cert "$transport_cert_sha" \
  --expected-sha256 "$signed_sha" --expected-signing-certificate-sha256 "$cert1" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" --run-attempt 1 \
  --version-code "$version_code" --output "$test_root/tampered.apk"; then
  echo 'CMS AES-GCM accepted a tampered encrypted handoff.' >&2
  exit 1
fi

if python3 "$repo_root/tools/android_beta_release.py" decrypt \
  --ciphertext "$ciphertext" --certificate "$test_root/transport-cert.pem" \
  --private-key "$test_root/transport-key.pem" --expected-cert "$transport_cert_sha" \
  --expected-sha256 "$signed_sha" --expected-signing-certificate-sha256 "$cert1" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id 987654321 --run-attempt 1 \
  --version-code "$version_code" --output "$test_root/replayed.apk"; then
  echo 'Encrypted handoff replayed across workflow runs.' >&2
  exit 1
fi

export TEST_TRANSPORT_PASSWORD_WRONG="$(openssl rand -hex 32)"
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 \
  -aes-256-cbc -pass env:TEST_TRANSPORT_PASSWORD_WRONG \
  -out "$test_root/wrong-transport-key.pem" >/dev/null 2>&1
export SEREA_BETA_TRANSPORT_KEY_PASSWORD="$TEST_TRANSPORT_PASSWORD_WRONG"
if python3 "$repo_root/tools/android_beta_release.py" decrypt \
  --ciphertext "$ciphertext" --certificate "$test_root/transport-cert.pem" \
  --private-key "$test_root/wrong-transport-key.pem" --expected-cert "$transport_cert_sha" \
  --expected-sha256 "$signed_sha" --expected-signing-certificate-sha256 "$cert1" \
  --source-sha "$source_sha" --tree-sha "$tree_sha" --run-id "$run_id" --run-attempt 1 \
  --version-code "$version_code" --output "$test_root/wrong-decrypted.apk"; then
  echo 'Encrypted handoff unexpectedly accepted a mismatched private key.' >&2
  exit 1
fi
printf 'TEST_KEY_SIGN_AND_VERIFY=PASS\nSAME_KEY_VERSION_101_TO_102_CERTIFICATE=PASS\nDIFFERENT_SIGNING_KEY_REJECTED=PASS\nPACKAGE_MISMATCH_REJECTED=PASS\nCMS_AES_256_GCM_AUTHENTICATED_ENCRYPTION=PASS\nFRESH_KEY_NONCE_RANDOMIZATION=PASS\nGCM_TAMPER_REJECTED=PASS\nCROSS_RUN_REPLAY_REJECTED=PASS\nWRONG_TRANSPORT_KEY_REJECTED=PASS\nOWNER_CREDENTIALS_USED=NO\n'
