//! Bounded public schedule for future ordered compact SMT batch composition.
//!
//! Actual updates come exclusively from the original whole-batch public
//! preparation, preserving its scales, key allocation and occurrence order.
//! Capacity is the next power of two in deltas. Unused capacity consists of
//! identity updates at the last real credit's public new leaf and allocated
//! path; these scheduled fillers are never additional public transfer claims.
//!
//! Row labels below describe only the base subgroup schedule. They are not a
//! coset evaluator, proof-selected opcodes or production geometry admission.
//! The eight-limb edge reference covers only nonfinal update root chaining;
//! callers still need all hash, leaf, old-root and final-root constraints.
//!
//! TODO: Derive and review the generalized fixed polynomial masks, bind the
//! complete public statement/actual count/padding rule before challenges, and
//! qualify any larger profile separately. This cfg(test) module changes no
//! proof engine, canonical domain, resource default or mandatory replay path.

use super::{
    compact_blake2b_air,
    compact_smt_air::{
        DigestLimbs, HASH_COUNT, HASHES_PER_LEVEL, PATH_LEVELS, PHYSICAL_HASH_ROWS,
        PHYSICAL_ROW_COUNT, PHYSICAL_ROWS_PER_UPDATE, PublicUpdate, UPDATE_COUNT,
    },
    public_transfer_statement::PreparedPublicTransfers,
    transfer_integer_air::IntegerAirField,
};
use crate::{Error, Result};

/// Checked integer counts; constructing these never allocates a trace or updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BatchScheduleCounts {
    /// Original public delta count, excluding all internal capacity fillers.
    pub(crate) actual_deltas: usize,
    /// Power-of-two number of delta slots in the prospective physical schedule.
    pub(crate) capacity_deltas: usize,
    /// Exact public debit/credit updates: twice the original delta count.
    pub(crate) actual_updates: usize,
    /// All scheduled updates, including deterministic identity fillers.
    pub(crate) scheduled_updates: usize,
    /// Old and new node-hash invocations across all scheduled updates.
    pub(crate) hashes: usize,
    /// Prospective physical base rows; this is not an admitted domain size.
    pub(crate) rows: usize,
    /// Checked payload bytes for the complete stored update vector.
    pub(crate) update_bytes: usize,
}

impl BatchScheduleCounts {
    fn checked(actual_deltas: usize, maximum: usize) -> Result<Self> {
        if actual_deltas == 0 {
            return Err(shape(
                "compact SMT batch schedule requires a nonempty public delta table",
            ));
        }
        if actual_deltas > maximum {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_compact_smt_batch_deltas",
                actual: actual_deltas,
                max: maximum,
            });
        }
        let capacity_deltas = actual_deltas
            .checked_next_power_of_two()
            .ok_or_else(|| shape("compact SMT batch capacity overflow"))?;
        let actual_updates = actual_deltas
            .checked_mul(UPDATE_COUNT)
            .ok_or_else(|| shape("compact SMT batch actual-update count overflow"))?;
        let scheduled_updates = capacity_deltas
            .checked_mul(UPDATE_COUNT)
            .ok_or_else(|| shape("compact SMT batch scheduled-update count overflow"))?;
        let hashes = capacity_deltas
            .checked_mul(HASH_COUNT)
            .ok_or_else(|| shape("compact SMT batch hash count overflow"))?;
        let rows = capacity_deltas
            .checked_mul(PHYSICAL_ROW_COUNT)
            .ok_or_else(|| shape("compact SMT batch row count overflow"))?;
        let update_bytes = scheduled_updates
            .checked_mul(core::mem::size_of::<PublicUpdate>())
            .ok_or_else(|| shape("compact SMT batch update storage overflow"))?;
        Ok(Self {
            actual_deltas,
            capacity_deltas,
            actual_updates,
            scheduled_updates,
            hashes,
            rows,
            update_bytes,
        })
    }
}

/// Base-subgroup schedule facts derived from one checked integer row index.
///
/// Never pass a sampled LDE index here to choose AIR equations. A future coset
/// evaluator must interpolate these fixed events on its declared subgroup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BatchBaseRow {
    /// Absolute base schedule row, always below `counts().rows`.
    pub(crate) index: usize,
    /// Scheduled delta-slot ordinal; ordinals at/after the actual count are fillers.
    pub(crate) delta: usize,
    /// Global debit/credit update ordinal.
    pub(crate) update: usize,
    /// Global old/new node-hash invocation ordinal.
    pub(crate) hash: usize,
    /// Hash-local physical phase, in the exact range 0 through 511.
    pub(crate) phase: usize,
    /// Least-significant-bit-first path level, in the range 0 through 31.
    pub(crate) level: usize,
    /// Whether the current hash computes the new child rather than the old child.
    pub(crate) is_new: bool,
    /// Whether this update is internal capacity padding, not a public delta.
    pub(crate) is_filler: bool,
    /// Existing per-hash zero-cell/carry padding phases 408 through 511.
    pub(crate) is_hash_padding: bool,
    /// The only globally initial row that binds the public source root.
    pub(crate) is_first_row: bool,
    /// Phase 407 of any hash, where its output is exported.
    pub(crate) is_hash_export: bool,
    /// Last physical row of a complete balance update, including the final update.
    pub(crate) is_update_end: bool,
    /// Nonfinal update end that seeds the next starting root from the new child.
    pub(crate) reset_starting_root: bool,
    /// Export of the globally final hash, separately from its final padding row.
    pub(crate) is_final_export: bool,
    /// Globally final padding row; it has no cyclic root-link edge.
    pub(crate) is_final_row: bool,
}

/// Bounded ordered public updates and their deterministic prospective capacity.
///
/// Construction establishes a schedule only. It does not verify a private SMT
/// path, bind a Fiat–Shamir transcript, authenticate public endpoints, admit an
/// enlarged domain or authorize a transfer. Actual and scheduled update views
/// deliberately remain separate so a bundle cannot mistake fillers for claims.
#[derive(Debug)]
pub(crate) struct CompactBatchSchedule {
    counts: BatchScheduleCounts,
    updates: Vec<PublicUpdate>,
    old_root: DigestLimbs,
    new_root: DigestLimbs,
}

impl CompactBatchSchedule {
    /// Derive exact ordered updates from the immutable whole public batch.
    ///
    /// `max_actual_deltas` is an explicit test caller limit. Counts, padding and
    /// storage products are checked before reserving or copying any updates.
    /// Empty schedules reject rather than inventing a last-credit filler seed.
    pub(crate) fn new(
        prepared: &PreparedPublicTransfers<'_>,
        max_actual_deltas: usize,
    ) -> Result<Self> {
        let pairs = prepared.pairs();
        let counts = BatchScheduleCounts::checked(pairs.len(), max_actual_deltas)?;
        let last_credit = pairs
            .last()
            .ok_or_else(|| shape("compact SMT batch schedule has no final credit"))?
            .updates[UPDATE_COUNT - 1];
        let filler = PublicUpdate {
            old_leaf: last_credit.new_leaf,
            new_leaf: last_credit.new_leaf,
            path: last_credit.path,
        };
        let mut updates = Vec::new();
        updates
            .try_reserve_exact(counts.scheduled_updates)
            .map_err(|_| shape("compact SMT batch update allocation failed"))?;
        for pair in pairs {
            updates.extend(pair.updates);
        }
        updates.resize(counts.scheduled_updates, filler);
        Ok(Self {
            counts,
            updates,
            old_root: digest_limbs(prepared.public_inputs().old_root),
            new_root: digest_limbs(prepared.public_inputs().new_root),
        })
    }

    /// Return checked actual/capacity/work counts without exposing mutable state.
    pub(crate) const fn counts(&self) -> BatchScheduleCounts {
        self.counts
    }

    /// Original public update sequence, excluding every internal filler.
    pub(crate) fn actual_updates(&self) -> &[PublicUpdate] {
        &self.updates[..self.counts.actual_updates]
    }

    /// Complete scheduled update sequence; its suffix is not additional public claims.
    pub(crate) fn scheduled_updates(&self) -> &[PublicUpdate] {
        &self.updates
    }

    /// Original complete public endpoint limbs, without field reduction or hashing.
    pub(crate) const fn public_roots(&self) -> (DigestLimbs, DigestLimbs) {
        (self.old_root, self.new_root)
    }

    /// Decode one checked base row into deterministic global schedule events.
    pub(crate) fn row(&self, index: usize) -> Result<BatchBaseRow> {
        if index >= self.counts.rows {
            return Err(Error::QueryIndexOutOfRange {
                index,
                len: self.counts.rows,
            });
        }
        let update = index / PHYSICAL_ROWS_PER_UPDATE;
        let hash = index / PHYSICAL_HASH_ROWS;
        let phase = index % PHYSICAL_HASH_ROWS;
        let is_final_row = index == self.counts.rows - 1;
        let is_update_end = index % PHYSICAL_ROWS_PER_UPDATE == PHYSICAL_ROWS_PER_UPDATE - 1;
        let is_hash_export = phase == compact_blake2b_air::ROW_COUNT - 1;
        Ok(BatchBaseRow {
            index,
            delta: update / UPDATE_COUNT,
            update,
            hash,
            phase,
            level: (hash / HASHES_PER_LEVEL) % PATH_LEVELS,
            is_new: hash % HASHES_PER_LEVEL == 1,
            is_filler: update >= self.counts.actual_updates,
            is_hash_padding: phase >= compact_blake2b_air::ROW_COUNT,
            is_first_row: index == 0,
            is_hash_export,
            is_update_end,
            reset_starting_root: is_update_end && !is_final_row,
            is_final_export: hash == self.counts.hashes - 1 && is_hash_export,
            is_final_row,
        })
    }

    /// Reference the eight linear nonfinal-update root-link numerators.
    ///
    /// At every debit→credit, credit→next-delta and real→filler update edge,
    /// return `next_starting_root - current_new_child` for all eight limbs.
    /// Other rows, including the globally final row, return `None`. Inputs obey
    /// the canonical field precondition of `IntegerAirField`; this helper is not
    /// a decoder, an acceptance decision or a fixed-domain selector polynomial.
    pub(crate) fn root_link_residues<F: IntegerAirField>(
        &self,
        index: usize,
        current_new_child: &[F; 8],
        next_starting_root: &[F; 8],
    ) -> Result<Option<[F; 8]>> {
        let row = self.row(index)?;
        Ok(row.reset_starting_root.then(|| {
            core::array::from_fn(|limb| next_starting_root[limb].sub(current_new_child[limb]))
        }))
    }
}

fn digest_limbs(bytes: [u8; 32]) -> DigestLimbs {
    core::array::from_fn(|limb| {
        u32::from_le_bytes([
            bytes[4 * limb],
            bytes[4 * limb + 1],
            bytes[4 * limb + 2],
            bytes[4 * limb + 3],
        ])
    })
}

fn shape(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GoldilocksFp4V1, OperationKind, ProofSemantics, PublicInputs, StateTransition,
        field::{GOLDILOCKS_MODULUS_V1, add_base},
        gadgets::{
            compact_smt_air::PhysicalRowIndex,
            public_transfer_statement::{
                PublicTransferDelta, PublicTransferLimits, PublicTransferTranscript,
                prepare_public_transfers,
            },
        },
    };
    use iroha_crypto::Hash;
    use iroha_data_model::{DomainId, asset::id::AssetDefinitionId};
    use iroha_primitives::numeric::{Numeric, Quantity};
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use iroha_zkp_halo2::poseidon::PoseidonByteHasher;
    use norito::codec::Encode as _;

    struct PublicFixture {
        rows: Vec<StateTransition>,
        claims: Vec<PublicTransferTranscript>,
        inputs: PublicInputs,
    }

    impl PublicFixture {
        fn new(deltas: usize, self_transfer: bool) -> Self {
            Self::with_amount(deltas, self_transfer, 5)
        }

        fn with_amount(deltas: usize, self_transfer: bool, amount: u64) -> Self {
            // These are public statement facts only. No private path or full
            // trace is generated, and placeholder roots are never called proven.
            let _flags =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            let asset = AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
            let mut rows = Vec::new();
            let mut public_deltas = Vec::new();
            for index in 0..deltas {
                let from_before = if self_transfer {
                    100
                } else {
                    100 - amount * index as u64
                };
                let from_after = from_before - amount;
                let to_before = if self_transfer {
                    from_after
                } else {
                    200 + amount * index as u64
                };
                let to_after = to_before + amount;
                let delta = PublicTransferDelta {
                    from_account: (*ALICE_ID).clone(),
                    to_account: if self_transfer {
                        (*ALICE_ID).clone()
                    } else {
                        (*BOB_ID).clone()
                    },
                    asset_definition: asset.clone(),
                    amount: Quantity::from(amount),
                    from_balance_before: Quantity::from(from_before),
                    from_balance_after: Quantity::from(from_after),
                    to_balance_before: Quantity::from(to_before),
                    to_balance_after: Quantity::from(to_after),
                };
                for (account, before, after) in [
                    (&delta.from_account, from_before, from_after),
                    (&delta.to_account, to_before, to_after),
                ] {
                    rows.push(StateTransition::new(
                        iroha_data_model::fastpq::transfer_balance_key(&asset, account).unwrap(),
                        before.to_le_bytes().to_vec(),
                        after.to_le_bytes().to_vec(),
                        OperationKind::Transfer,
                    ));
                }
                public_deltas.push(delta);
            }
            rows.sort_by(|left, right| left.key.cmp(&right.key));
            let batch_hash = Hash::new(b"compact public batch schedule fixture");
            let poseidon_preimage_digest = if let [delta] = public_deltas.as_slice() {
                Some(single_delta_digest(delta, &batch_hash))
            } else {
                None
            };
            let claims = if public_deltas.is_empty() {
                Vec::new()
            } else {
                vec![PublicTransferTranscript {
                    batch_hash,
                    authority_digest: Hash::new(b"public claimed authority only"),
                    deltas: public_deltas,
                    poseidon_preimage_digest,
                }]
            };
            let old_root = Hash::new(b"public claimed old touched root").into();
            Self {
                rows,
                claims,
                inputs: PublicInputs {
                    dsid: [9; 16],
                    slot: 17,
                    old_root,
                    new_root: if deltas == 0 || amount == 0 || self_transfer {
                        old_root
                    } else {
                        Hash::new(b"public claimed new touched root").into()
                    },
                    perm_root: Hash::new(b"public permission context").into(),
                    tx_set_hash: Hash::new(b"public transaction context").into(),
                },
            }
        }

        fn prepare(&self) -> PreparedPublicTransfers<'_> {
            prepare_public_transfers(
                &self.rows,
                &self.claims,
                self.inputs,
                ProofSemantics::StateTransition,
                PublicTransferLimits::default(),
            )
            .unwrap()
        }
    }

    fn single_delta_digest(delta: &PublicTransferDelta, batch_hash: &Hash) -> Hash {
        let mut hasher = PoseidonByteHasher::new();
        delta.from_account.encode_to(&mut hasher);
        delta.to_account.encode_to(&mut hasher);
        delta.asset_definition.encode_to(&mut hasher);
        delta.amount.encode_to(&mut hasher);
        hasher.update(batch_hash.as_ref());
        Hash::prehashed(hasher.finalize())
    }

    #[test]
    fn count_bounds_and_overflow_precede_update_storage() {
        assert!(matches!(
            BatchScheduleCounts::checked(0, 4),
            Err(Error::InvalidTraceShape { .. })
        ));
        assert!(matches!(
            BatchScheduleCounts::checked(1, 0),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_smt_batch_deltas",
                actual: 1,
                max: 0
            })
        ));
        let counts = BatchScheduleCounts::checked(3, 3).unwrap();
        assert_eq!(counts.actual_deltas, 3);
        assert_eq!(counts.capacity_deltas, 4);
        assert_eq!(counts.actual_updates, 6);
        assert_eq!(counts.scheduled_updates, 8);
        assert_eq!(counts.hashes, 512);
        assert_eq!(counts.rows, 262_144);
        assert_eq!(
            counts.update_bytes,
            8 * core::mem::size_of::<PublicUpdate>()
        );
        let high = 1_usize << (usize::BITS - 1);
        for count in [usize::MAX, high, high / 2, high / HASH_COUNT] {
            assert!(matches!(
                BatchScheduleCounts::checked(count, usize::MAX),
                Err(Error::InvalidTraceShape { .. })
            ));
        }
        // Large arithmetic geometry can be inspected without any vector or
        // trace allocation. It still grants no field-domain/profile support.
        let largest = high / PHYSICAL_ROW_COUNT;
        let large = BatchScheduleCounts::checked(largest, largest).unwrap();
        assert_eq!(large.rows, high);
        assert_eq!(large.hashes * PHYSICAL_HASH_ROWS, large.rows);
        let empty = PublicFixture::new(0, false);
        assert!(matches!(
            CompactBatchSchedule::new(&empty.prepare(), 4),
            Err(Error::InvalidTraceShape { .. })
        ));
        let two = PublicFixture::new(2, false);
        assert!(matches!(
            CompactBatchSchedule::new(&two.prepare(), 1),
            Err(Error::VerifierLimitExceeded {
                limit: "max_compact_smt_batch_deltas",
                actual: 2,
                max: 1
            })
        ));
    }

    #[test]
    fn one_delta_metadata_matches_every_existing_physical_position() {
        let fixture = PublicFixture::new(1, false);
        let prepared = fixture.prepare();
        let schedule = CompactBatchSchedule::new(&prepared, 1).unwrap();
        assert_eq!(schedule.counts().rows, PHYSICAL_ROW_COUNT);
        assert_eq!(schedule.counts().hashes, HASH_COUNT);
        assert_eq!(schedule.actual_updates(), &prepared.pairs()[0].updates);
        assert_eq!(schedule.actual_updates(), schedule.scheduled_updates());
        for index in 0..PHYSICAL_ROW_COUNT {
            let current = PhysicalRowIndex::new(index).unwrap();
            let row = schedule.row(index).unwrap();
            assert_eq!(row.index, current.get());
            assert_eq!(row.delta, 0);
            assert_eq!(row.update, current.update());
            assert_eq!(row.hash, current.hash_ordinal());
            assert_eq!(row.phase, current.phase());
            assert_eq!(row.level, current.level());
            assert_eq!(row.is_new, current.is_new());
            assert_eq!(row.is_hash_padding, current.is_padding());
            assert!(!row.is_filler);
            assert_eq!(row.is_first_row, index == 0);
            assert_eq!(row.is_final_row, index == PHYSICAL_ROW_COUNT - 1);
            assert_eq!(
                row.reset_starting_root,
                index == PHYSICAL_ROWS_PER_UPDATE - 1
            );
        }
        for index in [PHYSICAL_ROW_COUNT, usize::MAX] {
            assert!(
                matches!(schedule.row(index), Err(Error::QueryIndexOutOfRange { index: actual, len }) if actual == index && len == PHYSICAL_ROW_COUNT)
            );
            assert!(matches!(
                schedule.root_link_residues(index, &[0_u64; 8], &[0; 8]),
                Err(Error::QueryIndexOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn two_deltas_preserve_full_batch_order_and_link_all_three_real_boundaries() {
        for self_transfer in [false, true] {
            let fixture = PublicFixture::new(2, self_transfer);
            let prepared = fixture.prepare();
            let schedule = CompactBatchSchedule::new(&prepared, 2).unwrap();
            let expected: Vec<_> = prepared
                .pairs()
                .iter()
                .flat_map(|pair| pair.updates)
                .collect();
            assert_eq!(schedule.actual_updates(), expected);
            assert_eq!(schedule.scheduled_updates(), expected);
            assert_eq!(schedule.counts().actual_deltas, 2);
            assert_eq!(schedule.counts().capacity_deltas, 2);
            assert_eq!(prepared.claims()[0].deltas.len(), 2);
            for update in 0..4 {
                let index = (update + 1) * PHYSICAL_ROWS_PER_UPDATE - 1;
                let row = schedule.row(index).unwrap();
                assert_eq!(row.update, update);
                assert_eq!(row.delta, update / 2);
                assert!(row.is_update_end);
                assert_eq!(row.reset_starting_root, update != 3);
                assert_eq!(row.is_final_row, update == 3);
                assert!(!row.is_filler);
                assert_eq!(
                    schedule
                        .root_link_residues(index, &[7_u64; 8], &[7; 8])
                        .unwrap(),
                    (update != 3).then_some([0; 8])
                );
            }
            let crossing = schedule.row(PHYSICAL_ROW_COUNT - 1).unwrap();
            assert!(crossing.reset_starting_root);
            assert!(!crossing.is_final_row);
            assert_eq!(schedule.row(PHYSICAL_ROW_COUNT).unwrap().delta, 1);
            assert!(!schedule.row(PHYSICAL_ROW_COUNT).unwrap().is_first_row);
        }
    }

    #[test]
    fn three_real_deltas_have_one_deterministic_identity_filler_slot() {
        let fixture = PublicFixture::new(3, false);
        let prepared = fixture.prepare();
        let schedule = CompactBatchSchedule::new(&prepared, 3).unwrap();
        let counts = schedule.counts();
        assert_eq!((counts.actual_deltas, counts.capacity_deltas), (3, 4));
        assert_eq!(
            (
                schedule.actual_updates().len(),
                schedule.scheduled_updates().len()
            ),
            (6, 8)
        );
        let last = prepared.pairs()[2].updates[1];
        let filler = PublicUpdate {
            old_leaf: last.new_leaf,
            new_leaf: last.new_leaf,
            path: last.path,
        };
        assert_eq!(schedule.scheduled_updates()[6..], [filler; 2]);
        assert_ne!(filler, last);
        assert_eq!(prepared.pairs().len(), 3);
        assert_eq!(prepared.claims()[0].deltas.len(), 3);
        for (update, actual) in schedule.actual_updates().iter().enumerate() {
            assert_eq!(*actual, prepared.pairs()[update / 2].updates[update % 2]);
        }
        assert!(
            schedule
                .row(3 * PHYSICAL_ROW_COUNT - 1)
                .unwrap()
                .reset_starting_root
        );
        assert!(!schedule.row(3 * PHYSICAL_ROW_COUNT - 1).unwrap().is_filler);
        assert!(schedule.row(3 * PHYSICAL_ROW_COUNT).unwrap().is_filler);
        assert!(!schedule.row(counts.rows - 1).unwrap().reset_starting_root);
        assert!(
            schedule
                .root_link_residues(counts.rows - 1, &[1_u64; 8], &[99; 8])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn whole_batch_scale_and_order_are_preserved_across_transcript_boundaries() {
        let mut fixture = PublicFixture::new(2, false);
        let deltas = &mut fixture.claims[0].deltas;
        deltas[0].amount = Quantity::from(1_u64);
        deltas[0].from_balance_after = Quantity::from(99_u64);
        deltas[0].to_balance_after = Quantity::from(201_u64);
        deltas[1].amount = Quantity::try_from_numeric(Numeric::new(25, 2)).unwrap();
        deltas[1].from_balance_before = Quantity::from(99_u64);
        deltas[1].from_balance_after = Quantity::try_from_numeric(Numeric::new(9875, 2)).unwrap();
        deltas[1].to_balance_before = Quantity::from(201_u64);
        deltas[1].to_balance_after = Quantity::try_from_numeric(Numeric::new(20125, 2)).unwrap();
        let deltas = fixture.claims.pop().unwrap().deltas;
        fixture.rows.clear();
        for (ordinal, (delta, values)) in deltas
            .into_iter()
            .zip([
                [10_000_u64, 9_900, 20_000, 20_100],
                [9_900, 9_875, 20_100, 20_125],
            ])
            .enumerate()
        {
            for (account, before, after) in [
                (&delta.from_account, values[0], values[1]),
                (&delta.to_account, values[2], values[3]),
            ] {
                fixture.rows.push(StateTransition::new(
                    iroha_data_model::fastpq::transfer_balance_key(
                        &delta.asset_definition,
                        account,
                    )
                    .unwrap(),
                    before.to_le_bytes().to_vec(),
                    after.to_le_bytes().to_vec(),
                    OperationKind::Transfer,
                ));
            }
            let batch_hash = Hash::new([ordinal as u8]);
            let poseidon_preimage_digest = Some(single_delta_digest(&delta, &batch_hash));
            fixture.claims.push(PublicTransferTranscript {
                batch_hash,
                authority_digest: Hash::new(b"public claimed authority only"),
                deltas: vec![delta],
                poseidon_preimage_digest,
            });
        }
        fixture.rows.sort_by(|left, right| left.key.cmp(&right.key));
        let prepared = fixture.prepare();
        let schedule = CompactBatchSchedule::new(&prepared, 2).unwrap();
        assert_eq!(prepared.claims().len(), 2);
        assert!(prepared.rows().iter().all(|row| row.asset_scale == 2));
        for (ordinal, pair) in prepared.pairs().iter().enumerate() {
            assert_eq!(pair.occurrence.transcript_ordinal as usize, ordinal);
            assert_eq!(pair.occurrence.delta_ordinal, 0);
            assert_eq!(pair.occurrence.pair_ordinal as usize, ordinal);
            assert_eq!(
                schedule.actual_updates()[ordinal * 2..ordinal * 2 + 2],
                pair.updates
            );
        }
        let first_pair = &prepared.pairs()[0];
        // The same first public claim prepared alone would use scale zero.
        // This control makes independently re-preparing each delta observable.
        let mut first_rows: Vec<_> = first_pair
            .row_indices
            .iter()
            .map(|index| {
                let mut row = fixture.rows[*index].clone();
                for bytes in [&mut row.pre_value, &mut row.post_value] {
                    let value = u64::from_le_bytes(bytes.as_slice().try_into().unwrap()) / 100;
                    *bytes = value.to_le_bytes().to_vec();
                }
                row
            })
            .collect();
        first_rows.sort_by(|left, right| left.key.cmp(&right.key));
        let isolated = prepare_public_transfers(
            &first_rows,
            &fixture.claims[..1],
            fixture.inputs,
            ProofSemantics::StateTransition,
            PublicTransferLimits::default(),
        )
        .unwrap();
        assert!(isolated.rows().iter().all(|row| row.asset_scale == 0));
        assert_ne!(schedule.actual_updates()[..2], isolated.pairs()[0].updates);
        assert_eq!(schedule.scheduled_updates().len(), 4);
    }

    #[test]
    fn zero_amount_fillers_remain_internal_even_when_all_update_values_match() {
        for self_transfer in [false, true] {
            let fixture = PublicFixture::with_amount(3, self_transfer, 0);
            let prepared = fixture.prepare();
            let schedule = CompactBatchSchedule::new(&prepared, 3).unwrap();
            assert_eq!(schedule.actual_updates().len(), 6);
            assert_eq!(schedule.scheduled_updates().len(), 8);
            assert_eq!(
                schedule.scheduled_updates()[6..],
                [prepared.pairs()[2].updates[1]; 2]
            );
            assert!(
                schedule
                    .actual_updates()
                    .iter()
                    .all(|update| update.old_leaf == update.new_leaf)
            );
            assert_eq!(prepared.claims()[0].deltas.len(), 3);
            for update in 0..8 {
                let row = schedule.row(update * PHYSICAL_ROWS_PER_UPDATE).unwrap();
                assert_eq!(row.is_filler, update >= 6);
            }
            let (old, new) = schedule.public_roots();
            assert_eq!(old, new);
        }
    }

    #[test]
    fn every_hash_export_padding_and_import_has_exact_global_flags() {
        let fixture = PublicFixture::new(3, false);
        let schedule = CompactBatchSchedule::new(&fixture.prepare(), 3).unwrap();
        for hash in 0..schedule.counts().hashes {
            for phase in [0, 407, 408, 511] {
                let index = hash * PHYSICAL_HASH_ROWS + phase;
                let row = schedule.row(index).unwrap();
                assert_eq!(row.hash, hash);
                assert_eq!(row.phase, phase);
                assert_eq!(row.level, (hash / 2) % 32);
                assert_eq!(row.is_new, hash % 2 == 1);
                assert_eq!(row.is_hash_padding, phase >= 408);
                assert_eq!(row.is_hash_export, phase == 407);
                assert_eq!(row.is_update_end, phase == 511 && hash % 64 == 63);
                assert_eq!(
                    row.reset_starting_root,
                    phase == 511 && hash % 64 == 63 && hash != 511
                );
                assert_eq!(row.is_final_export, hash == 511 && phase == 407);
                assert_eq!(row.is_final_row, hash == 511 && phase == 511);
                assert_eq!(row.is_first_row, index == 0);
                assert_eq!(row.is_filler, hash >= 384);
                if phase == 511 && hash + 1 < schedule.counts().hashes {
                    let next = schedule.row(index + 1).unwrap();
                    assert_eq!(next.hash, hash + 1);
                    assert_eq!(next.phase, 0);
                    assert!(!next.is_hash_padding);
                }
                assert_eq!(
                    schedule
                        .root_link_residues(index, &[0_u64; 8], &[0; 8])
                        .unwrap()
                        .is_some(),
                    row.reset_starting_root
                );
            }
        }
    }

    #[test]
    fn public_root_bytes_are_copied_in_full_without_reduction() {
        let mut fixture = PublicFixture::new(2, false);
        fixture.inputs.old_root = [u8::MAX; 32];
        fixture.inputs.new_root =
            core::array::from_fn(|byte| if byte == 31 { 255 } else { byte as u8 });
        let schedule = CompactBatchSchedule::new(&fixture.prepare(), 2).unwrap();
        let (old, new) = schedule.public_roots();
        assert_eq!(old, [u32::MAX; 8]);
        for (bytes, limbs) in [
            (fixture.inputs.old_root, old),
            (fixture.inputs.new_root, new),
        ] {
            let restored: [u8; 32] =
                core::array::from_fn(|byte| limbs[byte / 4].to_le_bytes()[byte % 4]);
            assert_eq!(restored, bytes);
        }
    }

    #[test]
    fn each_root_link_limb_and_full_extension_coordinate_is_independent() {
        let fixture = PublicFixture::new(3, false);
        let schedule = CompactBatchSchedule::new(&fixture.prepare(), 3).unwrap();
        let root: [u64; 8] = core::array::from_fn(|limb| GOLDILOCKS_MODULUS_V1 - 1 - limb as u64);
        let extended: [GoldilocksFp4V1; 8] = core::array::from_fn(|limb| {
            GoldilocksFp4V1::new([
                root[limb],
                limb as u64 + 7,
                limb as u64 + 33,
                GOLDILOCKS_MODULUS_V1 - 50 - limb as u64,
            ])
            .unwrap()
        });
        for update in 0..schedule.counts().scheduled_updates - 1 {
            let index = (update + 1) * PHYSICAL_ROWS_PER_UPDATE - 1;
            assert_eq!(
                schedule.root_link_residues(index, &root, &root).unwrap(),
                Some([0; 8])
            );
            assert_eq!(
                schedule
                    .root_link_residues(index, &extended, &extended)
                    .unwrap(),
                Some([GoldilocksFp4V1::ZERO; 8])
            );
            for limb in 0..8 {
                for side in 0..2 {
                    let mut changed = root;
                    changed[limb] = add_base(changed[limb], 1);
                    let (current, next) = if side == 0 {
                        (&changed, &root)
                    } else {
                        (&root, &changed)
                    };
                    let actual = schedule
                        .root_link_residues(index, current, next)
                        .unwrap()
                        .unwrap();
                    for (position, residue) in actual.into_iter().enumerate() {
                        assert_eq!(residue == 0, position != limb);
                    }
                    let embedded_current =
                        current.map(|word| GoldilocksFp4V1::from_base(word).unwrap());
                    let embedded_next = next.map(|word| GoldilocksFp4V1::from_base(word).unwrap());
                    assert_eq!(
                        schedule
                            .root_link_residues(index, &embedded_current, &embedded_next)
                            .unwrap()
                            .unwrap(),
                        actual.map(|word| GoldilocksFp4V1::from_base(word).unwrap())
                    );
                    for coefficient in 0..4 {
                        let mut changed = extended;
                        let mut words = changed[limb].coefficients();
                        words[coefficient] = add_base(words[coefficient], 1);
                        changed[limb] = GoldilocksFp4V1::new(words).unwrap();
                        let (current, next) = if side == 0 {
                            (&changed, &extended)
                        } else {
                            (&extended, &changed)
                        };
                        let actual = schedule
                            .root_link_residues(index, current, next)
                            .unwrap()
                            .unwrap();
                        for (position, residue) in actual.into_iter().enumerate() {
                            for (coordinate, word) in residue.coefficients().into_iter().enumerate()
                            {
                                assert_eq!(
                                    word == 0,
                                    position != limb || coordinate != coefficient
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Degree(usize);
    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn sub(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn mul(self, other: Self) -> Self {
            Self(self.0 + other.0)
        }
    }

    #[test]
    fn returned_root_link_polynomials_are_exactly_eight_linear_slots() {
        let fixture = PublicFixture::new(3, false);
        let schedule = CompactBatchSchedule::new(&fixture.prepare(), 3).unwrap();
        for update in 0..schedule.counts().scheduled_updates {
            let index = (update + 1) * PHYSICAL_ROWS_PER_UPDATE - 1;
            let residues = schedule
                .root_link_residues(index, &[Degree(1); 8], &[Degree(1); 8])
                .unwrap();
            if update + 1 == schedule.counts().scheduled_updates {
                assert!(residues.is_none());
            } else {
                let residues = residues.unwrap();
                assert_eq!(residues.len(), 8);
                assert!(residues.into_iter().all(|value| value == Degree(1)));
            }
        }
        for index in [0, 407, 408, 511, 512, PHYSICAL_ROWS_PER_UPDATE] {
            assert!(
                schedule
                    .root_link_residues(index, &[Degree(1); 8], &[Degree(1); 8])
                    .unwrap()
                    .is_none()
            );
        }
    }
}
