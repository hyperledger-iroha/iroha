//! Pre-Advance Load preparation from an exact source-qualified ordinary receipt.
//!
//! Local preparation can extend an unfolded head. It verifies ordinary finality
//! before deriving the state and preserves the exact receipt/proof bytes for A.
//! It creates no folded state or permission to spend before the required fold.
//! TODO: qualify the public preparation path with the complete first ordinary
//! receipt proof and installed producer catalog; local derivation tests do not do so.

use crate::kagemusha_wallet_artifacts_v1::producer_inventory::QualifiedReceiptSourceV1;

use super::*;

/// Exact ordinary finality originals and the current recovery-map insertion.
#[derive(Clone, Copy)]
pub struct LoadOriginalsV1<'a> {
    /// Canonical original ordinary receipt, including its original online charge.
    pub receipt: &'a [u8],
    /// Canonical compact proof under the independently installed global root.
    pub finality: &'a [u8],
    /// Actual low-leaf and intermediate empty-slot openings for this receipt.
    pub insertion: &'a KagemushaWalletIndexedInsertV1,
}

/// Authenticated local Load proposal; only Native custody can commit its Advance.
pub struct LoadStepV1 {
    manifest_digest: [u8; 32],
    source_capsule_digest: [u8; 32],
    witness: LoadWitness,
    state: KagemushaWalletStateV1,
    statement: KagemushaWalletStatementV1,
    retained: [KagemushaWalletRetainedInputV1; 2],
    openings: [Vec<u8>; 2],
}
impl LoadStepV1 {
    /// Exact local sigma witness; its private projection is not Omega evidence.
    pub const fn witness(&self) -> &LoadWitness {
        &self.witness
    }
    /// Fully derived successor, adding only the receipt's net offline amount.
    pub const fn state(&self) -> &KagemushaWalletStateV1 {
        &self.state
    }
    /// Exact statement to prove and retain through Advance.
    pub const fn statement(&self) -> &KagemushaWalletStatementV1 {
        &self.statement
    }
    /// Original receipt/finality frames and the two authenticated insertion paths.
    pub fn originals(&self) -> (&[KagemushaWalletRetainedInputV1; 2], &[Vec<u8>; 2]) {
        (&self.retained, &self.openings)
    }
    /// Authenticated installation to recheck before proving and commit.
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.manifest_digest
    }
    /// Exact released source to recheck under Native's exclusive head lock.
    pub const fn source_capsule_digest(&self) -> [u8; 32] {
        self.source_capsule_digest
    }
}

// Arithmetic/map derivation only. The public entry point first verifies the exact
// retained receipt proof and current released source; this helper admits neither.
fn derive(
    credential: &KagemushaWalletCredentialV1,
    before: &KagemushaWalletStateV1,
    receipt: &KagemushaWalletLoadReceiptV1,
    insertion: &KagemushaWalletIndexedInsertV1,
    nonce: [u8; 32],
    relation: [u8; 32],
    omega: [u8; 32],
) -> Result<
    (
        KagemushaWalletStateV1,
        KagemushaWalletStatementV1,
        LoadWitness,
    ),
    Error,
> {
    authority(before.validate_for_credential(credential))?;
    authority(receipt.validate())?;
    if receipt.scheme_id != before.core.scheme_id
        || receipt.asset_digest != before.core.asset_digest
        || receipt.wallet_id != before.core.wallet_id
        || receipt.ordinal != before.core.next_load
    {
        return Err(Error::Authority);
    }
    let digest = authority(receipt.receipt_digest())?;
    let leaf = KagemushaWalletLoadLeafV1 {
        ordinal: receipt.ordinal,
        receipt_digest: digest,
        amount: receipt.amount,
    };
    let root = authority(insertion.verify(
        &before.core.load_redeem_recovery_root,
        &leaf.key(),
        &authority(leaf.leaf_value())?,
    ))?;
    let mut state = before.clone();
    state.core.sequence = before
        .core
        .sequence
        .checked_add(1)
        .ok_or(Error::Authority)?;
    state.core.next_load = receipt.ordinal.checked_add(1).ok_or(Error::Authority)?;
    state.core.balance = before
        .core
        .balance
        .checked_add(receipt.amount)
        .ok_or(Error::Authority)?;
    state.core.load_redeem_recovery_root = root;
    state.core.state_nonce = nonce;
    authority(state.validate_for_credential(credential))?;
    let statement = KagemushaWalletStatementV1 {
        version: KAGEMUSHA_WALLET_VERSION_V1,
        scheme_id: before.core.scheme_id,
        relation_id: relation,
        credential_digest: state.core.credential_digest,
        asset_digest: before.core.asset_digest,
        lifecycle: state.core.lifecycle,
        sequence: state.core.sequence,
        next_load: state.core.next_load,
        enabled_controls: before.core.enabled_controls,
        lineage_burned_total: 0,
        lineage_pending_outgoing_root: [0; 32],
        predecessor: authority(before.commitment())?,
        successor: authority(state.commitment())?,
        effect: KagemushaWalletEffectV1::Load {
            receipt_digest: digest,
            load_ordinal: receipt.ordinal,
            amount: receipt.amount,
            online_charge: receipt.online_charge,
        },
    };
    let witness = LoadWitness {
        predecessor: unfolded::local_state(credential, before, relation, omega)?,
        successor: unfolded::local_state(credential, &state, relation, omega)?,
        statement: fields(authority(statement.field_items())?)?,
    };
    Ok((state, statement, witness))
}

impl PreparationV1<'_> {
    /// Verify ordinary receipt finality, then derive a local Load from the released head.
    /// A current-head Omega is not required. The qualified receipt source must belong
    /// to this exact installation; the caller cannot replace its root, key or endpoints.
    /// Both transported curves and the complete receipt proof must verify before any
    /// prepared result is returned. Native still rechecks the source under its commit lock.
    ///
    /// # Errors
    /// Foreign installation, invalid source/receipt/finality, wrong wallet or ordinal,
    /// reused/nonempty recovery slot, malformed original, overflow or invalid nonce.
    pub fn prepare_load(
        &self,
        owner: &AuthenticatedCredentialV1,
        source: &ReleasedStep,
        finality: &QualifiedReceiptSourceV1,
        originals: LoadOriginalsV1<'_>,
        successor_nonce: [u8; 32],
        budget: MemoryBudget,
    ) -> Result<LoadStepV1, Error> {
        self.credential_owner(owner)?;
        let scheme = self.installed.verifier().scheme();
        if finality.installation() != (scheme.scheme_id(), owner.manifest_digest) {
            return Err(Error::Authority);
        }
        self.receipt_tape(owner, source, budget)?;
        let receipt = authority(KagemushaWalletLoadReceiptV1::decode_canonical(
            originals.receipt,
        ))?;
        // Bound both originals before retaining/copying untrusted network bytes.
        authority(KagemushaWalletLoadFinalityV1::decode_canonical(
            originals.finality,
        ))?;
        let retained = [
            KagemushaWalletRetainedInputV1 {
                role: KagemushaWalletRetainedInputRoleV1::LoadReceipt,
                bytes: originals.receipt.to_vec(),
            },
            KagemushaWalletRetainedInputV1 {
                role: KagemushaWalletRetainedInputRoleV1::LoadFinality,
                bytes: originals.finality.to_vec(),
            },
        ];
        let digest = authority(receipt.receipt_digest())?;
        let (_, evidence) = retained_load_source(&retained, finality.anchor().digest(), digest)?;
        finality
            .verify_receipt_evidence(fields::<1>(vec![digest])?[0], &evidence, budget)
            .map_err(|_| Error::Proof)?;
        let capsule = &source.frozen.capsule;
        let (state, statement, witness) = derive(
            &owner.credential,
            &capsule.successor_state,
            &receipt,
            originals.insertion,
            successor_nonce,
            scheme.relation_id,
            self.omega_key_digest,
        )?;
        self.statement_fields(owner, &statement)?;
        authority(statement.validate_successor_of(&capsule.statement))?;
        Ok(LoadStepV1 {
            manifest_digest: owner.manifest_digest,
            source_capsule_digest: authority(capsule.capsule_digest())?,
            witness,
            state,
            statement,
            retained,
            openings: [
                originals
                    .insertion
                    .low_opening
                    .leaf_transcript(&originals.insertion.low),
                originals.insertion.slot_opening.empty_transcript(),
            ],
        })
    }
}

#[cfg(test)]
#[path = "load/tests.rs"]
mod tests;
