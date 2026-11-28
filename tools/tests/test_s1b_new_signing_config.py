import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class DiagnosticSigningConfigTest(unittest.TestCase):
    def test_diagnostic_signing_rejects_configuration_cache_before_secret_read(self):
        text = (ROOT / 'android/app/build.gradle.kts').read_text()
        block = text.split('create("embeddedCoreDisposable") {', 1)[1].split('\n            }', 1)[0]
        guard = 'check(!gradle.startParameter.isConfigurationCacheRequested)'
        self.assertIn(guard, block)
        self.assertLess(block.index(guard), block.index('s1bDiagnosticKeychainPassword()'))

    def test_signed_offline_lane_is_build_only_and_has_no_runtime_tasks(self):
        text = (ROOT / 'android/app/build.gradle.kts').read_text()
        self.assertIn('s1bSignedBuildOnlyTaskNames', text)
        self.assertIn('if (offlineDiagnostic || embeddedCoreSignedDiagnostic)', text)
        self.assertIn('check(offlineDiagnostic && embeddedCoreSignedDiagnostic)', text)
        self.assertIn('requestedTaskNames.toSet() == s1bSignedBuildOnlyTaskNames', text)
        self.assertIn('name.startsWith("install")', text)
        self.assertIn('name.startsWith("connected")', text)
        self.assertIn('S1B signed diagnostic build-only graph contains runtime tasks', text)

    def test_offline_cargo_target_must_match_this_worktrees_local_config(self):
        text = (ROOT / 'android/app/build.gradle.kts').read_text()
        self.assertIn('reviewedOfflineCargoTarget', text)
        self.assertIn('resolve(".cargo/config.toml")', text)
        self.assertIn('requestedOfflineTarget.canonicalFile == configuredOfflineTarget', text)
        self.assertIn('configuredOfflineTarget.isDirectory', text)

    def test_campaign_signing_uses_new_generation_and_keychain_only(self):
        text = (ROOT / 'android/app/build.gradle.kts').read_text()
        block = text.split('create("embeddedCoreDisposable") {', 1)[1].split('\n            }', 1)[0]
        self.assertIn('SEREA_S1B_DIAGNOSTIC_KEYSTORE', text)
        self.assertIn('s1bDiagnosticKeyStore()', block)
        self.assertNotIn('System.getProperty("user.home")', block)
        self.assertNotIn('serea-s1b-diagnostic-keys/2026-11/diagnostic.p12', block)
        self.assertIn('serea-s1b-diagnostic-2026', block)
        self.assertIn('s1bDiagnosticKeychainPassword()', block)
        self.assertNotIn('diagnostic-only.p12', block)
        self.assertNotIn('synthetic-diagnostic"', block)
        self.assertIn('"/usr/bin/security", "find-generic-password"', text)
        self.assertIn('redirectError(ProcessBuilder.Redirect.DISCARD)', text)
        self.assertIn('check(s1bCampaign != null)', text)

if __name__ == '__main__':
    unittest.main()
