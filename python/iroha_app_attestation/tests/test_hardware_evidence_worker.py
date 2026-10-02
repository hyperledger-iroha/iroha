"""Hardware-only private worker grammar and genuine public-key equations.

Synthetic public certificates below exercise the existing PKIX/KeyDescription
kernel only; they construct no installed owner, Google verdict or issuer cap.
"""
import hashlib
import shutil
import tempfile
import time
import unittest
from pathlib import Path
from iroha_app_attestation.attestation import (AttestationRejected, ANDROID_KEY_DESCRIPTION_OID,
    verify_android_persistent_app_key_raw)
from iroha_app_attestation.hardware_evidence_worker import (_OriginalGenerationChallenge,
    require_original_window)
from test_synthetic_platform_evidence import SignedEnvelope, keymint_description

class HardwareEvidenceWorkerTests(unittest.TestCase):
    def test_exact_full_signed_archive_is_generation_nonce_original(self):
        original=b"full canonical signed C including issuer signature"
        selected=_OriginalGenerationChallenge(original)
        self.assertEqual(selected.transcript(),original)
        self.assertEqual(selected.attested_key_id,bytes(32))
        self.assertNotEqual(hashlib.sha256(selected.transcript()).digest(),hashlib.sha256(original[:-1]).digest())

    def test_original_window_refuses_future_expired_and_extended_attempts(self):
        require_original_window(100,200,100)
        require_original_window(100,200,199)
        for args in [(100,200,99),(100,200,200),(0,200,100),(100,120101,100),(True,200,100),(100,200,True)]:
            with self.subTest(args=args),self.assertRaises(AttestationRejected): require_original_window(*args)

    def test_full_archive_challenge_authenticates_both_hardware_levels_and_rejects_substitution(self):
        openssl=Path(shutil.which('openssl')).resolve()
        with tempfile.TemporaryDirectory() as directory:
            fixture=SignedEnvelope(Path(directory),openssl)
            selected=_OriginalGenerationChallenge(b"complete signed challenge original for synthetic PKIX test")
            now=int(time.time()*1000)+60000
            for level in (1,2):
                description=keymint_description(selected,'org.example.wallet',7,b'\x71'*32,
                    attestation_version=100,keymaster_version=100,security_level=level,keymint_security_level=level)
                leaf,root=fixture.sign(ANDROID_KEY_DESCRIPTION_OID,description)
                args=([leaf,root],selected,'org.example.wallet',7,b'\x71'*32,root,hashlib.sha256(root).digest(),now,openssl)
                proof=verify_android_persistent_app_key_raw(*args,allowed_security_levels=frozenset({1,2}))
                self.assertEqual(proof.android_security_level,level)
                self.assertEqual(proof.attested_public_key_sec1,fixture.point)
                changed=list(args);changed[1]=_OriginalGenerationChallenge(selected.original+b'changed issuer signature')
                with self.assertRaisesRegex(AttestationRejected,'challenge mismatch'):
                    verify_android_persistent_app_key_raw(*changed,allowed_security_levels=frozenset({1,2}))
                changed=list(args);changed[2]='org.changed.wallet'
                with self.assertRaises(AttestationRejected):
                    verify_android_persistent_app_key_raw(*changed,allowed_security_levels=frozenset({1,2}))

if __name__=='__main__':unittest.main()
