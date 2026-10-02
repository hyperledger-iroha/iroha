//! Exact account-authenticated original of one actual ordinary Mint issuer World grant.
//! Full policy data defines no root; the consumer independently selects the installed purpose.
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::{AccountId, AccountValue},
    asset::AssetDefinitionId,
    permission::{Permission, Permissions},
    role::Role,
    sumeragi_finality::{SumeragiFinalityAttestation, WorldStateSnapshotV1},
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
/// Sole original-purpose signed POST; unrelated account data is never published.
pub const ORDINARY_MINT_ISSUER_PURPOSE_ROUTE_V1: &str =
    "/v1/kagemusha/ordinary/mint-issuer-purpose";
/// Exact body and complete permission-value capture bound.
pub const ORDINARY_MINT_ISSUER_PURPOSE_REQUEST_MAX_BYTES_V1: usize = 96 * 1024;
/// Complete certified World plus one bounded grant original.
pub const ORDINARY_MINT_ISSUER_PURPOSE_MAX_BYTES_V1: usize = 128 * 1024 * 1024;
/// Maximum full direct permission set or complete selected role.
pub const ORDINARY_MINT_ISSUER_GRANT_MAX_BYTES_V1: usize = 64 * 1024;
/// Requested issuer-purpose data; the signed body selects no independent authority.
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
    name = "iroha_torii_shared::ordinary_mint_issuer_purpose::OrdinaryMintIssuerPurposeRequestV1",
    frame = "iroha.torii.v1.ordinary-mint-issuer-purpose.request"
)]
pub struct OrdinaryMintIssuerPurposeRequestV1 {
    /// Exactly one.
    pub version: u16,
    /// Independently installed genesis-derived network.
    pub network_id: NetworkId,
    /// Genuine current retained certified height.
    pub height: u64,
    /// Fresh request nonce, signed by the same reporting node.
    pub request_nonce: [u8; 32],
    /// Actual independently installed single-key Ed issuer and HTTP signer.
    pub issuer: AccountId,
    /// Exact asset definition owned by the independent Mint purpose.
    pub asset: AssetDefinitionId,
    /// Complete independently selected purpose token; returned only if actually granted.
    pub purpose: Permission,
}
fn rejected() -> norito::Error {
    norito::Error::Message("ordinary Mint issuer purpose original rejected".into())
}
impl OrdinaryMintIssuerPurposeRequestV1 {
    /// Validate supported signed-request coordinates only, never an issuer grant.
    /// # Errors
    /// Refuses malformed scope, unsupported signer, zero nonce or unbounded purpose.
    pub fn validate(&self) -> Result<&PublicKey, norito::Error> {
        let key = self.issuer.try_signatory().ok_or_else(rejected)?;
        if self.version != 1
            || self.height < 2
            || self.request_nonce == [0; 32]
            || key.algorithm() != Algorithm::Ed25519
            || norito::canonical_frame_len(&self.purpose)? > ORDINARY_MINT_ISSUER_GRANT_MAX_BYTES_V1
        {
            return Err(rejected());
        }
        Ok(key)
    }
    /// Sole bounded canonical body under the ordinary issuer account signature.
    /// # Errors
    /// Refuses unsupported coordinates or canonical framing bound.
    pub fn canonical_wire(&self) -> Result<Vec<u8>, norito::Error> {
        self.validate()?;
        if norito::canonical_frame_len(self)? > ORDINARY_MINT_ISSUER_PURPOSE_REQUEST_MAX_BYTES_V1 {
            return Err(rejected());
        }
        norito::encode_canonical(self)
    }
}
/// One positive exact World grant preimage; omission cannot create another grant.
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
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(
    name = "iroha_torii_shared::ordinary_mint_issuer_purpose::OrdinaryMintIssuerGrantOriginalV1"
)]
pub enum OrdinaryMintIssuerGrantOriginalV1 {
    /// Complete native direct-permission row for the authenticated issuer.
    Direct(Permissions),
    /// Complete native role; its assignment to the exact issuer must also be proved.
    Role(Role),
}
impl OrdinaryMintIssuerGrantOriginalV1 {
    /// Require the complete permission token, including its payload, in either grant original.
    fn contains_permission(&self, purpose: &Permission) -> bool {
        match self {
            Self::Direct(permissions) => permissions.contains(purpose),
            Self::Role(role) => role.permissions().any(|permission| permission == purpose),
        }
    }
}
/// Full native data response. Decoding supplies no policy, root, current clock or effect grant.
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
    name = "iroha_torii_shared::ordinary_mint_issuer_purpose::OrdinaryMintIssuerPurposeOriginalV1",
    frame = "iroha.torii.v1.ordinary-mint-issuer-purpose.original"
)]
pub struct OrdinaryMintIssuerPurposeOriginalV1 {
    /// Exact authenticated full request.
    pub request: OrdinaryMintIssuerPurposeRequestV1,
    /// Exact nonce-bound native node signed current statement.
    pub attestation: SumeragiFinalityAttestation,
    /// Complete certified native World commitments for the grant preimages.
    pub world_snapshot: WorldStateSnapshotV1,
    /// Complete actual registered issuer account value.
    pub issuer_value: AccountValue,
    /// Exact granted direct row or assigned role original.
    pub grant: OrdinaryMintIssuerGrantOriginalV1,
}
impl OrdinaryMintIssuerPurposeOriginalV1 {
    /// Shape/correlation gate only; independent installed policy/finality admission is required.
    /// # Errors
    /// Refuses changed original request, nonce/network/height or malformed grant.
    pub fn validate_request_correlation(
        &self,
        request: &OrdinaryMintIssuerPurposeRequestV1,
    ) -> Result<(), norito::Error> {
        request.validate()?;
        if &self.request != request
            || self.attestation.body.challenge != request.request_nonce
            || self.attestation.body.network_id != request.network_id
            || self.attestation.body.finality_proof.height() != request.height
            || norito::canonical_frame_len(&self.grant)?
                > ORDINARY_MINT_ISSUER_GRANT_MAX_BYTES_V1 + 1024
        {
            return Err(rejected());
        }
        if !self.grant.contains_permission(&request.purpose) {
            return Err(rejected());
        }
        self.attestation.verify().map_err(|_| rejected())
    }
}

/// Borrow the exact same owned first-release layout; World remains held by native capture.
#[derive(NoritoSerialize, JsonSerialize)]
pub struct OrdinaryMintIssuerPurposeOriginalRefV1<'a> {
    request: FieldRef<'a, OrdinaryMintIssuerPurposeRequestV1>,
    attestation: FieldRef<'a, SumeragiFinalityAttestation>,
    world_snapshot: FieldRef<'a, WorldStateSnapshotV1>,
    issuer_value: FieldRef<'a, AccountValue>,
    grant: FieldRef<'a, OrdinaryMintIssuerGrantOriginalV1>,
}
impl<'a> OrdinaryMintIssuerPurposeOriginalRefV1<'a> {
    /// Borrow same-cut originals for bounded serialization, with no authority conversion.
    #[must_use]
    pub fn new(
        request: &'a OrdinaryMintIssuerPurposeRequestV1,
        attestation: &'a SumeragiFinalityAttestation,
        world_snapshot: &'a WorldStateSnapshotV1,
        issuer_value: &'a AccountValue,
        grant: &'a OrdinaryMintIssuerGrantOriginalV1,
    ) -> Self {
        Self {
            request: FieldRef(request),
            attestation: FieldRef(attestation),
            world_snapshot: FieldRef(world_snapshot),
            issuer_value: FieldRef(issuer_value),
            grant: FieldRef(grant),
        }
    }
}
impl norito::NoritoSchema for OrdinaryMintIssuerPurposeOriginalRefV1<'_> {
    fn nominal_name() -> String {
        <OrdinaryMintIssuerPurposeOriginalV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <OrdinaryMintIssuerPurposeOriginalV1 as norito::NoritoSchema>::frame_name()
    }
}
// Payload-only forwarding preserves each field's exact codec and bounded JSON writer.
struct FieldRef<'a, T>(&'a T);
impl<T: norito::core::SerializePayload> norito::core::SerializePayload for FieldRef<'_, T> {
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::SerializePayload::serialize(self.0, out)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(self.0)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(self.0)
    }
}
impl<T: norito::json::JsonSerialize> norito::json::JsonSerialize for FieldRef<'_, T> {
    fn json_serialize(&self, out: &mut String) {
        self.0.json_serialize(out);
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
    fn request() -> OrdinaryMintIssuerPurposeRequestV1 {
        let fixture=iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        // Public data codec fixture; this generic token supplies no ordinary Mint grant.
        let purpose = Permission::new(
            "codec_only_permission".parse().unwrap(),
            iroha_data_model::prelude::Json::default(),
        );
        OrdinaryMintIssuerPurposeRequestV1 {
            version: 1,
            network_id: fixture.issuer_policy.runtime.network_id,
            height: 2,
            request_nonce: [1; 32],
            issuer: AccountId::new(fixture.issuer_policy.issuer_public_key),
            asset: fixture.issuer_policy.runtime.asset,
            purpose,
        }
    }
    #[test]
    fn complete_purpose_request_has_one_canonical_codec_and_pins_every_selector() {
        let request = request();
        let raw = request.canonical_wire().unwrap();
        let decoded: OrdinaryMintIssuerPurposeRequestV1 = norito::decode_canonical(&raw).unwrap();
        assert_eq!(decoded, request);
        for coordinate in 0..4 {
            let mut changed = request.clone();
            match coordinate {
                0 => changed.height += 1,
                1 => changed.request_nonce[0] ^= 1,
                2 => {
                    let mut aid_bytes = changed.asset.aid_bytes();
                    aid_bytes[0] ^= 1;
                    changed.asset = AssetDefinitionId::from_uuid_bytes(aid_bytes).unwrap();
                }
                _ => {
                    changed.purpose = Permission::new(
                        "other_permission".parse().unwrap(),
                        iroha_data_model::prelude::Json::default(),
                    )
                }
            };
            assert_ne!(changed.canonical_wire().unwrap(), raw);
        }
        let mut changed = request.clone();
        changed.request_nonce = [0; 32];
        assert!(changed.canonical_wire().is_err());
        changed = request.clone();
        changed.version = 2;
        assert!(changed.canonical_wire().is_err());
        changed = request;
        changed.height = 1;
        assert!(changed.canonical_wire().is_err());
    }

    #[test]
    fn direct_and_role_grants_require_the_exact_permission_token() {
        use iroha_data_model::Registrable as _;

        let request = request();
        let purpose = request.purpose;
        let changed_payload = Permission::new(
            purpose.name().parse().unwrap(),
            iroha_data_model::prelude::Json::new(&"other-payload"),
        );
        let changed_name = Permission::new(
            "other_permission".parse().unwrap(),
            purpose.payload().clone(),
        );
        let role = Role::new("issuer-purpose".parse().unwrap(), request.issuer.clone())
            .add_permission(purpose.clone())
            .build(&request.issuer);
        for grant in [
            OrdinaryMintIssuerGrantOriginalV1::Direct(Permissions::from([purpose.clone()])),
            OrdinaryMintIssuerGrantOriginalV1::Role(role),
        ] {
            assert!(grant.contains_permission(&purpose));
            assert!(!grant.contains_permission(&changed_payload));
            assert!(!grant.contains_permission(&changed_name));
        }
        let empty_role =
            Role::new("empty-role".parse().unwrap(), request.issuer.clone()).build(&request.issuer);
        assert!(
            !OrdinaryMintIssuerGrantOriginalV1::Direct(Permissions::new())
                .contains_permission(&purpose)
        );
        assert!(!OrdinaryMintIssuerGrantOriginalV1::Role(empty_role).contains_permission(&purpose));
    }
}
