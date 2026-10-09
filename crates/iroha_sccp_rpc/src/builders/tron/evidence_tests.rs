//! TRON evidence builders over the synthetic TRON chain (spec §4.13.5, §11).
//!
//! The light client is bootstrapped at block 2 600 of maintenance period `P0`. Burns earlier in
//! `P0` are proven by a solid signed segment until the 7-day window after `P0` ends (less the
//! builder's margin), and from then on by `raw_data` headers up to the bootstrap checkpoint:
//! directly up to 1 199 blocks below it, with `Backfill` segments beyond. Every built piece is
//! verified by the production verifier through [`LightClientReplayV1`].

use std::collections::BTreeMap;

use iroha_data_model::sccp::light_client::SccpLightClientParamsV1;
use iroha_sccp::{
    light_client::{self as lc, profile::TRON_MAINNET},
    test_support::tron::{SYNTHETIC_TRON_FIRST_PERIOD, SyntheticTronChainV1, trigger_transaction},
    v1::evm_abi::TransferToTairaCallV1,
};

use super::*;
use crate::builders::{FRESHNESS_MARGIN_MS, LightClientReplayV1};

const TRON: SccpNetworkV1 = SccpNetworkV1::TronMainnet;
const CHECKPOINT: u64 = 2_600;
const CONTRACT: [u8; 21] = tron_address(0x42);
const OWNER: [u8; 21] = tron_address(0x07);

/// A TRON address: the `0x41` prefix, then 20 bytes of `fill`.
const fn tron_address(fill: u8) -> [u8; 21] {
    let mut address = [fill; 21];
    address[0] = 0x41;
    address
}

fn params() -> SccpLightClientParamsV1 {
    SccpLightClientParamsV1::defaults_for(TRON).expect("external network")
}

/// TRON data served from the synthetic chain.
struct SyntheticSource {
    chain: SyntheticTronChainV1,
    head: u64,
    /// Transaction id → (height, index).
    transactions: BTreeMap<[u8; 32], (u64, usize)>,
}

impl TronSource for SyntheticSource {
    fn transaction(&self, tx_id: &[u8; 32]) -> Result<(u64, TronTransactionProofV1), BuildError> {
        let (height, index) = self
            .transactions
            .get(tx_id)
            .copied()
            .ok_or_else(|| BuildError::Unavailable("not solidified".into()))?;
        Ok((height, self.chain.transaction_proof(height, index)))
    }

    fn head_number(&self) -> Result<u64, BuildError> {
        Ok(self.head)
    }

    fn segment(&self, first: u64, last: u64) -> Result<TronSegmentV1, BuildError> {
        Ok(self.chain.segment(first, last))
    }

    fn raw_headers(&self, first: u64, last: u64) -> Result<Vec<Vec<u8>>, BuildError> {
        Ok(self.chain.raw_segment(first, last).headers)
    }
}

fn tx(height: u64) -> [u8; 32] {
    let mut id = [0_u8; 32];
    id[..8].copy_from_slice(&height.to_be_bytes());
    id
}

/// A chain with a `transferToTaira` call at each of `burns` and a light client bootstrapped at
/// `CHECKPOINT`.
fn bootstrapped(burns: &[u64]) -> (SyntheticSource, LightClientReplayV1) {
    let call = TransferToTairaCallV1 {
        taira_recipient: vec![1; 40],
        token_amount: 5_000_000_000,
        expected_nonce: 3,
    };
    let mut chain = SyntheticTronChainV1::new([3; 32]);
    for burn in burns {
        chain = chain.with_transactions(
            *burn,
            vec![
                trigger_transaction(&OWNER, &CONTRACT, &[0xde, 0xad], 1, 0),
                trigger_transaction(&OWNER, &CONTRACT, &call.calldata(), 1, 0),
            ],
        );
    }
    let now = chain.time_ms(CHECKPOINT) + 60_000;
    let initial = lc::verify_bootstrap(TRON, &params(), &chain.bootstrap(CHECKPOINT), now)
        .expect("the bootstrap verifies");
    let source = SyntheticSource {
        chain,
        head: CHECKPOINT + 50,
        transactions: burns.iter().map(|burn| (tx(*burn), (*burn, 1))).collect(),
    };
    (source, LightClientReplayV1::installed(TRON, &initial))
}

/// The first time at which the set of `P0` no longer anchors solid segments: `ws_bound_ms`
/// (7 d) after the period ends, less the builder's margin.
fn window_edge() -> u64 {
    TRON_MAINNET
        .period_end_ms(SYNTHETIC_TRON_FIRST_PERIOD)
        .expect("period end")
        + params().ws_bound_ms
        - FRESHNESS_MARGIN_MS
}

fn decoded(evidence: &SourceEvidenceV1) -> TronSourceProofV1 {
    let SccpSourceProofV1::Tron(proof) =
        SccpSourceProofV1::from_frame(evidence.proof.as_bytes()).expect("proof frame")
    else {
        panic!("a TRON proof");
    };
    proof
}

fn build(
    source: &SyntheticSource,
    replay: &LightClientReplayV1,
    burn: u64,
    now: u64,
) -> SourceEvidenceV1 {
    let evidence = build_evidence(source, &TRON_MAINNET, &tx(burn), replay, now).expect("evidence");
    let verified = replay
        .verify_evidence(&evidence, now)
        .expect("the evidence verifies");
    assert_eq!(verified.event.locator().source_height, burn);
    evidence
}

#[test]
fn solid_segments_anchor_burns_until_the_seven_day_window_closes() {
    let (source, replay) = bootstrapped(&[1_120]);
    let solid = build(&source, &replay, 1_120, window_edge() - 1);
    assert!(solid.backfills.is_empty());
    let TronProofAnchorV1::Solid(segment) = decoded(&solid).anchor else {
        panic!("a solid segment");
    };
    assert_eq!(segment.headers.len() as u64, SOLIDITY_TAIL + 1);
    // After the window the solid proof is refused and the checkpoint anchors the burn: two
    // backfills bring the checkpoint 1 480 blocks down to within 1 199 blocks.
    let late = window_edge() + FRESHNESS_MARGIN_MS;
    assert!(replay.verify_evidence(&solid, late).is_err());
    let anchored = build(&source, &replay, 1_120, window_edge());
    assert_eq!(anchored.backfills.len(), 2);
    let TronProofAnchorV1::Checkpoint(raw) = decoded(&anchored).anchor else {
        panic!("a checkpoint anchor");
    };
    assert_eq!(raw.headers.len(), 971);
}

#[test]
fn checkpoint_anchors_reach_1199_blocks_directly_and_1200_with_one_backfill() {
    let (source, replay) = bootstrapped(&[CHECKPOINT - 1_199, CHECKPOINT - 1_200]);
    let direct = build(&source, &replay, CHECKPOINT - 1_199, window_edge());
    assert!(direct.backfills.is_empty());
    let TronProofAnchorV1::Checkpoint(raw) = decoded(&direct).anchor else {
        panic!("a checkpoint anchor");
    };
    assert_eq!(raw.headers.len(), 1_200);
    let backfilled = build(&source, &replay, CHECKPOINT - 1_200, window_edge());
    assert_eq!(backfilled.backfills.len(), 1);
    let TronProofAnchorV1::Checkpoint(raw) = decoded(&backfilled).anchor else {
        panic!("a checkpoint anchor");
    };
    assert_eq!(raw.headers.len(), 946);
}

#[test]
fn burns_after_the_learned_periods_or_without_a_checkpoint_are_unavailable() {
    let (source, replay) = bootstrapped(&[1_120]);
    let bare = LightClientReplayV1::new(
        TRON,
        replay.light_client().expect("installed"),
        replay.sets().expect("sets"),
        Vec::new(),
    );
    assert!(matches!(
        build_evidence(&source, &TRON_MAINNET, &tx(1_120), &bare, window_edge()),
        Err(BuildError::Unavailable(_))
    ));
    let mut behind = replay.light_client().expect("installed");
    behind.head.latest_set_id = SYNTHETIC_TRON_FIRST_PERIOD - 1;
    let behind = LightClientReplayV1::new(TRON, behind, Vec::new(), Vec::new());
    let error = build_evidence(
        &source,
        &TRON_MAINNET,
        &tx(1_120),
        &behind,
        window_edge() - 1,
    )
    .expect_err("the period is not learned");
    assert!(error.to_string().contains("advance it first"), "{error}");
    assert!(!period_set_fresh_with_margin(
        &TRON_MAINNET,
        &replay.sets().expect("sets"),
        SYNTHETIC_TRON_FIRST_PERIOD + 1,
        params().ws_bound_ms,
        0
    ));
}
