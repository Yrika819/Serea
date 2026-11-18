from pathlib import Path
import unittest


BUILD_GRADLE = Path(__file__).resolve().parents[1] / "android/app/build.gradle.kts"


class PrivateBetaVersionCodeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = BUILD_GRADLE.read_text(encoding="utf-8")

    def test_private_packaging_guard_checks_the_original_explicit_property(self):
        self.assertIn(
            "val privateVersionCodeText = providers.gradleProperty(\"sereaPrivateBetaVersionCode\").orNull",
            self.source,
        )
        self.assertIn(
            "val privateBetaVersionCode = privateVersionCodeText?.toIntOrNull()?.takeIf { it in 2..2_100_000_000 } ?: 2",
            self.source,
        )
        self.assertIn(
            "val requestedPrivateVersionCode = privateVersionCodeText?.toIntOrNull()",
            self.source,
        )
        self.assertIn(
            "if (requestedPrivateVersionCode == null || requestedPrivateVersionCode !in 2..2_100_000_000)",
            self.source,
        )
        self.assertNotIn(
            "registerUnsignedBetaGuard(\"validatePrivateBetaUnsigned\", privateBetaVersionCode)",
            self.source,
        )

    def test_all_private_artifact_task_names_are_guarded(self):
        for task_name in ("assemblePrivateBeta", "packagePrivateBeta", "bundlePrivateBeta"):
            with self.subTest(task_name=task_name):
                self.assertIn(f'"{task_name}"', self.source)
        self.assertIn("dependsOn(validatePrivateBetaUnsigned)", self.source)


if __name__ == "__main__":
    unittest.main()
