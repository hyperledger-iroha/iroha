"""Exact data-only issuer envelope; no configured production issuer authority."""
import base64
import hashlib
import json
import unittest
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.ordinary_enrollment import CHALLENGE_BODY_BYTES
from iroha_app_attestation.ordinary_service import SCHEMA, PATH, RAW_SCHEMA, RAW_PATH, OrdinaryCredentialService, decode_request, decode_raw_request
from test_ordinary_enrollment import challenge


def request_body():
    subject=challenge()
    encode=lambda original:base64.b64encode(original).decode()
    return {'schema':SCHEMA,'operation':'issue','operation_id':subject.attestation_challenge().hex(),
        'signed_preparation_base64':encode(subject.signing_bytes()[-CHALLENGE_BODY_BYTES:]+bytes(64)),
        'attested_public_key_sec1_base64':encode(b'\x04'+b'\x22'*64),
        'raw_attestation_base64':encode(b'data-only raw original'),
        'app_possession':{'platform':'android_keystore','signature_der_base64':encode(b'data-only DER')},
        'play_integrity_token':'opaque-token'}


class OrdinaryServiceTests(unittest.TestCase):
    def test_raw_only_envelope_has_no_possession_or_policy_and_default_route_stays_closed(self):
        value=request_body();value['schema']=RAW_SCHEMA
        value.pop('app_possession');value.pop('play_integrity_token')
        original=json.dumps(value).encode();request=decode_raw_request(original)
        self.assertEqual(request.operation_id,challenge().attestation_challenge())
        self.assertFalse(hasattr(request,'raw_possession'))
        self.assertEqual(OrdinaryCredentialService().handle(method='POST',path=RAW_PATH,body=original,
            content_type='application/json',transport_context={'caller_identity':'claimed-Core'}),
            (503,b'{"error":"issuer_unavailable"}'))
        for name in ['app_possession','play_integrity_token','raw_admission','authority_public_key','policy','google_verdict']:
            bad=dict(value);bad[name]='claimed';
            with self.assertRaises(AttestationRejected):decode_raw_request(json.dumps(bad).encode())
        with self.assertRaises(AttestationRejected):decode_request(original)

    def test_exact_envelope_parses_only_as_data_and_default_service_stays_closed(self):
        original=json.dumps(request_body()).encode();request=decode_request(original)
        self.assertEqual(request.operation_id,challenge().attestation_challenge())
        self.assertEqual(request.raw_possession,b'data-only DER')
        result=OrdinaryCredentialService().handle(method='POST',path=PATH,body=original,
            content_type='application/json',transport_context={'caller_identity':'claimed-Core'})
        self.assertEqual(result,(503,b'{"error":"issuer_unavailable"}'))

    def test_no_old_width_policy_roots_verdicts_or_unsigned_operation_alias(self):
        for change in ({'root_der_base64':'AAA='},{'policy':{}},{'google_verdict':{}},
            {'operation_id':challenge().enrollment_id.hex()},{'operation':'unknown'},
            {'schema':'old'},{'signed_preparation_base64':base64.b64encode(bytes(507)).decode()},
            {'signed_preparation_base64':base64.b64encode(bytes(273)).decode()},
            {'attested_public_key_sec1_base64':base64.b64encode(bytes(65)).decode()},
            {'play_integrity_token':'x'*(64*1024+1)}):
            value=request_body();value.update(change)
            with self.subTest(change=change.keys()),self.assertRaises(AttestationRejected):decode_request(json.dumps(value).encode())
        original=json.dumps(request_body()).encode()[:-1]+b',"operation":"issue"}'
        with self.assertRaises(AttestationRejected):decode_request(original)

    def test_possession_envelope_encodings_and_platform_classes_are_closed(self):
        for possession in ({'platform':'android_keystore','signature_der_base64':'AA=='},
            {'platform':'android_keystore','signature_der_base64':base64.b64encode(b'\0'*73).decode()},
            {'platform':'apple_app_attest','raw_assertion_base64':base64.b64encode(b'assertion').decode()},
            {'platform':'android_keystore','signature_der_base64':'data!'},
            {'platform':'android_keystore','signature_der_base64':base64.b64encode(b'valid-length').decode(),'caller_ready':True}):
            value=request_body();value['app_possession']=possession
            with self.subTest(possession=possession.keys()),self.assertRaises(AttestationRejected):decode_request(json.dumps(value).encode())


if __name__=='__main__':unittest.main()
