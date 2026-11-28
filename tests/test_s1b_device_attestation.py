import unittest

from tools import s1b_campaign_safety as safety


class DeviceAttestationTest(unittest.TestCase):
    def observation(self):
        return {"serial": "emulator-5554", "avd": "serea_s1_16k", "api": 35,
                "abi": "x86_64", "page_size": 16384, "host_free_bytes": 40 * 1024**3}

    def test_accepts_exact_profile(self):
        self.assertIsNone(safety.validate_device_observation("api35-16k", self.observation()))

    def test_rejects_wrong_or_malformed_observation(self):
        for key, value in (("serial", "device"), ("avd", "serea_s1_01a3"),
                           ("api", 26), ("abi", "arm64-v8a"), ("page_size", 4096),
                           ("host_free_bytes", 1), ("api", True), ("serial", "emulator-5555")):
            observation = self.observation()
            observation[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(safety.SafetyError):
                safety.validate_device_observation("api35-16k", observation)

    def test_serial_port_range(self):
        observation = self.observation()
        observation["serial"] = "emulator-5682"
        self.assertIsNone(safety.validate_device_observation("api35-16k", observation))
        observation["serial"] = "emulator-5684"
        with self.assertRaises(safety.SafetyError):
            safety.validate_device_observation("api35-16k", observation)

    def test_other_profiles_and_capacity_boundary(self):
        for profile, avd, api, page_size in (
            ("api26", "serea_s1_api26", 26, 4096),
            ("api35-4k", "serea_s1_01a3", 35, 4096),
            ("api35-16k", "serea_s1_16k", 35, 16384),
        ):
            observation = self.observation()
            observation.update(avd=avd, api=api, page_size=page_size,
                               host_free_bytes=8 * 1024**3)
            with self.subTest(profile=profile):
                self.assertIsNone(safety.validate_device_observation(profile, observation))
                observation["host_free_bytes"] -= 1
                with self.assertRaises(safety.SafetyError):
                    safety.validate_device_observation(profile, observation)

    def test_rejects_unknown_fields_and_profile(self):
        observation = self.observation()
        observation["extra"] = 1
        with self.assertRaises(safety.SafetyError):
            safety.validate_device_observation("api35-16k", observation)
        for profile in ("unknown", None, [], {}):
            with self.subTest(profile=profile), self.assertRaises(safety.SafetyError):
                safety.validate_device_observation(profile, self.observation())


if __name__ == "__main__":
    unittest.main()
