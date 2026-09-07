//! Typed public-transfer boundary for the private one-delta compact protocol.
//!
//! The constructor accepts only the public constructor's validated table and
//! exact caller-expected PublicIO. It derives all SMT ports from that table and
//! binds the complete original public claims, transitions and selected semantics
//! in a canonical Norito envelope before any proof challenge. It reads no private
//! sibling, witness or trace. A caller must authenticate the expected PublicIO
//! and authority separately; these touched-balance roots are not consensus roots.
//!
//! TODO: Qualify the protocol and resource profile, and integrate an authenticated
//! outer statement before exposing this test-only boundary to production.

use iroha_crypto::Hash;
use iroha_data_model::{account::AccountId, asset::id::AssetDefinitionId};
use iroha_primitives::numeric::Quantity;
use norito::{NoritoSerialize, SerializePayload, codec::Encode};

use super::{
    compact_protocol::{FixedAir, FixedAirSchema, PreparedAir},
    compact_transfer_air::CompactTransferAir,
};
use crate::{
    Error, ProofSemantics, Result, StateTransition, VerifyLimits,
    gadgets::public_transfer_statement::{
        PreparedPublicTransfers, PublicTransferDelta, PublicTransferLimits,
    },
    proof::PublicIO,
};

const IDENTITY: &str = "fastpq:prototype:public-transfer:v1:342cols:923slots:65536rows";

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::PublicDeltaV1")]
struct BoundDelta {
    from_account: AccountId,
    to_account: AccountId,
    asset_definition: AssetDefinitionId,
    quantities: [Quantity; 5],
}

impl From<&PublicTransferDelta> for BoundDelta {
    fn from(delta: &PublicTransferDelta) -> Self {
        Self {
            from_account: delta.from_account.clone(),
            to_account: delta.to_account.clone(),
            asset_definition: delta.asset_definition.clone(),
            quantities: [
                delta.amount.clone(),
                delta.from_balance_before.clone(),
                delta.from_balance_after.clone(),
                delta.to_balance_before.clone(),
                delta.to_balance_after.clone(),
            ],
        }
    }
}

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::PublicTranscriptV1")]
struct BoundTranscript {
    batch_hash: Hash,
    authority_digest: Hash,
    deltas: Vec<BoundDelta>,
    poseidon_preimage_digest: Option<Hash>,
}

#[derive(NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_prototype::PublicTransferContextV1")]
struct BoundContext {
    version: u16,
    semantics: u8,
    public_io: PublicIO,
    transitions: Vec<StateTransition>,
    transcripts: Vec<BoundTranscript>,
}

/// Complete public arithmetic/identity/occurrence binding plus the private SMT AIR.
pub(super) struct PublicTransferAir {
    inner: CompactTransferAir,
}

impl PublicTransferAir {
    /// Derive one-delta SMT ports from validated claims and bind exact caller context.
    ///
    /// The expected public IO is an input from the trusted surrounding verifier,
    /// never copied from a proof. All seven fields must equal the prepared table's
    /// original inputs and independently computed ordering hash. No caller-supplied
    /// raw SMT ports or opaque context are accepted by this boundary.
    pub(super) fn new(prepared: &PreparedPublicTransfers<'_>, expected: &PublicIO) -> Result<Self> {
        if prepared.pairs().len() != 1 || prepared.transitions().len() != 2 {
            return Err(invariant(
                "compact public transfer requires exactly one complete delta",
            ));
        }
        let context = encode_context(prepared, expected)?;
        let statements = prepared.compact_statements(&[])?;
        let [statement] = statements.as_slice() else {
            return Err(invariant(
                "compact public transfer needs one derived SMT statement",
            ));
        };
        Ok(Self {
            inner: CompactTransferAir::new(statement, Some(&context))?,
        })
    }
}

/// Serialize the complete immutable public table with the original V1 envelope.
///
/// This shared helper retains the existing one-delta schema and field ordering.
/// Capacity and ordinary/AXT route selection remain the enclosing relation's
/// responsibility; the fixed public bounds and all seven caller inputs are
/// checked before cloning the original public fields.
pub(super) fn encode_context(
    prepared: &PreparedPublicTransfers<'_>,
    expected: &PublicIO,
) -> Result<Vec<u8>> {
    let limits = PublicTransferLimits::default();
    check_limit(
        "max_public_transfer_transcripts",
        prepared.claims().len(),
        limits.max_transcripts,
    )?;
    check_limit(
        "max_public_transfer_bytes",
        prepared.work().public_bytes,
        limits.max_public_bytes,
    )?;
    // The validated object's fields are private and its borrowed inputs
    // cannot be mutated while it exists. Reapplying these fixed bounds is
    // required even if its original constructor used a developer policy.
    let inputs = prepared.public_inputs();
    let actual = PublicIO {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
        ordering_hash: prepared.ordering_hash().into(),
    };
    if actual != *expected {
        return Err(Error::PublicIoMismatch {
            field: "compact_public_io",
        });
    }
    let semantics = match prepared.semantics() {
        ProofSemantics::StateTransition => 0,
        ProofSemantics::AxtTransferClaim => 1,
        ProofSemantics::AxtOpaqueEffect => {
            return Err(invariant(
                "opaque effects cannot select the compact transfer AIR",
            ));
        }
    };
    let context = BoundContext {
        version: 1,
        semantics,
        public_io: actual,
        transitions: prepared.transitions().to_vec(),
        transcripts: prepared
            .claims()
            .iter()
            .map(|claim| BoundTranscript {
                batch_hash: claim.batch_hash,
                authority_digest: claim.authority_digest,
                deltas: claim.deltas.iter().map(BoundDelta::from).collect(),
                poseidon_preimage_digest: claim.poseidon_preimage_digest,
            })
            .collect(),
    };
    // Count the complete fixed-layout envelope before allocating its encoded
    // bytes. The public-only clone above was bounded before construction.
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = norito::core::encoded_frame_len(&context)?;
    check_limit(
        "max_compact_statement_bytes",
        bytes,
        VerifyLimits::default().max_batch_bytes,
    )?;
    Ok(context.encode())
}

impl FixedAir for PublicTransferAir {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            identity: IDENTITY,
            ..self.inner.schema()
        }
    }

    fn statement_bytes(&self) -> &[u8] {
        self.inner.statement_bytes()
    }

    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
        self.inner.evaluate(point, current, next)
    }

    fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
        self.inner.prepare_prover()
    }
}

fn check_limit(limit: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        Err(Error::VerifierLimitExceeded { limit, actual, max })
    } else {
        Ok(())
    }
}

fn invariant(details: &str) -> Error {
    Error::TransferInvariant {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        OperationKind, PublicInputs,
        gadgets::{
            compact_smt_air::COLUMN_COUNT,
            public_transfer_statement::{
                PublicTransferTranscript, prepare_public_transfers, public_claims_from_transcripts,
            },
            transfer,
        },
    };
    use iroha_data_model::{
        DomainId,
        fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
    };
    use iroha_test_samples::{ALICE_ID, BOB_ID};

    fn fixture(
        amount: u64,
    ) -> (
        Vec<StateTransition>,
        Vec<PublicTransferTranscript>,
        PublicInputs,
    ) {
        let delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(100_u64),
            from_balance_after: Quantity::from(100 - amount),
            to_balance_before: Quantity::from(200_u64),
            to_balance_after: Quantity::from(200 + amount),
            // These deliberately absent private paths never enter the adapter.
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        };
        let batch_hash = Hash::new(b"typed-boundary transaction");
        let transcript = TransferTranscript {
            batch_hash,
            authority_digest: Hash::new(b"typed-boundary authority"),
            poseidon_preimage_digest: Some(transfer::compute_poseidon_digest(&delta, &batch_hash)),
            deltas: vec![delta.clone()],
        };
        let mut rows = [
            (&delta.from_account, 100_u64, 100 - amount),
            (&delta.to_account, 200_u64, 200 + amount),
        ]
        .into_iter()
        .map(|(account, before, after)| {
            StateTransition::new(
                iroha_data_model::fastpq::transfer_balance_key(&delta.asset_definition, account)
                    .unwrap(),
                before.to_le_bytes().to_vec(),
                after.to_le_bytes().to_vec(),
                OperationKind::Transfer,
            )
        })
        .collect::<Vec<_>>();
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        let claims =
            public_claims_from_transcripts(&[transcript], PublicTransferLimits::default()).unwrap();
        let inputs = PublicInputs {
            dsid: [0xA5; 16],
            slot: u64::MAX,
            old_root: Hash::new(b"declared old root").into(),
            new_root: Hash::new(b"declared new root").into(),
            perm_root: Hash::new(b"declared permission context").into(),
            tx_set_hash: Hash::new(b"declared transaction set").into(),
        };
        (rows, claims, inputs)
    }

    fn prepare<'a>(
        rows: &'a [StateTransition],
        claims: &'a [PublicTransferTranscript],
        inputs: PublicInputs,
        semantics: ProofSemantics,
    ) -> PreparedPublicTransfers<'a> {
        prepare_public_transfers(
            rows,
            claims,
            inputs,
            semantics,
            PublicTransferLimits::default(),
        )
        .unwrap()
    }

    fn expected(prepared: &PreparedPublicTransfers<'_>) -> PublicIO {
        let inputs = prepared.public_inputs();
        PublicIO {
            dsid: inputs.dsid,
            slot: inputs.slot,
            old_root: inputs.old_root,
            new_root: inputs.new_root,
            perm_root: inputs.perm_root,
            tx_set_hash: inputs.tx_set_hash,
            ordering_hash: prepared.ordering_hash().into(),
        }
    }

    #[test]
    fn typed_boundary_derives_exact_private_relation_from_path_free_claims() {
        for amount in [0, 17, 100] {
            let (rows, claims, inputs) = fixture(amount);
            let prepared = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
            let air = PublicTransferAir::new(&prepared, &expected(&prepared)).unwrap();
            let statements = prepared.compact_statements(&[]).unwrap();
            let raw = CompactTransferAir::new(&statements[0], None).unwrap();
            assert_eq!(air.schema().trace_rows, 65_536);
            assert_eq!(air.schema().width, 342);
            assert_eq!(air.schema().constraints, 923);
            assert_ne!(air.schema().identity, raw.schema().identity);
            assert_ne!(air.statement_bytes(), raw.statement_bytes());
            let current = core::array::from_fn::<_, COLUMN_COUNT, _>(|i| 19 + i as u64);
            let next = core::array::from_fn::<_, COLUMN_COUNT, _>(|i| 73 + i as u64);
            assert_eq!(
                air.evaluate(7, &current, &next).unwrap(),
                raw.evaluate(7, &current, &next).unwrap()
            );
        }
    }

    #[test]
    fn every_expected_public_io_field_is_checked_before_acceptance() {
        let (rows, claims, inputs) = fixture(17);
        let prepared = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        let original = expected(&prepared);
        for field in 0..7 {
            let mut changed = original;
            match field {
                0 => changed.dsid[15] ^= 1,
                1 => changed.slot ^= 1,
                2 => changed.old_root[0] ^= 1,
                3 => changed.new_root[0] ^= 1,
                4 => changed.perm_root[31] ^= 1 << 7,
                5 => changed.tx_set_hash[31] ^= 1 << 7,
                6 => changed.ordering_hash[31] ^= 1 << 7,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    PublicTransferAir::new(&prepared, &changed),
                    Err(Error::PublicIoMismatch {
                        field: "compact_public_io"
                    })
                ),
                "field={field}"
            );
        }
    }

    #[test]
    fn caller_inputs_original_claims_and_selected_semantics_all_bind_the_transcript() {
        let (rows, mut claims, inputs) = fixture(17);
        let prepared = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        let baseline = PublicTransferAir::new(&prepared, &expected(&prepared)).unwrap();
        for field in 0..6 {
            let mut changed = inputs;
            match field {
                0 => changed.dsid[15] ^= 1,
                1 => changed.slot ^= 1,
                2 => changed.old_root[0] ^= 1,
                3 => changed.new_root[0] ^= 1,
                4 => changed.perm_root[31] ^= 1 << 7,
                5 => changed.tx_set_hash[31] ^= 1 << 7,
                _ => unreachable!(),
            }
            let changed = prepare(&rows, &claims, changed, ProofSemantics::StateTransition);
            let air = PublicTransferAir::new(&changed, &expected(&changed)).unwrap();
            assert_ne!(
                air.statement_bytes(),
                baseline.statement_bytes(),
                "field={field}"
            );
        }
        let axt = prepare(&rows, &claims, inputs, ProofSemantics::AxtTransferClaim);
        assert_ne!(
            PublicTransferAir::new(&axt, &expected(&axt))
                .unwrap()
                .statement_bytes(),
            baseline.statement_bytes()
        );
        drop(axt);
        drop(prepared);
        claims[0].authority_digest = Hash::new(b"another exact authority claim");
        let changed = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        assert_ne!(
            PublicTransferAir::new(&changed, &expected(&changed))
                .unwrap()
                .statement_bytes(),
            baseline.statement_bytes()
        );
        let (other_rows, other_claims, other_inputs) = fixture(18);
        let changed = prepare(
            &other_rows,
            &other_claims,
            other_inputs,
            ProofSemantics::StateTransition,
        );
        assert_ne!(
            PublicTransferAir::new(&changed, &expected(&changed))
                .unwrap()
                .statement_bytes(),
            baseline.statement_bytes()
        );
    }

    #[test]
    fn exact_capacity_rejects_empty_and_multiple_deltas() {
        let (_, _, mut inputs) = fixture(0);
        inputs.new_root = inputs.old_root;
        let empty = prepare(&[], &[], inputs, ProofSemantics::StateTransition);
        assert!(PublicTransferAir::new(&empty, &expected(&empty)).is_err());
        let (mut rows, mut claims, inputs) = fixture(0);
        rows.extend(rows.clone());
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        claims.extend(claims.clone());
        let multiple = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        assert_eq!(multiple.pairs().len(), 2);
        assert!(PublicTransferAir::new(&multiple, &expected(&multiple)).is_err());
        assert!(check_limit("test", 1, 1).is_ok());
        assert!(matches!(
            check_limit("test", 2, 1),
            Err(Error::VerifierLimitExceeded {
                limit: "test",
                actual: 2,
                max: 1
            })
        ));
    }

    #[test]
    fn extracted_context_preserves_original_one_delta_envelope_bytes() {
        // Keep a separate schema spelling and original field order as the
        // extraction oracle; do not route this through the new shared encoder.
        #[derive(NoritoSerialize)]
        #[norito(schema_name = "fastpq_prover::compact_prototype::PublicDeltaV1")]
        struct OriginalDelta {
            from_account: AccountId,
            to_account: AccountId,
            asset_definition: AssetDefinitionId,
            quantities: [Quantity; 5],
        }
        #[derive(NoritoSerialize)]
        #[norito(schema_name = "fastpq_prover::compact_prototype::PublicTranscriptV1")]
        struct OriginalTranscript {
            batch_hash: Hash,
            authority_digest: Hash,
            deltas: Vec<OriginalDelta>,
            poseidon_preimage_digest: Option<Hash>,
        }
        #[derive(NoritoSerialize)]
        #[norito(schema_name = "fastpq_prover::compact_prototype::PublicTransferContextV1")]
        struct OriginalContext {
            version: u16,
            semantics: u8,
            public_io: PublicIO,
            transitions: Vec<StateTransition>,
            transcripts: Vec<OriginalTranscript>,
        }
        for amount in [0, 17, 100] {
            for (semantics, tag) in [
                (ProofSemantics::StateTransition, 0),
                (ProofSemantics::AxtTransferClaim, 1),
            ] {
                let (rows, claims, inputs) = fixture(amount);
                let prepared = prepare(&rows, &claims, inputs, semantics);
                let public_io = expected(&prepared);
                let original = OriginalContext {
                    version: 1,
                    semantics: tag,
                    public_io,
                    transitions: rows.clone(),
                    transcripts: claims
                        .iter()
                        .map(|claim| OriginalTranscript {
                            batch_hash: claim.batch_hash,
                            authority_digest: claim.authority_digest,
                            poseidon_preimage_digest: claim.poseidon_preimage_digest,
                            deltas: claim
                                .deltas
                                .iter()
                                .map(|delta| OriginalDelta {
                                    from_account: delta.from_account.clone(),
                                    to_account: delta.to_account.clone(),
                                    asset_definition: delta.asset_definition.clone(),
                                    quantities: [
                                        delta.amount.clone(),
                                        delta.from_balance_before.clone(),
                                        delta.from_balance_after.clone(),
                                        delta.to_balance_before.clone(),
                                        delta.to_balance_after.clone(),
                                    ],
                                })
                                .collect(),
                        })
                        .collect(),
                }
                .encode();
                assert_eq!(encode_context(&prepared, &public_io).unwrap(), original);
                let statements = prepared.compact_statements(&[]).unwrap();
                let original_air =
                    CompactTransferAir::new(&statements[0], Some(&original)).unwrap();
                let factored = PublicTransferAir::new(&prepared, &public_io).unwrap();
                assert_eq!(factored.statement_bytes(), original_air.statement_bytes());
            }
        }
    }

    #[test]
    fn typed_context_encoding_is_canonical_and_restores_ambient_layout() {
        let (rows, claims, inputs) = fixture(17);
        let prepared = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        let public_io = expected(&prepared);
        let expected = PublicTransferAir::new(&prepared, &public_io).unwrap();
        for flags in
            (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let actual = PublicTransferAir::new(&prepared, &public_io).unwrap();
            assert_eq!(actual.statement_bytes(), expected.statement_bytes());
            assert_eq!(norito::core::get_decode_flags(), flags);
        }
    }

    #[test]
    #[ignore = "explicit full typed-transfer 65536x342 proving and bounded verification diagnostic"]
    fn complete_typed_transfer_verifies_after_private_witnesses_are_dropped() {
        use crate::gadgets::{
            compact_smt_air::{PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
            compact_trace_columns::smt_row_cells,
        };
        let started = std::time::Instant::now();
        let (rows, claims, mut inputs) = fixture(17);
        let claim = &claims[0];
        let mut transcripts = vec![TransferTranscript {
            batch_hash: claim.batch_hash,
            authority_digest: claim.authority_digest,
            poseidon_preimage_digest: claim.poseidon_preimage_digest,
            deltas: claim
                .deltas
                .iter()
                .map(|delta| TransferDeltaTranscript {
                    from_account: delta.from_account.clone(),
                    to_account: delta.to_account.clone(),
                    asset_definition: delta.asset_definition.clone(),
                    amount: delta.amount.clone(),
                    from_balance_before: delta.from_balance_before.clone(),
                    from_balance_after: delta.from_balance_after.clone(),
                    to_balance_before: delta.to_balance_before.clone(),
                    to_balance_after: delta.to_balance_after.clone(),
                    from_smt_witness: TransferSmtWitness::default(),
                    to_smt_witness: TransferSmtWitness::default(),
                })
                .collect(),
        }];
        let (old_root, new_root) =
            transfer::attach_transfer_smt_witnesses(&mut transcripts).unwrap();
        inputs.old_root = old_root;
        inputs.new_root = new_root;
        let prepared = prepare(&rows, &claims, inputs, ProofSemantics::StateTransition);
        let statements = prepared.compact_statements(&[]).unwrap();
        let delta = &transcripts[0].deltas[0];
        let paths = [&delta.from_smt_witness, &delta.to_smt_witness];
        for (path, update) in paths.iter().zip(statements[0].updates) {
            assert_eq!(path.path_bits.as_slice(), update.path.to_le_bytes());
            assert_eq!(path.siblings.len(), PATH_LEVELS);
        }
        let siblings = core::array::from_fn(|update| {
            core::array::from_fn(|level| {
                let bytes = paths[update].siblings[level];
                core::array::from_fn(|limb| {
                    u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
                })
            })
        });
        let witness = SmtWitness::from_inputs(&statements[0], &siblings)
            .unwrap()
            .into_physical();
        let mut columns = (0..COLUMN_COUNT)
            .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
            .collect::<Vec<_>>();
        for row in witness.rows() {
            for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
                column.push(value);
            }
        }
        drop(witness);
        drop(transcripts);
        let construction = started.elapsed();
        let air = PublicTransferAir::new(&prepared, &expected(&prepared)).unwrap();
        let proving_started = std::time::Instant::now();
        let proof = super::super::compact_protocol::prove(&air, &columns).unwrap();
        let proving = proving_started.elapsed();
        drop(columns);

        struct VerifyOnly<'a>(&'a PublicTransferAir);
        impl FixedAir for VerifyOnly<'_> {
            fn schema(&self) -> FixedAirSchema {
                self.0.schema()
            }
            fn statement_bytes(&self) -> &[u8] {
                self.0.statement_bytes()
            }
            fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> Result<Vec<u64>> {
                self.0.evaluate(point, current, next)
            }
            fn prepare_prover(&self) -> Result<Box<dyn PreparedAir + '_>> {
                panic!("verification must not prepare or replay a private transfer trace")
            }
        }
        let verifier = VerifyOnly(&air);
        assert!(matches!(
            super::super::compact_protocol::verify(&verifier, &proof, VerifyLimits::default()),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        let limits = VerifyLimits {
            max_proof_bytes: 4 * 1024 * 1024,
            ..VerifyLimits::default()
        };
        let verifying_started = std::time::Instant::now();
        let work = super::super::compact_protocol::verify(&verifier, &proof, limits).unwrap();
        let verifying = verifying_started.elapsed();
        assert_eq!(work.air_evaluations, 136);
        assert_eq!(work.row_leaves, 272);
        assert_eq!(work.fri_queries, 136);
        assert_eq!(work.proof_bytes, 2_865_251);
        let shared_conversion_started = std::time::Instant::now();
        let shared = super::super::compact_protocol::shared_openings::from_compact(
            &verifier, &proof, limits,
        )
        .unwrap();
        let shared_conversion = shared_conversion_started.elapsed();
        let shared_verifying_started = std::time::Instant::now();
        let shared_work = super::super::compact_protocol::shared_openings::verify_shared(
            &verifier, &shared, limits,
        )
        .unwrap();
        let shared_verifying = shared_verifying_started.elapsed();
        let encoded = {
            let _canonical =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::core::to_bytes(&shared).unwrap()
        };
        let raw_verifying_started = std::time::Instant::now();
        let raw_work = super::super::compact_protocol::shared_openings::codec::decode_and_verify(
            &verifier, &encoded, limits,
        )
        .unwrap();
        let raw_verifying = raw_verifying_started.elapsed();
        assert_eq!(raw_work, shared_work);
        let facade_started = std::time::Instant::now();
        let verified = super::super::compact_public_api::verify_transfer(
            &prepared,
            &expected(&prepared),
            &encoded,
            limits,
        )
        .unwrap();
        let facade_verifying = facade_started.elapsed();
        assert_eq!(verified.public_io(), expected(&prepared));
        assert_eq!(verified.work(), shared_work);
        assert!(matches!(
            super::super::compact_public_api::verify_transfer(
                &prepared,
                &expected(&prepared),
                &encoded,
                VerifyLimits::default(),
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        assert_eq!(shared_work.air_evaluations, 136);
        assert!(shared_work.row_leaves <= 272);
        assert_eq!(shared_work.terminal_degree_checks, 1);
        assert!(shared_work.proof_bytes < work.proof_bytes);
        assert!(matches!(
            super::super::compact_protocol::shared_openings::verify_shared(
                &verifier,
                &shared,
                VerifyLimits::default()
            ),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                ..
            })
        ));
        let mut changed_inputs = inputs;
        changed_inputs.perm_root[31] ^= 1 << 7;
        let changed = prepare(
            &rows,
            &claims,
            changed_inputs,
            ProofSemantics::StateTransition,
        );
        let changed_air = PublicTransferAir::new(&changed, &expected(&changed)).unwrap();
        assert!(super::super::compact_protocol::verify(&changed_air, &proof, limits).is_err());
        assert!(
            super::super::compact_protocol::shared_openings::verify_shared(
                &changed_air,
                &shared,
                limits
            )
            .is_err()
        );
        assert!(
            super::super::compact_protocol::shared_openings::codec::decode_and_verify(
                &changed_air,
                &encoded,
                limits,
            )
            .is_err()
        );
        assert!(
            super::super::compact_public_api::verify_transfer(
                &changed,
                &expected(&changed),
                &encoded,
                limits,
            )
            .is_err()
        );
        // Retain only the public deterministic diagnostic proof, keyed by its
        // exact bytes, for later codec/backend comparisons without reproving.
        // No private witness, signing input or live account data is captured.
        use sha2::{Digest as _, Sha256};
        let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/fastpq-production-validation");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let artifact = artifact_dir.join(format!(
            "compact-shared-transfer-{}.bin",
            hex::encode(Sha256::digest(&encoded))
        ));
        std::fs::write(&artifact, &encoded).unwrap();
        eprintln!(
            "compact_raw_transfer_verifying={raw_verifying:?}; facade_verifying={facade_verifying:?}; work={raw_work:?}; public_fixture_artifact={}",
            artifact.display()
        );
        eprintln!(
            "compact_shared_transfer_conversion={shared_conversion:?}; verifying={shared_verifying:?}; work={shared_work:?}; default_admitted=false; production_security_qualified=false"
        );
        eprintln!(
            "compact_typed_transfer_construction={construction:?}; proving={proving:?}; verifying={verifying:?}; work={work:?}; default_admitted=false; diagnostic_limit={}; production_security_qualified=false",
            limits.max_proof_bytes
        );
    }
}
