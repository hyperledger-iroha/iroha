//! Explicit, bounded temporary game equipment authorizations and retained records.
//!
//! These clauses authorize returnable NFT custody only. Opaque application data,
//! wager items and fungible entry debits cannot substitute for these clauses.
use crate::{NetworkId, account::AccountId, game::GAME_MAX_PARTICIPANTS_V1, nft::NftId};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

/// Generic equipment bound; an adapter may require a smaller exact set.
pub const GAME_MAX_RESOURCES_PER_PARTICIPANT_V1: usize = 4;
/// All retained participant resources, including returned terminal records.
pub const GAME_MAX_RESOURCE_RECORDS_V1: usize =
    GAME_MAX_PARTICIPANTS_V1 * GAME_MAX_RESOURCES_PER_PARTICIPANT_V1;
/// Maximum UTF-8 length of a resource's canonical NFT identifier.
pub const GAME_RESOURCE_MAX_NFT_ID_BYTES_V1: usize = 512;
/// Maximum UTF-8 length of each canonical owner or custody account identifier.
pub const GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1: usize = 16 * 1024;
/// Maximum encoded individual authorization, independently of vector bounds.
pub const GAME_RESOURCE_MAX_CLAUSE_BYTES_V1: usize = 1024;
/// Maximum complete encoded retained set; projected admission must preflight it.
pub const GAME_RESOURCE_MAX_SET_BYTES_V1: usize = 1024 * 1024;

macro_rules! record {
    ($(#[$meta:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $field:ident: $ty:ty,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
        #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
        #[norito (deny_unknown_fields)]
        pub struct $name { $($(#[$fm])* pub $field: $ty,)* }
    };
}

/// Closed custody policy. Wins, ties and forfeits never change the recipient.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::game_resources::GameResourceReturnPolicyV1")]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    crate :: DeriveJsonSerialize,
    crate :: DeriveJsonDeserialize,
)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GameResourceReturnPolicyV1 {
    /// Release only with a legal terminal session transition, to the original owner.
    #[codec(index = 0)]
    ReturnToOriginalOwnerAtTerminal,
}

record! {
    /// An explicit wallet-signed temporary NFT reservation, separate from a wager.
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::game_resources::GameResourceReservationClauseV1")]
    pub struct GameResourceReservationClauseV1 {
        /// Exact NFT whose current ownership must be authenticated by Core.
        pub nft_id: NftId,
        /// Wallet-approved complete canonical metadata commitment.
        pub expected_metadata_hash: Hash,
        /// Compiled application role identity; ordered by its raw hash bytes.
        pub role_id: Hash,
        /// The sole supported temporary-return policy.
        pub policy: GameResourceReturnPolicyV1,
    }
}
record! {
    /// A compiled adapter's requirement; it never itself authorizes custody.
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::game_resources::GameResourceRequirementV1")]
    pub struct GameResourceRequirementV1 {
        /// Exact NFT declared by the compiled application input.
        pub nft_id: NftId,
        /// Required complete metadata commitment.
        pub expected_metadata_hash: Hash,
        /// Exact role requested by the adapter.
        pub role_id: Hash,
        /// Required return condition, matched to the wallet's typed clause.
        pub policy: GameResourceReturnPolicyV1,
    }
}
record! {
    /// Permanent audit record; it contains no configurable release recipient.
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::game_resources::GameResourceReservationRecordV1")]
    pub struct GameResourceReservationRecordV1 {
        /// Permanent participant slot.
        pub slot: u8,
        /// Exact NFT retained during admission.
        pub nft_id: NftId,
        /// Complete metadata commitment frozen by native custody.
        pub metadata_hash: Hash,
        /// Participant-local role identity.
        pub role_id: Hash,
        /// Exact wallet-approved temporary-return condition.
        pub policy: GameResourceReturnPolicyV1,
        /// Authenticated owner at admission and sole permitted final recipient.
        pub original_owner: AccountId,
        /// Derived non-signable custody identity; Core must recompute it.
        pub custody: AccountId,
        /// Consensus height at which admission atomically reserved this resource.
        pub reserved_at_height: u64,
        /// Terminal return height; absent while retained, never a deadline unlock.
        pub released_at_height: Option<u64>,
    }
}
record! {
    /// Bounded consensus set kept separate from existing game-session/wager shapes.
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::game_resources::GameResourceReservationSetV1")]
    pub struct GameResourceReservationSetV1 {
        /// Exactly one for this independent schema.
        pub version: u16,
        /// Network derived from the signed genesis trust context.
        pub network_id: NetworkId,
        /// Exact owning game session.
        pub session_id: Hash,
        /// Strict `(slot, role_id)` order; NFT and custody identities are unique.
        pub records: Vec<GameResourceReservationRecordV1>,
    }
}

/// Require a bounded NFT identity with exactly one lossless canonical text/JSON representation.
/// Domain construction and decoding enforce label boundaries; game admission also checks
/// the complete NFT spelling and its byte bound.
pub fn validate_game_nft_identity_v1(nft: &NftId) -> Result<(), &'static str> {
    let literal = nft.to_string();
    if literal.len() > GAME_RESOURCE_MAX_NFT_ID_BYTES_V1 {
        return Err("game NFT identifier exceeds its byte bound");
    }
    if literal.parse::<NftId>().ok().as_ref() != Some(nft) {
        return Err("game NFT identifier does not have an exact canonical text identity");
    }
    Ok(())
}

fn validate_fields<'a>(
    fields: impl IntoIterator<Item = (&'a NftId, &'a Hash)>,
) -> Result<(), &'static str> {
    let mut previous = None;
    let mut nfts = BTreeSet::new();
    for (nft, role) in fields {
        validate_game_nft_identity_v1(nft)?;
        if previous.is_some_and(|old| old >= role) {
            return Err("game resource roles must be unique and strictly ordered");
        }
        if !nfts.insert(nft) {
            return Err("game resource NFT identifiers must be unique");
        }
        previous = Some(role);
    }
    Ok(())
}
/// Pure validation; never sorts, deduplicates or mutates wallet authorizations.
pub fn validate_resource_clauses_v1(
    clauses: &[GameResourceReservationClauseV1],
) -> Result<(), &'static str> {
    if clauses.len() > GAME_MAX_RESOURCES_PER_PARTICIPANT_V1 {
        return Err("too many participant game resource clauses");
    }
    validate_fields(
        clauses
            .iter()
            .map(|clause| (&clause.nft_id, &clause.role_id)),
    )?;
    if clauses
        .iter()
        .any(|clause| clause.encode().len() > GAME_RESOURCE_MAX_CLAUSE_BYTES_V1)
    {
        return Err("game resource clause exceeds its encoded byte bound");
    }
    Ok(())
}
/// Validate a compiled adapter's bounded request before comparing authorization.
pub fn validate_resource_requirements_v1(
    requirements: &[GameResourceRequirementV1],
) -> Result<(), &'static str> {
    if requirements.len() > GAME_MAX_RESOURCES_PER_PARTICIPANT_V1 {
        return Err("too many participant game resource requirements");
    }
    validate_fields(
        requirements
            .iter()
            .map(|entry| (&entry.nft_id, &entry.role_id)),
    )?;
    if requirements
        .iter()
        .any(|entry| entry.encode().len() > GAME_RESOURCE_MAX_CLAUSE_BYTES_V1)
    {
        return Err("game resource requirement exceeds its encoded byte bound");
    }
    Ok(())
}
/// Require an exact match; opaque application data never supplies missing clauses.
pub fn match_resource_requirements_v1(
    clauses: &[GameResourceReservationClauseV1],
    requirements: &[GameResourceRequirementV1],
) -> Result<(), &'static str> {
    validate_resource_clauses_v1(clauses)?;
    validate_resource_requirements_v1(requirements)?;
    if clauses.len() != requirements.len()
        || clauses.iter().zip(requirements).any(|(clause, required)| {
            clause.nft_id != required.nft_id
                || clause.expected_metadata_hash != required.expected_metadata_hash
                || clause.role_id != required.role_id
                || clause.policy != required.policy
        })
    {
        return Err("typed game resource clauses do not match compiled requirements");
    }
    Ok(())
}
impl GameResourceReservationSetV1 {
    /// Validate geometry and canonical retained state without ledger mutations.
    ///
    /// Decode callers must separately enforce framing/allocation limits before
    /// creating this value. Core must also verify exact roster, phase, ownership,
    /// metadata, custody derivation and inverse reservation indexes in WSV.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1 || self.records.len() > GAME_MAX_RESOURCE_RECORDS_V1 {
            return Err("invalid game resource set version or record count");
        }
        let mut previous = None;
        let mut nfts = BTreeSet::new();
        let mut custody = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut slot_owners = BTreeMap::new();
        let mut counts = [0_usize; GAME_MAX_PARTICIPANTS_V1];
        let mut release_state = None;
        for record in &self.records {
            validate_game_nft_identity_v1(&record.nft_id)?;
            let slot = usize::from(record.slot);
            if slot >= GAME_MAX_PARTICIPANTS_V1
                || record.nft_id.to_string().len() > GAME_RESOURCE_MAX_NFT_ID_BYTES_V1
            {
                return Err("game resource record exceeds slot or identifier bounds");
            }
            if record.original_owner.to_string().len() > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1
                || record.custody.to_string().len() > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1
            {
                return Err("game resource account identifier exceeds its byte bound");
            }
            counts[slot] += 1;
            if counts[slot] > GAME_MAX_RESOURCES_PER_PARTICIPANT_V1 {
                return Err("too many resources retained for one game slot");
            }
            let key = (record.slot, record.role_id);
            if previous.is_some_and(|old| old >= key) {
                return Err("game resource records must be strictly ordered by slot and role");
            }
            previous = Some(key);
            if !nfts.insert(&record.nft_id) || !custody.insert(&record.custody) {
                return Err("game resources cannot repeat an NFT or custody identity");
            }
            if slot_owners
                .insert(record.slot, &record.original_owner)
                .is_some_and(|owner| owner != &record.original_owner)
                || owners
                    .insert(&record.original_owner, record.slot)
                    .is_some_and(|owner_slot| owner_slot != record.slot)
            {
                return Err("game resource owner does not uniquely match its participant slot");
            }
            if record.reserved_at_height == 0
                || record
                    .released_at_height
                    .is_some_and(|height| height < record.reserved_at_height)
            {
                return Err("game resource reservation or release height is invalid");
            }
            if release_state.is_some_and(|state| state != record.released_at_height) {
                return Err("game resources must return atomically at one terminal height");
            }
            release_state = Some(record.released_at_height);
        }
        if custody.iter().any(|account| owners.contains_key(account)) {
            return Err("game resource custody cannot be a participant owner");
        }
        if self.encode().len() > GAME_RESOURCE_MAX_SET_BYTES_V1 {
            return Err("game resource set exceeds its encoded byte bound");
        }
        Ok(())
    }

    /// Check original owners against the separately retained permanent roster.
    pub fn validate_for_owners(
        &self,
        participant_owners: &[AccountId],
    ) -> Result<(), &'static str> {
        self.validate()?;
        if participant_owners.len() > GAME_MAX_PARTICIPANTS_V1
            || participant_owners.iter().collect::<BTreeSet<_>>().len() != participant_owners.len()
            || participant_owners
                .iter()
                .any(|owner| owner.to_string().len() > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1)
            || self.records.iter().any(|record| {
                participant_owners.get(usize::from(record.slot)) != Some(&record.original_owner)
                    || participant_owners.contains(&record.custody)
            })
        {
            return Err("game resource records do not match the retained original owners");
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "game_nft_identity_fixture.rs"]
pub(crate) mod identity_test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        )
    }
    fn clause(index: u8) -> GameResourceReservationClauseV1 {
        GameResourceReservationClauseV1 {
            nft_id: format!("kit{index}$equipment.universal").parse().unwrap(),
            expected_metadata_hash: Hash::new(b"reviewed metadata"),
            role_id: Hash::prehashed([index; 32]),
            policy: GameResourceReturnPolicyV1::ReturnToOriginalOwnerAtTerminal,
        }
    }
    fn retained() -> GameResourceReservationSetV1 {
        GameResourceReservationSetV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(
                HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"resource test network",
                )),
            ),
            session_id: Hash::new(b"resource session"),
            records: (0..2)
                .map(|slot| {
                    let clause = clause(slot + 1);
                    GameResourceReservationRecordV1 {
                        slot,
                        nft_id: clause.nft_id,
                        metadata_hash: clause.expected_metadata_hash,
                        role_id: clause.role_id,
                        policy: clause.policy,
                        original_owner: account(slot + 1),
                        custody: account(slot + 11),
                        reserved_at_height: 10 + u64::from(slot),
                        released_at_height: None,
                    }
                })
                .collect(),
        }
    }
    #[test]
    fn clauses_and_requirements_reject_missing_extra_duplicate_and_unsorted_authorization() {
        let clauses = vec![clause(1), clause(2)];
        let before = clauses.clone();
        let requirements = clauses
            .iter()
            .map(|clause| GameResourceRequirementV1 {
                nft_id: clause.nft_id.clone(),
                expected_metadata_hash: clause.expected_metadata_hash,
                role_id: clause.role_id,
                policy: clause.policy,
            })
            .collect::<Vec<_>>();
        match_resource_requirements_v1(&clauses, &requirements).unwrap();
        assert!(match_resource_requirements_v1(&clauses, &[]).is_err());
        assert!(match_resource_requirements_v1(&[], &requirements).is_err());
        let mut wrong = requirements.clone();
        wrong[0].expected_metadata_hash = Hash::new(b"different metadata");
        assert!(match_resource_requirements_v1(&clauses, &wrong).is_err());
        assert!(validate_resource_clauses_v1(&[clause(2), clause(1)]).is_err());
        let mut duplicate = clauses.clone();
        duplicate[1].nft_id = duplicate[0].nft_id.clone();
        assert!(validate_resource_clauses_v1(&duplicate).is_err());
        assert!(validate_resource_clauses_v1(&vec![clause(1); 5]).is_err());
        assert!(validate_resource_requirements_v1(&vec![requirements[0].clone(); 5]).is_err());
        assert_eq!(clauses, before);
    }
    #[test]
    fn retained_resource_geometry_rejects_ambiguous_typed_nft_identity() {
        use identity_test_support::assert_ambiguous_domain_label_rejected;
        for (domain, dataspace, label) in [
            ("art-gallery", "universal", "art-gallery"),
            ("art", "gallery-universal", "gallery-universal"),
        ] {
            assert!(
                crate::domain::DomainId::try_new(
                    domain.replace('-', "."),
                    dataspace.replace('-', ".")
                )
                .is_err()
            );
            let mut canonical = retained();
            canonical.records[0].nft_id = NftId::new(
                crate::domain::DomainId::try_new(domain, dataspace).unwrap(),
                "kit".parse().unwrap(),
            );
            canonical.validate().unwrap();
            assert_ambiguous_domain_label_rejected(&canonical, label);
        }
        let mut control = retained();
        control.records[0].nft_id = NftId::new(
            crate::domain::DomainId::try_new("art", "universal").unwrap(),
            "kit".parse().unwrap(),
        );
        control.validate().unwrap();
    }
    #[test]
    fn retained_resources_reject_aliases_wrong_owners_and_partial_or_early_returns() {
        let set = retained();
        set.validate_for_owners(&[account(1), account(2)]).unwrap();
        assert!(set.validate_for_owners(&[account(2), account(1)]).is_err());
        let mut unequipped_alias = set.clone();
        unequipped_alias.records[0].custody = account(3);
        // The third wallet owns no resource, but still cannot be custody.
        unequipped_alias.validate().unwrap();
        assert!(
            unequipped_alias
                .validate_for_owners(&[account(1), account(2), account(3)])
                .is_err()
        );
        for kind in 0..8 {
            let mut bad = set.clone();
            match kind {
                0 => bad.records.reverse(),
                1 => bad.records[1].nft_id = bad.records[0].nft_id.clone(),
                2 => bad.records[1].custody = bad.records[0].custody.clone(),
                3 => bad.records[0].custody = bad.records[1].original_owner.clone(),
                4 => bad.records[0].released_at_height = Some(12),
                5 => bad.records[0].reserved_at_height = 0,
                6 => bad.records[0].released_at_height = Some(9),
                _ => bad.records[1].original_owner = bad.records[0].original_owner.clone(),
            }
            assert!(bad.validate().is_err(), "mutation {kind}");
        }
        let mut returned = set;
        for record in &mut returned.records {
            record.released_at_height = Some(20);
        }
        returned.validate().unwrap();
        returned.records[1].released_at_height = Some(21);
        assert!(returned.validate().is_err());
    }
    #[test]
    fn retained_bounds_and_native_bare_and_framed_roundtrips_are_explicit() {
        let set = retained();
        let bare = set.encode();
        assert_eq!(
            GameResourceReservationSetV1::decode(&mut bare.as_slice()).unwrap(),
            set
        );
        let framed = norito::to_bytes(&set).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<GameResourceReservationSetV1>(&framed).unwrap(),
            set
        );
        assert!(GameResourceReturnPolicyV1::decode(&mut [1_u8, 0, 0, 0].as_slice()).is_err());
        let mut excessive = set.clone();
        excessive.records = vec![set.records[0].clone(); GAME_MAX_RESOURCE_RECORDS_V1 + 1];
        assert!(excessive.validate().is_err());
        let mut wrong_version = set;
        wrong_version.version = 2;
        assert!(wrong_version.validate().is_err());
    }

    #[test]

    fn closed_resource_policy_requires_exact_native_json_unit_content() {
        let policy = norito::json::from_str::<GameResourceReturnPolicyV1>(
            r#"{"kind":"return_to_original_owner_at_terminal","value":null}"#,
        )
        .unwrap();
        assert_eq!(
            policy,
            GameResourceReturnPolicyV1::ReturnToOriginalOwnerAtTerminal
        );
        for wrong in [
            r#"{"kind":"return_to_original_owner_at_terminal"}"#,
            r#"{"kind":"return_to_original_owner_at_terminal","value":"winner"}"#,
            r#"{"kind":"return_to_winner_at_terminal","value":null}"#,
            r#"{"kind":"return_to_original_owner_at_terminal","value":null,"recipient":"winner"}"#,
        ] {
            assert!(norito::json::from_str::<GameResourceReturnPolicyV1>(wrong).is_err());
        }
    }
}

#[cfg(test)]
mod additional_frame_owner_identity_tests {
    //! Typed frame contracts observed with the original codec.

    #[test]
    fn captured_additional_frame_owner_identities() {
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::game_resources::GameResourceRequirementV1,
        >("iroha_data_model::game_resources::GameResourceRequirementV1");
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::game_resources::GameResourceReservationClauseV1,
        >("iroha_data_model::game_resources::GameResourceReservationClauseV1");
        crate::frame_owner_identity_tests::assert_bidirectional::<
            crate::game_resources::GameResourceReservationSetV1,
        >("iroha_data_model::game_resources::GameResourceReservationSetV1");
    }
}
