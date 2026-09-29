//! The global chain's AMX instructions and deadline step over a real World: every `Begin` and
//! `Decision` becomes a write of the block's execution witness, proven against genuine
//! certificates of two foreign fixture chains.

use iroha_data_model::{
    block::{
        consensus::{ExecKv, ExecWitness},
        decode_versioned_signed_block,
    },
    isi::sumeragi_amx::{BeginAmxV1, RegisterAmxDataspaceV1, RelayAmxPreparedV1},
    sumeragi_amx::{
        AmxCertifiedBlockV1, AmxForeignInstanceV1, AmxLegV1, AmxOutcomeV1, AmxPreparedV1,
        AmxRecordProofV1, AmxRecordV1, AmxTransactionV1, AmxVoteV1, SumeragiAmxState,
    },
    sumeragi_finality::{
        SUMERAGI_LANE_STATE_WITNESS_KEY, SumeragiLaneStateCommitment, genesis_epoch,
        test_fixtures::NativeFinalityFixture,
    },
    sumeragi_lanes::SumeragiLaneState,
};
use iroha_model_base::topology::DataSpaceId;

use crate::{
    smartcontracts::{
        Execute,
        isi::sccp::test_support::{authority, blank_state, header},
    },
    state::{State, StateBlock},
};

const DS1: DataSpaceId = DataSpaceId::new(21);
const DS2: DataSpaceId = DataSpaceId::new(22);

/// A foreign dataspace chain with genuine native certificates.
struct Dataspace {
    id: DataSpaceId,
    chain: NativeFinalityFixture,
}

impl Dataspace {
    fn new(id: DataSpaceId) -> Self {
        Self {
            id,
            chain: NativeFinalityFixture::start(&format!("amx-dataspace-{}", id.as_u64())),
        }
    }

    fn instance(&self) -> [u8; 32] {
        self.chain.verifier().instance().0
    }

    fn register(&self) -> RegisterAmxDataspaceV1 {
        RegisterAmxDataspaceV1 {
            dataspace: self.id,
            instance: self.instance(),
            anchor: norito::encode_canonical(&genesis_epoch(self.chain.genesis()).unwrap())
                .unwrap(),
        }
    }

    /// Certify the next block of this chain, whose execution wrote `record`, and prove it.
    fn prove(&mut self, record: &AmxRecordV1) -> AmxRecordProofV1 {
        let header = self.chain.next_header();
        let height = header.height().get();
        let lanes = SumeragiLaneStateCommitment::from_state(
            self.chain.network_id(),
            height,
            &SumeragiLaneState::default(),
        )
        .unwrap();
        let writes = vec![
            ExecKv {
                key: SUMERAGI_LANE_STATE_WITNESS_KEY.to_vec(),
                value: norito::encode_canonical(&lanes).unwrap(),
            },
            ExecKv {
                key: record.witness_key().to_vec(),
                value: record.witness_value().unwrap(),
            },
        ];
        let block = self.chain.block_with_submitted_work(header);
        let witness = ExecWitness {
            writes: writes.clone(),
            ..ExecWitness::default()
        };
        let proof = self.chain.certify_with_witness(block, &witness);
        let certified = decode_versioned_signed_block(&proof.block_wire).unwrap();
        AmxRecordProofV1::from_writes(
            AmxCertifiedBlockV1::from_certificate(certified.commit_certificate().unwrap()),
            writes
                .iter()
                .map(|write| (write.key.as_slice(), write.value.as_slice())),
            record.clone(),
        )
        .unwrap()
    }
}

fn transaction(deadline: u64, nonce: u8) -> AmxTransactionV1 {
    AmxTransactionV1 {
        legs: [DS1, DS2]
            .into_iter()
            .map(|dataspace| AmxLegV1 {
                dataspace,
                payload: vec![nonce],
            })
            .collect(),
        deadline,
        nonce: [nonce; 32],
    }
}

fn yes(tx: &AmxTransactionV1, participant: DataSpaceId) -> AmxRecordV1 {
    AmxRecordV1::Prepared(AmxPreparedV1 {
        tx: tx.id().unwrap(),
        participant,
        vote: AmxVoteV1::Yes([participant.as_u64() as u8; 32]),
    })
}

/// Execute `instruction` in its own transaction of `block`.
fn execute(block: &mut StateBlock<'_>, instruction: impl Execute) -> Result<(), String> {
    let mut transaction = block.transaction();
    instruction
        .execute(&authority(7), &mut transaction)
        .map_err(|error| error.to_string())?;
    transaction.apply();
    Ok(())
}

/// The AMX records among `writes`.
fn amx_records(writes: &[ExecKv]) -> Vec<AmxRecordV1> {
    writes
        .iter()
        .filter(|write| write.key.first() == Some(&0xD9))
        .map(|write| AmxRecordV1::from_witness(&write.key, &write.value).unwrap())
        .collect()
}

#[test]
fn sumeragi_amx_begin_and_decisions_are_witness_writes_of_the_block() {
    let mut ds1 = Dataspace::new(DS1);
    let mut ds2 = Dataspace::new(DS2);
    let tx = transaction(5, 1);
    let proof1 = ds1.prove(&yes(&tx, DS1));
    let proof2 = ds2.prove(&yes(&tx, DS2));
    let state: State = blank_state();
    let mut block = state.block(header(1));
    let _guard = crate::exec_witness::exec_witness_guard();
    crate::exec_witness::start_block();
    execute(&mut block, ds1.register()).unwrap();
    execute(&mut block, ds2.register()).unwrap();
    execute(
        &mut block,
        BeginAmxV1 {
            transaction: tx.clone(),
        },
    )
    .unwrap();
    // A second Begin for x is rejected.
    assert!(
        execute(
            &mut block,
            BeginAmxV1 {
                transaction: tx.clone(),
            }
        )
        .is_err()
    );
    // A proof certified by another dataspace's committee is rejected.
    let mut forged = proof1.clone();
    forged.record = yes(&tx, DS2);
    assert!(execute(&mut block, RelayAmxPreparedV1 { proof: forged }).is_err());
    execute(
        &mut block,
        RelayAmxPreparedV1 {
            proof: proof1.clone(),
        },
    )
    .unwrap();
    execute(&mut block, RelayAmxPreparedV1 { proof: proof2 }).unwrap();
    // Later proofs are ignored.
    execute(&mut block, RelayAmxPreparedV1 { proof: proof1 }).unwrap();
    let entry = block
        .world
        .sumeragi_amx
        .get()
        .transaction(&tx.id().unwrap())
        .cloned()
        .unwrap();
    assert_eq!(
        entry.decided.map(|decided| decided.outcome),
        Some(AmxOutcomeV1::Commit)
    );
    let witness = crate::exec_witness::drain_exec_witness();
    assert_eq!(
        amx_records(&witness.writes),
        vec![
            AmxRecordV1::Begin(tx.begin().unwrap()),
            AmxRecordV1::Decision(iroha_data_model::sumeragi_amx::AmxDecisionV1 {
                tx: tx.id().unwrap(),
                outcome: AmxOutcomeV1::Commit,
            }),
        ]
    );
}

#[test]
fn sumeragi_amx_registration_needs_genesis_or_permission() {
    let ds1 = Dataspace::new(DS1);
    let state = blank_state();
    {
        let mut block = state.block(header(2));
        let error = execute(&mut block, ds1.register()).unwrap_err();
        assert!(error.contains("CanSetParameters"), "{error}");
    }
    let mut malformed = ds1.register();
    malformed.anchor = vec![1, 2, 3];
    let mut genesis = state.block(header(1));
    assert!(execute(&mut genesis, malformed).is_err());
    execute(&mut genesis, ds1.register()).unwrap();
    assert!(execute(&mut genesis, ds1.register()).is_err());
}

#[test]
fn sumeragi_amx_deadline_step_aborts_in_the_first_block_after_the_deadline() {
    let ds1 = Dataspace::new(DS1);
    let ds2 = Dataspace::new(DS2);
    let mut seeded = SumeragiAmxState::default();
    for dataspace in [&ds1, &ds2] {
        seeded
            .register_dataspace(
                dataspace.id,
                AmxForeignInstanceV1::new(
                    dataspace.instance(),
                    genesis_epoch(dataspace.chain.genesis()).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let expiring = transaction(5, 2);
    let pending = transaction(9, 3);
    seeded.begin(2, &expiring).unwrap();
    seeded.begin(2, &pending).unwrap();
    let state = blank_state();
    for (height, expected) in [(5, vec![]), (6, vec![expiring.id().unwrap()])] {
        let mut block = state.block(header(height));
        *block.world.sumeragi_amx.get_mut() = seeded.clone();
        let _guard = crate::exec_witness::exec_witness_guard();
        crate::exec_witness::start_block();
        let decisions = block.advance_sumeragi_amx().unwrap();
        assert_eq!(
            decisions
                .iter()
                .map(|decision| decision.tx)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            decisions
                .iter()
                .all(|decision| decision.outcome == AmxOutcomeV1::Abort)
        );
        let witness = crate::exec_witness::drain_exec_witness();
        assert_eq!(
            amx_records(&witness.writes),
            decisions
                .iter()
                .copied()
                .map(AmxRecordV1::Decision)
                .collect::<Vec<_>>()
        );
        let amx = block.world.sumeragi_amx.get();
        assert_eq!(
            amx.transaction(&expiring.id().unwrap()).is_some(),
            expected.is_empty()
        );
        assert!(amx.transaction(&pending.id().unwrap()).is_some());
    }
    // Nothing to expire leaves the cell untouched.
    let mut idle = state.block(header(3));
    *idle.world.sumeragi_amx.get_mut() = seeded;
    assert!(idle.advance_sumeragi_amx().unwrap().is_empty());
}

#[test]
fn sumeragi_amx_write_proofs_match_the_executor_write_root() {
    use crate::exec_witness::smt::{KvPair, compute_post_state_root};
    use iroha_data_model::sumeragi_amx::{AmxDecisionV1, AmxWriteProofV1, write_set_root};

    // The executor's write-set tree and the record proofs' tree agree, so a record written by a
    // block's execution is proven against the ordinary-write root that `R` certifies.
    let tx = transaction(9, 4);
    let records = [
        AmxRecordV1::Begin(tx.begin().unwrap()),
        AmxRecordV1::Decision(AmxDecisionV1 {
            tx: tx.id().unwrap(),
            outcome: AmxOutcomeV1::Abort,
        }),
        yes(&tx, DS1),
    ];
    let mut writes: Vec<KvPair> = (0u8..48)
        .map(|index| KvPair::new(vec![index, 7, 7], vec![index; usize::from(index) + 1]))
        .collect();
    for record in &records {
        writes.push(KvPair::new(
            record.witness_key().to_vec(),
            record.witness_value().unwrap(),
        ));
    }
    // A later write of a key wins, in both trees.
    writes.push(KvPair::new(vec![3, 7, 7], b"latest".to_vec()));
    let pairs = || {
        writes
            .iter()
            .map(|write| (write.key.as_slice(), write.value.as_slice()))
    };
    let root = compute_post_state_root(&[], &writes);
    assert_eq!(write_set_root(pairs()).unwrap(), root);
    for record in &records {
        let key = record.witness_key();
        let (path, value) = AmxWriteProofV1::from_writes(pairs(), &key).unwrap();
        assert_eq!(value, record.witness_value().unwrap());
        assert_eq!(path.root(&key, &value).unwrap(), root);
    }
}
