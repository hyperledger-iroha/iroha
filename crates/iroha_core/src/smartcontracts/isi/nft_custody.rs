//! Shared native NFT reservation guards. No native custody account has a signing scalar.
use super::Error;
use crate::state::{StateReadOnly, StateTransaction, World, WorldReadOnly};
use iroha_crypto::{Hash, derive_non_signing_ed25519_public_key};
use iroha_data_model::{
    IntoKeyValue, NetworkId,
    account::{Account, AccountId},
    events::data::prelude::{NftEvent, NftOwnerChanged},
    nft::NftId,
    nft_market::{NftCustodyPurposeV1, NftCustodyRecordV1, NftSaleStatusV1},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
use std::collections::BTreeMap;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}
/// Stable non-signable identity for an exact network, protocol, reservation and NFT.
pub fn nft_custody_account_v1(
    network: &NetworkId,
    reservation_id: &Hash,
    purpose: NftCustodyPurposeV1,
    nft_id: &NftId,
) -> AccountId {
    AccountId::new(derive_non_signing_ed25519_public_key(
        b"iroha:nft:custody:v1",
        &[
            network.as_bytes(),
            reservation_id.as_ref(),
            &purpose.encode(),
            &nft_id.encode(),
        ],
    ))
}
/// All custody identities remain retained; original wallets are retained while reserved.
pub(crate) fn retained_nft_account(world: &impl WorldReadOnly, account: &AccountId) -> bool {
    world.nft_custody_records().get(account).is_some()
        || world
            .nft_custody_owner_refs()
            .get(account)
            .is_some_and(|count| *count > 0)
}
/// Ordinary instructions must not rewrite or transfer a currently reserved NFT.
pub(crate) fn ensure_nft_unreserved(world: &impl WorldReadOnly, nft: &NftId) -> Result<(), Error> {
    if world.nft_custody_by_nft().get(nft).is_some() {
        return Err(invalid("NFT is held by native custody"));
    }
    Ok(())
}
/// Native custody cannot receive unrelated ordinary NFTs.
pub(crate) fn ensure_nft_destination_unreserved(
    world: &impl WorldReadOnly,
    account: &AccountId,
) -> Result<(), Error> {
    if world.nft_custody_records().get(account).is_some() {
        return Err(invalid(
            "ordinary NFT transfers cannot target native custody",
        ));
    }
    Ok(())
}
/// Domain cascades cannot remove a reserved NFT or its frozen metadata.
pub(crate) fn ensure_nft_domain_unreserved(
    world: &impl WorldReadOnly,
    domain: &DomainId,
) -> Result<(), Error> {
    if world
        .nft_custody_domain_refs()
        .get(domain)
        .is_some_and(|count| *count > 0)
    {
        return Err(invalid("domain contains an NFT held by native custody"));
    }
    Ok(())
}
fn moved(
    st: &mut StateTransaction<'_, '_>,
    nft_id: &NftId,
    source: &AccountId,
    destination: &AccountId,
) {
    st.world
        .nft_mut(nft_id)
        .expect("preflight retained exact NFT")
        .owned_by = destination.clone();
    st.world
        .replace_nft_owner_index(nft_id, source, destination);
    st.world
        .emit_events(Some(NftEvent::OwnerChanged(NftOwnerChanged {
            nft: nft_id.clone(),
            new_owner: destination.clone(),
        })));
}
/// Fully checked bounded reservation set; funding may run after preparation.
pub(in crate::smartcontracts::isi) struct PreparedNftReservationsV1 {
    entries: Vec<(NftCustodyRecordV1, u32, u32)>,
}
impl PreparedNftReservationsV1 {
    pub(in crate::smartcontracts::isi) fn records(
        &self,
    ) -> impl Iterator<Item = &NftCustodyRecordV1> {
        self.entries.iter().map(|(record, _, _)| record)
    }
    /// Apply only after all accompanying preflights succeed; no NFT mutation may intervene.
    pub(in crate::smartcontracts::isi) fn apply(
        self,
        st: &mut StateTransaction<'_, '_>,
    ) -> Vec<NftCustodyRecordV1> {
        let mut records = Vec::with_capacity(self.entries.len());
        for (record, owner_refs, domain_refs) in self.entries {
            let (id, value) = Account {
                id: record.custody.clone(),
                metadata: Metadata::default(),
                label: None,
                uaid: None,
                opaque_ids: Vec::new(),
            }
            .into_key_value();
            st.world.accounts.insert(id, value);
            moved(st, &record.nft_id, &record.original_owner, &record.custody);
            st.world
                .nft_custody_records
                .insert(record.custody.clone(), record.clone());
            st.world
                .nft_custody_by_nft
                .insert(record.nft_id.clone(), record.custody.clone());
            st.world
                .nft_custody_owner_refs
                .insert(record.original_owner.clone(), owner_refs);
            st.world
                .nft_custody_domain_refs
                .insert(record.nft_id.domain().clone(), domain_refs);
            records.push(record);
        }
        records
    }
}
/// Validate every explicit NFT and cumulative reference increment before changing any state.
pub(in crate::smartcontracts::isi) fn prepare_nft_reservations_v1(
    st: &StateTransaction<'_, '_>,
    authority: &AccountId,
    reservation_id: Hash,
    purpose: NftCustodyPurposeV1,
    nfts: Vec<(NftId, Hash)>,
) -> Result<PreparedNftReservationsV1, Error> {
    if nfts.len() > iroha_data_model::game::GAME_MAX_NFT_RESERVATIONS_V1 {
        return Err(invalid(
            "native NFT reservation set exceeds its combined bound",
        ));
    }
    st.world.account(authority)?;
    ensure_nft_destination_unreserved(&st.world, authority)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut domains = BTreeMap::<DomainId, u32>::new();
    let mut owner_refs = st
        .world
        .nft_custody_owner_refs
        .get(authority)
        .copied()
        .unwrap_or(0);
    let network_id = *st.network_id();
    let mut entries = Vec::with_capacity(nfts.len());
    for (nft_id, expected_metadata_hash) in nfts {
        if !seen.insert(nft_id.clone()) {
            return Err(invalid("native NFT reservation repeats an item"));
        }
        ensure_nft_unreserved(&st.world, &nft_id)?;
        let nft = st.world.nft(&nft_id)?;
        let metadata_hash = Hash::new(nft.value().content.encode());
        if &nft.value().owned_by != authority || metadata_hash != expected_metadata_hash {
            return Err(invalid(
                "NFT reservation differs from its authenticated owner or approved metadata",
            ));
        }
        let custody = nft_custody_account_v1(&network_id, &reservation_id, purpose, &nft_id);
        if st.world.nft_custody_records.get(&custody).is_some()
            || st.world.account(&custody).is_ok()
        {
            return Err(invalid(
                "NFT custody identity already exists; reservations cannot replay",
            ));
        }
        owner_refs = owner_refs
            .checked_add(1)
            .ok_or_else(|| invalid("NFT owner reference overflow"))?;
        let domain_refs = domains.entry(nft_id.domain().clone()).or_insert_with(|| {
            st.world
                .nft_custody_domain_refs
                .get(nft_id.domain())
                .copied()
                .unwrap_or(0)
        });
        *domain_refs = domain_refs
            .checked_add(1)
            .ok_or_else(|| invalid("NFT domain reference overflow"))?;
        entries.push((
            NftCustodyRecordV1 {
                version: 1,
                network_id,
                reservation_id,
                purpose,
                nft_id,
                custody,
                original_owner: authority.clone(),
                metadata_hash,
                released_to: None,
            },
            owner_refs,
            *domain_refs,
        ));
    }
    Ok(PreparedNftReservationsV1 { entries })
}
/// Reserve an authenticated owner's current NFT through the same cumulative preflight.
pub(in crate::smartcontracts::isi) fn reserve_nft_v1(
    st: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    reservation_id: Hash,
    purpose: NftCustodyPurposeV1,
    nft_id: &NftId,
) -> Result<NftCustodyRecordV1, Error> {
    let metadata = Hash::new(st.world.nft(nft_id)?.value().content.encode());
    let prepared = prepare_nft_reservations_v1(
        st,
        authority,
        reservation_id,
        purpose,
        vec![(nft_id.clone(), metadata)],
    )?;
    Ok(prepared.apply(st).remove(0))
}
/// Release preflight binds every field before a caller performs any accompanying payment.
pub(in crate::smartcontracts::isi) struct PreparedNftReleaseV1 {
    record: NftCustodyRecordV1,
    owner_refs: u32,
    domain_refs: u32,
    destination: AccountId,
}
pub(in crate::smartcontracts::isi) fn prepare_nft_release_v1(
    st: &StateTransaction<'_, '_>,
    reservation_id: Hash,
    purpose: NftCustodyPurposeV1,
    nft_id: &NftId,
    destination: &AccountId,
) -> Result<PreparedNftReleaseV1, Error> {
    st.world.account(destination)?;
    ensure_nft_destination_unreserved(&st.world, destination)?;
    let custody = st
        .world
        .nft_custody_by_nft
        .get(nft_id)
        .ok_or_else(|| invalid("NFT is not reserved"))?;
    let record = st
        .world
        .nft_custody_records
        .get(custody)
        .ok_or_else(|| invalid("NFT reservation record is missing"))?
        .clone();
    let nft = st.world.nft(nft_id)?;
    if record.version != 1
        || record.network_id != *st.network_id()
        || record.reservation_id != reservation_id
        || record.purpose != purpose
        || record.nft_id != *nft_id
        || record.released_to.is_some()
        || record.custody != *custody
        || &nft.value().owned_by != custody
        || Hash::new(nft.value().content.encode()) != record.metadata_hash
    {
        return Err(invalid(
            "NFT release differs from exact retained native reservation",
        ));
    }
    let owner_refs = st
        .world
        .nft_custody_owner_refs
        .get(&record.original_owner)
        .copied()
        .unwrap_or(0)
        .checked_sub(1)
        .ok_or_else(|| invalid("NFT owner reference underflow"))?;
    let domain_refs = st
        .world
        .nft_custody_domain_refs
        .get(nft_id.domain())
        .copied()
        .unwrap_or(0)
        .checked_sub(1)
        .ok_or_else(|| invalid("NFT domain reference underflow"))?;
    Ok(PreparedNftReleaseV1 {
        record,
        owner_refs,
        domain_refs,
        destination: destination.clone(),
    })
}
impl PreparedNftReleaseV1 {
    /// Infallible application after exact preflight; accompanying operations must not mutate NFT state.
    pub(in crate::smartcontracts::isi) fn apply(mut self, st: &mut StateTransaction<'_, '_>) {
        moved(
            st,
            &self.record.nft_id,
            &self.record.custody,
            &self.destination,
        );
        st.world
            .nft_custody_by_nft
            .remove(self.record.nft_id.clone());
        if self.owner_refs == 0 {
            st.world
                .nft_custody_owner_refs
                .remove(self.record.original_owner.clone());
        } else {
            st.world
                .nft_custody_owner_refs
                .insert(self.record.original_owner.clone(), self.owner_refs);
        }
        if self.domain_refs == 0 {
            st.world
                .nft_custody_domain_refs
                .remove(self.record.nft_id.domain().clone());
        } else {
            st.world
                .nft_custody_domain_refs
                .insert(self.record.nft_id.domain().clone(), self.domain_refs);
        }
        self.record.released_to = Some(self.destination);
        st.world
            .nft_custody_records
            .insert(self.record.custody.clone(), self.record);
    }
}
/// Prepared atomic release set, with cumulative account/domain reference accounting.
pub(in crate::smartcontracts::isi) struct PreparedNftReleasesV1(Vec<PreparedNftReleaseV1>);
/// Validate every NFT before an accompanying payout changes state. Repeated NFTs are rejected.
pub(in crate::smartcontracts::isi) fn prepare_nft_releases_v1(
    st: &StateTransaction<'_, '_>,
    releases: Vec<(Hash, NftCustodyPurposeV1, NftId, AccountId)>,
) -> Result<PreparedNftReleasesV1, Error> {
    if releases.len() > iroha_data_model::game::GAME_MAX_NFT_RESERVATIONS_V1 {
        return Err(invalid("native NFT release set exceeds its combined bound"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut owners = BTreeMap::<AccountId, u32>::new();
    let mut domains = BTreeMap::<DomainId, u32>::new();
    let mut prepared = Vec::new();
    for (reservation, purpose, nft_id, destination) in releases {
        if !seen.insert(nft_id.clone()) {
            return Err(invalid("native NFT release set contains duplicate items"));
        }
        let mut release = prepare_nft_release_v1(st, reservation, purpose, &nft_id, &destination)?;
        let prior_owner = owners
            .entry(release.record.original_owner.clone())
            .or_default();
        release.owner_refs = release
            .owner_refs
            .checked_sub(*prior_owner)
            .ok_or_else(|| invalid("native NFT batch owner reference underflow"))?;
        *prior_owner += 1;
        let prior_domain = domains.entry(nft_id.domain().clone()).or_default();
        release.domain_refs = release
            .domain_refs
            .checked_sub(*prior_domain)
            .ok_or_else(|| invalid("native NFT batch domain reference underflow"))?;
        *prior_domain += 1;
        prepared.push(release);
    }
    Ok(PreparedNftReleasesV1(prepared))
}
impl PreparedNftReleasesV1 {
    /// Apply in the preflight's exact order so shared reservation reference counts remain exact.
    pub(in crate::smartcontracts::isi) fn apply(self, st: &mut StateTransaction<'_, '_>) {
        for release in self.0 {
            release.apply(st);
        }
    }
}

impl World {
    /// Rebuild only derived guards; reject inconsistent custody and sale tombstones first.
    pub(crate) fn rebuild_nft_custody_indexes(&mut self) -> Result<(), String> {
        let mut active = BTreeMap::<NftId, AccountId>::new();
        let mut owners = BTreeMap::<AccountId, u32>::new();
        let mut domains = BTreeMap::<DomainId, u32>::new();
        let records = self.nft_custody_records.view();
        let nfts = self.nfts.view();
        let accounts = self.accounts.view();
        for (custody, record) in records.iter() {
            if record.version != 1
                || custody != &record.custody
                || *custody
                    != nft_custody_account_v1(
                        &record.network_id,
                        &record.reservation_id,
                        record.purpose,
                        &record.nft_id,
                    )
                || record.original_owner == *custody
                || accounts.get(custody).is_none()
            {
                return Err("invalid native NFT custody identity".into());
            }
            if record.released_to.is_none() {
                let nft = nfts.get(&record.nft_id).ok_or("reserved NFT is missing")?;
                if nft.owned_by != *custody
                    || Hash::new(nft.content.encode()) != record.metadata_hash
                    || accounts.get(&record.original_owner).is_none()
                    || active
                        .insert(record.nft_id.clone(), custody.clone())
                        .is_some()
                {
                    return Err("inconsistent active native NFT custody".into());
                }
                let owner = owners.entry(record.original_owner.clone()).or_default();
                *owner = owner.checked_add(1).ok_or("NFT owner count overflow")?;
                let domain = domains.entry(record.nft_id.domain().clone()).or_default();
                *domain = domain.checked_add(1).ok_or("NFT domain count overflow")?;
            }
        }
        let offers = self.nft_sale_offers.view();
        for (id, sale) in offers.iter() {
            let reservation = records
                .get(&sale.custody)
                .ok_or("NFT sale custody record is missing")?;
            let expected_recipient = match &sale.status {
                NftSaleStatusV1::Open => None,
                NftSaleStatusV1::Purchased(account) => Some(account),
                NftSaleStatusV1::Cancelled => Some(&sale.offer.seller),
            };
            let invalid_purchase = match &sale.status {
                NftSaleStatusV1::Purchased(account) => {
                    account == &sale.offer.seller
                        || sale
                            .offer
                            .reserved_buyer
                            .as_ref()
                            .is_some_and(|buyer| buyer != account)
                        || sale
                            .closed_at_height
                            .is_none_or(|height| height > sale.offer.expires_at_height)
                }
                _ => false,
            };
            if sale.version != 1
                || *id != sale.offer.offer_id
                || sale.offer_hash != sale.offer.commitment()
                || sale.offer.price.is_zero()
                || sale.offer.expires_at_height <= sale.created_at_height
                || sale.offer.expires_at_height > sale.created_at_height.saturating_add(1_000_000)
                || sale.offer.reserved_buyer.as_ref() == Some(&sale.offer.seller)
                || invalid_purchase
                || sale.closed_at_height.is_some() != expected_recipient.is_some()
                || sale
                    .closed_at_height
                    .is_some_and(|height| height < sale.created_at_height)
                || reservation.network_id != sale.offer.network_id
                || reservation.reservation_id != *id
                || reservation.purpose != NftCustodyPurposeV1::Sale
                || reservation.nft_id != sale.offer.nft_id
                || reservation.original_owner != sale.offer.seller
                || reservation.metadata_hash != sale.offer.metadata_hash
                || reservation.released_to.as_ref() != expected_recipient
            {
                return Err("invalid native NFT offer terms or terminal decision".into());
            }
        }
        for (_, record) in records.iter() {
            if record.purpose == NftCustodyPurposeV1::Sale {
                let sale = offers
                    .get(&record.reservation_id)
                    .ok_or("sale custody lacks its permanent offer record")?;
                if sale.custody != record.custody || sale.offer.nft_id != record.nft_id {
                    return Err("sale custody lacks its exact permanent offer record".into());
                }
            }
            if record.purpose == NftCustodyPurposeV1::GameResource {
                let sessions = self.game_sessions.view();
                let session = sessions
                    .get(&record.reservation_id)
                    .ok_or("equipment custody lacks its retained session")?;
                if session.network_id != record.network_id
                    || session
                        .resources
                        .iter()
                        .filter(|item| {
                            item.nft_id == record.nft_id && item.custody == record.custody
                        })
                        .count()
                        != 1
                {
                    return Err("equipment custody lacks its exact unique retained resource".into());
                }
            }
            if record.purpose == NftCustodyPurposeV1::GameWager {
                let sessions = self.game_sessions.view();
                let session = sessions
                    .get(&record.reservation_id)
                    .ok_or("game NFT custody lacks its retained session")?;
                if session.network_id != record.network_id
                    || !session
                        .item_stakes
                        .iter()
                        .any(|item| item.nft_id == record.nft_id && item.custody == record.custody)
                {
                    return Err("game NFT custody lacks its exact retained item stake".into());
                }
            }
        }
        // Check forward session records as well as every custody record's inverse reference.
        for (_, session) in self.game_sessions.view().iter() {
            super::game::items::validate_restored_items(self, session)?;
            super::game::resources::validate_restored_resources(self, session)?;
        }
        drop(offers);
        drop(records);
        drop(nfts);
        drop(accounts);
        self.nft_custody_by_nft = active.into_iter().collect();
        self.nft_custody_owner_refs = owners.into_iter().collect();
        self.nft_custody_domain_refs = domains.into_iter().collect();
        Ok(())
    }
}
