"""Native-generated private configuration DATA agrees with the real Python policy parser."""
import base64
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import unittest

from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.google_oauth import select_google_decoder
from iroha_app_attestation.play_integrity import PlayIntegrityEnrollmentPolicy
from iroha_app_attestation.wallet_enrollment import WalletEnrollmentScope
from iroha_app_attestation.wallet_enrollment_worker import (
    CONFIG_SCHEMA, MAX_CONFIG, configured_policy, exact_json,
)


class NativeConfigurationTests(unittest.TestCase):
    def test_rust_originals_reconstruct_the_exact_model_policy_and_request(self):
        path = Path(__file__).resolve().parents[3] / "fixtures/kagemusha/wallet_enrollment_worker_configuration_v1.json"
        vectors = json.loads(path.read_text())
        self.assertEqual([v["platform"] for v in vectors], ["android", "apple"])
        for vector in vectors:
            with self.subTest(platform=vector["platform"]):
                self.assertEqual(vector["authority"], "unadmitted DATA only")
                original = base64.b64decode(vector["configuration_base64"], validate=True)
                self.assertEqual(hashlib.sha256(original).hexdigest(), vector["configuration_sha256"])
                value = exact_json(original, MAX_CONFIG)
                self.assertEqual(set(value), {"schema", "version", "platform", "app_policy_hex",
                    "enrollment_policy_hex", "openssl_path", "openssl_sha256", "store_directory", "policy"})
                self.assertEqual(value["schema"], CONFIG_SCHEMA)
                self.assertEqual(value["version"], 1)
                self.assertEqual(value["platform"], vector["platform"])
                policy = configured_policy(value["policy"], value["platform"])
                self.assertEqual(policy.app.policy_digest().hex(), value["app_policy_hex"])
                self.assertEqual(policy.enrollment.policy_digest().hex(), value["enrollment_policy_hex"])
                self.assertEqual(policy.app.transcript(), base64.b64decode(vector["app_transcript_base64"], validate=True))
                self.assertEqual(policy.enrollment.transcript(), base64.b64decode(vector["enrollment_transcript_base64"], validate=True))
                request = exact_json(base64.b64decode(vector["request_base64"], validate=True), MAX_CONFIG)
                scope = WalletEnrollmentScope(base64.b64decode(request["challenge_transcript_base64"], validate=True),
                    base64.b64decode(request["payment_key_base64"], validate=True))
                scope.validate()
                policy.validate_scope(scope.challenge_transcript, request["issued_at_ms"], request["trusted_time_ms"])
                self.assertEqual(request["expires_at_ms"], request["issued_at_ms"] + policy.enrollment.challenge_lifetime_ms)
                # These DATA paths/pins are deliberately never opened or admitted as a runtime.
                if value["platform"] == "android":
                    selected = policy.play_integrity_policy()
                    decoder = base64.b64decode(value["policy"]["google_policy_base64"], validate=True)
                    decoder_pin = bytes.fromhex(value["policy"]["google_policy_sha256"])
                    self.assertEqual(hashlib.sha256(decoder).digest(), decoder_pin)
                    native = PlayIntegrityEnrollmentPolicy(decoder_pin, selected.package_name,
                        selected.package_version, selected.app_signing_certificate_sha256,
                        selected.maximum_evidence_age_ms, selected.require_play_recognized,
                        selected.require_licensed, selected.minimum_device_integrity)
                    principal = select_google_decoder(decoder, native)
                    self.assertEqual(principal.project_id, "vector-project")
                    with self.assertRaises(AttestationRejected):
                        select_google_decoder(decoder, replace(native, package_name="foreign.app"))
                changed = dict(value["policy"], root_base64=base64.b64encode(b"different DATA root").decode())
                with self.assertRaises(AttestationRejected):
                    configured_policy(changed, value["platform"])


if __name__ == "__main__":
    unittest.main()
