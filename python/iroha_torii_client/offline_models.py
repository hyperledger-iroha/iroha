"""Closed first-release JSON shapes for Torii offline/Kagemusha requests."""

from __future__ import annotations

from typing import List, Literal, Optional, TypedDict, Union

OfflineAssetScale = Literal[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    13,
    14,
    15,
    16,
    17,
    18,
    19,
    20,
    21,
    22,
    23,
    24,
    25,
    26,
    27,
    28,
]


class OfflineScaledAmountJson(TypedDict):
    """Direct JSON shape of one positive, scale-bound offline amount."""

    atomic_units: int
    scale: OfflineAssetScale


class OfflineSpendableNoteJson(TypedDict):
    """Direct JSON shape of one scale-, network-, and asset-bound note."""

    network_id: str
    asset: str
    note_commitment: List[int]
    spend_nullifier: List[int]
    amount: OfflineScaledAmountJson


class _OfflineAuthorizationJsonOptional(TypedDict, total=False):
    app_attest_evidence_sha256: Optional[List[int]]
    app_attest_evidence: Optional[List[int]]


class OfflineAuthorizationJson(_OfflineAuthorizationJsonOptional):
    """Self-contained device authorization embedded in an offline command."""

    authority: str
    device_id: str
    operation_id: List[int]
    issued_at_ms: int
    expires_at_ms: int
    nonce: List[int]
    payload_digest: List[int]
    signature: str


class OfflineVerifierKeyIdJson(TypedDict):
    """Registry identity of one proof verifier."""

    backend: str
    name: str


class OfflineProofBoxJson(TypedDict):
    """Opaque proof bytes with their backend identity."""

    backend: str
    bytes: List[int]


class OfflineVerifyingKeyJson(TypedDict):
    """Opaque verifier bytes with their backend identity."""

    backend: str
    bytes: List[int]


OfflineProofBackend = Literal[
    "halo2/ipa",
    "halo2/pasta/kaigi-roster-v1",
    "halo2/pasta/kaigi-usage-v1",
    "halo2/pasta/ivm-execution-v1",
    "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    "stark/fri/poseidon-x7-goldilocks-6x64-v1",
]
OfflineVerifierStatus = Literal["Proposed", "Active", "Withdrawn"]


class _OfflineVerifyingKeyRecordJsonOptional(TypedDict, total=False):
    owner_manifest_id: Optional[str]
    gas_schedule_id: Optional[str]
    metadata_uri_cid: Optional[str]
    vk_bytes_cid: Optional[str]
    activation_height: Optional[int]
    withdraw_height: Optional[int]
    key: Optional[OfflineVerifyingKeyJson]


class OfflineVerifyingKeyRecordJson(_OfflineVerifyingKeyRecordJsonOptional):
    """Governance-managed verifier record submitted with offline proofs."""

    version: int
    circuit_id: str
    namespace: str
    backend: OfflineProofBackend
    curve: str
    public_inputs_schema_hash: List[int]
    commitment: List[int]
    vk_len: int
    max_proof_bytes: int
    status: OfflineVerifierStatus


class OfflineMerkleProofJson(TypedDict):
    """Merkle authentication path carried by a lane-privacy witness."""

    leaf_index: int
    audit_path: List[str]


class OfflineLanePrivacyMerkleWitnessJson(TypedDict):
    """Typed Merkle lane-privacy witness."""

    leaf: List[int]
    proof: OfflineMerkleProofJson


class OfflineLanePrivacyMerkleVariantJson(TypedDict):
    """Merkle variant of a lane-privacy witness."""

    kind: Literal["merkle"]
    payload: OfflineLanePrivacyMerkleWitnessJson


class OfflineLanePrivacyProofJson(TypedDict):
    """Lane commitment identity and its typed privacy witness."""

    commitment_id: List[int]
    witness: OfflineLanePrivacyMerkleVariantJson


class _OfflineProofAttachmentJsonOptional(TypedDict, total=False):
    vk_commitment: Optional[List[int]]
    envelope_hash: Optional[List[int]]
    lane_privacy: Optional[OfflineLanePrivacyProofJson]


class OfflineProofAttachmentJson(_OfflineProofAttachmentJsonOptional):
    """Typed proof attachment used by offline commands."""

    backend: str
    proof: OfflineProofBoxJson
    vk_ref: OfflineVerifierKeyIdJson


class OfflineTopUpShieldEvidenceJson(TypedDict):
    """Public-to-confidential insertion proof for one online top-up."""

    initial_root: List[int]
    finalized_root: List[int]
    leaf_index: int
    proof: OfflineProofAttachmentJson


class OfflineVerifiedFoldStepJson(TypedDict):
    """One checked confidential-transfer proof step."""

    root_before: List[int]
    input_nullifiers: List[List[int]]
    output_commitments: List[List[int]]
    root_after: List[int]
    attachment: OfflineProofAttachmentJson
    verifier_key: OfflineVerifyingKeyJson


class OfflineVerifiedFoldBundleJson(TypedDict):
    """Network- and asset-bound ordered transfer proof steps."""

    network_id: str
    asset: str
    steps: List[OfflineVerifiedFoldStepJson]


class OfflineVerifiedFoldVerifierRecordJson(TypedDict):
    """Registry record selected by one checked fold step."""

    id: OfflineVerifierKeyIdJson
    record: OfflineVerifyingKeyRecordJson


class OfflineVerifiedFoldRecordBundleJson(TypedDict):
    """Checked one-hop proof bundle in direct Norito JSON form."""

    bundle: OfflineVerifiedFoldBundleJson
    verifier_records: List[OfflineVerifiedFoldVerifierRecordJson]


class OfflineTopUpAnchorReferenceJson(TypedDict):
    """Compact chain-resolvable identity of one finalized top-up."""

    topup_operation_id: List[int]
    anchor_digest: List[int]


class OfflineBranchPathJson(TypedDict):
    """Canonical branch coordinate inside one top-up lineage."""

    lineage_root: List[int]
    depth: int
    path_bits: List[int]


class OfflineBranchClaimJson(TypedDict):
    """Replay-safe conflict claim for one spendable lineage leaf."""

    path: OfflineBranchPathJson
    transition_tags: str


class _OfflineTaggedUnitJsonOptional(TypedDict, total=False):
    value: None


class OfflineSpendBranchJson(_OfflineTaggedUnitJsonOptional):
    """Recipient or sender-change output role."""

    branch: Literal["recipient", "change"]


class KagemushaArtifactBindingV4Json(TypedDict):
    """Identity of the one authenticated Kagemusha ABI-21/V4 release."""

    version: Literal[4]
    generation: str
    manifest_sha256: List[int]


class OfflinePeerSplitTransitionJson(TypedDict):
    """Proof-bound peer-split transition payload."""

    binding_digest: List[int]
    branch: OfflineSpendBranchJson
    recipient_request_digest: List[int]
    operation_id: List[int]
    parent_max_proof_step_count: int
    parent_max_peer_hop_count: int


class OfflineRedemptionChangeTransitionJson(TypedDict):
    """Proof-bound partial-redemption change transition payload."""

    binding_digest: List[int]
    parent_bundle_digest: List[int]
    operation_id: List[int]
    parent_proof_step_count: int
    parent_peer_hop_count: int


class OfflinePeerSplitTransitionVariantJson(TypedDict):
    """Tagged peer-split transition."""

    transition: Literal["peer_split"]
    value: OfflinePeerSplitTransitionJson


class OfflineRedemptionChangeTransitionVariantJson(TypedDict):
    """Tagged partial-redemption change transition."""

    transition: Literal["redemption_change"]
    value: OfflineRedemptionChangeTransitionJson


OfflineRecursiveSpendTransitionJson = Union[
    OfflinePeerSplitTransitionVariantJson,
    OfflineRedemptionChangeTransitionVariantJson,
]


class _OfflineRecursiveSpendStatementJsonOptional(TypedDict, total=False):
    transition: Optional[OfflineRecursiveSpendTransitionJson]


class OfflineRecursiveSpendStatementJson(_OfflineRecursiveSpendStatementJsonOptional):
    """Exact public statement bound by one recursive spend proof."""

    network_id: str
    asset: str
    asset_scale: OfflineAssetScale
    final_root: List[int]
    next_zero_leaf_index: int
    topup_anchor_refs: List[OfflineTopUpAnchorReferenceJson]
    proof_step_count: int
    peer_hop_count: int
    current_note: OfflineSpendableNoteJson
    branch_claims: List[OfflineBranchClaimJson]
    artifact_binding: KagemushaArtifactBindingV4Json
    verifier_key_id: OfflineVerifierKeyIdJson


class OfflineRecursiveSpendProofJson(TypedDict):
    """Recursive proof and its exact verifier/public-statement bindings."""

    verifier_key_id: OfflineVerifierKeyIdJson
    public_statement_digest: List[int]
    proof: OfflineProofBoxJson


class OfflineRecursiveSpendBundleJson(TypedDict):
    """Scale-carrying recursive state submitted for redemption."""

    statement: OfflineRecursiveSpendStatementJson
    recursive_proof: OfflineRecursiveSpendProofJson


class OfflineUnshieldPublicInputsJson(TypedDict):
    """Canonical unshield public words bound by a redemption transition."""

    input_commitment_0: List[int]
    input_commitment_1: List[int]
    nullifier_0: List[int]
    nullifier_1: List[int]
    change_output_commitment: List[int]
    root: List[int]
    public_amount: List[int]
    asset_tag: List[int]
    network_tag: List[int]


class _OfflineRedemptionIntentJsonOptional(TypedDict, total=False):
    change_output: Optional[OfflineSpendableNoteJson]
    change_artifact_binding: Optional[KagemushaArtifactBindingV4Json]


class OfflineRedemptionIntentJson(_OfflineRedemptionIntentJsonOptional):
    """Canonical public redemption intent covered by the authorization."""

    network_id: str
    asset: str
    input_note: OfflineSpendableNoteJson
    parent_branch_claims: List[OfflineBranchClaimJson]
    parent_topup_anchor_refs: List[OfflineTopUpAnchorReferenceJson]
    parent_proof_step_count: int
    parent_peer_hop_count: int
    parent_bundle_digest: List[int]
    input_root: List[int]
    recipient: str
    public_amount: OfflineScaledAmountJson
    unshield_public_inputs: OfflineUnshieldPublicInputsJson
    unshield_public_inputs_digest: List[int]
    operation_id: List[int]


class OfflineRedeemChangeJson(TypedDict):
    """Proof-bound change branch retained after partial redemption."""

    output: OfflineSpendableNoteJson
    branch_claims: List[OfflineBranchClaimJson]
    bundle: OfflineRecursiveSpendBundleJson
