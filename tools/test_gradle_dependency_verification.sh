#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
gradle_cmd="${SEREA_GRADLE_BIN:-$repo_root/android/gradlew}"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/serea-gradle-verification.XXXXXX")"
trap 'rm -rf "$temporary_root"' EXIT

write_metadata() {
  local file="$1" group="$2" name="$3" pom_sha="$4" jar_sha="$5"
  mkdir -p "$(dirname "$file")"
  cat > "$file" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<verification-metadata xmlns="https://schema.gradle.org/dependency-verification"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
    xsi:schemaLocation="https://schema.gradle.org/dependency-verification https://schema.gradle.org/dependency-verification/dependency-verification-1.3.xsd">
  <configuration>
    <verify-metadata>true</verify-metadata>
    <verify-signatures>false</verify-signatures>
  </configuration>
  <components>
    <component group="$group" name="$name" version="1.0">
      <artifact name="$name-1.0.pom"><sha256 value="$pom_sha" origin="SEC-A2 negative-test fixture"/></artifact>
      <artifact name="$name-1.0.jar"><sha256 value="$jar_sha" origin="SEC-A2 negative-test fixture"/></artifact>
    </component>
  </components>
</verification-metadata>
EOF
}

make_project() {
  local case_name="$1" group="$2" name="$3"
  local project="$temporary_root/$case_name"
  local repository="$project/maven"
  local module_dir="$repository/${group//./\/}/$name/1.0"
  mkdir -p "$module_dir"
  printf 'SEC-A2 fixture bytes for %s\n' "$case_name" > "$module_dir/$name-1.0.jar"
  cat > "$module_dir/$name-1.0.pom" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>$group</groupId>
  <artifactId>$name</artifactId>
  <version>1.0</version>
</project>
EOF
  cat > "$project/settings.gradle" <<EOF
rootProject.name = '$case_name'
dependencyResolutionManagement {
  repositories {
    maven { url = uri('$repository') }
  }
}
EOF
  cat > "$project/build.gradle" <<EOF
configurations { probe }
dependencies { probe '$group:$name:1.0' }
tasks.register('resolveProbe') {
  doLast { configurations.probe.resolve(); println 'FIXTURE_RESOLVED' }
}
EOF
  printf '%s\n' "$project"
}

sha256_file() {
  sha256sum "$1" | awk '{print $1}'
}

run_case() {
  local name="$1" project="$2" expected="$3" result=0
  local output="$temporary_root/$name.log"
  "$gradle_cmd" --no-daemon --offline --console=plain -p "$project" resolveProbe > "$output" 2>&1 || result=$?
  if [[ "$expected" == PASS ]]; then
    if [[ "$result" -ne 0 ]] || ! grep -Fq 'FIXTURE_RESOLVED' "$output"; then
      cat "$output" >&2
      echo "FAIL: valid fixture was rejected (exit=$result)." >&2
      return 1
    fi
  else
    if [[ "$result" -eq 0 ]] || ! grep -Eiq 'dependency verification failed|no checksums|verification metadata|checksum' "$output"; then
      cat "$output" >&2
      echo "FAIL: $name did not fail specifically at dependency verification." >&2
      return 1
    fi
  fi
  echo "PASS: $name ($expected)"
}

valid_group="org.serea.sec_a2.fixture.valid"
valid_project="$(make_project valid "$valid_group" fixture)"
valid_module="$valid_project/maven/${valid_group//./\/}/fixture/1.0"
write_metadata "$valid_project/gradle/verification-metadata.xml" "$valid_group" fixture \
  "$(sha256_file "$valid_module/fixture-1.0.pom")" "$(sha256_file "$valid_module/fixture-1.0.jar")"
run_case valid-checksum "$valid_project" PASS

unknown_group="org.serea.sec_a2.fixture.unregistered"
unknown_project="$(make_project unregistered "$unknown_group" fixture)"
unknown_module="$unknown_project/maven/${unknown_group//./\/}/fixture/1.0"
registered_group="org.serea.sec_a2.fixture.registered"
write_metadata "$unknown_project/gradle/verification-metadata.xml" "$registered_group" other \
  "$(sha256_file "$unknown_module/fixture-1.0.pom")" "$(sha256_file "$unknown_module/fixture-1.0.jar")"
run_case unregistered-dependency "$unknown_project" REJECT

mismatch_group="org.serea.sec_a2.fixture.mismatch"
mismatch_project="$(make_project checksum-mismatch "$mismatch_group" fixture)"
mismatch_module="$mismatch_project/maven/${mismatch_group//./\/}/fixture/1.0"
write_metadata "$mismatch_project/gradle/verification-metadata.xml" "$mismatch_group" fixture \
  "$(sha256_file "$mismatch_module/fixture-1.0.pom")" "$(printf 'intentionally incorrect checksum\n' | sha256sum | awk '{print $1}')"
run_case checksum-mismatch "$mismatch_project" REJECT

tampered_group="org.serea.sec_a2.fixture.tampered"
tampered_project="$(make_project tampered-artifact "$tampered_group" fixture)"
tampered_module="$tampered_project/maven/${tampered_group//./\/}/fixture/1.0"
original_jar_sha="$(sha256_file "$tampered_module/fixture-1.0.jar")"
write_metadata "$tampered_project/gradle/verification-metadata.xml" "$tampered_group" fixture \
  "$(sha256_file "$tampered_module/fixture-1.0.pom")" "$original_jar_sha"
printf 'tampered artifact bytes\n' > "$tampered_module/fixture-1.0.jar"
run_case tampered-artifact "$tampered_project" REJECT

echo 'All Gradle dependency verification fixtures behaved as expected.'
