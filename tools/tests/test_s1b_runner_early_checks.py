"""Structural guards for low-side-effect campaign checks and the final runner fence.

These checks do not establish Android framework startup order or runtime behavior.
"""
import unittest
from pathlib import Path


SOURCE = (Path(__file__).resolve().parents[2] /
          "android/app/src/androidTest/java/app/serea/android/SereaIsolatedAndroidJUnitRunner.kt").read_text()


class EarlyRunnerChecksTest(unittest.TestCase):
    def test_platform_and_campaign_arguments_are_rejected_before_super_on_create(self):
        create = SOURCE.split("override fun onCreate(arguments: Bundle)", 1)[1].split("override fun onStart()", 1)[0]
        prefix = create.split("super.onCreate(arguments)", 1)[0]
        for guard in (
            "checkSupportedPlatform()",
            "Regex(\"\"\"s1b[a-f0-9]{10}\"\"\").matches(proposedId)",
            "explicitClassFilter in S1B_ALLOWED_TEST_METHODS",
            "Regex(\"\"\"[0-9a-f]{32}\"\"\").matches(proposedToken)",
        ):
            self.assertIn(guard, prefix)
        for context_or_storage_access in (
            "targetContext", "packageManager", "filesDir", "databaseList()",
            "noBackupFilesDir", "File(",
        ):
            self.assertNotIn(context_or_storage_access, prefix)
        self.assertIn("super.onCreate(arguments)", create)

    def test_final_target_freshness_and_one_shot_guards_remain_before_start(self):
        start = SOURCE.split("override fun onStart()", 1)[1].split("private fun checkSupportedPlatform", 1)[0]
        for guard in (
            "val target = targetContext",
            "target.packageManager.getApplicationInfo(",
            "target.databaseList().isEmpty()",
            "campaignMarkerMatches",
            "check(marker.renameTo(claimedMarker))",
            "super.onStart()",
        ):
            self.assertIn(guard, start)
        self.assertLess(start.index("check(marker.renameTo(claimedMarker))"), start.index("super.onStart()"))

    def test_campaign_allowlist_contains_only_approved_ownership_and_chat_candidates(self):
        approved_methods = (
            '"app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement"',
            '"app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp"',
        )
        for method in approved_methods:
            self.assertIn(method, SOURCE)
        self.assertNotIn("EmbeddedCoreNativeRaceTest", SOURCE)
        allowlist = SOURCE.split("val S1B_ALLOWED_TEST_METHODS = setOf(", 1)[1].split(")", 1)[0]
        self.assertEqual(allowlist.count('"app.serea.android.'), len(approved_methods))
        chat_test = (Path(__file__).resolve().parents[2] /
                     "android/app/src/embeddedCoreAndroidTest/java/app/serea/android/EmbeddedCoreDiagnosticChatUiTest.kt").read_text()
        self.assertIn("fun diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp()", chat_test)


if __name__ == "__main__":
    unittest.main()
