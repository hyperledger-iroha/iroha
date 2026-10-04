"""Closed periodic-refresh envelope for the authenticated Native parent only.

TODO: spec §2.2 deletes the periodic Play Integrity refresh lease. Remove this
route with ``ordinary_refresh_issuance.py``, ``play_integrity_refresh.py`` and
their Native consumers.
"""
import base64
import hashlib
import json

from .attestation import AttestationRejected, VerificationUnavailable, require
from .ordinary_refresh_issuance import OrdinaryIntegrityRefreshRequest
from .service import _decode_base64, _decode_hex32, _unique_object

SCHEMA='iroha.kagemusha.play-integrity-refresh-request.v1'
PATH='/v1/kagemusha/ordinary-app-integrity-refresh'
FIELDS={'schema','operation','operation_id','signed_refresh_challenge_base64',
        'signature_der_base64','play_integrity_token'}


def decode_request(body:bytes) -> OrdinaryIntegrityRefreshRequest:
    require(type(body) is bytes and 0 < len(body) <= 96*1024,"refresh request outside bound")
    try:
        value=json.loads(body.decode('utf-8'),object_pairs_hook=_unique_object,
            parse_constant=lambda _:(_ for _ in ()).throw(ValueError()))
    except (ValueError,UnicodeError,RecursionError):
        raise AttestationRejected('invalid refresh JSON') from None
    require(type(value) is dict and set(value)==FIELDS and value['schema']==SCHEMA,
            "invalid refresh request fields")
    request=OrdinaryIntegrityRefreshRequest(value['operation'],_decode_hex32(value['operation_id'],'refresh operation'),
        _decode_base64(value['signed_refresh_challenge_base64'],'signed refresh challenge',514),
        _decode_base64(value['signature_der_base64'],'original refresh possession',72),value['play_integrity_token'])
    request.validate();return request


def handle_refresh(issuer,body:bytes) -> tuple[int,bytes]:
    # Only OrdinaryCredentialService reaches this after its actual parent check.
    from .ordinary_refresh_issuance import DurableOrdinaryIntegrityRefreshIssuer
    if type(issuer) is not DurableOrdinaryIntegrityRefreshIssuer:
        return 503,b'{"error":"issuer_unavailable"}'
    try:request=decode_request(body)
    except AttestationRejected:return 400,b'{"error":"invalid_request"}'
    try:original=issuer.refresh(request)
    # An unavailable Google decoder or OAuth token is retryable, not a rejection.
    except VerificationUnavailable:return 503,b'{"error":"issuer_unavailable"}'
    except AttestationRejected:return 409,b'{"error":"refresh_rejected"}'
    except Exception:return 503,b'{"error":"issuer_unavailable"}'
    return 200,json.dumps({'lease_base64':base64.b64encode(original).decode('ascii'),
        'lease_sha256_hex':hashlib.sha256(original).hexdigest()},sort_keys=True,separators=(',',':')).encode('ascii')
