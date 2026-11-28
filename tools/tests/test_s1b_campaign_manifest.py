"""Verify the campaign target's merged manifest, not a source manifest.

Run with SEREA_S1B_MERGED_MANIFEST pointing to the opt-in target manifest.
This does not verify framework effects or the separate instrumentation APK.
"""
import os
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"
TOOLS = "{http://schemas.android.com/tools}"
CAMPAIGN_META = "app.serea.android.S1B_CAMPAIGN_PACKAGE"
COMPONENTS = ("activity", "provider", "receiver", "service")


def assert_no_app_components(test, application, allow_main_activity=False,
                             allow_androidx_empty_activity=False):
    for component in COMPONENTS:
        nodes = [node for node in application.findall(component)
                 if node.get(TOOLS + "node") not in {"removeAll", "remove"}]
        if allow_main_activity and component == "activity":
            test.assertEqual(1, len(nodes), "campaign target must retain exactly MainActivity")
            test.assertEqual("app.serea.android.MainActivity", nodes[0].get(ANDROID + "name"))
            test.assertEqual("false", nodes[0].get(ANDROID + "exported"))
        elif allow_androidx_empty_activity and component == "activity":
            test.assertEqual(1, len(nodes), "campaign test package may retain only AndroidX EmptyActivity")
            test.assertEqual(
                "androidx.test.core.app.InstrumentationActivityInvoker$EmptyActivity",
                nodes[0].get(ANDROID + "name"),
            )
            test.assertEqual("true", nodes[0].get(ANDROID + "exported"))
            test.assertEqual("@style/WhiteBackgroundTheme", nodes[0].get(ANDROID + "theme"))
        else:
            test.assertEqual([], nodes, f"unexpected {component} in campaign app")


class CampaignManifestTest(unittest.TestCase):
    def test_campaign_source_removes_entrypoints_without_changing_isolated_source(self):
        root = Path(__file__).resolve().parents[2] / "android/app/src"
        isolated = ET.parse(root / "isolated/AndroidManifest.xml").getroot()
        campaign = ET.parse(root / "s1bCampaign/AndroidManifest.xml").getroot()
        isolated_app = isolated.find("application")
        campaign_app = campaign.find("application")
        self.assertTrue(any(isolated_app.findall(name) for name in COMPONENTS))
        self.assertEqual(
            {"activity", "provider", "receiver", "service"},
            {child.tag for child in campaign_app if child.tag in COMPONENTS},
        )
        self.assertEqual(
            {"activity", "provider", "receiver", "service"},
            {child.tag for child in campaign_app if child.tag in COMPONENTS
             and child.get(TOOLS + "node") == "removeAll"},
        )
        named_components = [child for child in campaign_app if child.tag in COMPONENTS
                            and child.get(ANDROID + "name")]
        self.assertEqual(["app.serea.android.MainActivity"],
                         [child.get(ANDROID + "name") for child in named_components])
        self.assertEqual("false", named_components[0].get(ANDROID + "exported"))
        self.assertEqual("replace", named_components[0].get(TOOLS + "node"))
        self.assertEqual([CAMPAIGN_META], [item.get(ANDROID + "name")
                                           for item in campaign_app.findall("meta-data")])
        removed_permissions = {
            (item.get(ANDROID + "name"), item.get(TOOLS + "node"))
            for item in campaign.findall("uses-permission")
        }
        self.assertEqual({("android.permission.INTERNET", "remove")}, removed_permissions)

    def test_campaign_gradle_selects_manifest_only_when_opted_in(self):
        project = Path(__file__).resolve().parents[2]
        gradle = (project / "android/app/build.gradle.kts").read_text()
        self.assertRegex(
            gradle,
            r'if \(s1bCampaign != null\) \{\s*getByName\("androidTest"\)\.manifest\.srcFile\('
            r'"src/s1bCampaignAndroidTest/AndroidManifest\.xml"\)\s*\}',
        )
        normal_test = ET.parse(project / "android/app/src/androidTest/AndroidManifest.xml").getroot()
        self.assertTrue(any(normal_test.find("application").find(name) is not None
                            for name in COMPONENTS))

    def test_campaign_instrumentation_source_allows_only_androidx_empty_activity(self):
        root = Path(__file__).resolve().parents[2] / "android/app/src"
        normal_test = ET.parse(root / "androidTest/AndroidManifest.xml").getroot()
        campaign_test = ET.parse(root / "s1bCampaignAndroidTest/AndroidManifest.xml").getroot()
        normal_app = normal_test.find("application")
        campaign_app = campaign_test.find("application")
        self.assertTrue(any(normal_app.findall(name) for name in COMPONENTS))
        self.assertEqual(
            {"activity", "provider", "receiver", "service"},
            {child.tag for child in campaign_app if child.tag in COMPONENTS
             and child.get(TOOLS + "node") == "removeAll"},
        )
        assert_no_app_components(self, campaign_app, allow_androidx_empty_activity=True)
        activity = next(node for node in campaign_app.findall("activity")
                        if node.get(TOOLS + "node") not in {"removeAll", "remove"})
        self.assertEqual("replace", activity.get(TOOLS + "node"))
        self.assertEqual([], campaign_test.findall("uses-permission"))

    @unittest.skipUnless(os.environ.get("SEREA_S1B_TEST_MERGED_MANIFEST"), "merged campaign test manifest not supplied")
    def test_campaign_merged_test_package_has_no_components_but_keeps_runner(self):
        manifest = ET.parse(Path(os.environ["SEREA_S1B_TEST_MERGED_MANIFEST"])).getroot()
        self.assertRegex(manifest.attrib["package"],
                         r"^app\.serea\.android\.isolated\.s1b[0-9a-f]{10}\.test$")
        application = manifest.find("application")
        self.assertIsNotNone(application)
        self.assertNotIn(ANDROID + "name", application.attrib)
        assert_no_app_components(self, application, allow_androidx_empty_activity=True)
        instrumentation = manifest.find("instrumentation")
        self.assertIsNotNone(instrumentation)
        self.assertEqual("app.serea.android.SereaIsolatedAndroidJUnitRunner",
                         instrumentation.get(ANDROID + "name"))
        self.assertEqual(manifest.attrib["package"].removesuffix(".test"),
                         instrumentation.get(ANDROID + "targetPackage"))

    @unittest.skipUnless(os.environ.get("SEREA_S1B_MERGED_MANIFEST"), "merged campaign manifest not supplied")
    def test_campaign_merged_target_is_component_free_and_attested(self):
        manifest = ET.parse(Path(os.environ["SEREA_S1B_MERGED_MANIFEST"])).getroot()
        self.assertRegex(manifest.attrib["package"], r"^app\.serea\.android\.isolated\.s1b[0-9a-f]{10}$")
        application = manifest.find("application")
        self.assertIsNotNone(application)
        self.assertNotIn(ANDROID + "name", application.attrib)
        assert_no_app_components(self, application, allow_main_activity=True)
        self.assertEqual([CAMPAIGN_META], [item.get(ANDROID + "name")
                                           for item in application.findall("meta-data")
                                           if item.get(ANDROID + "name") == CAMPAIGN_META])
        permissions = {item.get(ANDROID + "name") for item in manifest.findall("uses-permission")}
        self.assertNotIn("android.permission.INTERNET", permissions)
        self.assertNotIn("app.serea.android.r25testime.permission.DIAGNOSTICS", permissions)

    def test_component_policy_rejects_an_injected_exported_entrypoint(self):
        app = ET.fromstring('<application xmlns:android="http://schemas.android.com/apk/res/android">'
                            '<receiver android:name="Injected" android:exported="true" />'
                            '</application>')
        with self.assertRaises(AssertionError):
            assert_no_app_components(self, app)


if __name__ == "__main__":
    unittest.main()
