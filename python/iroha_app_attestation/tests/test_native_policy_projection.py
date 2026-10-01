"""Strict public fixture projections; no test constructs Native custody."""
import base64
import copy
import hashlib
import json
import unittest
from pathlib import Path
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.native_policy_projection import decode_native_policy_projection, PROVIDER_SCHEMA, SCHEMA
from iroha_app_attestation.provider import _apple_release_digest


def encoded(value):return base64.b64encode(value).decode()
def original(value):return json.dumps(value,sort_keys=True,separators=(',',':')).encode()
def fixture(apple=False):
    directory=Path(__file__).parent/'fixtures';profile='03'*32
    config={'package_name':'org.example.wallet','package_version':28,'app_signing_certificate_sha256':'22'*32,
            'attestation_roots_der_base64':[encoded((directory/'google_factory_root_2016.der').read_bytes())]}
    if apple:
        config={'app_id':'TEAM123456.org.example.wallet','environment':'production','expected_validation_category':3,
                'expected_bundle_version':'28','attestation_root_der_base64':encoded((directory/'apple_app_attestation_root.der').read_bytes()),
                'receipt_root_der_base64':encoded((directory/'apple_root_ca_g3.der').read_bytes())}
    platform='apple_app_attest' if apple else 'android_keymint'
    provider={'schema':PROVIDER_SCHEMA,'version':1,'profiles':[{'hardware_profile_id':profile,'platform_class':platform,'configuration':config}]}
    provider_raw=original(provider)
    public_google=original({'schema':'iroha.kagemusha.play-integrity-verification-policy.v1','version':1,
        'cloudProject':{'id':'example-cloud-project','number':642560099159},'packageName':'org.example.wallet',
        'packageVersion':28,'appSigningCertificateSha256Hex':'22'*32,
        'credentialSubject':{'email':'kina-integrity-decoder@example-cloud-project.iam.gserviceaccount.com','clientId':'116801097826214108702'}})
    trust=b'isolated original trust fixture';authority=b'isolated original authority fixture'
    integrity={'policy_digest':hashlib.sha256(public_google).hexdigest(),'maximum_evidence_age_ms':5000,
               'maximum_refresh_interval_ms':30000,'require_play_recognized':True,'require_licensed':True,'minimum_device_integrity':1}
    entry={'version':1,'release_id':'01'*32,'network_id':'02'*32,'hardware_profile_id':profile,'platform_class':platform,
        'suite_id':'04'*32,'policy_epoch':21,'profile_valid_from_ms':1000,'profile_expires_at_ms':10000000,
        'provider_policy_root':'33'*32,'authority_policy_digest':'06'*32,'trust_policy_digest':'05'*32,'issuer_policy_digest':'07'*32,
        'app_signing_identity_digest':hashlib.sha256(config['app_id'].encode()).hexdigest() if apple else '22'*32,
        'app_release_digest':_apple_release_digest(3,'28').hex() if apple else '08'*32,
        'authority_public_key':'74'*32,'core_preparation_public_key':'73'*32,'maximum_credential_lifetime_ms':3600000,
        'allowed_android_security_levels':[] if apple else [1,2],'play_integrity_policy':None if apple else integrity,
        'play_integrity_policy_base64':None if apple else encoded(public_google),
        'ordinary_trust_policy_base64':encoded(trust),'app_authority_policy_base64':encoded(authority),
        'original_sha256':{'release_manifest':'11'*32,'issuer_policy':'12'*32,'ordinary_trust_policy':hashlib.sha256(trust).hexdigest(),
            'app_authority_policy':hashlib.sha256(authority).hexdigest(),'play_integrity_policy':None if apple else hashlib.sha256(public_google).hexdigest()}}
    return {'schema':SCHEMA,'version':1,'profiles':[entry],'provider_selection_base64':encoded(provider_raw),
            'original_sha256':{'provider_selection':hashlib.sha256(provider_raw).hexdigest()}}


class NativePolicyProjectionTests(unittest.TestCase):
    def test_android_public_originals_match_platform_policy_and_retained_google_principal(self):
        raw=original(fixture());projection=decode_native_policy_projection(raw)
        self.assertEqual(projection.original_sha256,hashlib.sha256(raw).digest())
        self.assertEqual(len(projection.policies),1);policy=projection.policies[0]
        self.assertEqual(policy.allowed_android_levels,frozenset({1,2}))
        self.assertEqual(policy.platform_policy.package_version,28)
        self.assertEqual(len(projection.google_public_originals),1)
        self.assertEqual(policy.platform_policy.root_for_chain(policy.platform_policy.attestation_root_der),policy.platform_policy.attestation_root_der)
        with self.assertRaises(AttestationRejected):policy.platform_policy.root_for_chain(b'caller supplied root')

    def test_apple_app_id_distribution_and_real_published_roots_are_required(self):
        value=fixture(True);policy=decode_native_policy_projection(original(value)).policies[0]
        self.assertEqual(policy.platform_class,2);self.assertEqual(policy.platform_policy.environment,'production')
        self.assertIsNone(policy.play_integrity_policy)
        value['profiles'][0]['app_signing_identity_digest']='22'*32
        with self.assertRaises(AttestationRejected):decode_native_policy_projection(original(value))

    def test_substituted_or_missing_originals_and_client_policy_fields_are_rejected(self):
        for mutate in (
            lambda v:v.update({'authority':True}),
            lambda v:v['original_sha256'].update({'provider_selection':'99'*32}),
            lambda v:v['profiles'][0]['original_sha256'].update({'play_integrity_policy':'99'*32}),
            lambda v:v['profiles'][0].update({'app_signing_identity_digest':'99'*32}),
            lambda v:v['profiles'][0].update({'play_integrity_policy_base64':None}),
            lambda v:v['profiles'][0].update({'maximum_credential_lifetime_ms':True}),
            lambda v:v['profiles'][0].update({'allowed_android_security_levels':[1,1]}),
            lambda v:v['profiles'][0]['play_integrity_policy'].update({'minimum_device_integrity':True}),
        ):
            value=fixture();mutate(value)
            with self.assertRaises(AttestationRejected):decode_native_policy_projection(original(value))

    def test_duplicate_profiles_public_json_and_unserved_provider_selection_fail(self):
        value=fixture();value['profiles'].append(copy.deepcopy(value['profiles'][0]))
        with self.assertRaises(AttestationRejected):decode_native_policy_projection(original(value))
        raw=original(fixture());raw=raw[:-1]+b',"version":1}'
        with self.assertRaises(AttestationRejected):decode_native_policy_projection(raw)
        value=fixture();provider=json.loads(base64.b64decode(value['provider_selection_base64']))
        provider['profiles'].append(copy.deepcopy(provider['profiles'][0]));provider['profiles'][-1]['hardware_profile_id']='99'*32
        raw_provider=original(provider);value['provider_selection_base64']=encoded(raw_provider)
        value['original_sha256']['provider_selection']=hashlib.sha256(raw_provider).hexdigest()
        with self.assertRaises(AttestationRejected):decode_native_policy_projection(original(value))
