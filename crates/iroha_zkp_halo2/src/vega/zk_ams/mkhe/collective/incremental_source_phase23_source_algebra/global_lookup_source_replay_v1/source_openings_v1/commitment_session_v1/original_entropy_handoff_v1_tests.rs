//! Original concrete RNG continuation, fallibility, bounds and custody controls.
use super::*;
use crate::vega::{MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1};
use core::cell::Cell;

#[derive(Clone, Copy)]
enum Fault {
    None,
    PartialError,
    PartialPanic,
}
struct OriginalSource<'a> {
    bytes: &'a [u8],
    position: &'a Cell<usize>,
    calls: &'a Cell<usize>,
    drops: &'a Cell<usize>,
    fault: Fault,
}
impl Drop for OriginalSource<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl MaskedRelaxedRandomSourceV1 for OriginalSource<'_> {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        self.calls.set(self.calls.get() + 1);
        match self.fault {
            Fault::PartialError => {
                destination[..3].fill(0x91);
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            }
            Fault::PartialPanic => {
                destination[..3].fill(0x92);
                panic!("intentional original entropy unwind");
            }
            Fault::None => {}
        }
        let end = self
            .position
            .get()
            .checked_add(destination.len())
            .ok_or(MaskedRelaxedRandomErrorV1::Unavailable)?;
        let bytes = self
            .bytes
            .get(self.position.get()..end)
            .ok_or(MaskedRelaxedRandomErrorV1::Unavailable)?;
        destination.copy_from_slice(bytes);
        self.position.set(end);
        Ok(())
    }
}

// This is an isolated sampler/inventory fixture. It grants no materialized
// source, correspondence, replay or proof authority, and bypasses no production
// factory. Its concrete borrowed source exercises the actual Production branch.
fn isolated_session<R: MaskedRelaxedRandomSourceV1>(
    random: R,
) -> GlobalLookupCommitmentSessionV1<R, SourceOpeningEntropyStageV1> {
    GlobalLookupCommitmentSessionV1 {
        live: Some(GlobalLookupCommitmentSessionLiveV1 {
            entropy: GlobalLookupProofSessionEntropySourceV1::Production {
                original_random: random,
                commitment_entropy_bytes: 0,
            },
            inventory: GlobalLookupCommitmentInventorySkeletonV1::new_v1().unwrap(),
            proof_session_context_digest: [0x91; 32],
            source_opening_context_digest: None,
            next_global_ordinal: 0,
            next_purpose: GlobalLookupCommitmentPurposeV1::Source,
            next_purpose_ordinal: 0,
            pending_source: None,
        }),
        state: PhantomData,
    }
}

#[test]
fn borrowed_original_rng_continues_without_reseed_copy_or_rewind_and_drops_once() {
    let mut bytes = [0_u8; 39];
    bytes[..7].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7]);
    bytes[38] = 9;
    let position = Cell::new(0);
    let calls = Cell::new(0);
    let drops = Cell::new(0);
    let mut random = OriginalSource {
        bytes: &bytes,
        position: &position,
        calls: &calls,
        drops: &drops,
        fault: Fault::None,
    };
    let mut earlier = [0; 7];
    random.fill_bytes(&mut earlier).unwrap();
    assert_eq!(earlier, [1, 2, 3, 4, 5, 6, 7]);
    let mut session = isolated_session(random);
    session.bind_source_opening_context_v1([0x92; 32]).unwrap();
    let (chunk, scalar) = session.sample_source_blinding_v1(0).unwrap();
    assert_eq!(scalar.get(), Scalar::from_u64(9));
    assert_eq!(chunk.as_slice_v1(), &bytes[7..]);
    assert_eq!(position.get(), 39);
    assert_eq!(calls.get(), 2);
    assert_eq!(drops.get(), 0);
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        ..
    } = &session.live.as_ref().unwrap().entropy
    else {
        unreachable!()
    };
    assert_eq!(*commitment_entropy_bytes, 32);
    // A repeated coordinate fails before sampling, closes the same source, and
    // cannot retry through the now-empty session.
    assert!(session.sample_source_blinding_v1(0).is_err());
    assert!(session.live.is_none());
    assert_eq!(calls.get(), 2);
    assert_eq!(drops.get(), 1);
    assert!(session.sample_source_blinding_v1(0).is_err());
    drop(session);
    assert_eq!(drops.get(), 1);
}

#[test]
fn partial_error_and_unwind_consume_original_rng_and_prevent_retry() {
    for fault in [Fault::PartialError, Fault::PartialPanic] {
        let position = Cell::new(0);
        let calls = Cell::new(0);
        let drops = Cell::new(0);
        let source = OriginalSource {
            bytes: &[],
            position: &position,
            calls: &calls,
            drops: &drops,
            fault,
        };
        let mut session = isolated_session(source);
        session.bind_source_opening_context_v1([0x92; 32]).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.sample_source_blinding_v1(0)
        }));
        match fault {
            Fault::PartialError => assert!(matches!(
                result,
                Ok(Err(ZkAmsMkheErrorV1::RandomUnavailable))
            )),
            Fault::PartialPanic => assert!(result.is_err()),
            Fault::None => unreachable!(),
        }
        assert!(session.live.is_none());
        assert_eq!(drops.get(), 1);
        assert_eq!(calls.get(), 1);
        assert!(session.sample_source_blinding_v1(0).is_err());
        assert_eq!(calls.get(), 1);
    }
}

#[test]
fn exact_derived_request_bound_and_bad_metadata_never_call_original_rng() {
    let position = Cell::new(0);
    let calls = Cell::new(0);
    let drops = Cell::new(0);
    let bytes = [1; 32];
    let source = OriginalSource {
        bytes: &bytes,
        position: &position,
        calls: &calls,
        drops: &drops,
        fault: Fault::None,
    };
    let mut entropy = GlobalLookupProofSessionEntropySourceV1::Production {
        original_random: source,
        commitment_entropy_bytes: MAX_COMMITMENT_ENTROPY_BYTES_V1 - 32,
    };
    let mut destination = [0; 32];
    fill_entropy_v1(&mut entropy, 72_385, 127, &mut destination).unwrap();
    assert_eq!(destination, [1; 32]);
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        fill_entropy_v1(&mut entropy, 0, 0, &mut destination),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(destination, [0; 32]);
    for (ordinal, attempt, len) in [
        (72_386, 0, 32),
        (u32::MAX, 0, 32),
        (0, 128, 32),
        (0, u16::MAX, 32),
        (0, 0, 0),
        (0, 0, 31),
        (0, 0, 33),
    ] {
        let mut buffer = vec![0x55; len];
        assert!(fill_entropy_v1(&mut entropy, ordinal, attempt, &mut buffer).is_err());
        assert!(buffer.iter().all(|b| *b == 0));
        assert_eq!(calls.get(), 1);
    }
    let GlobalLookupProofSessionEntropySourceV1::Production {
        commitment_entropy_bytes,
        ..
    } = &mut entropy
    else {
        unreachable!()
    };
    *commitment_entropy_bytes = u64::MAX;
    destination.fill(0x55);
    assert!(matches!(
        fill_entropy_v1(&mut entropy, 0, 0, &mut destination),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(destination, [0; 32]);
    drop(entropy);
    assert_eq!(drops.get(), 1);
}

#[test]
fn original_handoff_is_source_typed_and_gated_before_consumption() {
    let source = include_str!("original_entropy_handoff_v1.rs");
    let compact = source.split_whitespace().collect::<String>();
    let factory = compact
        .split("fnfrom_original_materialized_source_v1<")
        .nth(1)
        .unwrap();
    assert!(factory.contains("owner:&mutZkAmsPhase23MaterializedEncryptedSourceOwnerV1<R,K,P>"));
    assert!(
        factory.find("owner.validate_v1()?").unwrap()
            < factory.find("owner.original_random.take()").unwrap()
    );
    assert!(factory.contains("proof_session_context_digest:owner.bundle_digest"));
    for forbidden in [
        "random:R,",
        "context_digest:[u8;32]",
        "fn into_parts",
        "HealthCheckedCryptoRngV1",
        "OsRng",
        "seed_from",
        "Box<",
        "dyn ",
        "ProductionReady",
    ] {
        assert!(
            !source.contains(forbidden),
            "unapproved entropy factory surface: {forbidden}"
        );
    }
    let replay = include_str!("../../../global_lookup_source_replay_v1.rs");
    let entry = replay
        .split("fn replay_global_lookup_source_v1")
        .nth(1)
        .unwrap()
        .split('{')
        .next()
        .unwrap();
    assert!(!entry.contains("proof_session_entropy"));
    let ingress =
        include_str!("../../../global_lookup_source_replay_v1/original_source_ingress_v1.rs");
    let ingress = ingress.split_whitespace().collect::<String>();
    assert!(
        ingress.find("validate_prerequisite_record_v2").unwrap()
            < ingress
                .find("from_original_materialized_source_v1")
                .unwrap()
    );
    assert!(
        ingress
            .find("from_original_materialized_source_v1")
            .unwrap()
            < ingress.find("SourceOpeningAssemblyV1::begin_v1").unwrap()
    );
}

#[test]
fn original_rng_rejection_budget_is_exact_and_never_reduces_noncanonical_scalars() {
    for noncanonical in [[0_u8; 32], [0xff_u8; 32]] {
        let bytes = noncanonical.repeat(MAX_RANDOM_REJECTION_ATTEMPTS_V1);
        let position = Cell::new(0);
        let calls = Cell::new(0);
        let drops = Cell::new(0);
        let mut session = isolated_session(OriginalSource {
            bytes: &bytes,
            position: &position,
            calls: &calls,
            drops: &drops,
            fault: Fault::None,
        });
        session.bind_source_opening_context_v1([0x92; 32]).unwrap();
        assert!(matches!(
            session.sample_source_blinding_v1(0),
            Err(ZkAmsMkheErrorV1::RandomUnavailable)
        ));
        assert_eq!(calls.get(), 128);
        assert_eq!(position.get(), 4096);
        assert_eq!(drops.get(), 1);
        assert!(session.live.is_none());
        assert!(session.sample_source_blinding_v1(0).is_err());
        assert_eq!(calls.get(), 128);
    }
}
