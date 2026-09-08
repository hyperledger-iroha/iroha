//! Publisher-authenticated catalog policy for generic returnable game resources.
//!
//! This module is intentionally not wired into an ISI or WSV yet. The eventual
//! catalog publication instruction must persist the successful preflight result
//! under its content-derived identity, without update or deletion instructions.
//! The ledger reader is an internal native-state boundary, never a wallet or
//! endpoint assertion. A compiled adapter supplies the manifest's catalog and
//! role after validating its own exact application parameters.
//!
//! Catalog membership means that the publisher reviewed an exact NFT identifier
//! and complete metadata commitment. Native NFTs have no immutable mint-instance
//! or issuer field: this does not assert an NFT's historical mint provenance.
//! Shared native custody still owns atomic transfers, reserve accounting, replay
//! protection, and legal terminal returns. Catalog policy cannot release an NFT.

use super::nft_custody::nft_custody_account_v1;
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    domain::{Domain, DomainId},
    game::{
        GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1, GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1,
        GameAdmissionBodyV1, GameManifestV1, GameParticipantV1, GamePhaseV1, GameSessionRecordV1,
    },
    game_resources::{
        GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1, GameResourceRequirementV1,
        GameResourceReservationClauseV1, GameResourceReservationSetV1, GameResourceReturnPolicyV1,
        validate_game_nft_identity_v1, validate_resource_clauses_v1,
    },
    nft::{NftData, NftId},
    nft_market::{NftCustodyPurposeV1, NftCustodyRecordV1},
};
use norito::codec::{Decode, Encode};

/// Count limit before any catalog encoding or ledger membership traversal.
pub(crate) const GAME_EQUIPMENT_CATALOG_MAX_MEMBERS_V1: usize = 4096;
/// Complete canonical catalog descriptor bound, independent of execution proofs.
pub(crate) const GAME_EQUIPMENT_CATALOG_MAX_BYTES_V1: usize = 1024 * 1024;

/// One reviewed NFT identity and its complete native metadata commitment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct GameEquipmentCatalogMemberV1 {
    /// Exact, losslessly printable native identifier.
    pub nft_id: NftId,
    /// Hash of the complete canonical Norito metadata, not a selected JSON field.
    pub metadata_hash: Hash,
}

/// Immutable publication terms, independent of any particular game's class enum.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct GameEquipmentCatalogV1 {
    /// Exactly one for this first-release format.
    pub version: u16,
    /// Signed-genesis network identity.
    pub network_id: NetworkId,
    /// Authenticated publisher at creation; later domain transfers do not replace it.
    pub publisher: AccountId,
    /// Publisher-chosen application seed, shared across the application's classes.
    pub application_seed: Hash,
    /// Exact collection domain owned by the publisher at publication.
    pub collection: DomainId,
    /// Compiled execution relation that may use this catalog.
    pub profile_id: Hash,
    /// Opaque compiled resource role; metadata cannot introduce another role.
    pub role_id: Hash,
    /// Closed original-owner return policy.
    pub policy: GameResourceReturnPolicyV1,
    /// Nonempty, strictly native-ID-ordered closed membership set.
    pub members: Vec<GameEquipmentCatalogMemberV1>,
}

/// Immutable native record created only by authenticated publication preflight.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct GameEquipmentCatalogRecordV1 {
    /// Exact terms whose canonical encoding determines the catalog identity.
    pub catalog: GameEquipmentCatalogV1,
    /// Nonzero consensus height of publication.
    pub published_at_height: u64,
}

/// Read-only native ledger projection required by the catalog policy.
///
/// A future WSV adapter must read the actual maps in one transaction/snapshot.
/// Implementing this trait for an HTTP response or participant payload is invalid.
pub(crate) trait GameEquipmentLedgerV1 {
    /// Look up an immutable catalog under its exact content-derived key.
    fn catalog(&self, id: &Hash) -> Option<&GameEquipmentCatalogRecordV1>;
    /// Look up the native collection and its current owner for publication only.
    fn domain(&self, id: &DomainId) -> Option<&Domain>;
    /// Read the current complete NFT data under an exact native identity.
    fn nft(&self, id: &NftId) -> Option<&NftData>;
    /// Read the current native reservation inverse index.
    fn reservation(&self, id: &NftId) -> Option<&AccountId>;
    /// Read retained native custody history, including already returned records.
    fn custody(&self, id: &AccountId) -> Option<&NftCustodyRecordV1>;
    /// Check the native account map, including collisions with derived custody.
    fn account_exists(&self, id: &AccountId) -> bool;
}

/// One publisher/network-derived namespace for all classes of an application.
/// An unrelated publisher cannot reuse the same seed to acquire this identity.
pub(crate) fn game_equipment_application_id_v1(
    network: &NetworkId,
    publisher: &AccountId,
    application_seed: &Hash,
) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha:game:application:v1\0",
        network.as_bytes(),
        &(publisher.clone(), *application_seed).encode(),
    ])
}

impl GameEquipmentCatalogV1 {
    /// Reject malformed or unbounded descriptors without normalizing their order.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.members.is_empty()
            || self.members.len() > GAME_EQUIPMENT_CATALOG_MAX_MEMBERS_V1
        {
            return Err("equipment catalog version or member count invalid".into());
        }
        if self.publisher.to_string().len() > GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1
            || self.publisher.encode().len() > GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1
            || DomainId::parse_fully_qualified(&self.collection.to_string())
                .ok()
                .as_ref()
                != Some(&self.collection)
        {
            return Err("equipment publisher or collection is not bounded and canonical".into());
        }
        let mut previous = None;
        for member in &self.members {
            validate_game_nft_identity_v1(&member.nft_id).map_err(str::to_owned)?;
            if member.nft_id.domain != self.collection
                || member.nft_id.encode().len() > GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1
                || previous.is_some_and(|id| id >= &member.nft_id)
            {
                return Err(
                    "equipment members must be exact, unique, sorted collection NFTs".into(),
                );
            }
            previous = Some(&member.nft_id);
        }
        if self.encode().len() > GAME_EQUIPMENT_CATALOG_MAX_BYTES_V1 {
            return Err("equipment catalog exceeds its encoded byte bound".into());
        }
        Ok(())
    }

    /// Exact immutable identity; validate the descriptor before admitting it.
    pub(crate) fn id(&self) -> Hash {
        Hash::new_from_chunks(&[b"iroha:game:equipment-catalog:v1\0", &self.encode()])
    }

    fn member(&self, nft_id: &NftId, metadata_hash: Hash) -> Result<(), String> {
        let index = self
            .members
            .binary_search_by(|member| member.nft_id.cmp(nft_id))
            .map_err(|_| "NFT is not a reviewed catalog member")?;
        if self.members[index].metadata_hash != metadata_hash {
            return Err("NFT metadata differs from the reviewed catalog commitment".into());
        }
        Ok(())
    }
}

/// Prepare immutable publication from native authenticated authority and state.
///
/// Every member must currently be publisher-owned and unreserved. The instruction
/// applying this result must be atomic, meter the descriptor/membership work, and
/// reject replacement of an existing record. This function performs no writes.
pub(crate) fn prepare_game_equipment_catalog_v1(
    ledger: &impl GameEquipmentLedgerV1,
    network: &NetworkId,
    authority: &AccountId,
    height: u64,
    catalog: GameEquipmentCatalogV1,
) -> Result<(Hash, GameEquipmentCatalogRecordV1), String> {
    catalog.validate()?;
    let id = catalog.id();
    let domain = ledger
        .domain(&catalog.collection)
        .ok_or("equipment collection absent")?;
    if height == 0
        || catalog.network_id != *network
        || catalog.publisher != *authority
        || domain.id != catalog.collection
        || domain.owned_by != *authority
        || !ledger.account_exists(authority)
        || ledger.custody(authority).is_some()
        || ledger.catalog(&id).is_some()
    {
        return Err(
            "equipment publication authority, network, height or uniqueness invalid".into(),
        );
    }
    for member in &catalog.members {
        let nft = ledger.nft(&member.nft_id).ok_or("catalog NFT absent")?;
        if nft.owned_by != *authority
            || Hash::new(nft.content.encode()) != member.metadata_hash
            || ledger.reservation(&member.nft_id).is_some()
        {
            return Err("catalog member is not publisher-owned, exact and unreserved".into());
        }
    }
    Ok((
        id,
        GameEquipmentCatalogRecordV1 {
            catalog,
            published_at_height: height,
        },
    ))
}

/// Resolve exact retained terms for a compiled adapter's manifest and role.
/// Later domain ownership and sale sellers never replace the retained publisher.
pub(crate) fn validate_game_equipment_binding_v1<'a>(
    ledger: &'a impl GameEquipmentLedgerV1,
    network: &NetworkId,
    height: u64,
    manifest: &GameManifestV1,
    catalog_id: &Hash,
    role_id: &Hash,
) -> Result<&'a GameEquipmentCatalogRecordV1, String> {
    let record = ledger
        .catalog(catalog_id)
        .ok_or("immutable equipment catalog absent")?;
    let catalog = &record.catalog;
    catalog.validate()?;
    if record.published_at_height == 0
        || record.published_at_height > height
        || catalog.id() != *catalog_id
        || catalog.network_id != *network
        || catalog.profile_id != manifest.profile_id
        || catalog.role_id != *role_id
        || manifest.application_id
            != game_equipment_application_id_v1(
                network,
                &catalog.publisher,
                &catalog.application_seed,
            )
    {
        return Err(
            "equipment catalog differs from the exact application, network, profile or role".into(),
        );
    }
    Ok(record)
}

/// Validate a compiled single-role adapter's explicit Join equipment selection.
///
/// The caller first validates compiled manifest/participant bytes and later runs
/// shared custody preflight/apply in the same transaction. A returned requirement
/// is not a wallet authorization and cannot itself move an NFT.
pub(crate) fn prepare_game_equipment_join_v1(
    ledger: &impl GameEquipmentLedgerV1,
    catalog_id: &Hash,
    role_id: &Hash,
    height: u64,
    session: &GameSessionRecordV1,
    participant: &GameParticipantV1,
    clauses: &[GameResourceReservationClauseV1],
) -> Result<Vec<GameResourceRequirementV1>, String> {
    let record = validate_game_equipment_binding_v1(
        ledger,
        &session.network_id,
        height,
        &session.manifest,
        catalog_id,
        role_id,
    )?;
    if height < record.published_at_height
        || session.phase != GamePhaseV1::Lobby
        || session.profile_id != session.manifest.profile_id
        || clauses.len() != 1
        || !ledger.account_exists(&participant.account)
        || ledger.custody(&participant.account).is_some()
        || session
            .participants
            .iter()
            .any(|p| p.account == participant.account)
    {
        return Err("equipment entry phase, height, participant or resource count invalid".into());
    }
    let clause = &clauses[0];
    let catalog = &record.catalog;
    validate_resource_clauses_v1(clauses).map_err(str::to_owned)?;
    catalog.member(&clause.nft_id, clause.expected_metadata_hash)?;
    let custody = nft_custody_account_v1(
        &session.network_id,
        &session.session_id,
        NftCustodyPurposeV1::GameResource,
        &clause.nft_id,
    );
    let nft = ledger
        .nft(&clause.nft_id)
        .ok_or("entry equipment NFT absent")?;
    if clause.role_id != catalog.role_id
        || clause.policy != catalog.policy
        || nft.owned_by != participant.account
        || Hash::new(nft.content.encode()) != clause.expected_metadata_hash
        || ledger.reservation(&clause.nft_id).is_some()
        || ledger.custody(&custody).is_some()
        || ledger.account_exists(&custody)
        || session
            .item_stakes
            .iter()
            .any(|item| item.nft_id == clause.nft_id)
        || session
            .resources
            .iter()
            .any(|item| item.nft_id == clause.nft_id)
    {
        return Err(
            "entry equipment ownership, content, role, wager separation or reservation invalid"
                .into(),
        );
    }
    Ok(vec![GameResourceRequirementV1 {
        nft_id: clause.nft_id.clone(),
        expected_metadata_hash: clause.expected_metadata_hash,
        role_id: clause.role_id,
        policy: clause.policy,
    }])
}

/// Revalidate catalog entitlements against immutable admission and native custody.
///
/// For open sessions, check live ownership/content and the exact reservation
/// inverse index. For terminal sessions, validate the historical return record;
/// a subsequently sold, edited, re-reserved or deleted NFT is not a restore error.
/// Generic lifecycle, dispute, roster and reserve-account validation remain
/// mandatory independently of this single-role catalog policy.
pub(crate) fn validate_restored_game_equipment_v1(
    ledger: &impl GameEquipmentLedgerV1,
    catalog_id: &Hash,
    role_id: &Hash,
    height: u64,
    session: &GameSessionRecordV1,
) -> Result<(), String> {
    let catalog_record = validate_game_equipment_binding_v1(
        ledger,
        &session.network_id,
        height,
        &session.manifest,
        catalog_id,
        role_id,
    )?;
    let catalog = &catalog_record.catalog;
    GameAdmissionBodyV1::from_session(session).validate()?;
    GameResourceReservationSetV1 {
        version: 1,
        network_id: session.network_id,
        session_id: session.session_id,
        records: session.resources.clone(),
    }
    .validate_for_owners(
        &session
            .participants
            .iter()
            .map(|p| p.account.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(str::to_owned)?;
    let closed = matches!(session.phase, GamePhaseV1::Settled | GamePhaseV1::Cancelled);
    if session.profile_id != session.manifest.profile_id
        || session.resources.len() != session.participants.len()
        || closed != session.terminal_at_height.is_some()
        || session.terminal_at_height == Some(0)
        || session.terminal_at_height.is_some_and(|terminal| {
            terminal > height || terminal < catalog_record.published_at_height
        })
    {
        return Err("retained equipment cardinality, profile or terminal phase invalid".into());
    }
    for (slot, resource) in session.resources.iter().enumerate() {
        catalog.member(&resource.nft_id, resource.metadata_hash)?;
        let owner = &session.participants[slot].account;
        let expected_custody = nft_custody_account_v1(
            &session.network_id,
            &session.session_id,
            NftCustodyPurposeV1::GameResource,
            &resource.nft_id,
        );
        let custody = ledger
            .custody(&expected_custody)
            .ok_or("retained equipment custody absent")?;
        if usize::from(resource.slot) != slot
            || resource.original_owner != *owner
            || resource.role_id != catalog.role_id
            || resource.policy != catalog.policy
            || resource.custody != expected_custody
            || resource.reserved_at_height < catalog_record.published_at_height
            || resource.reserved_at_height > height
            || resource.released_at_height != session.terminal_at_height
            || session
                .terminal_at_height
                .is_some_and(|h| h < resource.reserved_at_height)
            || custody.version != 1
            || custody.network_id != session.network_id
            || custody.reservation_id != session.session_id
            || custody.purpose != NftCustodyPurposeV1::GameResource
            || custody.nft_id != resource.nft_id
            || custody.custody != expected_custody
            || custody.original_owner != *owner
            || custody.metadata_hash != resource.metadata_hash
            || custody.released_to.as_ref() != closed.then_some(owner)
            || !ledger.account_exists(&expected_custody)
        {
            return Err(
                "retained catalog equipment differs from exact original-owner custody".into(),
            );
        }
        if !closed {
            let nft = ledger
                .nft(&resource.nft_id)
                .ok_or("reserved equipment NFT absent")?;
            if nft.owned_by != expected_custody
                || Hash::new(nft.content.encode()) != resource.metadata_hash
                || ledger.reservation(&resource.nft_id) != Some(&expected_custody)
                || !ledger.account_exists(owner)
            {
                return Err("active catalog equipment differs from live native custody".into());
            }
        } else if ledger.reservation(&resource.nft_id) == Some(&expected_custody) {
            return Err("returned equipment retains its closed reservation index".into());
        }
    }
    Ok(())
}
