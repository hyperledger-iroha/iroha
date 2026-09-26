//! Durable experimental mint-credit ledger fed only by verified testnet value admissions.
//!
//! This ledger counts finalized top-up credits in one signed release/network/asset/reserve
//! scope. It neither authorizes offline payment nor enters production monetary state. On
//! recovery, every retained credit is rederived from the durable proof-observation owner;
//! the ledger's own WAL is never treated as proof of issuance.

use std::{collections::BTreeMap, path::Path};

use iroha_data_model::isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1;

use super::{
    DigestV1, KagemushaRecursionErrorV1, KagemushaTestnetProofObservationOwnerV1,
    KagemushaTestnetStateObservationScopeV1, KagemushaTestnetValueAdmissionV1,
};
use crate::zk::kagemusha_v1_state::{PrivateJournal, PrivateJournalError, PrivateJournalFormat};

const RECORD_MAX_BYTES: usize = 1024;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "testnet-value-ledger.norito.wal",
    magic: b"IKGTVL1\0",
    hash_domain: b"iroha:kagemusha:v1:testnet-value-ledger-disk:frame\0",
    maximum_payload_bytes: RECORD_MAX_BYTES as u64,
};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_recursion::testnet_value_ledger::Record")]
enum Record {
    Initialize {
        scope: KagemushaTestnetStateObservationScopeV1,
    },
    Credit(CreditFacts),
}

/// Canonical disk facts; these bytes never act as a credit capability by themselves.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Decode, norito::Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_recursion::testnet_value_ledger::CreditFacts")]
struct CreditFacts {
    scope: KagemushaTestnetStateObservationScopeV1,
    operation_id: DigestV1,
    credit_id: DigestV1,
    amount: u128,
    mint_envelope_digest: DigestV1,
    candidate_envelope_digest: DigestV1,
    successor_state_commitment: DigestV1,
    finality_network_id: DigestV1,
    finality_block_height: u64,
    finality_height_context_id: DigestV1,
}

impl CreditFacts {
    fn from_admission(
        admission: &KagemushaTestnetValueAdmissionV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let anchor: KagemushaFinalityTrustAnchorV1 = admission.finality_anchor();
        anchor
            .validate()
            .map_err(|error| ledger_error(&format!("invalid finality anchor: {error}")))?;
        let facts = Self {
            scope: admission.scope(),
            operation_id: admission.operation_id(),
            credit_id: admission.credit_id(),
            amount: admission.amount(),
            mint_envelope_digest: admission.mint_envelope_digest(),
            candidate_envelope_digest: admission.candidate_envelope_digest(),
            successor_state_commitment: admission.successor_state_commitment(),
            finality_network_id: *anchor.network_id.as_bytes(),
            finality_block_height: anchor.block_height,
            finality_height_context_id: *anchor.height_context_id.0.as_ref(),
        };
        facts.validate()?;
        Ok(facts)
    }

    fn validate(self) -> Result<(), KagemushaRecursionErrorV1> {
        if self.operation_id == [0; 32]
            || self.credit_id == [0; 32]
            || self.amount == 0
            || self.mint_envelope_digest == [0; 32]
            || self.candidate_envelope_digest == [0; 32]
            || self.successor_state_commitment == [0; 32]
            || self.finality_network_id != self.scope.network_id()
            || self.finality_block_height == 0
            || self.finality_height_context_id == [0; 32]
        {
            return Err(ledger_error("invalid testnet mint-credit facts"));
        }
        Ok(())
    }
}

/// A counted experimental mint credit, with no production or offline-spend authority.
///
/// The record can be copied for inspection. A wallet must not treat it as a payment credential;
/// it only names an exact finalized top-up counted by this durable testnet ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaTestnetMintLedgerCreditV1 {
    facts: CreditFacts,
}

impl KagemushaTestnetMintLedgerCreditV1 {
    /// Return the exact signed release/network/asset/reserve scope.
    #[must_use]
    pub const fn scope(self) -> KagemushaTestnetStateObservationScopeV1 {
        self.facts.scope
    }

    /// Return the finalized top-up operation ID.
    #[must_use]
    pub const fn operation_id(self) -> DigestV1 {
        self.facts.operation_id
    }

    /// Return the unique proof-bound credit ID.
    #[must_use]
    pub const fn credit_id(self) -> DigestV1 {
        self.facts.credit_id
    }

    /// Return positive atomic value admitted in this scope.
    #[must_use]
    pub const fn amount(self) -> u128 {
        self.facts.amount
    }
}

/// Exclusive, append-only testnet mint-credit registry for one authenticated release scope.
///
/// The proof owner must use its own distinct durable journal. The ledger rejects changed
/// evidence and duplicate credit IDs, and acknowledges a new credit only after its private
/// journal append is durable. Its WAL detects malformed/changed frames but cannot detect
/// rollback of an older complete directory without an external trusted checkpoint.
pub struct KagemushaTestnetMintCreditLedgerV1 {
    scope: KagemushaTestnetStateObservationScopeV1,
    wal: PrivateJournal,
    by_operation: BTreeMap<DigestV1, KagemushaTestnetMintLedgerCreditV1>,
    credit_owners: BTreeMap<DigestV1, DigestV1>,
    total_admitted: u128,
}

impl KagemushaTestnetMintCreditLedgerV1 {
    /// Create an empty private ledger for the release-authenticated observation owner's scope.
    ///
    /// # Errors
    /// Rejects an existing or unsafe path, uncertain storage, or failed durable initialization.
    pub fn create_new(
        path: &Path,
        owner: &KagemushaTestnetProofObservationOwnerV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        Self::create_new_with_scope(path, owner.scope())
    }

    fn create_new_with_scope(
        path: &Path,
        scope: KagemushaTestnetStateObservationScopeV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let mut wal = PrivateJournal::create_new(path, FORMAT).map_err(storage_error)?;
        wal.append(&encode_record(&Record::Initialize { scope })?)
            .map_err(storage_error)?;
        Ok(Self {
            scope,
            wal,
            by_operation: BTreeMap::new(),
            credit_owners: BTreeMap::new(),
            total_admitted: 0,
        })
    }

    /// Recover only credits that the durable proof owner rederives from exact original evidence.
    ///
    /// The caller must first recover the proof owner's private journal with independent finality
    /// anchors. A copied FFI archive or this ledger's WAL alone cannot satisfy recovery.
    ///
    /// # Errors
    /// Rejects scope mismatch, malformed or duplicate disk records, missing or changed source
    /// proofs, uncertain storage, or arithmetic overflow.
    pub fn open_existing(
        path: &Path,
        owner: &KagemushaTestnetProofObservationOwnerV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        Self::open_existing_with(path, owner.scope(), |operation_id| {
            owner.admit_finalized_testnet_value(operation_id)
        })
    }

    fn open_existing_with(
        path: &Path,
        scope: KagemushaTestnetStateObservationScopeV1,
        mut rederive: impl FnMut(
            DigestV1,
        )
            -> Result<KagemushaTestnetValueAdmissionV1, KagemushaRecursionErrorV1>,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let mut wal = PrivateJournal::open_existing(path, FORMAT).map_err(storage_error)?;
        let Some((0, first)) = wal.replay_next().map_err(storage_error)? else {
            return Err(ledger_error("missing testnet ledger initialization"));
        };
        if decode_record(&first)? != (Record::Initialize { scope }) {
            return Err(ledger_error(
                "testnet ledger scope differs from trusted proof owner",
            ));
        }
        let mut ledger = Self {
            scope,
            wal,
            by_operation: BTreeMap::new(),
            credit_owners: BTreeMap::new(),
            total_admitted: 0,
        };
        let mut expected_sequence = 1_u64;
        while let Some((sequence, bytes)) = ledger.wal.replay_next().map_err(storage_error)? {
            if sequence != expected_sequence {
                return Err(ledger_error("testnet ledger sequence changed"));
            }
            expected_sequence = expected_sequence
                .checked_add(1)
                .ok_or_else(|| ledger_error("testnet ledger sequence overflow"))?;
            let Record::Credit(stored) = decode_record(&bytes)? else {
                return Err(ledger_error("duplicate testnet ledger initialization"));
            };
            if stored.scope != scope {
                return Err(ledger_error("testnet ledger credit changed release scope"));
            }
            let verified = CreditFacts::from_admission(&rederive(stored.operation_id)?)?;
            if verified != stored {
                return Err(ledger_error(
                    "testnet ledger credit differs from recovered finalized proof",
                ));
            }
            ledger.insert_new(stored)?;
        }
        ledger.wal.check_owned().map_err(storage_error)?;
        Ok(ledger)
    }

    /// Durably count one opaque, proof-verified Applied top-up; exact retry is idempotent.
    ///
    /// # Errors
    /// Rejects a different release/network/asset/reserve scope, nonpositive or invalid facts,
    /// changed evidence for an operation, reuse of a credit ID, overflow, or uncertain storage.
    pub fn credit(
        &mut self,
        admission: &KagemushaTestnetValueAdmissionV1,
    ) -> Result<KagemushaTestnetMintLedgerCreditV1, KagemushaRecursionErrorV1> {
        self.wal.check_owned().map_err(storage_error)?;
        let facts = CreditFacts::from_admission(admission)?;
        if facts.scope != self.scope {
            return Err(ledger_error(
                "testnet mint credit is outside signed release scope",
            ));
        }
        if let Some(existing) = self.by_operation.get(&facts.operation_id) {
            return if existing.facts == facts {
                Ok(*existing)
            } else {
                Err(ledger_error(
                    "testnet mint operation changed its credited evidence",
                ))
            };
        }
        if self.credit_owners.contains_key(&facts.credit_id) {
            return Err(ledger_error("testnet mint credit ID is already owned"));
        }
        self.total_admitted
            .checked_add(facts.amount)
            .ok_or_else(|| ledger_error("testnet admitted value overflow"))?;
        self.wal
            .append(&encode_record(&Record::Credit(facts))?)
            .map_err(storage_error)?;
        self.insert_new(facts)
    }

    fn insert_new(
        &mut self,
        facts: CreditFacts,
    ) -> Result<KagemushaTestnetMintLedgerCreditV1, KagemushaRecursionErrorV1> {
        facts.validate()?;
        if facts.scope != self.scope
            || self.by_operation.contains_key(&facts.operation_id)
            || self.credit_owners.contains_key(&facts.credit_id)
        {
            return Err(ledger_error(
                "duplicate or out-of-scope testnet ledger credit",
            ));
        }
        let total = self
            .total_admitted
            .checked_add(facts.amount)
            .ok_or_else(|| ledger_error("testnet admitted value overflow"))?;
        let credit = KagemushaTestnetMintLedgerCreditV1 { facts };
        self.by_operation.insert(facts.operation_id, credit);
        self.credit_owners
            .insert(facts.credit_id, facts.operation_id);
        self.total_admitted = total;
        Ok(credit)
    }

    /// Return the exact signed release scope of every counted credit.
    #[must_use]
    pub const fn scope(&self) -> KagemushaTestnetStateObservationScopeV1 {
        self.scope
    }

    /// Return the sum of durably counted, proof-verified top-up amounts.
    #[must_use]
    pub const fn total_admitted(&self) -> u128 {
        self.total_admitted
    }

    /// Return the number of unique finalized top-ups counted.
    #[must_use]
    pub fn credit_count(&self) -> usize {
        self.by_operation.len()
    }

    /// Inspect a counted credit by its finalized top-up operation ID.
    #[must_use]
    pub fn credit_by_operation(
        &self,
        operation_id: DigestV1,
    ) -> Option<KagemushaTestnetMintLedgerCreditV1> {
        self.by_operation.get(&operation_id).copied()
    }

    /// Inspect a counted credit by its unique proof-bound credit ID.
    #[must_use]
    pub fn credit_by_credit_id(
        &self,
        credit_id: DigestV1,
    ) -> Option<KagemushaTestnetMintLedgerCreditV1> {
        self.credit_owners
            .get(&credit_id)
            .and_then(|operation_id| self.credit_by_operation(*operation_id))
    }
}

fn encode_record(record: &Record) -> Result<Vec<u8>, KagemushaRecursionErrorV1> {
    let bytes = norito::encode_canonical(record)
        .map_err(|_| ledger_error("cannot encode testnet ledger record"))?;
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(ledger_error("testnet ledger record exceeds bound"));
    }
    Ok(bytes)
}

fn decode_record(bytes: &[u8]) -> Result<Record, KagemushaRecursionErrorV1> {
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(ledger_error("testnet ledger record exceeds bound"));
    }
    let record = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES * 4,
            RECORD_MAX_BYTES * 8,
            16,
        ),
    )
    .map_err(|_| ledger_error("invalid canonical testnet ledger record"))?;
    if encode_record(&record)? != bytes {
        return Err(ledger_error("noncanonical testnet ledger record"));
    }
    Ok(record)
}

fn ledger_error(message: &str) -> KagemushaRecursionErrorV1 {
    KagemushaRecursionErrorV1::StateProofRejected(message.to_owned())
}

fn storage_error(error: PrivateJournalError) -> KagemushaRecursionErrorV1 {
    ledger_error(&format!("testnet value ledger: {error}"))
}

#[cfg(test)]
#[path = "testnet_value_ledger_tests.rs"]
mod tests;
