"""Narrow hermetic-profile controls; no compiler or Native wallet is executed."""
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location("private_mobile_hermetic",Path(__file__).parents[1]/"run_mobile_hermetic_command.py")
runner=importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
class AndroidRuntimeTrustTests(unittest.TestCase):
    def environment(self,profile):
        return dict.fromkeys(runner.PROFILES[profile],"SYNTHETIC public DATA")
    def test_absence_preserves_exact_ordinary_android_and_diagnostic_profiles(self):
        for profile in ["android-cargo","android-armv7-diagnostic-cargo"]:
            runner.validate_profile_environment(profile,self.environment(profile))
    def test_valid_public_key_preserved_in_both_android_profiles(self):
        for profile in ["android-cargo","android-armv7-diagnostic-cargo"]:
            env=self.environment(profile);env[runner.WALLET_RUNTIME_TRUST_INPUT]="3"*64
            runner.validate_profile_environment(profile,env)
            self.assertEqual(runner.android_runtime_trust(env[runner.WALLET_RUNTIME_TRUST_INPUT]),"3"*64)
    def test_malformed_configured_key_fails_before_command_authentication(self):
        for value in ["","0"*64,"A"*64,"g"*64,"1"*63,"1"*65,"1"*64+"\n"]:
            with self.subTest(value=value):
                env=self.environment("android-cargo");env[runner.WALLET_RUNTIME_TRUST_INPUT]=value
                with self.assertRaises(RuntimeError):runner.validate_profile_environment("android-cargo",env)
    def test_key_cannot_leak_into_host_apple_or_gradle_profiles(self):
        for profile in ["host-cargo","apple-macos","gradle-jvm"]:
            env=self.environment(profile);env[runner.WALLET_RUNTIME_TRUST_INPUT]="3"*64
            with self.assertRaises(RuntimeError):runner.validate_profile_environment(profile,env)
    def test_other_ambient_flags_remain_closed(self):
        for name in ["RUSTFLAGS","RUSTC_WRAPPER","CARGO_ENCODED_RUSTFLAGS","NATIVE_READY"]:
            env=self.environment("android-cargo");env[name]="offered"
            with self.assertRaises(RuntimeError):runner.validate_profile_environment("android-cargo",env)
    def test_missing_base_variables_cannot_be_replaced_by_public_trust(self):
        for name in runner.PROFILES["android-cargo"]:
            env=self.environment("android-cargo");del env[name];env[runner.WALLET_RUNTIME_TRUST_INPUT]="3"*64
            with self.assertRaises(RuntimeError):runner.validate_profile_environment("android-cargo",env)
if __name__=="__main__":unittest.main()
