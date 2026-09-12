//! Data-availability helpers shared across SDKs.
use crate::{
    crypto::{HashOf, KeyPair},
    data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        isi::{InstructionBox, Transfer},
    },
};
use base64::{Engine, engine::general_purpose::STANDARD as Base64Standard};
use blake3::Hasher;
use eyre::{Result, WrapErr, eyre};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    da::{
        commitment::{
            DaCommitmentKey, DaCommitmentLocation, DaCommitmentProof, DaCommitmentWithLocation,
            DaProofPolicyBundle,
        },
        ingest::{DaIngestReceipt, DaIngestRequest, DaIngestRequestIntentV1},
        pin_intent::DaPinIntentWithLocation,
        types::{
            BlobClass, BlobCodec, BlobDigest, Compression, DaRentLedgerProjection, ErasureProfile,
            ExtraMetadata, GovernanceTag, RetentionPolicy, StorageTicketId,
        },
    },
    sorafs::pin_registry::{ManifestDigest, StorageClass},
};
use iroha_model_base::topology::LaneId;
use iroha_primitives::numeric::XorQuantity;
use norito::{
    decode_from_bytes,
    derive::{JsonDeserialize, JsonSerialize},
};
use sorafs_manifest::pdp::PdpCommitmentV1;
use std::num::NonZeroU64;
/// Canonical HTTP header carrying the base64-encoded PDP commitment bytes.
pub const PDP_COMMITMENT_HEADER: &str = "sora-pdp-commitment";
/// Decode the `sora-pdp-commitment` header into a typed PDP commitment.
///
/// # Errors
///
/// Returns an error if the header is not valid base64 or the decoded bytes fail
/// Norito deserialization.
pub fn decode_pdp_commitment_header(value: &str) -> Result<PdpCommitmentV1> {
    let bytes = Base64Standard
        .decode(value.as_bytes())
        .map_err(|err| eyre!("invalid {PDP_COMMITMENT_HEADER} header: {err}"))?;
    decode_pdp_commitment_bytes(&bytes)
}
/// Decode Norito-encoded PDP commitment bytes into a typed structure.
///
/// # Errors
///
/// Returns an error if the bytes cannot be decoded via Norito.
pub fn decode_pdp_commitment_bytes(bytes: &[u8]) -> Result<PdpCommitmentV1> {
    decode_from_bytes(bytes).map_err(|err| eyre!("failed to decode PDP commitment: {err}"))
}
/// Decode the optional PDP commitment embedded in a DA receipt.
///
/// # Errors
///
/// Returns an error if the commitment bytes contained in the receipt fail Norito decoding.
pub fn receipt_pdp_commitment(receipt: &DaIngestReceipt) -> Result<Option<PdpCommitmentV1>> {
    receipt.pdp_commitment.as_deref().map_or_else(
        || Ok(None),
        |bytes| decode_pdp_commitment_bytes(bytes).map(Some),
    )
}
/// Canonical ledger tip that binds a DA list cursor to one immutable view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaListSnapshot {
    /// Committed chain height observed while constructing the page.
    pub block_height: u64,
    /// Hash of the block at `block_height`, absent only for the empty chain.
    pub block_hash: Option<HashOf<BlockHeader>>,
}
/// Forward-only cursor for canonically ordered DA commitments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentListCursor {
    /// Immutable ledger view this cursor was issued against.
    pub snapshot: DaListSnapshot,
    /// Last raw commitment examined in `(lane_id, epoch, sequence)` order.
    pub after: DaCommitmentKey,
}
/// Request payload for `/v1/da/commitments`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentListRequest {
    /// Maximum raw index rows to inspect, capped by Torii at 1,000.
    pub limit: Option<NonZeroU64>,
    /// Server-issued continuation cursor from the preceding page.
    pub cursor: Option<DaCommitmentListCursor>,
}
/// Request payload for `/v1/da/commitments/prove`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentProofRequest {
    /// Optional manifest digest used as the primary lookup key.
    pub manifest_hash: Option<ManifestDigest>,
    /// Optional lane id used with `epoch` and `sequence` fallback lookup.
    pub lane_id: Option<u32>,
    /// Optional epoch used with `lane_id` and `sequence` fallback lookup.
    pub epoch: Option<u64>,
    /// Optional sequence used with `lane_id` and `epoch` fallback lookup.
    pub sequence: Option<u64>,
}
/// Response payload for `/v1/da/commitments`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentListResponse {
    /// Active proof-policy bundle for DA commitments.
    pub policies: DaProofPolicyBundle,
    /// Matching commitment records with on-chain location metadata.
    pub commitments: Vec<DaCommitmentWithLocation>,
    /// Cursor for the next bounded scan, or `None` when the index is exhausted.
    pub next_cursor: Option<DaCommitmentListCursor>,
}
/// Response payload for `/v1/da/commitments/prove`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentProofResponse {
    /// Proof-policy sidecar committed by the referenced block.
    pub policies: DaProofPolicyBundle,
    /// Commitment proof bound to the requested record.
    pub proof: DaCommitmentProof,
}
/// Response payload for `/v1/da/commitments/verify`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaCommitmentVerifyResponse {
    /// Indicates whether the proof verified against the canonical committed block and policy
    /// sidecar loaded from Kura.
    pub valid: bool,
    /// Optional verification failure detail when `valid` is false.
    pub error: Option<String>,
}
/// Forward-only cursor for canonically ordered DA pin intents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentListCursor {
    /// Immutable ledger view this cursor was issued against.
    pub snapshot: DaListSnapshot,
    /// Last raw pin intent examined in canonical block-location order.
    pub after: DaCommitmentLocation,
}
/// Request payload for `/v1/da/pin-intents`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentListRequest {
    /// Maximum raw index rows to inspect, capped by Torii at 1,000.
    pub limit: Option<NonZeroU64>,
    /// Server-issued continuation cursor from the preceding page.
    pub cursor: Option<DaPinIntentListCursor>,
}
/// Response payload for `/v1/da/pin-intents`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentListResponse {
    /// Visible intents among the bounded raw index rows examined for this page.
    pub intents: Vec<DaPinIntentWithLocation>,
    /// Cursor for the next bounded scan, or `None` when the index is exhausted.
    pub next_cursor: Option<DaPinIntentListCursor>,
}
/// Request payload for `/v1/da/pin-intents/prove`.
#[derive(Debug, Default, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentQueryRequest {
    /// Optional manifest digest used as a lookup key.
    pub manifest_hash: Option<ManifestDigest>,
    /// Optional storage ticket used as a lookup key.
    pub storage_ticket: Option<StorageTicketId>,
    /// Optional human-readable alias used as a lookup key (at most 256 UTF-8 bytes).
    pub alias: Option<String>,
    /// Optional lane id used with `epoch` and `sequence` fallback lookup.
    pub lane_id: Option<u32>,
    /// Optional epoch used with `lane_id` and `sequence` fallback lookup.
    pub epoch: Option<u64>,
    /// Optional sequence used with `lane_id` and `epoch` fallback lookup.
    pub sequence: Option<u64>,
}
/// Response payload for `/v1/da/pin-intents/verify`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
pub struct DaPinIntentVerifyResponse {
    /// Indicates whether the supplied pin-intent membership proof verified.
    pub valid: bool,
    /// Optional verification failure detail when `valid` is false.
    pub error: Option<String>,
}
/// Canonical ingest parameters shared by CLI and SDK clients.
#[derive(Debug, Clone)]
pub struct DaIngestParams {
    /// Lane identifier recorded in the DA request.
    pub lane_id: LaneId,
    /// Epoch identifier recorded in the DA request.
    pub epoch: u64,
    /// Monotonic sequence scoped to `(lane_id, epoch)`.
    pub sequence: u64,
    /// Semantic blob classification (e.g., `nexus_lane_sidecar`).
    pub blob_class: BlobClass,
    /// Codec label associated with the payload (e.g., `custom.binary`).
    pub blob_codec: BlobCodec,
    /// Requested erasure profile for chunking.
    pub erasure_profile: ErasureProfile,
    /// Retention policy to enforce.
    pub retention_policy: RetentionPolicy,
    /// Chunk size in bytes.
    pub chunk_size: u32,
    /// Optional caller-supplied blob digest override. Defaults to BLAKE3(payload).
    pub client_blob_id: Option<BlobDigest>,
}
impl DaIngestParams {
    /// Override the client-provided blob digest.
    #[must_use]
    pub fn with_client_blob_id(mut self, digest: BlobDigest) -> Self {
        self.client_blob_id = Some(digest);
        self
    }
}
impl Default for DaIngestParams {
    fn default() -> Self {
        Self {
            lane_id: LaneId::new(0),
            epoch: 0,
            sequence: 0,
            blob_class: BlobClass::NexusLaneSidecar,
            blob_codec: BlobCodec::new("custom.binary"),
            erasure_profile: ErasureProfile::default(),
            retention_policy: default_retention_policy(),
            chunk_size: 262_144,
            client_blob_id: None,
        }
    }
}
fn default_retention_policy() -> RetentionPolicy {
    RetentionPolicy {
        storage_class: StorageClass::Warm,
        governance_tag: GovernanceTag::new("da.generic"),
        ..RetentionPolicy::default()
    }
}
/// Build and sign a canonical `DaIngestRequest` using the supplied key pair.
///
/// # Errors
///
/// Returns an error if signature generation fails (should be infallible under normal conditions).
pub fn build_da_request(
    network_id: NetworkId,
    owner: AccountId,
    payload_bytes: Vec<u8>,
    params: &DaIngestParams,
    metadata: ExtraMetadata,
    key_pair: &KeyPair,
    manifest_bytes: Option<Vec<u8>>,
) -> Result<DaIngestRequest> {
    let client_blob_id = params.client_blob_id.unwrap_or_else(|| {
        let mut hasher = Hasher::new();
        hasher.update(&payload_bytes);
        BlobDigest::from_hash(hasher.finalize())
    });
    DaIngestRequestIntentV1 {
        network_id,
        owner,
        client_blob_id,
        lane_id: params.lane_id,
        epoch: params.epoch,
        sequence: params.sequence,
        blob_class: params.blob_class,
        codec: params.blob_codec.clone(),
        erasure_profile: params.erasure_profile,
        retention_policy: params.retention_policy.clone(),
        chunk_size: params.chunk_size,
        total_size: payload_bytes.len() as u64,
        payload_hash: BlobDigest::from_hash(blake3::hash(&payload_bytes)),
        compression: Compression::Identity,
        norito_manifest: manifest_bytes,
        payload: payload_bytes,
        metadata,
    }
    .try_sign(key_pair)
    .wrap_err("failed to sign canonical DA ingest request intent")
}
/// Planned rent ledger movements derived from a [`DaRentLedgerProjection`].
#[derive(Debug, Clone)]
pub struct DaRentLedgerPlan {
    /// Total rent owed for the retention period.
    pub rent_due: XorQuantity,
    /// Protocol reserve allocation sourced from the rent.
    pub protocol_reserve_due: XorQuantity,
    /// Provider payout sourced from the rent (excludes bonuses).
    pub provider_reward_due: XorQuantity,
    /// PDP bonus pool earmarked per evaluation cycle.
    pub pdp_bonus_pool: XorQuantity,
    /// `PoTR` bonus pool earmarked per evaluation cycle.
    pub potr_bonus_pool: XorQuantity,
    /// Credit per GiB to reimburse fetch egress.
    pub egress_credit_per_gib: XorQuantity,
    /// Transfer instructions required to enact the plan.
    pub instructions: Vec<InstructionBox>,
}
/// Accounts participating in the rent ledger settlement plan.
#[derive(Debug, Clone, Copy)]
pub struct DaRentLedgerAccounts<'a> {
    /// Account paying the rent bill.
    pub payer: &'a AccountId,
    /// Treasury account receiving the rent payment.
    pub treasury: &'a AccountId,
    /// Account that accrues the protocol reserve portion.
    pub protocol_reserve: &'a AccountId,
    /// Account receiving the provider reward payout.
    pub provider: &'a AccountId,
    /// Account credited with the PDP bonus portion.
    pub pdp_bonus: &'a AccountId,
    /// Account credited with the `PoTR` bonus portion.
    pub potr_bonus: &'a AccountId,
}
/// Build the transfer plan used to settle a rent ledger projection.
///
/// This mirrors the rent-ledger workflow exposed through `iroha da rent-ledger`,
/// allowing host automation to derive the same instructions without shelling
/// out to the CLI.
#[must_use]
pub fn build_da_rent_ledger_plan(
    projection: &DaRentLedgerProjection,
    accounts: &DaRentLedgerAccounts<'_>,
    asset_definition: &AssetDefinitionId,
) -> DaRentLedgerPlan {
    let mut instructions = Vec::new();
    push_rent_instruction(
        &mut instructions,
        accounts.payer,
        accounts.treasury,
        projection.rent_due.clone(),
        asset_definition,
    );
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.protocol_reserve,
        projection.protocol_reserve_due.clone(),
        asset_definition,
    );
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.provider,
        projection.provider_reward_due.clone(),
        asset_definition,
    );
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.pdp_bonus,
        projection.pdp_bonus_pool.clone(),
        asset_definition,
    );
    push_rent_instruction(
        &mut instructions,
        accounts.treasury,
        accounts.potr_bonus,
        projection.potr_bonus_pool.clone(),
        asset_definition,
    );
    DaRentLedgerPlan {
        rent_due: projection.rent_due.clone(),
        protocol_reserve_due: projection.protocol_reserve_due.clone(),
        provider_reward_due: projection.provider_reward_due.clone(),
        pdp_bonus_pool: projection.pdp_bonus_pool.clone(),
        potr_bonus_pool: projection.potr_bonus_pool.clone(),
        egress_credit_per_gib: projection.egress_credit_per_gib.clone(),
        instructions,
    }
}
fn push_rent_instruction(
    instructions: &mut Vec<InstructionBox>,
    source_account: &AccountId,
    destination_account: &AccountId,
    amount: XorQuantity,
    asset_definition: &AssetDefinitionId,
) {
    if amount.is_zero() {
        return;
    }
    let asset_id = AssetId::new(asset_definition.clone(), source_account.clone());
    let transfer = Transfer::asset_quantity(
        asset_id,
        amount.into_quantity(),
        destination_account.clone(),
    );
    instructions.push(InstructionBox::from(transfer));
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KeyPair;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        da::{
            ingest::DaStripeLayout,
            types::{
                BlobClass, BlobCodec, BlobDigest, DaRentQuote, ErasureProfile, ExtraMetadata,
                FecScheme, GovernanceTag, RetentionPolicy, StorageTicketId,
            },
        },
        prelude::AccountId,
        sorafs::pin_registry::StorageClass,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::topology::LaneId;
    use sorafs_manifest::{ChunkingProfileV1, pdp::PdpMerkleTreeV1};
    fn checked_seed_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives DA Ed25519 keypair")
    }
    #[test]
    fn build_da_request_hashes_payload_when_digest_absent() {
        let key_pair = checked_seed_keypair(0x11);
        let params = sample_ingest_params(None);
        let payload = vec![0xCA, 0xFE, 0xBA, 0xBE];
        let request = build_da_request(
            crate::client::test_network_id(),
            AccountId::new(key_pair.public_key().clone()),
            payload.clone(),
            &params,
            ExtraMetadata { items: Vec::new() },
            &key_pair,
            None,
        )
        .expect("build DA request");
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let expected = BlobDigest::from_hash(hasher.finalize());
        assert_eq!(request.client_blob_id, expected);
        assert_eq!(request.payload, payload);
    }
    #[test]
    fn build_da_request_respects_digest_override() {
        let key_pair = checked_seed_keypair(0x22);
        let override_digest = BlobDigest::new([0xAB; 32]);
        let params = sample_ingest_params(Some(override_digest));
        let request = build_da_request(
            crate::client::test_network_id(),
            AccountId::new(key_pair.public_key().clone()),
            vec![0x01, 0x02],
            &params,
            ExtraMetadata { items: Vec::new() },
            &key_pair,
            None,
        )
        .expect("build DA request");
        assert_eq!(request.client_blob_id, override_digest);
        assert_eq!(request.chunk_size, params.chunk_size);
    }
    #[test]
    fn header_decodes_commitment() {
        let commitment = sample_commitment();
        let bytes = norito::to_bytes(&commitment).expect("encode commitment");
        let header_value = BASE64.encode(&bytes);
        let decoded = decode_pdp_commitment_header(&header_value).expect("decode header");
        assert_eq!(decoded, commitment);
    }
    #[test]
    fn receipt_helper_respects_absent_commitment() {
        let mut receipt = sample_receipt();
        receipt.pdp_commitment = None;
        assert!(
            receipt_pdp_commitment(&receipt)
                .expect("decode receipt commitment")
                .is_none()
        );
    }
    #[test]
    fn receipt_helper_decodes_bytes() {
        let commitment = sample_commitment();
        let bytes = norito::to_bytes(&commitment).expect("encode commitment");
        let mut receipt = sample_receipt();
        receipt.pdp_commitment = Some(bytes);
        let decoded = receipt_pdp_commitment(&receipt)
            .expect("decode commitment")
            .expect("commitment present");
        assert_eq!(decoded, commitment);
    }
    #[test]
    fn invalid_header_surfaces_error() {
        let err = decode_pdp_commitment_header("###").expect_err("expected failure");
        assert!(
            err.to_string().contains("invalid"),
            "unexpected error: {err:?}"
        );
    }
    #[test]
    fn da_commitment_proof_request_roundtrips_json() {
        let request = DaCommitmentProofRequest {
            manifest_hash: Some(ManifestDigest::new([0x11; 32])),
            lane_id: Some(7),
            epoch: Some(9),
            sequence: Some(12),
        };
        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaCommitmentProofRequest =
            norito::json::from_slice(&bytes).expect("decode request");
        assert_eq!(decoded.manifest_hash, request.manifest_hash);
        assert_eq!(decoded.lane_id, request.lane_id);
        assert_eq!(decoded.epoch, request.epoch);
        assert_eq!(decoded.sequence, request.sequence);
    }
    #[test]
    fn da_commitment_list_cursor_roundtrips_json() {
        let request = DaCommitmentListRequest {
            limit: NonZeroU64::new(2),
            cursor: Some(DaCommitmentListCursor {
                snapshot: DaListSnapshot {
                    block_height: 0,
                    block_hash: None,
                },
                after: DaCommitmentKey {
                    lane_id: LaneId::new(7),
                    epoch: 9,
                    sequence: 12,
                },
            }),
        };
        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaCommitmentListRequest =
            norito::json::from_slice(&bytes).expect("decode request");
        assert_eq!(decoded, request);
    }
    #[test]
    fn da_pin_intent_query_request_roundtrips_json() {
        let request = DaPinIntentQueryRequest {
            manifest_hash: Some(ManifestDigest::new([0x22; 32])),
            storage_ticket: Some(StorageTicketId::new([0x33; 32])),
            alias: Some("news/latest".to_string()),
            lane_id: Some(4),
            epoch: Some(8),
            sequence: Some(16),
        };
        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaPinIntentQueryRequest =
            norito::json::from_slice(&bytes).expect("decode request");
        assert_eq!(decoded.manifest_hash, request.manifest_hash);
        assert_eq!(decoded.storage_ticket, request.storage_ticket);
        assert_eq!(decoded.alias, request.alias);
        assert_eq!(decoded.lane_id, request.lane_id);
        assert_eq!(decoded.epoch, request.epoch);
        assert_eq!(decoded.sequence, request.sequence);
    }
    #[test]
    fn da_pin_intent_list_cursor_roundtrips_json() {
        let request = DaPinIntentListRequest {
            limit: NonZeroU64::new(5),
            cursor: Some(DaPinIntentListCursor {
                snapshot: DaListSnapshot {
                    block_height: 0,
                    block_hash: None,
                },
                after: DaCommitmentLocation {
                    block_height: 7,
                    index_in_bundle: 3,
                },
            }),
        };
        let bytes = norito::json::to_vec(&request).expect("encode request");
        let decoded: DaPinIntentListRequest =
            norito::json::from_slice(&bytes).expect("decode request");
        assert_eq!(decoded, request);
    }
    #[test]
    fn rent_ledger_plan_emits_expected_transfers() {
        let projection = DaRentLedgerProjection {
            rent_due: "340282366920938463463374607431768211456.000000001"
                .parse()
                .expect("wide scale-nine XOR quantity"),
            protocol_reserve_due: "0.25".parse().expect("XOR quantity"),
            provider_reward_due: "1.25".parse().expect("XOR quantity"),
            pdp_bonus_pool: "0.000000001".parse().expect("sub-micro XOR quantity"),
            potr_bonus_pool: "0.025".parse().expect("XOR quantity"),
            egress_credit_per_gib: "0.012".parse().expect("XOR quantity"),
        };
        let _wonderland_domain: DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain");
        let _sora_domain: DomainId = DomainId::try_new("sora", "universal").expect("domain");
        let payer_key = checked_seed_keypair(1);
        let payer = AccountId::new(payer_key.public_key().clone());
        let treasury_key = checked_seed_keypair(2);
        let treasury = AccountId::new(treasury_key.public_key().clone());
        let reserve_key = checked_seed_keypair(3);
        let protocol_reserve = AccountId::new(reserve_key.public_key().clone());
        let provider_key = checked_seed_keypair(4);
        let provider = AccountId::new(provider_key.public_key().clone());
        let pdp_key = checked_seed_keypair(5);
        let pdp_bonus = AccountId::new(pdp_key.public_key().clone());
        let potr_key = checked_seed_keypair(6);
        let potr_bonus = AccountId::new(potr_key.public_key().clone());
        let asset_definition: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "xor".parse().unwrap(),
            );
        let accounts = DaRentLedgerAccounts {
            payer: &payer,
            treasury: &treasury,
            protocol_reserve: &protocol_reserve,
            provider: &provider,
            pdp_bonus: &pdp_bonus,
            potr_bonus: &potr_bonus,
        };
        let plan = build_da_rent_ledger_plan(&projection, &accounts, &asset_definition);
        assert_eq!(plan.rent_due, projection.rent_due);
        assert_eq!(plan.protocol_reserve_due, projection.protocol_reserve_due);
        assert_eq!(plan.provider_reward_due, projection.provider_reward_due);
        assert_eq!(plan.egress_credit_per_gib, projection.egress_credit_per_gib);
        let rent_amount = projection.rent_due.as_quantity().clone();
        let reserve_amount = projection.protocol_reserve_due.as_quantity().clone();
        let provider_amount = projection.provider_reward_due.as_quantity().clone();
        let pdp_amount = projection.pdp_bonus_pool.as_quantity().clone();
        let potr_amount = projection.potr_bonus_pool.as_quantity().clone();
        let expected = vec![
            InstructionBox::from(Transfer::asset_quantity(
                AssetId::new(asset_definition.clone(), payer.clone()),
                rent_amount,
                treasury.clone(),
            )),
            InstructionBox::from(Transfer::asset_quantity(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                reserve_amount,
                protocol_reserve.clone(),
            )),
            InstructionBox::from(Transfer::asset_quantity(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                provider_amount,
                provider.clone(),
            )),
            InstructionBox::from(Transfer::asset_quantity(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                pdp_amount,
                pdp_bonus.clone(),
            )),
            InstructionBox::from(Transfer::asset_quantity(
                AssetId::new(asset_definition.clone(), treasury.clone()),
                potr_amount,
                potr_bonus.clone(),
            )),
        ];
        assert_eq!(plan.instructions, expected);
    }
    fn sample_commitment() -> PdpCommitmentV1 {
        let payload = vec![0x5A; 64 * 1024];
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("build sample PDP tree");
        let chunk_profile = ChunkingProfileV1::from_descriptor(
            sorafs_manifest::chunker_registry::default_descriptor(),
        );
        PdpCommitmentV1::from_tree(&tree, [0x11; 32], chunk_profile, 16, 1_701_800_000)
            .expect("build canonical sample PDP commitment")
    }
    fn sample_receipt() -> DaIngestReceipt {
        DaIngestReceipt {
            client_blob_id: iroha_data_model::da::types::BlobDigest::new([0xAA; 32]),
            lane_id: LaneId::new(7),
            epoch: 9,
            blob_hash: iroha_data_model::da::types::BlobDigest::new([0xBB; 32]),
            chunk_root: iroha_data_model::da::types::BlobDigest::new([0xCC; 32]),
            manifest_hash: iroha_data_model::da::types::BlobDigest::new([0xDD; 32]),
            storage_ticket: iroha_data_model::da::types::StorageTicketId::new([0u8; 32]),
            pdp_commitment: None,
            stripe_layout: DaStripeLayout {
                total_stripes: 1,
                shards_per_stripe: 1,
                row_parity_stripes: 0,
            },
            queued_at_unix: 1_701_900_000,
            rent_quote: DaRentQuote::default(),
            operator_signature: iroha_crypto::Signature::try_from_bytes(&[0x42u8; 64])
                .expect("nonzero DA receipt signature fixture"),
        }
    }
    fn sample_ingest_params(override_digest: Option<BlobDigest>) -> DaIngestParams {
        DaIngestParams {
            lane_id: LaneId::new(5),
            epoch: 9,
            sequence: 3,
            blob_class: BlobClass::NexusLaneSidecar,
            blob_codec: BlobCodec::new("custom.binary"),
            erasure_profile: ErasureProfile {
                data_shards: 10,
                parity_shards: 4,
                row_parity_stripes: 0,
                chunk_alignment: 8,
                fec_scheme: FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 600,
                cold_retention_secs: 3_600,
                required_replicas: 3,
                storage_class: StorageClass::Warm,
                governance_tag: GovernanceTag::new("da.tests"),
            },
            chunk_size: 262_144,
            client_blob_id: override_digest,
        }
    }
}
