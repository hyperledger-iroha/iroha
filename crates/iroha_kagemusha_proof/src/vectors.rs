//! **Prototype** deterministic sample witnesses (the M7 witness
//! distributions of `m7_step`, extended to the spec section 3 core) and the
//! relation mutations, for tests and measurements.
//!
//! [`sample_witness`] is a pure function of the seed (a `SplitMix64` stream),
//! so every test and measurement can name its witness by `(seed, relation,
//! mutation)`. The values follow M7: a balance near `2^100` (or at the
//! mutation's boundary), amounts below `2^56`, fees below `2^24`, `u64`
//! sequence and ordinals, policy epochs from 256 and accepted times in a
//! 600-second window above the floor. A `sigma_send` takes a lineage
//! `burned_total` below `2^49` that is at least the core's.

use iroha_pasta::PastaField;
use iroha_plonk_gadgets::statement::StepRelation;

use crate::witness::{
    Controls, CoreState, Identity, LIFECYCLE_ACTIVE, LineageInputs, MapRoots, OTHER_CARRIED_FIELDS,
    ReceiveInputs, RequestTerms, STATE_VERSION, SendInputs, StateRemainder, StateV1, StepInputs,
    StepWitness,
};

/// A relation mutation (each must be rejected by the relation it targets;
/// the other step stays honest).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mutation {
    /// The honest witness.
    #[default]
    None,
    /// The balance is one below `amount + fee` (an overdraft for
    /// `sigma_send`).
    Overdraft,
    /// The balance is `2^128 - amount`, so `balance + amount = 2^128` (an
    /// overflow for `sigma_recv`).
    Overflow,
    /// `sigma_send`: the Request policy epoch is one newer than the
    /// payer's.
    StaleEpoch,
    /// `sigma_send`: the accepted lower time (and the Request time) is one
    /// below the floor.
    EarlyTime,
    /// `sigma_send`: the balance covers `amount + fee` exactly, but the
    /// lineage `burned_total` (at least 1) leaves less spendable.
    Burned,
    /// The counterparty wallet is the wallet's own (payer = receiver).
    SelfPayment,
    /// `sigma_send`: an enabled control in the mask.
    ControlsEnabled,
}

impl Mutation {
    /// Every mutation, honest first.
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::Overdraft,
        Self::Overflow,
        Self::StaleEpoch,
        Self::EarlyTime,
        Self::Burned,
        Self::SelfPayment,
        Self::ControlsEnabled,
    ];
}

/// The `SplitMix64` stream (Steele, Lea and Flood), the M7 sampler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    /// A stream seeded with `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next word.
    pub const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// The next 128-bit value.
    pub const fn next_u128(&mut self) -> u128 {
        let high = self.next_u64() as u128;
        let low = self.next_u64() as u128;
        (high << 64) | low
    }

    /// The next 32 bytes.
    pub fn next_bytes(&mut self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for chunk in bytes.chunks_exact_mut(8) {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        bytes
    }

    /// The next field element (a 256-bit word reduced modulo the field).
    pub fn next_field<F: PastaField>(&mut self) -> F {
        F::from_raw_reduced([
            self.next_u64(),
            self.next_u64(),
            self.next_u64(),
            self.next_u64(),
        ])
    }
}

/// The sample witness of `relation` for `seed`, with `mutation` applied.
#[must_use]
pub fn sample_witness<F: PastaField>(
    seed: u64,
    relation: StepRelation,
    mutation: Mutation,
) -> StepWitness<F> {
    let mut rng = SplitMix64::new(seed ^ 0x6b67_6d73_6967_6d61);
    let amount = u128::from(rng.next_u64() >> 8).saturating_add(1);
    let fee = u128::from(rng.next_u64() >> 40);
    let core_burned = u128::from(rng.next_u64() >> 17);
    let lineage_burned = match mutation {
        Mutation::Burned => core_burned.saturating_add(1),
        _ => core_burned.saturating_add(u128::from(rng.next_u64() >> 17)),
    };
    let debit = amount.saturating_add(fee);
    let balance = match mutation {
        Mutation::Overdraft => debit.saturating_sub(1),
        Mutation::Burned => debit,
        Mutation::Overflow => (u128::MAX - amount).saturating_add(1),
        _ => (1_u128 << 100) | (rng.next_u128() >> 30),
    };
    let policy_epoch = (rng.next_u64() >> 24).saturating_add(256);
    let accepted_time_floor = (rng.next_u64() >> 24).saturating_add(1);
    let identity = Identity {
        scheme_id: rng.next_bytes(),
        asset: rng.next_bytes(),
        wallet_id: rng.next_bytes(),
        credential: rng.next_bytes(),
    };
    let core = CoreState {
        lifecycle: LIFECYCLE_ACTIVE,
        identity,
        balance,
        burned_total: core_burned,
        sequence: u128::from(rng.next_u64()),
        next_send: u128::from(rng.next_u64()),
        next_load: u128::from(rng.next_u64()),
        next_redeem: u128::from(rng.next_u64() >> 32),
        send_chain: rng.next_field(),
        recv_chain: rng.next_field(),
        roots: MapRoots {
            consumed_credit: rng.next_field(),
            pending_outgoing: rng.next_field(),
            load_redeem_recovery: rng.next_field(),
            fee_claim_recovery: rng.next_field(),
            quota_usage: rng.next_field(),
        },
        controls: Controls {
            enabled: u64::from(mutation == Mutation::ControlsEnabled),
            quota_windows_root: rng.next_field(),
            blacklist_version: rng.next_u64() >> 40,
            blacklist_root: rng.next_field(),
            lease_expiry: rng.next_u64() >> 20,
        },
        policy_epoch,
        accepted_time_floor,
        state_nonce: rng.next_field(),
    };
    let remainder = StateRemainder {
        version: STATE_VERSION,
        regulatory_policy: rng.next_bytes(),
        other: core::array::from_fn::<F, OTHER_CARRIED_FIELDS, _>(|_| rng.next_field()),
    };
    let request_policy_epoch = match mutation {
        Mutation::StaleEpoch => policy_epoch.saturating_add(1),
        _ => policy_epoch.saturating_sub(rng.next_u64() & 0xff),
    };
    let request_time = match mutation {
        // Below the floor, so the early accepted time breaks only the floor.
        Mutation::EarlyTime => accepted_time_floor.saturating_sub(1),
        _ => accepted_time_floor.saturating_add(rng.next_u64() & 0xffff),
    };
    let request = RequestTerms {
        amount,
        fee,
        fee_schedule: rng.next_bytes(),
        policy_epoch: request_policy_epoch,
        scheme_policy: rng.next_bytes(),
        request_time,
        certificates: rng.next_bytes(),
        nonce: rng.next_bytes(),
    };
    let counterparty = if mutation == Mutation::SelfPayment {
        identity.wallet_id
    } else {
        rng.next_bytes()
    };
    let inputs = match relation {
        StepRelation::Send => {
            let accepted_lower = match mutation {
                Mutation::EarlyTime => accepted_time_floor.saturating_sub(1),
                _ => request_time.saturating_add(rng.next_u64() & 0xffff),
            };
            StepInputs::Send(Box::new(SendInputs {
                receiver_wallet: counterparty,
                receiver_credential: rng.next_bytes(),
                request,
                // A stand-in Request digest derived from the nonce, so the
                // sample stream of every other value is unchanged.
                request_digest: request.nonce.map(|byte| byte ^ 0x5a),
                accepted_lower,
                accepted_upper: accepted_lower.saturating_add(600_000),
                lineage: LineageInputs {
                    burned_total: lineage_burned,
                    pending_outgoing_root: rng.next_field(),
                },
                successor_pending_outgoing: rng.next_field(),
                successor_fee_claim: rng.next_field(),
            }))
        }
        StepRelation::Receive => StepInputs::Receive(Box::new(ReceiveInputs {
            payer_wallet: counterparty,
            send_ordinal: u128::from(rng.next_u64()),
            request,
            successor_consumed_credit: rng.next_field(),
        })),
    };
    StepWitness {
        predecessor: StateV1 { core, remainder },
        successor_nonce: rng.next_field(),
        inputs,
    }
}

#[cfg(test)]
mod tests {
    use iroha_pasta::{Fp, Fq};

    use super::*;
    use crate::witness::{StateLayout, Violation};

    #[test]
    fn splitmix_matches_the_reference_stream() {
        // Reference values of SplitMix64 seeded with 0 (Vigna's C code).
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        let mut rng = SplitMix64::new(0);
        assert_eq!(rng.next_u128(), 0xe220_a839_7b1d_cdaf_6e78_9e6a_a1b9_65f4);
        let bytes = SplitMix64::new(0).next_bytes();
        assert_eq!(bytes[..8], 0xe220_a839_7b1d_cdaf_u64.to_le_bytes());
        let field: Fq = SplitMix64::new(0).next_field();
        assert_eq!(
            field,
            Fq::from_raw_reduced([
                0xe220_a839_7b1d_cdaf,
                0x6e78_9e6a_a1b9_65f4,
                0x06c4_5d18_8009_454f,
                0xf88b_b8a8_724c_81ec,
            ])
        );
    }

    #[test]
    fn samples_are_deterministic_and_honest() {
        for relation in [StepRelation::Send, StepRelation::Receive] {
            for seed in 0..8 {
                let witness = sample_witness::<Fp>(seed, relation, Mutation::None);
                assert_eq!(
                    witness,
                    sample_witness::<Fp>(seed, relation, Mutation::None)
                );
                assert_eq!(witness.relation(), relation);
                for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
                    assert!(witness.evaluate(layout).is_honest(), "seed {seed}");
                }
            }
        }
        assert_ne!(
            sample_witness::<Fp>(1, StepRelation::Send, Mutation::None),
            sample_witness::<Fp>(2, StepRelation::Send, Mutation::None)
        );
    }

    #[test]
    fn mutations_hit_their_boundaries() {
        let send = |mutation| sample_witness::<Fp>(4, StepRelation::Send, mutation);
        let StepInputs::Send(inputs) = send(Mutation::Overdraft).inputs else {
            panic!("send");
        };
        assert_eq!(
            send(Mutation::Overdraft).predecessor.core.balance + 1,
            inputs.request.amount + inputs.request.fee
        );
        // Burned: the balance alone covers the debit, the lineage
        // burned_total does not leave enough.
        let burned = send(Mutation::Burned);
        let StepInputs::Send(inputs) = &burned.inputs else {
            panic!("send");
        };
        assert_eq!(
            burned.predecessor.core.balance,
            inputs.request.amount + inputs.request.fee
        );
        assert!(inputs.lineage.burned_total >= 1);
        assert!(inputs.lineage.burned_total >= burned.predecessor.core.burned_total);
        let receive = sample_witness::<Fp>(4, StepRelation::Receive, Mutation::Overflow);
        assert_eq!(
            receive
                .predecessor
                .core
                .balance
                .checked_add(receive.inputs.amount() - 1),
            Some(u128::MAX)
        );
        // The send-only mutations leave a receive honest.
        for mutation in [
            Mutation::Overdraft,
            Mutation::StaleEpoch,
            Mutation::EarlyTime,
            Mutation::Burned,
            Mutation::ControlsEnabled,
        ] {
            let receive = sample_witness::<Fp>(4, StepRelation::Receive, mutation);
            assert!(
                receive.evaluate(StateLayout::TwoLevel).is_honest(),
                "{mutation:?}"
            );
        }
        // A balance of `2^128 - amount` affords a send.
        assert!(
            sample_witness::<Fp>(4, StepRelation::Send, Mutation::Overflow)
                .evaluate(StateLayout::TwoLevel)
                .is_honest()
        );
        assert_eq!(
            send(Mutation::Overdraft)
                .evaluate(StateLayout::Flat)
                .violations,
            vec![Violation::Overdraft]
        );
        for step in [StepRelation::Send, StepRelation::Receive] {
            assert_eq!(
                sample_witness::<Fp>(4, step, Mutation::SelfPayment)
                    .evaluate(StateLayout::Flat)
                    .violations,
                vec![Violation::SelfPayment]
            );
        }
        assert_eq!(Mutation::ALL.len(), 8);
        assert_eq!(Mutation::default(), Mutation::None);
    }
}
