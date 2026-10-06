//! Separate earned-fee Payment custody and exact finalized-payout acknowledgements.
use super::*;
use crate::kagemusha_wallet_advance_v1::kagemusha_wallet_provider_digest_v1 as digest;
use iroha_data_model::sumeragi_finality::{VerifiedSumeragiBlock, VerifiedWorldStateSnapshotV1};

/// Opaque native finality and authenticated World evidence for one immutable payout row.
/// The wallet additionally checks its own artifact-selected scheme and Global chain scope.
/// A transaction submission result or a server acknowledgement cannot construct this evidence.
pub struct FinalizedPayoutEvidence<'a> {
    /// Certified native successor on the selected Global chain.
    pub block: &'a VerifiedSumeragiBlock,
    /// Complete World snapshot already authenticated against that exact successor.
    pub world: &'a VerifiedWorldStateSnapshotV1,
    /// Original immutable payout row, authenticated below by its native table key/value.
    pub payout: KagemushaWalletPayoutRecordV1,
}

/// Canonical originals retained independently of delivery and fold witnesses until fee payout.
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::RetainedFeeClaim")]
pub struct RetainedFeeClaim {
    /// Exact committed Payment bytes used by the online fee claim.
    pub payment: Vec<u8>,
    /// Complete original Request, including historical receiver credential, fee schedule and
    /// certificates, so it can still be relayed to the ledger's immutable historical records.
    pub request: Vec<u8>,
}
const CLAIM_LIMIT: usize = KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
    + KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1
    + archive::METADATA_BOUND;

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::FeeClaimEntry")]
struct FeeClaimEntry {
    payment: [u8; 32],
    content: [u8; 32],
    fee: u128,
    payout: Option<KagemushaWalletPayoutRecordV1>,
}

fn verify_payout(
    scheme: &KagemushaWalletSchemeV1,
    chain: &str,
    credit: [u8; 32],
    claim: &FeeClaimEntry,
    evidence: &FinalizedPayoutEvidence<'_>,
) -> Result<(), Error> {
    valid(scheme.validate())?;
    let network = evidence.block.commitment().schedule.current.network_id;
    if network.as_bytes() != &scheme.network_id
        || evidence.world.height() != evidence.block.height()
        || evidence.world.context_id() != evidence.block.context_id()
        || evidence.world.world_root() != evidence.block.execution().world_state_root
        || evidence.payout.key != KagemushaWalletPayoutKeyV1::Fee(credit)
        || evidence.payout.source != claim.payment
        || evidence.payout.amount != claim.fee
        || claim.fee == 0
        || evidence.payout.transaction == [0; 32]
    {
        return Err(Error::Invalid("finalized fee payout binding"));
    }
    evidence
        .block
        .verify_global_scope(network, chain)
        .map_err(|_| Error::Invalid("finalized fee payout scope"))?;
    // Core records the canonical framed row as Vec<u8>; authenticate that exact semantic
    // value, not the payout structure under a different snapshot schema.
    let bytes =
        norito::to_bytes(&evidence.payout).map_err(|_| Error::Invalid("payout encoding"))?;
    evidence
        .world
        .verify_table_value(
            "world.kagemusha_wallet_ledger",
            &evidence.payout.key.ledger_key(scheme.scheme_id()),
            &bytes,
        )
        .map_err(|_| Error::Invalid("finalized fee payout row"))
}

impl<C: Custody, A: ArchiveStore, N: NativeProofs> Coordinator<C, A, N> {
    pub(super) fn retain_fee_claim(
        &mut self,
        manifest: &mut manifest::Manifest,
        step: &ReleasedStep,
    ) -> Result<(), Error> {
        if !matches!(
            step.frozen.capsule.statement.effect,
            KagemushaWalletEffectV1::Send { .. }
        ) {
            return Ok(());
        }
        let bytes = &step.retained.record.output;
        let payment = valid(KagemushaWalletPaymentV1::decode_canonical(
            bytes,
            &self.scheme_id,
        ))?;
        let fee = payment.request.body.fee;
        if fee == 0 {
            return Ok(());
        }
        let digests = valid(payment.digests())?;
        if manifest
            .claims
            .get(&mut self.archive, &digests.credit_id)?
            .is_some()
        {
            return Err(Error::WitnessLost("duplicate committed fee claim"));
        }
        let request = step
            .frozen
            .capsule
            .retained_inputs
            .iter()
            .find(|input| input.role == KagemushaWalletRetainedInputRoleV1::Request)
            .ok_or(Error::WitnessLost("fee Request original"))?;
        let original = RetainedFeeClaim {
            payment: bytes.clone(),
            request: request.bytes.clone(),
        };
        Self::check_claim_original(
            &original,
            &self.scheme_id,
            &digests.credit_id,
            &digests.payment,
            fee,
        )?;
        let bytes = archive::encode(&original)?;
        if bytes.len() > CLAIM_LIMIT {
            return Err(Error::Invalid("fee claim size"));
        }
        let claim = FeeClaimEntry {
            payment: digests.payment,
            content: digest("wallet-fee-claim", &bytes),
            fee,
            payout: None,
        };
        self.archive
            .put(ArchiveKey::FeeClaim(digests.credit_id), &bytes)?;
        manifest.claims = manifest.claims.set(
            &mut self.archive,
            digests.credit_id,
            &archive::encode(&claim)?,
        )?;
        Ok(())
    }

    pub(super) fn require_fee_claim(
        &mut self,
        manifest: &manifest::Manifest,
        credit: [u8; 32],
        expected: &[u8],
    ) -> Result<(), Error> {
        let bytes = manifest
            .claims
            .get(&mut self.archive, &credit)?
            .ok_or(Error::WitnessLost("earned fee index"))?;
        let claim: FeeClaimEntry = archive::decode(&bytes)?;
        let payment = valid(KagemushaWalletPaymentV1::decode_canonical(
            expected,
            &self.scheme_id,
        ))?;
        if valid(payment.digests())?.payment != claim.payment {
            return Err(Error::WitnessLost("fee claim source"));
        }
        if claim.payout.is_none() && self.read_fee_claim(&claim, credit)?.payment != expected {
            return Err(Error::WitnessLost("fee claim source"));
        }
        Ok(())
    }
    fn check_claim_original(
        original: &RetainedFeeClaim,
        scheme: &[u8; 32],
        credit: &[u8; 32],
        expected: &[u8; 32],
        fee: u128,
    ) -> Result<(), Error> {
        if original.payment.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1
            || original.request.len() > KAGEMUSHA_WALLET_SESSION_MAX_BYTES_V1
        {
            return Err(Error::WitnessLost("earned fee originals size"));
        }
        let payment = valid(KagemushaWalletPaymentV1::decode_canonical(
            &original.payment,
            scheme,
        ))?;
        let request: KagemushaWalletRequestV1 = archive::decode(&original.request)?;
        valid(request.validate())?;
        let identity = valid(payment.digests())?;
        if request.body.scheme_id != *scheme
            || request.signed() != payment.request
            || identity.payment != *expected
            || identity.credit_id != *credit
            || payment.request.body.fee != fee
        {
            return Err(Error::WitnessLost("earned fee originals binding"));
        }
        Ok(())
    }
    fn read_fee_claim(
        &mut self,
        claim: &FeeClaimEntry,
        credit: [u8; 32],
    ) -> Result<RetainedFeeClaim, Error> {
        let bytes = self
            .archive
            .get(ArchiveKey::FeeClaim(credit), CLAIM_LIMIT)?
            .ok_or(Error::WitnessLost("earned fee originals"))?;
        if digest("wallet-fee-claim", &bytes) != claim.content {
            return Err(Error::WitnessLost("earned fee originals digest"));
        }
        let original: RetainedFeeClaim = archive::decode(&bytes)?;
        Self::check_claim_original(
            &original,
            &self.scheme_id,
            &credit,
            &claim.payment,
            claim.fee,
        )?;
        Ok(original)
    }
    /// Return original Payment and Request bytes retained separately for the earned fee.
    /// `None` means no earned claim is indexed or its exact payout is durably acknowledged.
    ///
    /// # Errors
    /// Missing, changed or unavailable pending-claim bytes are custody loss, not an acknowledgement.
    pub fn fee_claim(&mut self, credit: [u8; 32]) -> Result<Option<RetainedFeeClaim>, Error> {
        let (_, manifest) = self.sync_manifest()?;
        let Some(entry) = manifest.claims.get(&mut self.archive, &credit)? else {
            return Ok(None);
        };
        let claim: FeeClaimEntry = archive::decode(&entry)?;
        if claim.payout.is_some() {
            return Ok(None);
        }
        self.read_fee_claim(&claim, credit).map(Some)
    }

    /// Record an exact finalized fee payout before deleting either separate Payment copy.
    /// Every retry re-verifies the evidence; an unknown storage result never permits changing
    /// the payout or dropping unpaid claim bytes. This does not collect Send delivery data.
    ///
    /// # Errors
    /// Unknown claims, foreign/mutated finality, conflicting payouts or unavailable custody.
    pub fn acknowledge_fee_payout(
        &mut self,
        credit: [u8; 32],
        evidence: &FinalizedPayoutEvidence<'_>,
    ) -> Result<(), Error> {
        let (old, mut manifest) = self.sync_manifest()?;
        let bytes = manifest
            .claims
            .get(&mut self.archive, &credit)?
            .ok_or(Error::Invalid("unknown fee claim"))?;
        let mut claim: FeeClaimEntry = archive::decode(&bytes)?;
        let (scheme, chain) = self.proofs.ledger_scope()?;
        if scheme.scheme_id() != self.scheme_id {
            return Err(Error::Invalid("native artifact ledger scope"));
        }
        verify_payout(&scheme, &chain, credit, &claim, evidence)?;
        if claim.payout.is_some_and(|old| old != evidence.payout) {
            return Err(Error::Invalid("conflicting finalized fee payout"));
        }
        if claim.payout.is_none() {
            claim.payout = Some(evidence.payout);
            manifest.claims =
                manifest
                    .claims
                    .set(&mut self.archive, credit, &archive::encode(&claim)?)?;
            self.publish_manifest(old, &manifest)?;
        }
        self.archive.remove(ArchiveKey::FeeClaim(credit))
    }
}

#[cfg(test)]
mod tests;
