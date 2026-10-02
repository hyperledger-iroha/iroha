//! Exact same-account original publication for ordinary Native wallet startup/current recheck.
//! All carriers are data only; installed finality, runtime, clock and account owners remain external.
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, AccountValue},
    sumeragi_finality::{SumeragiFinalityAttestation, WorldStateSnapshotV1},
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
/// The sole first-release account-scoped signed POST.
pub const ORDINARY_WALLET_CURRENT_ROUTE_V1: &str = "/v1/kagemusha/ordinary/current-wallet";
/// Finite typed request original ceiling.
pub const ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1: usize = 8 * 1024;
/// Original complete commitment snapshot and exact two account rows ceiling.
pub const ORDINARY_WALLET_CURRENT_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
/// Exact current prefix/nonce and Native-selected S/W. Decoding grants no authority.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_torii_shared::ordinary_wallet_current::OrdinaryWalletCurrentRequestV1",
    frame = "iroha.torii.v1.ordinary-wallet-current.request"
)]
pub struct OrdinaryWalletCurrentRequestV1 {
    /// Exactly one; no retired-format fallback.
    pub version: u16,
    /// Installed genesis-derived network.
    pub network_id: NetworkId,
    /// Exact currently retained certified prefix height, at least two.
    pub height: u64,
    /// Fresh actual Native-generated nonce, unchanged in all four signed originals.
    pub request_nonce: [u8; 32],
    /// Registered Ed account S holding the actual private signing key.
    pub signatory: AccountId,
    /// Registered threshold-one, one-member wallet W.
    pub wallet: AccountId,
}
fn rejected() -> norito::Error {
    norito::Error::Message("ordinary current wallet original rejected".into())
}
/// Enforce the sole first-release S/W relation; it selects no root or actual signer custody.
/// # Errors
/// Refuses unsupported controllers, another member/key/threshold or identical S/W.
pub fn require_ordinary_wallet_relation_v1(
    signatory: &AccountId,
    wallet: &AccountId,
) -> Result<PublicKey, norito::Error> {
    let key = signatory.try_signatory().ok_or_else(rejected)?;
    let policy = wallet.multisig_policy().ok_or_else(rejected)?;
    if key.algorithm() != Algorithm::Ed25519
        || policy.threshold() != 1
        || policy.members().len() != 1
        || policy.members()[0].weight() != 1
        || policy.members()[0].public_key() != key
        || signatory == wallet
    {
        return Err(rejected());
    }
    Ok(key.clone())
}
impl OrdinaryWalletCurrentRequestV1 {
    /// Require exact supported scope and finite nonzero current-read coordinates.
    /// # Errors
    /// Refuses malformed version/height/nonce/controller.
    pub fn validate(&self) -> Result<(), norito::Error> {
        if self.version != 1 || self.height < 2 || self.request_nonce == [0; 32] {
            return Err(rejected());
        }
        require_ordinary_wallet_relation_v1(&self.signatory, &self.wallet).map(|_| ())
    }
    /// Sole bounded canonical signed body.
    /// # Errors
    /// Refuses malformed scope, serialization or request size.
    pub fn canonical_wire(&self) -> Result<Vec<u8>, norito::Error> {
        self.validate()?;
        if norito::canonical_frame_len(self)? > ORDINARY_WALLET_CURRENT_REQUEST_MAX_BYTES_V1 {
            return Err(rejected());
        }
        norito::encode_canonical(self)
    }
}
/// Full exact one-node original at the signed nonce/current prefix. Native requires all four.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    JsonSerialize,
    JsonDeserialize,
    NoritoSerialize,
    NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields, no_fast_from_json)]
#[norito_schema(
    name = "iroha_torii_shared::ordinary_wallet_current::OrdinaryWalletCurrentOriginalV1",
    frame = "iroha.torii.v1.ordinary-wallet-current.original"
)]
pub struct OrdinaryWalletCurrentOriginalV1 {
    /// Exact full original request, including selected S/W and nonce.
    pub request: OrdinaryWalletCurrentRequestV1,
    /// Exact original installed reporting node statement; never a root supplied by this response.
    pub attestation: SumeragiFinalityAttestation,
    /// Complete certified World commitment preimages, without unrelated original values.
    pub world_snapshot: WorldStateSnapshotV1,
    /// Complete exact current selected S value.
    pub signatory_value: AccountValue,
    /// Complete exact current selected W value.
    pub wallet_value: AccountValue,
}
impl OrdinaryWalletCurrentOriginalV1 {
    /// Validate correlation/shape only. Actual Native independently verifies all four identities,
    /// retained certified root, exact World membership/current lifetime and actual account key.
    /// # Errors
    /// Refuses changed selected request, nonce/network/prefix or malformed signed statement.
    pub fn validate_request_correlation(
        &self,
        request: &OrdinaryWalletCurrentRequestV1,
    ) -> Result<(), norito::Error> {
        request.validate()?;
        if &self.request != request
            || self.attestation.body.challenge != request.request_nonce
            || self.attestation.body.network_id != request.network_id
            || self.attestation.body.finality_proof.height() != request.height
        {
            return Err(rejected());
        }
        self.attestation.verify().map_err(|_| rejected())
    }
}
/// Borrowed original encoder: no cloned complete snapshot or account row allocation.
#[derive(NoritoSerialize, JsonSerialize)]
pub struct OrdinaryWalletCurrentOriginalRefV1<'a> {
    request: FieldRef<'a, OrdinaryWalletCurrentRequestV1>,
    attestation: FieldRef<'a, SumeragiFinalityAttestation>,
    world_snapshot: FieldRef<'a, WorldStateSnapshotV1>,
    signatory_value: FieldRef<'a, AccountValue>,
    wallet_value: FieldRef<'a, AccountValue>,
}
impl<'a> OrdinaryWalletCurrentOriginalRefV1<'a> {
    /// Borrow only the same current cut originals.
    #[must_use]
    pub fn new(
        request: &'a OrdinaryWalletCurrentRequestV1,
        attestation: &'a SumeragiFinalityAttestation,
        world_snapshot: &'a WorldStateSnapshotV1,
        signatory_value: &'a AccountValue,
        wallet_value: &'a AccountValue,
    ) -> Self {
        Self {
            request: FieldRef(request),
            attestation: FieldRef(attestation),
            world_snapshot: FieldRef(world_snapshot),
            signatory_value: FieldRef(signatory_value),
            wallet_value: FieldRef(wallet_value),
        }
    }
}
impl norito::NoritoSchema for OrdinaryWalletCurrentOriginalRefV1<'_> {
    fn nominal_name() -> String {
        OrdinaryWalletCurrentOriginalV1::nominal_name()
    }
    fn frame_name() -> String {
        OrdinaryWalletCurrentOriginalV1::frame_name()
    }
}
struct FieldRef<'a, T: ?Sized>(&'a T);
impl<T: norito::core::SerializePayload + ?Sized> norito::core::SerializePayload
    for FieldRef<'_, T>
{
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(out)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.encoded_len_hint()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}
impl<T: norito::json::JsonSerialize + ?Sized> norito::json::JsonSerialize for FieldRef<'_, T> {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out)
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        account::{AccountDetails, MultisigMember, MultisigPolicy},
        common::Owned,
        sumeragi::SumeragiStatus,
        sumeragi_finality::{
            SumeragiFinalityAttestationBody, test_fixtures::NativeFinalityFixture,
        },
    };
    use iroha_model_base::peer::PeerId;
    use norito::codec::Encode as _;
    fn fixture() -> (
        OrdinaryWalletCurrentRequestV1,
        OrdinaryWalletCurrentOriginalV1,
    ) {
        let key = KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519);
        let signatory = AccountId::new(key.public_key().clone());
        let wallet = AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![MultisigMember::new(key.public_key().clone(), 1).unwrap()],
            )
            .unwrap(),
        );
        let world = WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"explicit synthetic codec schema"),
            entries: vec![],
        };
        let mut chain = NativeFinalityFixture::start("ordinary-wallet-current-codec");
        let proof = chain.certify_with_world_root(
            chain.block_with_submitted_work(chain.next_header()),
            world.root().unwrap(),
        );
        let node = KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal);
        let node_id = PeerId::new(node.public_key().clone());
        let config = Hash::new(b"fixture config");
        let request = OrdinaryWalletCurrentRequestV1 {
            version: 1,
            network_id: chain.network_id(),
            height: proof.height(),
            request_nonce: [7; 32],
            signatory,
            wallet,
        };
        let body = SumeragiFinalityAttestationBody {
            observed_at_unix_ms: 1_000_000,
            challenge: request.request_nonce,
            network_id: request.network_id,
            node_fingerprint: Hash::new(node_id.encode()),
            node_id,
            build_fingerprint: Hash::new(b"fixture binary"),
            config_fingerprint: config,
            genesis_block_hash: chain.genesis().hash(),
            genesis_finality_proof: chain.genesis_proof().clone(),
            status: SumeragiStatus {
                protocol_version: 1,
                config_fingerprint: config,
                beacon_horizon: None,
                instance: chain.verifier().instance().0,
                height: 3,
                view: 0,
                stage: 0,
                leader: None,
                proxy_tail: None,
                high_qc_view: None,
                level: 0,
                start_level: 0,
                t_retx_ms: 100,
                committed_height: 2,
                applied_height: 2,
                awaiting: false,
                signer: Some(node.public_key().clone()),
                unanchored: false,
                abstaining: false,
                halted: None,
                footprint: Default::default(),
            },
            finality_proof: proof,
        };
        let attestation = SumeragiFinalityAttestation {
            signature: SignatureOf::try_from_hash(node.private_key(), body.signing_hash()).unwrap(),
            body,
        };
        let value = Owned::new(AccountDetails::new(Default::default(), None, None, vec![]));
        let original = OrdinaryWalletCurrentOriginalV1 {
            request: request.clone(),
            attestation,
            world_snapshot: world,
            signatory_value: value.clone(),
            wallet_value: value,
        };
        (request, original)
    }
    #[test]
    fn ordinary_wallet_request_and_borrowed_full_original_are_exact_canonical() {
        let (request, original) = fixture();
        let raw = request.canonical_wire().unwrap();
        assert_eq!(
            norito::decode_canonical::<OrdinaryWalletCurrentRequestV1>(&raw).unwrap(),
            request
        );
        original.validate_request_correlation(&request).unwrap();
        let borrowed = OrdinaryWalletCurrentOriginalRefV1::new(
            &request,
            &original.attestation,
            &original.world_snapshot,
            &original.signatory_value,
            &original.wallet_value,
        );
        let raw = norito::encode_canonical(&borrowed).unwrap();
        assert_eq!(raw, norito::encode_canonical(&original).unwrap());
        assert_eq!(
            norito::decode_canonical::<OrdinaryWalletCurrentOriginalV1>(&raw).unwrap(),
            original
        );
        assert_eq!(
            norito::json::to_vec(&borrowed).unwrap(),
            norito::json::to_vec(&original).unwrap()
        );
    }
    #[test]
    fn ordinary_wallet_original_rejects_request_prefix_nonce_and_signature_substitution() {
        let (request, original) = fixture();
        let mut changed = request.clone();
        changed.request_nonce[0] ^= 1;
        assert!(original.validate_request_correlation(&changed).is_err());
        changed = request.clone();
        changed.height += 1;
        assert!(original.validate_request_correlation(&changed).is_err());
        let mut changed = original.clone();
        changed.attestation.body.observed_at_unix_ms += 1;
        assert!(changed.validate_request_correlation(&request).is_err());
        let mut invalid = request;
        invalid.wallet = invalid.signatory.clone();
        assert!(invalid.canonical_wire().is_err());
        invalid.request_nonce = [0; 32];
        assert!(invalid.validate().is_err());
    }
}
