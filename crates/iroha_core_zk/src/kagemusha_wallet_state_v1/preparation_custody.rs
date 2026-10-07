//! Retained originals and map drafts over the coordinator's one exclusive archive.
//!
//! Reads authenticate the selected state. Updates remain unpublished until the native
//! verifier accepts the successor and Advance selects its capsule. Signed peer objects
//! never supply local clock custody or replace a previously issued Request.

use super::{map_custody::SourceMapsV1, *};

pub(super) const SOURCE_CUSTODY_MAX_BYTES: usize = 32 * 1024;

/// Fixed local original roles. These are not caller-selected storage paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationOriginalV1 {
    /// Canonical current credential, bound to the source state's credential digest.
    CurrentCredential,
    /// Canonical certificate set containing the current credential's issuer.
    EnrollmentCertificates,
    /// Held scheme policy, or absence when the state commits zero.
    SchemePolicy,
    /// Held signed blacklist, or absence when the state commits zero.
    Blacklist,
    /// Held signed quota share, or absence when the state commits zero.
    QuotaShare,
    /// Held signed time response; this alone does not establish a direct exchange.
    TimeAnchor,
}

impl PreparationOriginalV1 {
    const ALL: [Self; 6] = [
        Self::CurrentCredential,
        Self::EnrollmentCertificates,
        Self::SchemePolicy,
        Self::Blacklist,
        Self::QuotaShare,
        Self::TimeAnchor,
    ];
    const fn index(self) -> usize {
        match self {
            Self::CurrentCredential => 0,
            Self::EnrollmentCertificates => 1,
            Self::SchemePolicy => 2,
            Self::Blacklist => 3,
            Self::QuotaShare => 4,
            Self::TimeAnchor => 5,
        }
    }
    const fn maximum(self) -> usize {
        match self {
            Self::CurrentCredential => KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1,
            Self::Blacklist => KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1,
            Self::EnrollmentCertificates => KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            Self::SchemePolicy => KAGEMUSHA_WALLET_SCHEME_POLICY_MAX_BYTES_V1,
            Self::QuotaShare => KAGEMUSHA_WALLET_QUOTA_SHARE_MAX_BYTES_V1,
            Self::TimeAnchor => KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1,
        }
    }
    fn expected(self, state: &KagemushaWalletStateV1) -> Option<[u8; 32]> {
        match self {
            Self::CurrentCredential => Some(state.core.credential_digest),
            Self::EnrollmentCertificates => None,
            Self::SchemePolicy => Some(state.rest.scheme_policy),
            Self::Blacklist => Some(state.rest.blacklist),
            Self::QuotaShare => Some(state.rest.quota_share),
            Self::TimeAnchor => Some(state.rest.time_anchor),
        }
    }
    fn digest(self, bytes: &[u8], scheme: &[u8; 32]) -> Result<Option<[u8; 32]>, Error> {
        if bytes.is_empty() || bytes.len() > self.maximum() {
            return Err(Error::WitnessLost("retained original bound"));
        }
        Ok(Some(match self {
            Self::CurrentCredential => {
                valid(KagemushaWalletCredentialV1::decode_canonical(bytes, scheme))?
                    .credential_digest()
            }
            Self::EnrollmentCertificates => {
                let set: KagemushaWalletCertificateSetV1 = archive::decode(bytes)?;
                valid(set.validate())?;
                if set.certificates.iter().any(|c| c.body.scheme_id != *scheme) {
                    return Err(Error::WitnessLost("retained certificate scheme"));
                }
                return Ok(None);
            }
            Self::SchemePolicy => valid(KagemushaWalletSchemePolicyV1::decode_canonical(
                bytes, scheme,
            ))?
            .scheme_policy_digest(),
            Self::Blacklist => valid(KagemushaWalletBlacklistV1::decode_canonical(bytes, scheme))?
                .blacklist_digest(),
            Self::QuotaShare => {
                valid(KagemushaWalletQuotaShareV1::decode_canonical(bytes, scheme))?
                    .quota_share_digest()
            }
            Self::TimeAnchor => {
                valid(KagemushaWalletTimeAnchorV1::decode_canonical(bytes, scheme))?
                    .time_anchor_digest()
            }
        }))
    }
}

#[derive(Debug, Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::SourceCustodyV1")]
pub(super) struct SourceCustodyV1 {
    version: u16,
    pub(super) maps: SourceMapsV1,
    originals: [Option<[u8; 32]>; 6],
}

impl SourceCustodyV1 {
    /// Called only by the native enrollment owner after issuer admission.
    pub(super) fn bootstrap(
        store: &mut dyn ObjectStore,
        state: &KagemushaWalletStateV1,
        credential: &[u8],
        certificates: &[u8],
    ) -> Result<Self, Error> {
        let mut value = Self {
            version: 1,
            maps: SourceMapsV1::default(),
            originals: [None; 6],
        };
        value.maps.require(state)?;
        for (role, bytes) in [
            (PreparationOriginalV1::CurrentCredential, credential),
            (PreparationOriginalV1::EnrollmentCertificates, certificates),
        ] {
            role.digest(bytes, &state.core.scheme_id)?;
            value.originals[role.index()] = Some(store.write_object(bytes, role.maximum())?);
        }
        value.require(store, state)?;
        Ok(value)
    }

    pub(super) fn original(
        &self,
        store: &mut dyn ObjectStore,
        state: &KagemushaWalletStateV1,
        role: PreparationOriginalV1,
    ) -> Result<Option<Vec<u8>>, Error> {
        let expected = role.expected(state);
        let address = self.originals[role.index()];
        if expected == Some([0; 32]) {
            return if address.is_none() {
                Ok(None)
            } else {
                Err(Error::WitnessLost("unheld original is populated"))
            };
        }
        let address = address.ok_or(Error::WitnessLost("required original address"))?;
        let bytes = store.read_object(&address, role.maximum())?;
        if role
            .digest(&bytes, &state.core.scheme_id)
            .map_err(|_| Error::WitnessLost("retained original encoding"))?
            != expected
        {
            return Err(Error::WitnessLost("original state digest"));
        }
        Ok(Some(bytes))
    }

    pub(super) fn require(
        &self,
        store: &mut dyn ObjectStore,
        state: &KagemushaWalletStateV1,
    ) -> Result<(), Error> {
        if self.version != 1 {
            return Err(Error::WitnessLost("source custody version"));
        }
        self.maps.require(state)?;
        self.require_originals(store, state)
    }

    fn require_originals(
        &self,
        store: &mut dyn ObjectStore,
        state: &KagemushaWalletStateV1,
    ) -> Result<(), Error> {
        for role in PreparationOriginalV1::ALL {
            self.original(store, state, role)?;
        }
        let credential: KagemushaWalletCredentialV1 = archive::decode(
            &self
                .original(store, state, PreparationOriginalV1::CurrentCredential)?
                .ok_or(Error::WitnessLost("current credential"))?,
        )?;
        let certificates: KagemushaWalletCertificateSetV1 = archive::decode(
            &self
                .original(store, state, PreparationOriginalV1::EnrollmentCertificates)?
                .ok_or(Error::WitnessLost("current issuer certificates"))?,
        )?;
        if !certificates
            .certificates
            .iter()
            .any(|c| c.certificate_digest() == credential.body.issuer_certificate)
        {
            return Err(Error::WitnessLost("current credential issuer original"));
        }
        valid(state.validate_for_credential(&credential))?;
        Ok(())
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::IssuedRequestCustodyV1")]
pub(super) struct IssuedRequestCustodyV1 {
    pub(super) request: [u8; 32],
    pub(super) gap: Option<[u8; 32]>,
}

// The object-store adapter avoids exposing a second archive or changing IndexRoot's
// bounded generic implementation merely to accept this internally erased store.
struct Store<'a>(&'a mut dyn ObjectStore);
impl ObjectStore for Store<'_> {
    fn read_object(&mut self, key: &[u8; 32], maximum: usize) -> Result<Vec<u8>, Error> {
        self.0.read_object(key, maximum)
    }
    fn write_object(&mut self, bytes: &[u8], maximum: usize) -> Result<[u8; 32], Error> {
        self.0.write_object(bytes, maximum)
    }
}

/// Native preparation's sealed view of source-selected originals and unpublished maps.
/// Every construction is coordinator-owned; neither roots nor archive selection are public.
pub struct PreparationCustodyV1<'a> {
    maps: PreparationMapsV1<'a>,
    source: SourceCustodyV1,
    state: KagemushaWalletStateV1,
    originals: [Option<[u8; 32]>; 6],
    updated: [bool; 6],
    issued: IndexRoot,
    anchors: IndexRoot,
    refresh: Option<KagemushaWalletPolicyUpdateKindV1>,
}

impl<'a> PreparationCustodyV1<'a> {
    pub(super) fn snapshot(&self) -> SourceCustodyV1 {
        SourceCustodyV1 {
            version: 1,
            maps: self.maps.snapshot(),
            originals: self.originals,
        }
    }
    pub(super) fn new(
        store: &'a mut dyn ObjectStore,
        selected: &SourceCustodyV1,
        state: &KagemushaWalletStateV1,
        kind: KagemushaWalletOperationKindV1,
        refresh: Option<KagemushaWalletPolicyUpdateKindV1>,
        issued: IndexRoot,
        anchors: IndexRoot,
    ) -> Result<Self, Error> {
        selected.require(store, state)?;
        let maps = PreparationMapsV1::new(store, &selected.maps, state, kind, refresh)?;
        Ok(Self {
            maps,
            source: selected.clone(),
            state: *state,
            originals: selected.originals,
            updated: [false; 6],
            issued,
            anchors,
            refresh,
        })
    }

    /// Access only this operation's unpublished, source-rooted map draft.
    pub fn maps(&mut self) -> &mut PreparationMapsV1<'a> {
        &mut self.maps
    }

    /// Read the exact selected original. Only a state-committed zero means absence.
    ///
    /// # Errors
    /// Missing, corrupt or foreign originals are custody loss; unavailable reads remain errors.
    pub fn original(&mut self, role: PreparationOriginalV1) -> Result<Option<Vec<u8>>, Error> {
        self.source.original(self.maps.store(), &self.state, role)
    }

    /// Retain one successor original for the selected signed Refresh class.
    /// Repeated identical retention is idempotent; conflicting bytes reject. Source reads
    /// continue to return the predecessor originals. Full successor binding is checked at finish.
    ///
    /// # Errors
    /// Wrong update role, malformed bytes, conflicting retention or storage failure.
    pub fn retain_successor_original(
        &mut self,
        role: PreparationOriginalV1,
        bytes: &[u8],
    ) -> Result<(), Error> {
        use KagemushaWalletPolicyUpdateKindV1 as K;
        use PreparationOriginalV1 as R;
        let allowed = matches!(
            (self.refresh, role),
            (
                Some(K::Credential),
                R::CurrentCredential | R::EnrollmentCertificates
            ) | (Some(K::SchemePolicy), R::SchemePolicy)
                | (Some(K::Blacklist), R::Blacklist)
                | (Some(K::QuotaShare), R::QuotaShare)
                | (Some(K::TimeAnchor), R::TimeAnchor)
        );
        if !allowed {
            return Err(Error::Invalid("successor original role"));
        }
        role.digest(bytes, &self.state.core.scheme_id)?;
        let address = self.maps.store().write_object(bytes, role.maximum())?;
        if self.updated[role.index()] && self.originals[role.index()] != Some(address) {
            return Err(Error::Invalid("conflicting successor original"));
        }
        self.originals[role.index()] = Some(address);
        self.updated[role.index()] = true;
        Ok(())
    }

    fn request_record(
        &mut self,
        digest: &[u8; 32],
    ) -> Result<(KagemushaWalletRequestV1, Vec<u8>, IssuedRequestCustodyV1), Error> {
        let value = self
            .issued
            .get(&mut Store(self.maps.store()), digest)?
            .ok_or(Error::WitnessLost("issued Request custody"))?;
        let record: IssuedRequestCustodyV1 = archive::decode(&value)?;
        let bytes = self
            .maps
            .store()
            .read_object(&record.request, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        let request: KagemushaWalletRequestV1 = archive::decode(&bytes)?;
        valid(request.validate())?;
        if request.request_digest() != *digest
            || request.body.scheme_id != self.state.core.scheme_id
            || request.receiver_credential.body.wallet_id != self.state.core.wallet_id
        {
            return Err(Error::WitnessLost("issued Request binding"));
        }
        if (request.body.receiver_blacklist_version != 0) != record.gap.is_some() {
            return Err(Error::WitnessLost("issued Request gap presence"));
        }
        Ok((request, bytes, record))
    }

    /// Recover the exact locally issued Request, never a peer substitute.
    ///
    /// # Errors
    /// Missing/corrupt original or index, foreign identity or storage unavailability.
    pub fn issued_request(&mut self, digest: &[u8; 32]) -> Result<Vec<u8>, Error> {
        Ok(self.request_record(digest)?.1)
    }

    /// Authenticate the saved gap under the Request's recorded root and payer identity.
    ///
    /// # Errors
    /// Required gap absent, malformed transcript, mismatching historical root or unavailable storage.
    pub fn issued_request_gap(
        &mut self,
        digest: &[u8; 32],
    ) -> Result<Option<KagemushaWalletBlacklistGapOpeningV1>, Error> {
        let (request, _, record) = self.request_record(digest)?;
        record
            .gap
            .map(|address| {
                let bytes = self.maps.store().read_object(
                    &address,
                    KAGEMUSHA_WALLET_BLACKLIST_GAP_OPENING_TRANSCRIPT_BYTES_V1,
                )?;
                let opening = valid(KagemushaWalletBlacklistGapOpeningV1::from_transcript(
                    &bytes,
                ))?;
                valid(opening.verify(
                    &request.body.receiver_blacklist_root,
                    &request.body.payer_account_digest,
                ))?;
                Ok(opening)
            })
            .transpose()
    }

    /// Recover native direct-exchange observations for the exact committed anchor.
    /// A signed TimeAnchor or Refresh by itself never populates this index.
    ///
    /// # Errors
    /// A committed anchor without direct-exchange custody, corrupt binding or storage failure.
    pub fn anchored_time(&mut self) -> Result<Option<KagemushaWalletAnchoredTimeV1>, Error> {
        if self.state.rest.time_anchor == [0; 32] {
            return Ok(None);
        }
        let address = self
            .anchors
            .get(&mut Store(self.maps.store()), &self.state.rest.time_anchor)?
            .ok_or(Error::WitnessLost("direct time exchange"))?;
        let address: [u8; 32] = address
            .try_into()
            .map_err(|_| Error::WitnessLost("direct time address"))?;
        let bytes = self
            .maps
            .store()
            .read_object(&address, KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1 + 1024)?;
        let anchored: KagemushaWalletAnchoredTimeV1 = archive::decode(&bytes)?;
        valid(anchored.validate(self.state.core.time_anchor_max_response_ms))?;
        if anchored.anchor.time_anchor_digest() != self.state.rest.time_anchor
            || anchored.anchor.body.scheme_id != self.state.core.scheme_id
        {
            return Err(Error::WitnessLost("direct anchor binding"));
        }
        Ok(Some(anchored))
    }

    pub(super) fn finish(
        mut self,
        successor: &KagemushaWalletStateV1,
    ) -> Result<SourceCustodyV1, Error> {
        // Authenticate originals while the one archive borrow is still available.
        let mut value = self.source;
        value.originals = self.originals;
        // Maps are checked after consuming the draft; original checks use their own roots.
        value.require_originals(self.maps.store(), successor)?;
        value.maps = self.maps.finish(successor)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
