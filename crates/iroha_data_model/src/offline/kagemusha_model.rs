#[model]
mod model {
    use super::*;
    #[cfg(feature = "json")]
    use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
    /// Sole first-release Kagemusha device authority key.
    ///
    /// The wire value is exactly one canonical uncompressed SEC1 NIST P-256
    /// point (`0x04 || x || y`). There is deliberately no algorithm tag or
    /// selector in this type or any request carrying it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct KagemushaDevicePublicKeyV2(
        pub(super) [u8; KAGEMUSHA_DEVICE_PUBLIC_KEY_SEC1_BYTES_V2],
    );
    /// Sole first-release Kagemusha device signature.
    ///
    /// The wire value is exactly the fixed-width big-endian ECDSA scalar pair
    /// `r || s`. Both scalars must be in `1..n`, and `s` must be low. DER and
    /// recoverable encodings are not part of the protocol.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    pub struct KagemushaDeviceSignatureV2(pub(super) [u8; KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2]);
    /// Exact amount contract for fractional recursive Kagemusha cash.
    ///
    /// `atomic_units` is the positive proof amount. `scale` is copied from the
    /// authoritative asset definition and determines the public quantity
    /// spelling used when charging or crediting the online balance.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaScaledAmountV2 {
        /// Positive proof amount in the asset's smallest unit.
        pub atomic_units: u128,
        /// Authoritative on-chain asset scale.
        pub scale: u32,
    }
    /// Scale-, network-, and asset-bound spendable note descriptor.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaSpendableNoteDescriptorV2 {
        /// Exact network that scopes the commitment and nullifier.
        pub network_id: NetworkId,
        /// Asset committed by the confidential note.
        pub asset: AssetDefinitionId,
        /// Current note commitment.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Nullifier consumed by the next split or redemption.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Exact amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
    }
    /// Secret-free Merkle authentication path retained with an owned note.
    ///
    /// Witness nodes are deliberately absent: native verification recomputes
    /// every Poseidon node from the note commitment and these canonical fields.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaConfidentialMerklePathV2 {
        /// One canonical sibling field element per tree level.
        pub siblings: Vec<[u8; 32]>,
        /// One left (`0`) or right (`1`) direction per tree level.
        pub directions: Vec<u8>,
        /// Root authenticated by the complete path.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub root: [u8; 32],
    }
    /// Proof-bound membership state required to spend one recursive output.
    ///
    /// `input_path` authenticates the owned note at `leaf_index`.
    /// `dummy_input_path` authenticates a distinct canonical empty leaf against
    /// the same root for the fixed two-input confidential circuits.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaNoteMembershipWitnessV2 {
        /// Confidential-tree position of the owned note.
        pub leaf_index: u32,
        /// Path authenticating the owned note commitment.
        pub input_path: KagemushaConfidentialMerklePathV2,
        /// Path authenticating a distinct empty leaf for dummy input two.
        pub dummy_input_path: KagemushaConfidentialMerklePathV2,
    }
    /// Canonical branch coordinate inside one top-up lineage.
    ///
    /// The first `depth` most-significant bits of `path_bits` identify the
    /// branch. Unused bits must be zero. A recipient output appends bit `0` and
    /// a sender-change output appends bit `1`. This makes sibling redemptions
    /// disjoint while allowing the ledger to reject an ancestor and any of its
    /// descendants by a deterministic prefix check.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendBranchPathV2 {
        /// Stable top-up lineage root, unique for one online-to-offline operation.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub lineage_root: [u8; 32],
        /// Number of significant path bits, from zero through 64.
        pub depth: u8,
        /// Big-endian branch bits; unused low-order bits are canonical zeroes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub path_bits: [u8; 8],
    }
    /// Replay-safe conflict claim for one independently spendable lineage leaf.
    ///
    /// `transition_tags` is one contiguous byte string containing exactly
    /// `path.depth` consecutive 24-byte entries. Entry `i` is the non-zero,
    /// domain-separated 192-bit tag of the complete proof-bound transition
    /// digest selected at the edge from depth `i` to `i + 1`.
    /// Carrying every ancestor choice prevents recipient/change outputs from
    /// alternative splits of the same parent from being mixed to inflate value.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendBranchClaimV2 {
        /// Canonical leaf coordinate used for ancestor/descendant conflicts.
        pub path: KagemushaRecursiveSpendBranchPathV2,
        /// Contiguous exact-depth transition-selection history with no padding.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
        pub transition_tags: Vec<u8>,
    }
    /// Public inputs used by the native bridge to derive one receiver-owned
    /// confidential output.
    ///
    /// The receiver's local note opening is deliberately not part of this
    /// archive. It is supplied through a separate native-only archive and must
    /// never cross a payment, Torii, or peer protocol boundary.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecipientOutputDerivationRequestV2 {
        /// Exact network that scopes the output commitment and nullifier.
        pub network_id: NetworkId,
        /// Asset definition committed by the receiver output.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Non-zero receiver-created nonce that domain-separates derivation.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
    }
    /// Public descriptor plus sender-prover material derived for one receiver
    /// output by the native bridge.
    ///
    /// `sender_output_prover_material` may contain only the amount opening,
    /// `rho`, and owner tag required by the sender's proof. It must never
    /// contain the receiver spend key or the output diversifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecipientOutputDerivationResultV2 {
        /// Receiver-owned confidential output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Opaque, bounded opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
    }
    /// Canonical unsigned fields of a receiver-created payment request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecipientPaymentRequestSigningPayloadV2 {
        /// Exact network that scopes the note and its nullifier.
        pub network_id: NetworkId,
        /// Asset definition requested by the receiver.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Online account used only for recipient/request identity binding.
        pub recipient: AccountId,
        /// Domain-separated receiver-device public-key reference.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_key_reference: [u8; 32],
        /// Registered receiver device identifier.
        pub receiver_device_id: String,
        /// Device-bound key that authenticates this request and its later ACK.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
        /// Unique request/nonce identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Exclusive Unix expiry in milliseconds.
        pub expires_at_ms: u64,
        /// Requested recipient output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Peer-carried opaque output-opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
    }
    /// Receiver-created, nonce-bound and device-signed request for one exact offline payment.
    ///
    /// `sender_output_prover_material` is part of the signed peer request but
    /// remains opaque to wallet code. The native bridge derives it from a
    /// receiver-held local note opening and the public request fields. It
    /// contains only the amount opening, `rho`, and owner tag needed to prove
    /// the requested commitment; it must never contain the receiver's spend
    /// key or diversifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecipientPaymentRequestV2 {
        /// Exact network that scopes the note and its nullifier.
        pub network_id: NetworkId,
        /// Asset definition requested by the receiver.
        pub asset: AssetDefinitionId,
        /// Exact requested amount at the authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Online account used only for recipient/request identity binding.
        pub recipient: AccountId,
        /// Stable receiver-side key reference; not secret key bytes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_key_reference: [u8; 32],
        /// Registered receiver device identifier.
        pub receiver_device_id: String,
        /// Device-bound public key authenticating the request and later ACK.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
        /// Unique request/nonce identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub request_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Exclusive Unix expiry in milliseconds.
        pub expires_at_ms: u64,
        /// Requested recipient output descriptor.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Peer-carried opaque output-opening archive consumed by the sender prover.
        pub sender_output_prover_material: Vec<u8>,
        /// Receiver-device signature over the canonical unsigned fields.
        pub signature: KagemushaDeviceSignatureV2,
    }
    /// Platform assertion made by the exact hardware key admitted at registration.
    ///
    /// Both platforms carry the same canonical raw low-S P-256 signature. iOS
    /// additionally carries the App Attest authenticator data that Apple binds
    /// ahead of the client-data hash.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaAndroidKeyMintHardwareAssertionV1 {
        /// Canonical raw low-S P-256 signature (`r || s`).
        pub signature: KagemushaDeviceSignatureV2,
    }
    /// Apple App Attest assertion result for an online operation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaIosAppAttestHardwareAssertionV1 {
        /// Exact authenticator data returned by `generateAssertion`.
        pub authenticator_data: Vec<u8>,
        /// Canonical raw low-S P-256 signature (`r || s`).
        pub signature: KagemushaDeviceSignatureV2,
    }
    /// Typed platform assertion, without a stringly-typed fallback variant.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(
        tag = "platform",
        content = "assertion",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum KagemushaOnlineHardwareAssertionV1 {
        /// Android `KeyMint` `SHA256withECDSA` assertion from a maxUsageCount=1 key.
        AndroidKeyMint(KagemushaAndroidKeyMintHardwareAssertionV1),
        /// Apple App Attest assertion over authenticatorData || clientDataHash.
        IosAppAttest(KagemushaIosAppAttestHardwareAssertionV1),
    }
    /// Self-contained payer/recipient hardware authorization carried inside one V2 archive.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRequestAuthorizationV2 {
        /// Account bound by the registered hardware assertion key.
        pub authority: AccountId,
        /// Registered device identifier used for exact registration lookup.
        pub device_id: String,
        /// Asset definition bound into the hardware-signed operation.
        pub asset_definition_id: AssetDefinitionId,
        /// Globally unique chain idempotency/replay identifier.
        ///
        /// Unlike nonces and payload digests, this identifier is not scoped by
        /// `authority`; every Kagemusha V2 chain operation shares one replay
        /// namespace.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Request creation time in Unix milliseconds.
        pub issued_at_ms: u64,
        /// Exclusive request expiry time in Unix milliseconds.
        pub expires_at_ms: u64,
        /// Unique signed nonce.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub nonce: [u8; 32],
        /// Digest of the canonical unsigned top-up or redemption payload.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub payload_digest: [u8; 32],
        /// Canonical Iroha hash of the exact registration admitted by consensus.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub registration_hash: [u8; 32],
        /// Typed assertion from the registered online hardware key.
        pub hardware_assertion: KagemushaOnlineHardwareAssertionV1,
    }
    /// Typed public-to-confidential shield evidence for one online top-up.
    ///
    /// The proof bytes remain opaque to wallets. The duplicated root and leaf
    /// fields let Torii reject stale requests before execution; the executor
    /// parses the proof public inputs and rechecks them against authoritative
    /// ledger state before mutating balances or the confidential tree.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpShieldEvidenceV2 {
        /// Authoritative confidential root before inserting the top-up note.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after inserting exactly the requested top-up note.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub finalized_root: [u8; 32],
        /// Authoritative zero-leaf position consumed by the insertion.
        pub leaf_index: u32,
        /// Canonical shield proof and registered verifier reference.
        pub proof: ProofAttachment,
    }
    /// Compact, ledger-resolvable reference carried by spendable peer bundles.
    ///
    /// The complete finalized anchor remains in chain state and in the init
    /// transition archive. Peer payloads carry only this strict identity pair;
    /// redemption resolves it before crediting any value.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpAnchorRefV2 {
        /// Stable top-up operation identifier used for the chain-state lookup.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub topup_operation_id: [u8; 32],
        /// Canonical digest of the complete finalized anchor.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub anchor_digest: [u8; 32],
    }
    /// Bounded projection of the live Sumeragi-v2 height context needed to
    /// authenticate one Commit certificate offline.
    ///
    /// `context_id` is copied from the persisted [`HeightContext`] and is part
    /// of the exact live [`crate::block::consensus_v2::Vote::signature_preimage`]
    /// through the certificate round. Every non-roster identity field is
    /// retained so verification can reconstruct and validate the complete
    /// context with the manifest-authenticated roster window, then require its
    /// computed identifier to equal `context_id`. This avoids duplicating the
    /// current roster in every proof without making the context identifier an
    /// opaque, attacker-selected cross-network binding.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityHeightContextV2 {
        /// Typed identifier of the complete persisted height context.
        pub context_id: HeightContextId,
        /// Exact genesis-derived network identity committed by the complete height context.
        pub network_id: NetworkId,
        /// Live Sumeragi-v2 wire protocol revision.
        pub protocol_version: u16,
        /// Height governed by the projected context.
        pub height: u64,
        /// Finalized validator-election epoch.
        pub epoch: u64,
        /// Last height governed by the current frozen epoch snapshot.
        pub epoch_end_height: u64,
        /// Complete next-epoch transition on an epoch-boundary height.
        #[norito(required)]
        pub next_epoch_snapshot: Option<FinalizedNextEpochSnapshot>,
        /// Consensus mode governing the frozen roster.
        pub mode: ConsensusMode,
        /// Parent Commit certificate, absent at genesis or an audited snapshot boundary.
        #[norito(required)]
        pub parent_commit_qc: Option<QuorumCertificate>,
        /// Audited snapshot anchor when no parent `CommitQC` exists.
        #[norito(required)]
        pub snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
        /// Frozen Nexus/AMX context commitment.
        pub nexus_amx_context_hash: Hash,
        /// Canonical V1 identity of the process-local execution policy.
        pub execution_policy_hash: Hash,
        /// Frozen data-availability layout.
        pub da_layout: DataAvailabilityLayout,
        /// Finalized leader-rotation seed.
        pub leader_seed: [u8; 32],
    }
    /// Canonical Sumeragi-v2 height-context projection and Commit certificate.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityCompactQcV2 {
        /// Bounded immutable consensus-context projection for the finalized height.
        pub height_context: KagemushaTopUpFinalityHeightContextV2,
        /// Exact Sumeragi-v2 Commit certificate persisted by Kura.
        pub certificate: QuorumCertificate,
    }
    /// Canonical balanced-Merkle inclusion path for one finalized top-up.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpAnchorMerkleProofV2 {
        /// Position in strict operation-id order.
        pub leaf_index: u32,
        /// Number of real leaves in the block-local tree.
        pub leaf_count: u32,
        /// Siblings from leaf level to root.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub siblings: Vec<[u8; 32]>,
    }
    /// Offline-verifiable proof that a finalized Commit QC authenticated one
    /// exact `(operation_id, anchor_digest)` write.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityProofV2 {
        /// Proof layout version.
        pub version: u16,
        /// Exact compact anchor identity bound by the recursive init proof.
        pub anchor: KagemushaRecursiveSpendTopUpAnchorRefV2,
        /// Commit QC with its roster `PoPs` supplied by the trusted artifact.
        pub commit_qc: KagemushaTopUpFinalityCompactQcV2,
        /// Bounded block-local inclusion proof.
        pub anchor_path: KagemushaTopUpAnchorMerkleProofV2,
    }
    /// Ordered validator set trusted for one non-overlapping height window.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityRosterWindowV2 {
        /// First accepted block height, inclusive.
        pub activates_at_height: u64,
        /// First rejected block height, exclusive.
        pub withdraws_at_height: u64,
        /// Consensus mode governing this immutable roster window.
        pub consensus_mode: ConsensusMode,
        /// Exact ordered BLS validator identities and voting powers.
        pub validator_set: Vec<ValidatorPower>,
        /// Fixed-size BLS proofs of possession aligned one-to-one with `validator_set`.
        pub validator_set_pops: Vec<[u8; 96]>,
    }
    /// Content-addressed trust artifact prefetched before any peer exchange.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityRosterArtifactV2 {
        /// Artifact layout version.
        pub version: u16,
        /// Exact genesis-derived network whose consensus votes are trusted.
        pub network_id: NetworkId,
        /// Human-readable roster generation selected by the manifest descriptor.
        pub artifact_generation: String,
        /// Strictly ordered, non-overlapping validator windows.
        pub windows: Vec<KagemushaTopUpFinalityRosterWindowV2>,
    }
    /// Canonical descriptor for one previous branch consumed by a V2 split.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendInputBranchV2 {
        /// Canonical digest of the complete previous recursive bundle.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub bundle_digest: [u8; 32],
        /// Exact note consumed by the confidential transfer.
        pub input_note: KagemushaSpendableNoteDescriptorV2,
        /// Canonical conflict claims of the consumed branch. A joined note
        /// carries one transition-bound claim per contributing ancestor.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Root at which the input transfer output was created.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub input_root: [u8; 32],
        /// Recursive proof-step count of the consumed bundle.
        pub proof_step_count: u32,
        /// Peer-hop count of the consumed bundle.
        pub peer_hop_count: u32,
    }
    /// Role of one independently spendable output from a V2 split.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(
        tag = "branch",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum KagemushaRecursiveSpendBranchV2 {
        /// Receiver-owned output branch.
        Recipient,
        /// Sender-owned change branch.
        Change,
    }
    /// Canonical unshield-v3 public words cross-checked by the V4 redemption transition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaUnshieldPublicInputsBindingV2 {
        /// First input note commitment.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub input_commitment_0: [u8; 32],
        /// Optional second input note commitment; zero for Kagemusha redemption.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub input_commitment_1: [u8; 32],
        /// First input nullifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub nullifier_0: [u8; 32],
        /// Optional second input nullifier; zero for Kagemusha redemption.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub nullifier_1: [u8; 32],
        /// Zero for full redemption or the partial-redemption change commitment.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub change_output_commitment: [u8; 32],
        /// Root at which the input note is proved live.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub root: [u8; 32],
        /// Confidential-circuit encoding of the exact credited atomic amount.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub public_amount: [u8; 32],
        /// Canonical asset tag.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub asset_tag: [u8; 32],
        /// Canonical network tag.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub network_tag: [u8; 32],
    }
    /// Curve role of one proof in the current two-proof Pasta recursion pair.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(
        tag = "parity",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum KagemushaPastaCycleParityV1 {
        /// EqAffine/Vesta recursive step over the Pallas scalar field.
        StepEq,
        /// EpAffine/Pallas recursive step over the Vesta scalar field.
        StepEp,
    }
    /// Canonical exact state vector carried across the Pasta field boundary.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendStateBoundaryV5 {
        /// State-boundary layout version.
        pub layout_version: u16,
        /// All 138 canonical `u32` limbs, including compact branch-history accumulators.
        pub state_limbs: Vec<u32>,
    }
    /// Exact dynamic offsets for one authenticated V4 public instance column.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaPastaPublicLayoutV4 {
        /// Authenticated IPA degree and accumulator round count.
        pub ipa_round_count: u32,
        /// Field-neutral limbs occupied by either carried IPA accumulator.
        pub accumulator_limbs: u32,
        /// First Eq carried-accumulator limb.
        pub parent_eq_accumulator_offset: u32,
        /// First Ep carried-accumulator limb.
        pub parent_ep_accumulator_offset: u32,
        /// First Eq deferred-audit word.
        pub parent_eq_deferred_offset: u32,
        /// First Ep deferred-audit word.
        pub parent_ep_deferred_offset: u32,
        /// Final public limb selecting bootstrap (`0`) or a live Step (`1`).
        pub live_selector_offset: u32,
        /// Exact length of the single public instance column.
        pub instance_column_limbs: u32,
    }
    /// Canonical Halo2 base-circuit configuration authenticated by a V4 profile.
    ///
    /// `Default` is intentionally an invalid sentinel. Key readers and runtime
    /// constructors must receive this value from an authenticated V4 manifest;
    /// no FFI or local configuration value may substitute for it.
    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaStepCircuitParamsV4 {
        /// Exact parameter-layout version.
        pub version: u16,
        /// Halo2 domain exponent and IPA round count.
        pub k: u32,
        /// Advice-column count in each challenge phase.
        pub num_advice_per_phase: Vec<u32>,
        /// Lookup-advice-column count in each challenge phase.
        pub num_lookup_advice_per_phase: Vec<u32>,
        /// Fixed-column count.
        pub num_fixed: u32,
        /// Range-table lookup width.
        pub lookup_bits: u32,
        /// Exact number of public instance columns.
        pub num_instance_columns: u32,
        /// Exact dynamic length of the single public instance column.
        pub public_input_limbs: u32,
        /// Row reservation used during deterministic layout calibration.
        pub minimum_unusable_rows: u32,
        /// Exact release cap for one ordinary parent proof transcript.
        pub max_parent_proof_bytes: u32,
    }
    /// Kind of content-addressed material bound to one V4 Pasta profile.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(
        tag = "kind",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum KagemushaPastaCycleArtifactKindV4 {
        /// Canonical `ParamsIPA` generator material.
        ParamsIpa,
        /// Halo2 processed proving key.
        ProvingKey,
        /// Halo2 processed verifying key.
        VerifyingKey,
        /// Genuine selector-zero proof and terminally verified folds for absent slots.
        BootstrapWitness,
    }
    /// One immutable file in a V4 recursive-spend artifact manifest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaPastaCycleArtifactV4 {
        /// Material kind within the parity profile.
        pub kind: KagemushaPastaCycleArtifactKindV4,
        /// Safe single-component V4 file name.
        pub file_name: String,
        /// Exact framed byte length.
        pub size_bytes: u64,
        /// SHA-256 of the exact framed file bytes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact byte length of the unframed cryptographic payload.
        pub payload_size_bytes: u64,
        /// SHA-256 of the unframed cryptographic payload.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub payload_sha256: [u8; 32],
    }
    /// Canonical public header preceding one streamed `KRV4KEY` payload.
    ///
    /// The header intentionally contains no release-sized byte vector. A
    /// streaming loader validates these small role bindings first, then hashes
    /// exactly `payload_size_bytes` trailing bytes before exposing the payload.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaPastaCycleFramedArtifactHeaderV4 {
        /// Exact `KRV4KEY` public-header layout version.
        pub version: u16,
        /// Manifest schema which authorizes this file.
        pub manifest_schema: String,
        /// Native bridge ABI required by this file.
        pub bridge_abi_version: u32,
        /// Exact paired-proof backend profile.
        pub proof_backend: String,
        /// Exact transcript profile.
        pub transcript_profile: String,
        /// Release generation selected by the manifest.
        pub generation: String,
        /// Curve/parity selected by this artifact.
        pub parity: KagemushaPastaCycleParityV1,
        /// Exact V4 circuit identifier for `parity`.
        pub circuit_id: String,
        /// `ParamsIPA` generation selected by the profile.
        pub parameter_generation: String,
        /// Authenticated IPA degree.
        pub ipa_k: u32,
        /// Domain-separated identity of the embedded canonical circuit parameters.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub circuit_params_sha256: [u8; 32],
        /// Value-free compiled protocol structure selected by this profile.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub compiled_protocol_structure_sha256: [u8; 32],
        /// Measured ordinary Step proof bytes selected by this profile.
        pub step_proof_size_bytes: u32,
        /// Role of the following payload.
        pub kind: KagemushaPastaCycleArtifactKindV4,
        /// Exact byte length of the following unframed payload.
        pub payload_size_bytes: u64,
        /// Raw SHA-256 of the following unframed payload.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub payload_sha256: [u8; 32],
    }
    /// V4 reference to the unchanged canonical top-up finality roster type.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaTopUpFinalityRosterArtifactReferenceV4 {
        /// Safe single-component V4 file name.
        pub file_name: String,
        /// Exact canonical Norito byte length.
        pub size_bytes: u64,
        /// SHA-256 of the exact canonical roster bytes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sha256: [u8; 32],
        /// Exact generation declared by the roster archive.
        pub artifact_generation: String,
        /// Native finality verifier/circuit role.
        pub circuit_id: String,
        /// Stable product purpose.
        pub purpose: String,
        /// Exact Norito type name contained by the file.
        pub artifact_type: String,
        /// Required V4 bridge ABI.
        pub required_bridge_abi_version: u32,
    }
    /// Authenticated fixed configuration and key material for one V4 parity.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaPastaCycleProofProfileV4 {
        /// Curve/parity implemented by this profile.
        pub parity: KagemushaPastaCycleParityV1,
        /// Exact V4 circuit identifier.
        pub circuit_id: String,
        /// Canonical `ParamsIPA` generation identifier.
        pub parameter_generation: String,
        /// Redundant, fail-closed IPA degree; must equal `circuit_params.k`.
        pub ipa_k: u32,
        /// Complete authenticated Halo2 base-circuit configuration.
        pub circuit_params: KagemushaStepCircuitParamsV4,
        /// Value-free structure identity shared by bootstrap and final protocol.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub compiled_protocol_structure_sha256: [u8; 32],
        /// Measured augmented proof bytes for this exact key and layout.
        pub step_proof_size_bytes: u32,
        /// Exactly one `ParamsIPA`, processed proving key, processed verifying
        /// key, and final-key selector-zero bootstrap-witness package, in that
        /// order. `circuit_params` is authenticated inline, never as a file.
        pub artifacts: Vec<KagemushaPastaCycleArtifactV4>,
    }
    /// One raw-byte-qualified untracked regular file in a reviewed source closure.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReviewedSourceClosureManifestEntryV1 {
        /// SHA-256 of the exact regular-file bytes.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub blob_sha256: [u8; 32],
        /// Canonical lowercase SHA-1 Git blob object id of the same bytes.
        pub git_blob_oid: String,
        /// Exact Git regular-file mode, `100644` or `100755`.
        pub git_mode: String,
        /// UTF-8 display form of the exact relative path bytes.
        pub path: String,
        /// Canonical Base64 of the exact relative POSIX path bytes.
        pub path_bytes_base64: String,
    }
    /// Canonical independently reviewed clean source closure for one candidate.
    ///
    /// Its JSON representation matches the reviewed descriptor: SHA-256 fields,
    /// including those in untracked-file entries, are lowercase hex strings.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReviewedSourceClosureV1 {
        /// Exact reviewed-source-closure schema.
        pub schema: String,
        /// Signed commit against which the necessarily empty tracked diff is defined.
        pub base_commit: String,
        /// Exact checked-out source commit; first release requires `base_commit`.
        pub source_commit: String,
        /// Derived dirty state; first release requires `false`.
        pub source_repo_dirty: bool,
        /// Producer full-tree SHA-256 of the tracked tree, including `Cargo.lock`.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub source_tree_sha256: [u8; 32],
        /// SHA-256 of the canonical full-index binary Git diff from `source_commit`.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub tracked_binary_diff_sha256: [u8; 32],
        /// Exact number of raw-byte-sorted untracked manifest entries.
        pub untracked_file_count: u64,
        /// Raw-byte-sorted path/mode/blob identities for all untracked source files.
        pub untracked_path_mode_blob_oid_manifest:
            Vec<KagemushaReviewedSourceClosureManifestEntryV1>,
        /// SHA-256 of each entry's canonical compact sorted-key JSON plus LF.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub untracked_path_mode_blob_oid_manifest_sha256: [u8; 32],
        /// Exact tracked root `Cargo.lock` byte length.
        pub tracked_cargo_lock_size_bytes: u64,
        /// SHA-256 of the exact tracked root `Cargo.lock` bytes.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub tracked_cargo_lock_sha256: [u8; 32],
        /// Fingerprint proving the tracked diff and untracked manifest are empty.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub combined_source_fingerprint_sha256: [u8; 32],
    }
    /// Exact descriptor of the signed root `Cargo.lock` tracked by a V2 source closure.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReviewedTrackedCargoLockV2 {
        /// Fixed signed-tree path, exactly `Cargo.lock`.
        pub path: String,
        /// Canonical lowercase SHA-1 Git blob object id of the exact lock bytes.
        pub git_blob_oid: String,
        /// Exact signed-tree regular-file mode, `100644`.
        pub git_mode: String,
        /// SHA-256 of the exact tracked lock bytes.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_bytes_hex")
        )]
        pub sha256: [u8; 32],
        /// Exact byte length of the tracked lock file.
        pub size_bytes: u64,
    }
    /// Production release manifest for degree-parameterized paired Pasta proofs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendArtifactManifestV4 {
        /// Exact V4 manifest schema identifier.
        pub schema: String,
        /// Manifest layout version.
        pub version: u16,
        /// Required native bridge ABI.
        pub bridge_abi_version: u32,
        /// Exact V4 paired-proof backend profile.
        pub proof_backend: String,
        /// Exact V4 transcript profile.
        pub transcript_profile: String,
        /// Human-readable release generation.
        pub generation: String,
        /// Lowercase 40-hex source revision.
        pub source_commit: String,
        /// SHA-256 of the exact tracked and untracked build source tree.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Whether the exact build tree differed from `source_commit`.
        pub source_repo_dirty: bool,
        /// Complete independently pinned reviewed clean source closure.
        pub reviewed_source_closure: KagemushaReviewedSourceClosureV1,
        /// SHA-256 of the exact canonical descriptor JSON bytes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// SHA-256 of the canonical authenticated source-seal projection embedded by the build.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub authenticated_source_seal_projection_sha256: [u8; 32],
        /// SHA-256 of the reviewed Cargo binary that built the sealed candidate executable.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_cargo_binary_sha256: [u8; 32],
        /// SHA-256 of the reviewed rustc binary that built the sealed candidate executable.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_rustc_binary_sha256: [u8; 32],
        /// SHA-256 of the exact sealed generator executable admitted by the launcher.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub generator_binary_sha256: [u8; 32],
        /// SHA-256 of the canonical sealed double-build report authenticating that executable.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sealed_candidate_build_report_sha256: [u8; 32],
        /// Exact network for which the release was built.
        pub network_id: NetworkId,
        /// Asset definition for which the release was built.
        pub asset: AssetDefinitionId,
        /// Authoritative fixed asset scale.
        pub asset_scale: u32,
        /// First block at which this release may issue notes.
        pub activation_height: u64,
        /// First block at which new issuance must stop.
        pub withdrawal_height: u64,
        /// Exact measured upper bound for one canonical V4 proof-pair payload.
        pub max_proof_bytes: u32,
        /// Effective in-process physical-memory ceiling used for generation and publication.
        pub generation_memory_limit_bytes: u64,
        /// Exact mandatory in-process memory enforcement profile.
        pub generation_memory_enforcement_profile: String,
        /// SHA-256 of the canonical actual-recursion qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualification_receipt_sha256: [u8; 32],
        /// Domain-separated identity of the immutable candidate and qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualified_candidate_sha256: [u8; 32],
        /// SHA-256 of the exact canonical runner-signed internal-validation receipt.
        ///
        /// Immutable candidates carry zero here; finalized releases require a
        /// nonzero digest whose exact bytes are retained by the release record.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub internal_validation_receipt_sha256: [u8; 32],
        /// Eq then Ep V4 recursive-step profiles.
        pub profiles: Vec<KagemushaPastaCycleProofProfileV4>,
        /// Release-bound validator roster reference.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4,
        /// Digest of signed physical-device benchmark evidence.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of independent cryptographic review evidence.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
        /// Digest of the V4 signed release attestation.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
    }
    /// Immutable ABI-21 candidate captured before external review and device evidence exist.
    ///
    /// The embedded manifest commits the independently reviewed clean source
    /// closure, authenticated source-seal projection, reviewed Cargo/rustc
    /// binaries, network parameters, inline circuit configuration, exact eight
    /// recursive artifacts, and finality roster. Its benchmark, review,
    /// qualification, internal-validation, and external-evidence digest slots
    /// must all be zero.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCandidateV4 {
        /// Exact candidate-record schema identifier.
        pub schema: String,
        /// Candidate-record layout version.
        pub version: u16,
        /// Complete pre-evidence manifest with its three promotion digest slots zeroed.
        pub manifest: KagemushaRecursiveSpendArtifactManifestV4,
    }
    /// Canonical proof-bearing receipt proving one exact candidate reached step two.
    ///
    /// Counters, parent cardinality, semantic statements, and terminal decisions
    /// are deliberately absent. Consumers must derive them from these exact proof
    /// pairs while authenticating every candidate artifact role.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendQualificationReceiptV4 {
        /// Exact receipt schema identifier.
        pub(super) schema: String,
        /// Receipt layout version.
        pub(super) version: u16,
        /// SHA-256 of the exact canonical unsigned candidate record.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) candidate_sha256: [u8; 32],
        /// SHA-256 of the candidate's exact canonical unsigned manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) manifest_sha256: [u8; 32],
        /// Authenticated source-seal projection digest copied from the candidate manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) authenticated_source_seal_projection_sha256: [u8; 32],
        /// Reviewed Cargo binary digest copied from the candidate manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) reviewed_cargo_binary_sha256: [u8; 32],
        /// Reviewed rustc binary digest copied from the candidate manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) reviewed_rustc_binary_sha256: [u8; 32],
        /// Generator executable digest copied from the candidate manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) generator_binary_sha256: [u8; 32],
        /// Sealed double-build report digest copied from the candidate manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub(super) sealed_candidate_build_report_sha256: [u8; 32],
        /// Exact in-process physical-memory ceiling committed by the candidate.
        pub(super) generation_memory_limit_bytes: u64,
        /// Exact mandatory in-process memory enforcement profile.
        pub(super) generation_memory_enforcement_profile: String,
        /// Framed then payload SHA-256 for all eight canonical artifact roles.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
        pub(super) artifact_role_digests: [[u8; 32]; 16],
        /// Exact canonical Eq/Ep initialization proof pair bytes.
        pub(super) initialization_pair: Vec<u8>,
        /// Exact canonical Eq/Ep one-parent child proof pair bytes.
        pub(super) append_pair: Vec<u8>,
    }
    /// Immutable release identity reviewed before evidence finalization.
    ///
    /// `candidate_sha256` commits the complete artifact/profile/roster/window
    /// inventory. The repeated human-auditable fields prevent a correctly signed
    /// review from being presented with an ambiguous release description.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewSubjectV4 {
        /// SHA-256 of the canonical immutable pre-evidence candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub candidate_sha256: [u8; 32],
        /// SHA-256 of the canonical actual-recursion qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualification_receipt_sha256: [u8; 32],
        /// Domain-separated identity of the candidate and qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualified_candidate_sha256: [u8; 32],
        /// Exact release generation copied from the candidate.
        pub generation: String,
        /// Exact source revision copied from the candidate.
        pub source_commit: String,
        /// Exact source-tree identity copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Exact reviewed clean-tree state copied from the candidate (`false`).
        pub source_repo_dirty: bool,
        /// Exact independently pinned closure descriptor digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// Exact authenticated source-seal projection digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub authenticated_source_seal_projection_sha256: [u8; 32],
        /// Exact reviewed Cargo binary digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_cargo_binary_sha256: [u8; 32],
        /// Exact reviewed rustc binary digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_rustc_binary_sha256: [u8; 32],
        /// Exact generator executable digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub generator_binary_sha256: [u8; 32],
        /// Exact sealed double-build report digest copied from the candidate.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sealed_candidate_build_report_sha256: [u8; 32],
        /// Exact network for which the reviewed candidate was built.
        pub network_id: NetworkId,
        /// Asset definition for which the reviewed candidate was built.
        pub asset: AssetDefinitionId,
        /// Native bridge ABI required by the reviewed candidate.
        pub bridge_abi_version: u32,
    }
    /// Production disposition recorded by an independent cryptographic review.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(tag = "decision", content = "value", rename_all = "snake_case")]
    #[norito(deny_unknown_fields)]
    pub enum KagemushaRecursiveSpendCryptographicReviewDecisionV4 {
        /// The exact candidate is approved for release finalization.
        Approved,
        /// The exact candidate is rejected and must not be finalized.
        Rejected,
    }
    /// Closed, canonically ordered set of security properties reviewed for V4.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(tag = "check", content = "value", rename_all = "snake_case")]
    #[norito(deny_unknown_fields)]
    pub enum KagemushaRecursiveSpendCryptographicReviewCheckV4 {
        /// Recursive-circuit constraints cover every claimed state transition.
        RecursiveCircuitConstraintCoverage,
        /// Pasta-cycle recursion and transcripts are domain- and lineage-bound.
        RecursiveCycleAndTranscriptBinding,
        /// Public inputs bind the complete state transition and operation.
        PublicInputAndStateTransitionBinding,
        /// Parameters, artifacts, and verifying keys bind the reviewed candidate.
        ArtifactParameterAndVerifyingKeyBinding,
        /// Nullifiers, replay protection, and finality inputs are correctly constrained.
        NullifierReplayAndFinalityBinding,
        /// Parsers are canonical and all attacker-controlled resources are bounded.
        ParserCanonicalizationAndResourceBounds,
    }
    /// Result of one mandatory cryptographic-review check.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(tag = "status", content = "value", rename_all = "snake_case")]
    #[norito(deny_unknown_fields)]
    pub enum KagemushaRecursiveSpendCryptographicReviewCheckStatusV4 {
        /// The referenced evidence supports the reviewed property.
        Passed,
        /// The referenced evidence does not support the reviewed property.
        Failed,
    }
    /// One content-addressed mandatory check inside a V4 review.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewCheckResultV4 {
        /// Mandatory reviewed property.
        pub check: KagemushaRecursiveSpendCryptographicReviewCheckV4,
        /// Review result; production finalization requires `Passed`.
        pub status: KagemushaRecursiveSpendCryptographicReviewCheckStatusV4,
        /// SHA-256 of property-specific evidence retained by the reviewer.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub evidence_sha256: [u8; 32],
    }
    /// Exact domain-separated payload signed by every V4 reviewer.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewPayloadV4 {
        /// Cross-protocol replay separator.
        pub domain: String,
        /// Exact immutable candidate under review.
        pub subject: KagemushaRecursiveSpendCryptographicReviewSubjectV4,
        /// Review disposition; production requires `Approved`.
        pub decision: KagemushaRecursiveSpendCryptographicReviewDecisionV4,
        /// SHA-256 of the complete retained review report.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub report_sha256: [u8; 32],
        /// Exact Eq-then-Ep cryptographic artifact roles reviewed for ABI-21.
        pub artifact_roles: Vec<String>,
        /// Exact ordered set of mandatory, independently evidenced checks.
        pub checks: Vec<KagemushaRecursiveSpendCryptographicReviewCheckResultV4>,
    }
    /// One policy-authorized signature over a complete V4 review payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewApprovalV4 {
        /// Reviewer identity selected by the trusted release policy.
        pub public_key: PublicKey,
        /// Signature over the exact domain, candidate, report, roles, and checks.
        pub signature: SignatureOf<KagemushaRecursiveSpendCryptographicReviewPayloadV4>,
    }
    /// Canonical signed independent cryptographic-review evidence for ABI-21/V4.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
        /// Exact V4 cryptographic-review schema.
        pub schema: String,
        /// Cryptographic-review envelope version.
        pub version: u16,
        /// Candidate-bound review decision signed by every approval.
        pub payload: KagemushaRecursiveSpendCryptographicReviewPayloadV4,
        /// Strictly ascending, unique reviewer approvals.
        pub approvals: Vec<KagemushaRecursiveSpendCryptographicReviewApprovalV4>,
    }
    /// Independent authority role required to promote an authenticated release.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(tag = "role", content = "value", rename_all = "snake_case")]
    #[norito(deny_unknown_fields)]
    pub enum KagemushaRecursiveSpendReleaseApprovalRoleV1 {
        /// Operational release authority approving publication.
        Release,
        /// Independent cryptographic reviewer approving the referenced report.
        CryptographicReview,
        /// Device-lab authority approving the referenced physical-device measurements.
        PhysicalDeviceBenchmark,
    }
    /// Immutable subject shared by every role-specific V4 release approval.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
        /// SHA-256 of the canonical V4 manifest with its attestation slot zeroed.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub manifest_subject_sha256: [u8; 32],
        /// SHA-256 of the canonical actual-recursion qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualification_receipt_sha256: [u8; 32],
        /// Domain-separated identity of the candidate and qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualified_candidate_sha256: [u8; 32],
        /// Exact runner-signed internal-validation receipt selected by the release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub internal_validation_receipt_sha256: [u8; 32],
        /// Exact release generation copied from the V4 manifest.
        pub generation: String,
        /// Exact source revision copied from the V4 manifest.
        pub source_commit: String,
        /// Exact source-tree identity copied from the V4 manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub source_tree_sha256: [u8; 32],
        /// Exact clean-tree state copied from the V4 manifest (`false`).
        pub source_repo_dirty: bool,
        /// Exact independently pinned closure descriptor digest copied from the manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_source_closure_descriptor_sha256: [u8; 32],
        /// Exact authenticated source-seal projection digest copied from the manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub authenticated_source_seal_projection_sha256: [u8; 32],
        /// Exact reviewed Cargo binary digest copied from the manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_cargo_binary_sha256: [u8; 32],
        /// Exact reviewed rustc binary digest copied from the manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_rustc_binary_sha256: [u8; 32],
        /// Exact generator executable digest copied from the V4 manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub generator_binary_sha256: [u8; 32],
        /// Exact sealed double-build report digest copied from the V4 manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sealed_candidate_build_report_sha256: [u8; 32],
        /// Digest of the signed physical-device evidence file.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub benchmark_evidence_sha256: [u8; 32],
        /// Digest of the independent cryptographic review file.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub cryptographic_review_sha256: [u8; 32],
    }
    /// Domain-separated value signed for one independent V4 approval role.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
        /// Cross-protocol replay separator.
        pub domain: String,
        /// Authority role for which this signature is valid.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Complete V4 release subject approved by the signer.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV4,
    }
    /// One role-bound signature inside a V4 release attestation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseApprovalV4 {
        /// Independent authority role represented by this signature.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Exact signer key selected by the trusted release policy.
        pub public_key: PublicKey,
        /// Signature over the V4 domain, role, and complete subject.
        pub signature: SignatureOf<KagemushaRecursiveSpendReleaseApprovalPayloadV4>,
    }
    /// Authenticated release envelope whose digest occupies the V4 manifest slot.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseAttestationV4 {
        /// Exact V4 attestation schema.
        pub schema: String,
        /// Attestation layout version.
        pub version: u16,
        /// Immutable V4 subject approved by all roles.
        pub subject: KagemushaRecursiveSpendReleaseAttestationSubjectV4,
        /// Strictly ordered, unique role/signer approvals.
        pub approvals: Vec<KagemushaRecursiveSpendReleaseApprovalV4>,
    }
    /// Trusted signer threshold for one independent release-approval role.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseRolePolicyV1 {
        /// Role governed by this threshold.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Number of distinct authorized signatures required for the role.
        pub threshold: u16,
        /// Strictly ordered authorized signer keys.
        pub authorized_signers: Vec<PublicKey>,
    }
    /// Locally trusted policy for authenticating a release envelope.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleasePolicyV1 {
        /// Exact policy schema.
        pub schema: String,
        /// Policy layout version.
        pub version: u16,
        /// Portable identifier selected by deployment policy.
        pub policy_id: String,
        /// Domain-separated identity of the only internal-validation runner authorized by policy.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub internal_validation_runner_identity_sha256: [u8; 32],
        /// Exactly release, cryptographic-review, and device-benchmark policies.
        pub roles: Vec<KagemushaRecursiveSpendReleaseRolePolicyV1>,
    }
    /// Verified signer identity retained in a machine-readable promotion record.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendApprovedSignerV1 {
        /// Approval role satisfied by this signer.
        pub role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
        /// Verified signer key.
        pub public_key: PublicKey,
    }
    /// Deterministic ABI-21 deployment marker written only after V4 release verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendPromotedReleaseV4 {
        /// Exact V4 promotion-record schema.
        pub schema: String,
        /// Promotion-record layout version.
        pub version: u16,
        /// Authenticated V4 release generation.
        pub generation: String,
        /// Authenticated source-seal projection selected by the promoted release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub authenticated_source_seal_projection_sha256: [u8; 32],
        /// Reviewed Cargo binary selected by the promoted release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_cargo_binary_sha256: [u8; 32],
        /// Reviewed rustc binary selected by the promoted release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub reviewed_rustc_binary_sha256: [u8; 32],
        /// Generator executable selected by the promoted release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub generator_binary_sha256: [u8; 32],
        /// Canonical sealed double-build report selected by the promoted release.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub sealed_candidate_build_report_sha256: [u8; 32],
        /// SHA-256 of the immutable pre-evidence candidate record.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub candidate_sha256: [u8; 32],
        /// SHA-256 of the canonical actual-recursion qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualification_receipt_sha256: [u8; 32],
        /// Domain-separated identity of the candidate and qualification receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub qualified_candidate_sha256: [u8; 32],
        /// SHA-256 of the exact canonical runner-signed internal-validation receipt.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub internal_validation_receipt_sha256: [u8; 32],
        /// SHA-256 of the complete canonical V4 manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// SHA-256 of the canonical signed V4 release attestation.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub release_attestation_sha256: [u8; 32],
        /// SHA-256 of the locally trusted release policy.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub release_policy_sha256: [u8; 32],
        /// Canonically ordered role/signer identities whose signatures were verified.
        pub approved_signers: Vec<KagemushaRecursiveSpendApprovedSignerV1>,
        /// Whether every content-addressed V4 artifact was verified before publication.
        pub artifact_inventory_verified: bool,
        /// Native bridge ABI required to consume this promoted release.
        pub bridge_abi_version: u32,
        /// Exact Eq-then-Ep eight-role artifact inventory selected by ABI-21.
        pub artifact_roles: Vec<String>,
        /// Authenticated release-specific proof-pair byte ceiling.
        pub max_proof_bytes: u32,
    }
    /// Complete signed ABI-21 release material persisted by consensus activation.
    ///
    /// The receipt and two external-evidence fields contain canonical signed
    /// artifacts, never raw device logs, parameters, proving keys, or bootstrap
    /// witness payloads.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseRecordV4 {
        /// Canonical evidence-bearing release manifest.
        pub manifest: KagemushaRecursiveSpendArtifactManifestV4,
        /// Role-threshold signatures over the finalized release subject.
        pub release_attestation: KagemushaRecursiveSpendReleaseAttestationV4,
        /// Exact canonical runner-signed internal-validation receipt bytes.
        pub internal_validation_receipt: Vec<u8>,
        /// Canonical signed physical-device benchmark summary bytes.
        pub physical_device_benchmark_summary: Vec<u8>,
        /// Canonical signed independent cryptographic-review summary bytes.
        pub cryptographic_review_summary: Vec<u8>,
        /// Promotion marker binding the candidate, policy, release, and inventory.
        pub promotion_record: KagemushaRecursiveSpendPromotedReleaseV4,
    }
    /// Atomic consensus payload for one ABI-21 release and its two terminal verifiers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendReleaseActivationV4 {
        /// Fully authenticated signed release record.
        pub release_record: KagemushaRecursiveSpendReleaseRecordV4,
        /// SHA-256 of the operator-configured canonical release policy.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub configured_policy_sha256: [u8; 32],
        /// Registry id for the EqAffine/Vesta verifying key.
        pub step_eq_verifier_key_id: VerifyingKeyId,
        /// Inline EqAffine/Vesta verifying-key record.
        pub step_eq_verifier_record: VerifyingKeyRecord,
        /// Registry id for the EpAffine/Pallas verifying key.
        pub step_ep_verifier_key_id: VerifyingKeyId,
        /// Inline EpAffine/Pallas verifying-key record.
        pub step_ep_verifier_record: VerifyingKeyRecord,
    }
    /// Installed authenticated V4 release selected by a degree-parameterized operation.
    ///
    /// The explicit wire version prevents an unversioned historical binding
    /// from being interpreted as an ABI-21 release identity.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendArtifactBindingV4 {
        /// Exact ABI-21 binding version. Only `4` is accepted.
        pub version: u16,
        /// Human-readable authenticated V4 release generation.
        pub generation: String,
        /// SHA-256 of the exact signed V4 manifest bytes.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
    }
    /// Canonical fields signed by a receiver after durable payment persistence.
    ///
    /// The receiver must persist the final acknowledgement bytes under
    /// `(operation_id, recipient_request_digest)` in the same atomic operation
    /// that persists the accepted bundle. Duplicate delivery returns those
    /// exact bytes instead of signing a new timestamp.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReceiverAcknowledgementPayloadV2 {
        /// Sender operation whose reserved inputs may be committed after ACK verification.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Canonical digest of the receiver-created payment request.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Canonical digest of the accepted recipient bundle.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub payment_bundle_digest: [u8; 32],
        /// Recipient output commitment persisted by the receiver.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_commitment: [u8; 32],
        /// Receiver wall-clock time captured once at the durable commit boundary.
        pub accepted_at_ms: u64,
        /// Registered receiver device identifier used for device-lineage lookup.
        pub receiver_device_id: String,
        /// Domain-separated reference to `receiver_public_key`.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub receiver_key_reference: [u8; 32],
        /// Device-bound acknowledgement verification key.
        pub receiver_public_key: KagemushaDevicePublicKeyV2,
    }
    /// Signed durable receiver acknowledgement for one offline payment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReceiverAcknowledgementV2 {
        /// Canonical signed bindings.
        pub payload: KagemushaReceiverAcknowledgementPayloadV2,
        /// Device-key signature over the domain-separated canonical payload.
        pub signature: KagemushaDeviceSignatureV2,
    }
    /// Typed result returned after native acknowledgement verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaReceiverAcknowledgementVerifyResultV2 {
        /// All request, bundle, key-reference, and signature bindings passed.
        pub valid: bool,
        /// Stable sender operation id copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Canonical receiver request digest copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Canonical accepted-bundle digest copied from the verified acknowledgement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub payment_bundle_digest: [u8; 32],
        /// Canonical identity digest of the complete acknowledgement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub acknowledgement_digest: [u8; 32],
    }
    /// Native capability record for the explicitly versioned ABI-21 backend.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendNativeCapabilitiesV4 {
        /// Native bridge ABI reported by the loaded library.
        pub bridge_abi_version: u32,
        /// Required V4 artifact manifest schema.
        pub artifact_manifest_schema: String,
        /// Required V4 proof backend.
        pub proof_backend: String,
        /// Required V4 transcript profile.
        pub transcript_profile: String,
        /// Proof-envelope format version.
        pub proof_envelope_version: u16,
        /// Eq recursive-step circuit id.
        pub step_eq_circuit_id: String,
        /// Ep recursive-step circuit id.
        pub step_ep_circuit_id: String,
        /// Exact ordered eight-role cryptographic inventory.
        pub artifact_roles: Vec<String>,
        /// Maximum proof-pair payload accepted by the installed V4 release.
        pub max_proof_bytes: u32,
        /// Whether all proof, audit, release, and performance gates passed.
        pub proof_backend_available: bool,
        /// Stable remaining backend gates.
        pub missing_gates: Vec<String>,
    }
    /// Degree-parameterized Pasta-cycle envelope carried by a V4 proof wrapper.
    ///
    /// The backend-native Eq/Ep pair remains canonical opaque bytes inside
    /// `proof`; wallets and bridge carriers do not reinterpret its internal
    /// accumulators or fold transcripts.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaPastaCycleProofEnvelopeV4 {
        /// Exact V4 proof-envelope version.
        pub version: u16,
        /// Exact V4 paired-proof backend.
        pub proof_backend: String,
        /// Exact V4 transcript profile.
        pub transcript_profile: String,
        /// Exact Eq recursive-step circuit id.
        pub step_eq_circuit_id: String,
        /// Exact Ep recursive-step circuit id.
        pub step_ep_circuit_id: String,
        /// Authenticated artifact generation.
        pub artifact_generation: String,
        /// SHA-256 of the exact authenticated V4 manifest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub manifest_sha256: [u8; 32],
        /// Eq parameter generation identifier.
        pub step_eq_parameter_generation: String,
        /// Ep parameter generation identifier.
        pub step_ep_parameter_generation: String,
        /// Domain-separated identity of the Eq circuit configuration.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_circuit_params_sha256: [u8; 32],
        /// Domain-separated identity of the Ep circuit configuration.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_circuit_params_sha256: [u8; 32],
        /// SHA-256 of the exact Eq processed verifier-key payload.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub step_eq_verifier_key_sha256: [u8; 32],
        /// SHA-256 of the exact Ep processed verifier-key payload.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub step_ep_verifier_key_sha256: [u8; 32],
        /// Canonical cross-field state boundary exposed by the proof.
        pub state_boundary: KagemushaRecursiveSpendStateBoundaryV5,
        /// Canonical adapter-owned V4 Eq/Ep proof-pair bytes.
        pub proof: ProofBox,
    }
    /// Exact fixed-size ABI-21 public operation row bound by the terminal proof.
    ///
    /// Each consecutive group of eight limbs is one canonical Pallas-field
    /// element in little-endian `u32` order. Core rejects non-canonical field
    /// encodings before proof verification.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendOperationVectorV4 {
        /// All 1,080 exact public limbs; no compact or legacy encoding is accepted.
        #[cfg_attr(
            feature = "json",
            norito(json = "crate::json_helpers::fixed_u32_limbs")
        )]
        pub limbs: [u32; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4],
    }
    /// Peer-to-peer split transition carried by an ABI-21 recursive output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendPeerSplitTransitionV4 {
        /// Circuit-exposed digest of the exact local split intent.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub binding_digest: [u8; 32],
        /// Independently spendable output selected by this statement.
        pub branch: KagemushaRecursiveSpendBranchV2,
        /// Receiver request digest bound by the split.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable split operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Maximum proof-step count among the consumed parent bundles.
        pub parent_max_proof_step_count: u32,
        /// Maximum peer-hop count among the consumed parent bundles.
        pub parent_max_peer_hop_count: u32,
    }
    /// Partial-redemption change transition carried by an ABI-21 child statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedemptionChangeTransitionV4 {
        /// Circuit-exposed digest of the exact redemption/change intent.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub binding_digest: [u8; 32],
        /// Parent bundle identity consumed by the unshield transition.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub parent_bundle_digest: [u8; 32],
        /// Stable redemption operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Parent proof-step count.
        pub parent_proof_step_count: u32,
        /// Parent peer-hop count.
        pub parent_peer_hop_count: u32,
    }
    /// Mutually exclusive semantic transition that produced an ABI-21 state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(tag = "transition", content = "value", rename_all = "snake_case")]
    #[norito(deny_unknown_fields)]
    pub enum KagemushaRecursiveSpendTransitionV4 {
        /// Ordinary offline peer split.
        PeerSplit(KagemushaRecursiveSpendPeerSplitTransitionV4),
        /// Proof-bound partial-redemption change child.
        RedemptionChange(KagemushaRecursiveSpendRedemptionChangeTransitionV4),
    }
    /// Canonical public statement bound by an ABI-21 recursive proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendPublicStatementV4 {
        /// Exact network that scopes this cash state.
        pub network_id: NetworkId,
        /// Asset committed by every note in the transition.
        pub asset: AssetDefinitionId,
        /// Authoritative asset scale.
        pub asset_scale: u32,
        /// Root after the current transition.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// First empty commitment-tree leaf after this transition.
        pub next_zero_leaf_index: u32,
        /// One or two canonical finalized top-up references funding this state.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Total recursive proof transitions.
        pub proof_step_count: u32,
        /// Number of peer-to-peer spends after top-up.
        pub peer_hop_count: u32,
        /// Current independently spendable note.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Transition-bound conflict claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Binding-only semantic transition under the sole ABI-21 wire layout.
        #[norito(required)]
        pub transition: Option<KagemushaRecursiveSpendTransitionV4>,
        /// Authenticated V4 proving-artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
    }
    /// V4 recursive proof whose public instance includes the statement digest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendProofV4 {
        /// Verifier selected by the statement.
        pub verifier_key_id: VerifyingKeyId,
        /// Circuit-exposed digest of the complete V4 public statement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
        /// Explicitly versioned envelope containing the opaque native pair.
        pub proof_envelope: KagemushaPastaCycleProofEnvelopeV4,
    }
    /// Independently spendable ABI-21 recursive state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendBundleV4 {
        /// Exact public statement bound by the proof.
        pub statement: KagemushaRecursiveSpendPublicStatementV4,
        /// Canonical public operation row independently carried by this bundle.
        pub operation: KagemushaRecursiveSpendOperationVectorV4,
        /// Degree-parameterized recursive proof.
        pub recursive_proof: KagemushaRecursiveSpendProofV4,
    }
    /// Finalized top-up anchor selecting a V4 recursive release.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpAnchorV4 {
        /// Anchor schema version.
        pub version: u16,
        /// Exact network that finalized the top-up.
        pub network_id: NetworkId,
        /// Payer whose online balance funded the anchor.
        pub payer: AccountId,
        /// Exact payer asset, including its balance scope.
        pub asset: AssetId,
        /// Authoritative fixed scale.
        pub asset_scale: u32,
        /// Exact positive amount reserved into escrow.
        pub amount: KagemushaScaledAmountV2,
        /// Confidential root before the finalized transfer.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Confidential root finalized by the transfer.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub finalized_root: [u8; 32],
        /// Confidential tree position consumed by the top-up note.
        pub shield_leaf_index: u32,
        /// Exact first spendable note.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Stable top-up operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub topup_operation_id: [u8; 32],
        /// Active shield verifier selected at finalization.
        pub shield_verifier_id: VerifyingKeyId,
        /// Registered shield verifier commitment.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub shield_verifier_commitment: [u8; 32],
        /// Authenticated V4 recursive artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Finalization block height.
        pub finalized_height: u64,
        /// Canonical transaction hash that created the anchor.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub finalized_tx_hash: [u8; 32],
        /// Canonical digest of every preceding receipt field.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub anchor_digest: [u8; 32],
    }
    /// Canonical unsigned ABI-21 online-to-offline fields covered by payer authorization.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpUnsignedV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated ABI-21 release selected for recursive initialization.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Globally unique replay-stable operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// Authoritative ABI-21 chain-facing online-to-offline request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(schema_name = "iroha.torii.v1.offline.top_up.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpRequestV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Online asset balance charged for the top-up.
        pub asset: AssetId,
        /// Exact positive amount charged at the live asset-definition scale.
        pub amount: KagemushaScaledAmountV2,
        /// First spendable note produced by the shield transition.
        pub current_note: KagemushaSpendableNoteDescriptorV2,
        /// Proof that inserts this note without consuming a confidential input.
        pub shield_evidence: KagemushaTopUpShieldEvidenceV2,
        /// Authenticated ABI-21 release selected for recursive initialization.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Globally unique replay-stable operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained payer/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }
    /// Public V4 split transition with an ABI-21 output binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendSplitIntentV4 {
        /// Exact network inherited from the parent state and receiver request.
        pub network_id: NetworkId,
        /// Asset inherited from the parent state and receiver request.
        pub asset: AssetDefinitionId,
        /// One or two canonical previous branches.
        pub inputs: Vec<KagemushaRecursiveSpendInputBranchV2>,
        /// Canonical finalized top-up references contributing value.
        pub topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Authoritative asset-definition scale.
        pub asset_scale: u32,
        /// Authenticated V4 release selected for the output proof.
        pub output_artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Exact amount assigned to the recipient output.
        pub transfer_amount: KagemushaScaledAmountV2,
        /// Recipient-owned output note.
        pub recipient_output: KagemushaSpendableNoteDescriptorV2,
        /// Sender-owned remainder, if any.
        #[norito(required)]
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Digest of the receiver's nonce-bound payment request.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Stable idempotency/replay identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// Public V4 redemption transition with an optional ABI-21 change binding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedemptionIntentV4 {
        /// Exact network inherited from the input bundle.
        pub network_id: NetworkId,
        /// Asset inherited from the input bundle.
        pub asset: AssetDefinitionId,
        /// Exact note consumed by unshield-v3.
        pub input_note: KagemushaSpendableNoteDescriptorV2,
        /// Canonical live conflict claims consumed by this redemption.
        pub parent_branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Canonical finalized top-up references carried by the parent.
        pub parent_topup_anchor_refs: Vec<KagemushaRecursiveSpendTopUpAnchorRefV2>,
        /// Recursive proof-step count of the parent bundle.
        pub parent_proof_step_count: u32,
        /// Peer-hop count of the parent bundle.
        pub parent_peer_hop_count: u32,
        /// Canonical digest of the complete input bundle.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub parent_bundle_digest: [u8; 32],
        /// Input confidential root exposed by unshield-v3.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub input_root: [u8; 32],
        /// Online account receiving the public credit.
        pub recipient: AccountId,
        /// Exact credited amount at the authoritative scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Proof-bound change descriptor.
        #[norito(required)]
        pub change_output: Option<KagemushaSpendableNoteDescriptorV2>,
        /// Authenticated V4 output release, present exactly with change.
        #[norito(required)]
        pub change_artifact_binding: Option<KagemushaRecursiveSpendArtifactBindingV4>,
        /// Canonical unshield-v3 public words.
        pub unshield_public_inputs: KagemushaUnshieldPublicInputsBindingV2,
        /// Digest of the unshield words exposed by the V4 transition circuit.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub unshield_public_inputs_digest: [u8; 32],
        /// Stable authorization/idempotency operation id.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// ABI-21 local initialization request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendInitRequestV4 {
        /// Finalized chain receipt consumed by the initial proof.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        /// Offline-verifiable finality proof for the compact anchor reference.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
        /// Exact content-addressed validator roster.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
    }
    /// ABI-21 initialization result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendInitResultV4 {
        /// Independently spendable state created from the finalized top-up.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state and next-zero frontier for the initialized note.
        pub membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete offline-verifiable origin provenance for the initialized branch.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Circuit-exposed digest of the complete public statement.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub public_statement_digest: [u8; 32],
    }
    /// One V4 previous-proof package consumed by append.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendAppendInputV4 {
        /// Previous spendable ABI-21 recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV4,
        /// Complete authenticated top-up provenance required to verify this parent offline.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }
    /// ABI-21 recursive append request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendAppendRequestV4 {
        /// One or two previous-proof packages in canonical order.
        pub previous_inputs: Vec<KagemushaRecursiveSpendAppendInputV4>,
        /// Confidential-transfer proof containing both output commitments.
        pub confidential_transfer_proof: ProofAttachment,
        /// Scale, outputs, replay id, and V4 output release.
        pub split: KagemushaRecursiveSpendSplitIntentV4,
        /// Signed proof-evaluation snapshot; verifiers must also be live at execution.
        pub block_height: u64,
    }
    /// Result of one ABI-21 recursive split append.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendSplitResultV4 {
        /// Exact value-conserving transition shared by both branches.
        pub split: KagemushaRecursiveSpendSplitIntentV4,
        /// Circuit-exposed binding to the split and parent accumulator.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub split_binding_digest: [u8; 32],
        /// Receiver-owned independently spendable output.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state for the recipient output.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete offline-verifiable provenance for the recipient branch.
        pub recipient_topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Sender-owned remainder, present only for a partial transfer.
        #[norito(required)]
        pub change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Proof-bound membership state for sender change.
        #[norito(required)]
        pub change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete provenance for sender change, present exactly with change.
        #[norito(required)]
        pub change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
    }
    /// Recipient-only ABI-21 peer payload emitted from a local split result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendPeerPaymentV4 {
        /// Receiver-owned independently spendable ABI-21 branch.
        pub recipient_bundle: KagemushaRecursiveSpendBundleV4,
        /// Proof-bound membership state required for the recipient's next spend.
        pub recipient_membership_witness: KagemushaNoteMembershipWitnessV2,
        /// Complete authenticated provenance needed for offline receiver verification.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }
    /// Complete finalized V4 origin carried to an offline receiver.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        /// Complete finalized ABI-21 top-up receipt.
        pub topup_anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        /// Consensus proof for the compact anchor reference.
        pub topup_finality_proof: KagemushaTopUpFinalityProofV2,
    }
    /// Complete authenticated top-up provenance carried by every spendable ABI-21 branch.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendTopUpProvenanceV4 {
        /// Exact manifest-bound validator roster shared by every origin proof.
        pub topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactV2,
        /// Complete evidence in the exact order of the branch statement's anchor references.
        pub topup_finality_evidence: Vec<KagemushaRecursiveSpendTopUpFinalityEvidenceV4>,
    }
    /// ABI-21 receiver-verification request.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendVerifyRequestV4 {
        /// Scale- and split-bound ABI-21 recursive bundle.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Receiver request that the final branch must match.
        pub recipient_request: KagemushaRecipientPaymentRequestV2,
        /// Complete branch provenance received with the peer payment.
        pub topup_provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        /// Maximum hop count accepted by the receiver.
        pub maximum_hops: u32,
        /// Expected authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Signed proof-evaluation snapshot; verifiers must also be live at receipt time.
        pub block_height: u64,
        /// Authoritative current Unix time in milliseconds.
        pub verified_at_ms: u64,
    }
    /// Opaque-safe summary decoded from an ABI-21 bundle.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendBundleSummaryV4 {
        /// Asset definition bound by the proof.
        pub asset: AssetDefinitionId,
        /// Exact current spendable amount.
        pub amount: KagemushaScaledAmountV2,
        /// Current note commitment.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Current note nullifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Current peer-hop count.
        pub hop_count: u32,
        /// Current recursive transition count.
        pub proof_step_count: u32,
        /// Canonical transition-bound conflict claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Authenticated V4 artifact release.
        pub artifact_binding: KagemushaRecursiveSpendArtifactBindingV4,
        /// Recursive verifier selected by the proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Canonical identity digest of the complete opaque bundle.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub bundle_digest: [u8; 32],
    }
    /// Typed ABI-21 receiver-verification result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendVerifyResultV4 {
        /// Cryptographic proof and all public bindings verified.
        pub valid: bool,
        /// Bundle satisfies current chain admission rules.
        pub chain_admissible: bool,
        /// Persisted lineage material can be redeemed.
        pub lineage_redeemable: bool,
        /// Chain supports redemption without a record-backed witness.
        pub witnessless_redemption_supported: bool,
        /// Verified ABI-21 bundle summary.
        pub summary: KagemushaRecursiveSpendBundleSummaryV4,
        /// Canonical receiver request digest.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub recipient_request_digest: [u8; 32],
        /// Digest binding request, output, and bundle.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub request_output_binding_digest: [u8; 32],
        /// Active recursive verifier record identifier.
        pub verifier_key_id: VerifyingKeyId,
        /// Active V4 recursive circuit id.
        pub verifier_circuit_id: String,
        /// Inclusive verifier activation height.
        #[norito(required)]
        pub verifier_activation_height: Option<u64>,
        /// Exclusive verifier withdrawal height.
        #[norito(required)]
        pub verifier_withdraw_height: Option<u64>,
        /// Height used for activation-window verification.
        pub verified_at_block_height: u64,
        /// Authoritative Unix time used for acceptance.
        pub verified_at_ms: u64,
    }
    /// Proof-bound V4 offline change child created by partial redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemChangeBranchV4 {
        /// Exact change descriptor exposed by unshield-v3.
        pub output: KagemushaSpendableNoteDescriptorV2,
        /// Deterministic transition-bound change claims.
        pub branch_claims: Vec<KagemushaRecursiveSpendBranchClaimV2>,
        /// Recursive proof making the child independently spendable.
        pub bundle: KagemushaRecursiveSpendBundleV4,
    }
    /// ABI-21 native redemption builder input.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemBuildRequestV4 {
        /// Spendable recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub public_amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof for credit and optional change.
        pub unshield_proof: ProofAttachment,
        /// Exact public V4 redemption transition.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// Signed proof-evaluation snapshot, bounded by the eventual execution height.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// Canonical unsigned ABI-21 chain redemption fields.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemUnsignedV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change.
        pub redeem_proof: ProofAttachment,
        /// Canonical public V4 redemption intent.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// All-or-none proof-bound V4 change child.
        #[norito(required)]
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV4>,
        /// Signed proof-evaluation snapshot, bounded by the eventual execution height.
        pub block_height: u64,
        /// Stable idempotency identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// Prepared unsigned ABI-21 redemption result.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemBuildResultV4 {
        /// Complete unsigned V4 chain-request fields.
        pub unsigned: KagemushaRecursiveSpendRedeemUnsignedV4,
        /// Exact digest that device authorization must sign.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub authorization_digest: [u8; 32],
        /// Independently spendable proof-bound change.
        #[norito(required)]
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Membership state for proof-bound change.
        #[norito(required)]
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete inherited origin provenance for proof-bound change.
        #[norito(required)]
        pub offline_change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
    /// Versioned ABI-21 offline-to-online request.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(schema_name = "iroha.torii.v1.offline.redeem.request")]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemRequestV4 {
        /// Exact chain-request wire version. Only `4` is accepted.
        pub version: u16,
        /// Scale-carrying recursive state being redeemed.
        pub bundle: KagemushaRecursiveSpendBundleV4,
        /// Online account credited by redemption.
        pub recipient: AccountId,
        /// Exact public amount and authoritative asset scale.
        pub amount: KagemushaScaledAmountV2,
        /// Unshield-v3 proof bound to the current note and optional change.
        pub redeem_proof: ProofAttachment,
        /// Canonical public V4 redemption intent.
        pub redemption: KagemushaRecursiveSpendRedemptionIntentV4,
        /// All-or-none proof-bound V4 change child.
        #[norito(required)]
        pub offline_change: Option<KagemushaRecursiveSpendRedeemChangeBranchV4>,
        /// Signed proof-evaluation snapshot; chain execution also checks the current window.
        pub block_height: u64,
        /// Globally unique idempotency identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
        /// Self-contained recipient/device authorization.
        pub authorization: KagemushaRequestAuthorizationV2,
    }
    /// Typed native ABI-21 redemption output.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    #[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
    #[norito(deny_unknown_fields)]
    pub struct KagemushaRecursiveSpendRedeemResultV4 {
        /// Exact ABI-21 result wire version. Only `4` is accepted.
        pub version: u16,
        /// Canonical `KagemushaRecursiveSpendRedeemRequestV4` archive.
        pub redeem_request_archive: Vec<u8>,
        /// Proof-bound offline change branch.
        #[norito(required)]
        pub offline_change_bundle: Option<KagemushaRecursiveSpendBundleV4>,
        /// Membership state for proof-bound change.
        #[norito(required)]
        pub offline_change_membership_witness: Option<KagemushaNoteMembershipWitnessV2>,
        /// Complete inherited origin provenance for proof-bound change.
        #[norito(required)]
        pub offline_change_topup_provenance: Option<KagemushaRecursiveSpendTopUpProvenanceV4>,
        /// Stable operation identifier.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub operation_id: [u8; 32],
    }
}
