//! Private, append-only testnet proof-lineage evidence; it conveys no spending authority.
//!
//! The descriptor-owned store prevents concurrent writers and makes each acknowledged record
//! durable. On restart its complete prefix must be replayed through the concrete authenticated
//! recursive verifier before the owner can return any observation. The reservation contains a
//! private mint opening and must be kept in the private 0700 directory, never exported to an SDK.
//! WAL framing alone cannot detect restoration of an older complete directory; production
//! rollback exclusion still requires an independently selected hardware checkpoint.

use std::path::Path;

use norito::{Decode, Encode};

use super::{KagemushaRecursionErrorV1, KagemushaTestnetStateObservationScopeV1};
use crate::zk::kagemusha_v1_state::{PrivateJournal, PrivateJournalError, PrivateJournalFormat};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
    sumeragi_finality::{MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint},
};

// Cover Torii's full canonical status limit plus the paired proof, public inputs, anchor,
// and Norito framing. Every component is bounded before decode or append. JSON is decoded
// separately at Torii and has its own larger transport limit.
const RECORD_MAX_BYTES: usize =
    iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1
        + MAX_FINALITY_CHECKPOINT_BYTES
        + 1024 * 1024;
const STATUS_MAX_BYTES: usize =
    iroha_torii_shared::kagemusha_api::KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1;
const STATE_PUBLIC_INPUTS_MAX_BYTES: usize = 4 * 1024;
const PAIRED_PROOF_MAX_BYTES: usize =
    iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "testnet-mint-lineage.norito.wal",
    magic: b"IKGTML1\0",
    hash_domain: b"iroha:kagemusha:v1:testnet-mint-lineage-disk:frame\0",
    maximum_payload_bytes: RECORD_MAX_BYTES as u64,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_recursion::testnet_mint_journal::Record")]
pub(super) enum Record {
    Initialize {
        scope: KagemushaTestnetStateObservationScopeV1,
    },
    ReserveMint {
        reservation: Vec<u8>,
    },
    ObserveNonMint {
        public_inputs: Vec<u8>,
        proof: Vec<u8>,
    },
    ObserveFinalizedMint {
        operation_id: [u8; 32],
        status: Vec<u8>,
        trust_anchor: JournalAnchor,
        public_inputs: Vec<u8>,
        proof: Vec<u8>,
    },
}

/// Canonical checkpoint evidence retained from the caller's independently selected anchor.
///
/// Decoding or validating this disk copy never grants authority. Replay must obtain and
/// match the independent verified chain token before verifying the stored status or mint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::testnet_mint_journal::JournalAnchor"
)]
pub(super) struct JournalAnchor {
    network_id: [u8; 32],
    checkpoint: Vec<u8>,
}

impl JournalAnchor {
    pub(super) fn capture(
        value: &KagemushaFinalityTrustAnchorV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        value.validate().map_err(|_| invalid_record())?;
        Ok(Self {
            network_id: *value.network_id.as_bytes(),
            checkpoint: value
                .checkpoint
                .encode_canonical()
                .map_err(|_| invalid_record())?,
        })
    }

    pub(super) fn try_into_anchor(
        self,
    ) -> Result<KagemushaFinalityTrustAnchorV1, KagemushaRecursionErrorV1> {
        // Parse exact original network bytes: prehashed would change the marker bit.
        let network: Hash = hex::encode(self.network_id)
            .parse()
            .map_err(|_| invalid_record())?;
        let anchor = KagemushaFinalityTrustAnchorV1 {
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(network)),
            checkpoint: SumeragiFinalityCheckpoint::decode_canonical(&self.checkpoint)
                .map_err(|_| invalid_record())?,
        };
        anchor.validate().map_err(|_| invalid_record())?;
        Ok(anchor)
    }
}

pub(super) struct TestnetMintJournal {
    wal: PrivateJournal,
}

impl TestnetMintJournal {
    pub(super) fn create_new(
        path: &Path,
        scope: KagemushaTestnetStateObservationScopeV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let mut owner = Self {
            wal: PrivateJournal::create_new(path, FORMAT).map_err(storage_error)?,
        };
        owner.append(&Record::Initialize { scope })?;
        Ok(owner)
    }

    pub(super) fn open_existing(path: &Path) -> Result<Self, KagemushaRecursionErrorV1> {
        Ok(Self {
            wal: PrivateJournal::open_existing(path, FORMAT).map_err(storage_error)?,
        })
    }

    pub(super) fn check_owned(&self) -> Result<(), KagemushaRecursionErrorV1> {
        self.wal.check_owned().map_err(storage_error)
    }

    pub(super) fn next_replayed(
        &mut self,
    ) -> Result<Option<(u64, Record)>, KagemushaRecursionErrorV1> {
        let Some((sequence, bytes)) = self.wal.replay_next().map_err(storage_error)? else {
            return Ok(None);
        };
        Ok(Some((sequence, decode(&bytes)?)))
    }

    pub(super) fn append(&mut self, record: &Record) -> Result<(), KagemushaRecursionErrorV1> {
        let bytes = encode(record)?;
        self.wal.append(&bytes).map_err(storage_error)
    }

    #[cfg(test)]
    pub(super) fn inject_failure(
        &self,
        failure: crate::zk::kagemusha_v1_state::TestPersistenceFailure,
    ) {
        self.wal.failure.set(Some(failure));
    }
}

fn encode(record: &Record) -> Result<Vec<u8>, KagemushaRecursionErrorV1> {
    check_component_bounds(record)?;
    let bytes = norito::encode_canonical(record).map_err(|_| invalid_record())?;
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(invalid_record());
    }
    Ok(bytes)
}

fn check_component_bounds(record: &Record) -> Result<(), KagemushaRecursionErrorV1> {
    let valid = match record {
        Record::Initialize { .. } => true,
        Record::ReserveMint { reservation } => !reservation.is_empty(),
        Record::ObserveNonMint {
            public_inputs,
            proof,
        } => {
            !public_inputs.is_empty()
                && public_inputs.len() <= STATE_PUBLIC_INPUTS_MAX_BYTES
                && !proof.is_empty()
                && proof.len() <= PAIRED_PROOF_MAX_BYTES
        }
        Record::ObserveFinalizedMint {
            operation_id,
            status,
            trust_anchor,
            public_inputs,
            proof,
        } => {
            *operation_id != [0; 32]
                && !status.is_empty()
                && status.len() <= STATUS_MAX_BYTES
                && !trust_anchor.checkpoint.is_empty()
                && trust_anchor.checkpoint.len() <= MAX_FINALITY_CHECKPOINT_BYTES
                && !public_inputs.is_empty()
                && public_inputs.len() <= STATE_PUBLIC_INPUTS_MAX_BYTES
                && !proof.is_empty()
                && proof.len() <= PAIRED_PROOF_MAX_BYTES
        }
    };
    if valid { Ok(()) } else { Err(invalid_record()) }
}

pub(super) fn encode_value<T: norito::NoritoSerialize>(
    value: &T,
) -> Result<Vec<u8>, KagemushaRecursionErrorV1> {
    let bytes = norito::encode_canonical(value).map_err(|_| invalid_record())?;
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(invalid_record());
    }
    Ok(bytes)
}

pub(super) fn decode_value<T>(bytes: &[u8]) -> Result<T, KagemushaRecursionErrorV1>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(invalid_record());
    }
    let value = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES * 4,
            RECORD_MAX_BYTES * 8,
            32,
        ),
    )
    .map_err(|_| invalid_record())?;
    if encode_value(&value)? != bytes {
        return Err(invalid_record());
    }
    Ok(value)
}

fn decode(bytes: &[u8]) -> Result<Record, KagemushaRecursionErrorV1> {
    if bytes.is_empty() || bytes.len() > RECORD_MAX_BYTES {
        return Err(invalid_record());
    }
    let record = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES,
            RECORD_MAX_BYTES * 4,
            RECORD_MAX_BYTES * 8,
            32,
        ),
    )
    .map_err(|_| invalid_record())?;
    if encode(&record)? != bytes {
        return Err(invalid_record());
    }
    Ok(record)
}

fn invalid_record() -> KagemushaRecursionErrorV1 {
    KagemushaRecursionErrorV1::StateProofRejected(
        "testnet mint-lineage journal has invalid canonical record".to_owned(),
    )
}

fn storage_error(error: PrivateJournalError) -> KagemushaRecursionErrorV1 {
    KagemushaRecursionErrorV1::StateProofRejected(format!("testnet mint-lineage journal: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_state::TestPersistenceFailure;

    fn scope() -> KagemushaTestnetStateObservationScopeV1 {
        KagemushaTestnetStateObservationScopeV1::new(
            [1; 32], [2; 32], [3; 32], 2, [4; 32], [5; 32], [6; 32],
        )
        .unwrap()
    }

    #[test]
    fn canonical_journal_locks_and_replays_exact_prefix() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("private-testnet-lineage");
        let mut journal = TestnetMintJournal::create_new(&path, scope()).unwrap();
        let reservation = Record::ReserveMint {
            reservation: vec![0xA5; 64],
        };
        journal.append(&reservation).unwrap();
        assert!(TestnetMintJournal::open_existing(&path).is_err());
        drop(journal);
        let mut recovered = TestnetMintJournal::open_existing(&path).unwrap();
        assert_eq!(
            recovered.next_replayed().unwrap(),
            Some((0, Record::Initialize { scope: scope() }))
        );
        assert_eq!(recovered.next_replayed().unwrap(), Some((1, reservation)));
        assert_eq!(recovered.next_replayed().unwrap(), None);
    }

    #[test]
    fn uncertain_append_poisoned_until_restart_and_never_acknowledged() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("private-testnet-lineage");
        let mut journal = TestnetMintJournal::create_new(&path, scope()).unwrap();
        journal.inject_failure(TestPersistenceFailure::BeforeSync);
        assert!(
            journal
                .append(&Record::ReserveMint {
                    reservation: vec![0xA5; 64],
                })
                .is_err()
        );
        assert!(journal.check_owned().is_err());
    }

    #[test]
    fn decoder_rejects_noncanonical_and_oversized_records() {
        let mut bytes = encode(&Record::Initialize { scope: scope() }).unwrap();
        assert_eq!(
            decode(&bytes).unwrap(),
            Record::Initialize { scope: scope() }
        );
        bytes.push(0);
        assert!(decode(&bytes).is_err());
        assert!(decode(&vec![0; RECORD_MAX_BYTES + 1]).is_err());
        assert!(
            encode(&Record::ObserveFinalizedMint {
                operation_id: [1; 32],
                status: vec![0; STATUS_MAX_BYTES + 1],
                trust_anchor: JournalAnchor {
                    network_id: [1; 32],
                    checkpoint: vec![1],
                },
                public_inputs: vec![1],
                proof: vec![1],
            })
            .is_err()
        );
        assert!(
            encode(&Record::ObserveNonMint {
                public_inputs: vec![1; STATE_PUBLIC_INPUTS_MAX_BYTES + 1],
                proof: vec![1],
            })
            .is_err()
        );
    }

    #[test]
    fn journal_anchor_roundtrip_preserves_exact_checkpoint_and_rejects_foreign_network() {
        let fixture = iroha_data_model::testing::native_finality::NativeFinalityFixture::new();
        let selected = KagemushaFinalityTrustAnchorV1 {
            network_id: fixture.network_id(),
            checkpoint: fixture.checkpoint(),
        };
        let anchor = JournalAnchor::capture(&selected).unwrap();
        let reconstructed = anchor.clone().try_into_anchor().unwrap();
        assert_eq!(reconstructed, selected);
        assert_eq!(JournalAnchor::capture(&reconstructed).unwrap(), anchor);
        let unmarked = JournalAnchor {
            network_id: [2; 32],
            ..anchor.clone()
        };
        assert!(unmarked.try_into_anchor().is_err());
        let changed = JournalAnchor {
            network_id: *Hash::new(b"foreign native network").as_ref(),
            ..anchor.clone()
        };
        assert!(changed.try_into_anchor().is_err());
        let mut malformed = anchor;
        malformed.checkpoint.push(0);
        assert!(malformed.try_into_anchor().is_err());
    }
}
