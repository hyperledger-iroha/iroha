"""Native authentication of Time outputs from original registration through retries.

This API does not authenticate an application's choice of original operation or
replace its fee, artifact, account, and event policies. It has no Python crypto
or business-execution fallback.
"""
from __future__ import annotations

import json
from typing import Any

from .crypto import _crypto, _require_chain_discriminant, _require_network_id


def verify_scheduled_trigger_execution_v1(
    original_signed_registration: bytes,
    trigger_id: str,
    *,
    native_finality_proof_chain_json: str,
    expected_network_id: Any,
    expected_chain: str,
    trusted_checkpoint: bytes,
    expected_chain_discriminant: int,
    lifecycle_activation_json: str | None = None,
    lifecycle_activation_policy_json: str | None = None,
) -> dict[str, Any]:
    """Authenticate the complete consecutive prefix including registration.

    The result contains Time execution-call identities, never synthetic
    transaction hashes. Its internal route is the authenticated Global root
    with no Network lane assignment. Keep the registration's external route
    receipt separately. Persist original wires and the activation witness for recovery.

    V2 lifecycle rows additionally require ``lifecycle_activation_json``: the
    registration block's native ``runtime_proof``, ordered ``continuation_proofs``
    for every later block, and typed runtime selection, services and implementation
    originals. Bounded sparse branches prove the same native selection at every
    certified cut without enumerating World entries. Row variants alone never
    establish activation. Registrations preceding activation are refused if the
    proof window reaches lifecycle-selected execution. The optional argument
    leaves V1 verification unchanged.

    ``lifecycle_activation_policy_json`` optionally pins the PoC validator's
    explicit activation policy response. Every certified row must match its
    height boundary. A supplied policy with a null height means never; omitting
    the argument permits the committed-runtime corridor. Neither the response
    nor its readiness flag replaces native selection and finality proofs.
    """
    network = _require_network_id(expected_network_id, "expected_network_id")
    prefix = _require_chain_discriminant(expected_chain_discriminant, "expected_chain_discriminant")
    for name, value, maximum in (
        ("original_signed_registration", original_signed_registration, 32 * 1024 * 1024),
        ("trusted_checkpoint", trusted_checkpoint, 68 * 1024 * 1024),
    ):
        if type(value) is not bytes or not 0 < len(value) <= maximum:
            raise ValueError(f"{name} must be bounded exact immutable bytes")
    for name, text_value, maximum in (
        ("trigger_id", trigger_id, 255),
        ("expected_chain", expected_chain, 1024),
        ("native_finality_proof_chain_json", native_finality_proof_chain_json, 16 * 1024 * 1024),
    ):
        if type(text_value) is not str or not 0 < len(text_value.encode("utf-8")) <= maximum:
            raise ValueError(f"{name} must be bounded exact UTF-8 text")
    native_options: dict[str, Any] = {"expected_chain_discriminant": prefix}
    if lifecycle_activation_json is not None:
        if (type(lifecycle_activation_json) is not str
                or not 0 < len(lifecycle_activation_json) <= 32 * 1024 * 1024
                or len(lifecycle_activation_json.encode("utf-8")) > 32 * 1024 * 1024):
            raise ValueError("lifecycle_activation_json must be bounded exact UTF-8 text")
        native_options["lifecycle_activation_json"] = lifecycle_activation_json
    if lifecycle_activation_policy_json is not None:
        if (type(lifecycle_activation_policy_json) is not str
                or not 0 < len(lifecycle_activation_policy_json) <= 4096
                or len(lifecycle_activation_policy_json.encode("utf-8")) > 4096):
            raise ValueError("lifecycle_activation_policy_json must be bounded exact UTF-8 text")
        native_options["lifecycle_activation_policy_json"] = lifecycle_activation_policy_json
    verify = getattr(_crypto, "verify_scheduled_trigger_execution_v1", None)
    if not callable(verify):
        raise RuntimeError("selected native SDK lacks scheduled Time authentication")
    result = verify(
        original_signed_registration, trigger_id, native_finality_proof_chain_json,
        network, expected_chain, trusted_checkpoint,
        **native_options,
    )
    if type(result) is not tuple or len(result) != 2:
        raise RuntimeError("native Time verifier returned a malformed result")
    projection, checkpoint = result
    if (type(projection) is not str or type(checkpoint) is not bytes
            or not 0 < len(checkpoint) <= 68 * 1024 * 1024):
        raise RuntimeError("native Time verifier returned a malformed checkpoint")
    payload = json.loads(projection)
    if type(payload) is not dict or "promoted_checkpoint" in payload:
        raise RuntimeError("native Time verifier returned a malformed projection")
    return {**payload, "promoted_checkpoint": checkpoint}
