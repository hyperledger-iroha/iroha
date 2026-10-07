"""Public SOFTWARE DATA profile controls; no compiler, keys or Native runtime."""
import importlib.util
from pathlib import Path
import unittest
spec=importlib.util.spec_from_file_location("private_mobile_hermetic",Path(__file__).parents[1]/"run_mobile_hermetic_command.py")
runner=importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
class RuntimeTrustTests(unittest.TestCase):
    def environment(self,profile):
        env=dict.fromkeys(runner.PROFILES[profile],"SYNTHETIC public DATA")
        if profile in runner.AUTHENTICATED_CARGO_PROFILES: env[runner.WALLET_RUNTIME_TRUST_INPUT]="3"*64
        return env
    def test_every_authenticated_mobile_profile_requires_the_public_root(self):
        for profile in runner.AUTHENTICATED_CARGO_PROFILES:
            with self.subTest(profile=profile):
                env=self.environment(profile);del env[runner.WALLET_RUNTIME_TRUST_INPUT]
                with self.assertRaises(RuntimeError):runner.validate_profile_environment(profile,env)
    def test_valid_root_is_preserved_by_all_mobile_profiles(self):
        for profile in runner.AUTHENTICATED_CARGO_PROFILES:
            env=self.environment(profile);runner.validate_profile_environment(profile,env)
            self.assertEqual(runner.wallet_runtime_trust(env[runner.WALLET_RUNTIME_TRUST_INPUT]),"3"*64)
    def test_malformed_public_root_refuses_every_mobile_profile(self):
        for profile in runner.AUTHENTICATED_CARGO_PROFILES:
            for value in ["","0"*64,"A"*64,"g"*64,"1"*63,"1"*65,"1"*64+"\n"]:
                with self.subTest(profile=profile,value=value):
                    env=self.environment(profile);env[runner.WALLET_RUNTIME_TRUST_INPUT]=value
                    with self.assertRaises(RuntimeError):runner.validate_profile_environment(profile,env)
    def test_non_native_build_profiles_do_not_accept_the_root(self):
        for profile in set(runner.PROFILES)-runner.AUTHENTICATED_CARGO_PROFILES:
            env=self.environment(profile);runner.validate_profile_environment(profile,env)
            env[runner.WALLET_RUNTIME_TRUST_INPUT]="3"*64
            with self.assertRaises(RuntimeError):runner.validate_profile_environment(profile,env)
    def test_other_ambient_flags_remain_closed(self):
        for name in ["RUSTFLAGS","RUSTC_WRAPPER","CARGO_ENCODED_RUSTFLAGS","NATIVE_READY"]:
            env=self.environment("android-cargo");env[name]="offered"
            with self.assertRaises(RuntimeError):runner.validate_profile_environment("android-cargo",env)
    def test_public_root_cannot_replace_any_other_required_input(self):
        for profile in runner.AUTHENTICATED_CARGO_PROFILES:
            for name in runner.PROFILES[profile]:
                env=self.environment(profile);del env[name]
                with self.assertRaises(RuntimeError):runner.validate_profile_environment(profile,env)
if __name__=="__main__":unittest.main()
