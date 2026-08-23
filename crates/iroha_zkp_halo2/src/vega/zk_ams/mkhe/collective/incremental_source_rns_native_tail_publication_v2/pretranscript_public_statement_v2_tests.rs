use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use super::super::super::super::super::{
    rns_native_profile::{zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_topology_v1},
    rns_native_source::{
        ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
        ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
};
use super::*;
use crate::vega::sponge::Keccak256;

fn fixture_digest_v2(label: &[u8], ordinal: u64) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.mkhe.pretranscript.fixture");
    hash.update(&(label.len() as u16).to_be_bytes());
    hash.update(label);
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

fn fixture_layout_v2() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        fixture_digest_v2(b"release", 1),
        fixture_digest_v2(b"statement", 2),
        fixture_digest_v2(b"operational", 3),
    )
    .expect("source layout")
}

struct FixtureChunkV2 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: Vec<u8>,
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for FixtureChunkV2 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

#[derive(Clone, Copy)]
enum FixtureSourceFaultV2 {
    None,
    Authentication,
    Panic,
}

struct RepeatableFixtureSnapshotV2 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    reads: Rc<Cell<u64>>,
    drops: Rc<Cell<u32>>,
    poisoned: Rc<Cell<bool>>,
    fault: FixtureSourceFaultV2,
}

impl Drop for RepeatableFixtureSnapshotV2 {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for RepeatableFixtureSnapshotV2 {
    type Chunk = FixtureChunkV2;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        let label: &[u8] = match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => b"main-snapshot",
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => b"nonce-snapshot",
        };
        fixture_digest_v2(label, 0)
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if self.poisoned.get() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Poisoned);
        }
        if slot >= arena.slot_count() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        let prior = self.reads.get();
        self.reads.set(prior + 1);
        if prior == 0 {
            match self.fault {
                FixtureSourceFaultV2::None => {}
                FixtureSourceFaultV2::Authentication => {
                    self.poisoned.set(true);
                    return Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication);
                }
                FixtureSourceFaultV2::Panic => panic!("fixture read panic"),
            }
        }
        let mut bytes = vec![0_u8; arena.plaintext_bytes() as usize];
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => {
                // Valid as both canonical 32-byte coefficients and signed
                // 8-byte coefficients; every ephemeral block is nonzero.
                *bytes.last_mut().expect("main byte") = 1;
            }
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => {
                bytes[0] = 1;
                *bytes.last_mut().expect("nonce byte") =
                    u8::try_from(slot + 1).expect("nonce ordinal");
            }
        }
        Ok(FixtureChunkV2 { arena, bytes })
    }
}

impl ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1 for RepeatableFixtureSnapshotV2 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArtifactCallV2 {
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<usize>,
    limb: usize,
}

struct FixtureBridgeV2 {
    calls: Rc<RefCell<Vec<ArtifactCallV2>>>,
    drops: Rc<Cell<u32>>,
    duplicate_artifacts: bool,
    panic_on_drop: bool,
}

impl Drop for FixtureBridgeV2 {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.panic_on_drop, "fixture bridge drop panic");
    }
}

impl RnsNativePreTranscriptFixtureBridgeV2 for FixtureBridgeV2 {
    fn epoch_v2(&self) -> u64 {
        19
    }

    fn governed_roster_digest_v2(&self) -> [u8; 32] {
        fixture_digest_v2(b"roster", 19)
    }

    fn statement_artifact_digest_v2(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Option<[u8; 32]> {
        self.calls
            .borrow_mut()
            .push(ArtifactCallV2 { role, record, limb });
        if self.duplicate_artifacts {
            return Some(fixture_digest_v2(b"duplicate-artifact", 0));
        }
        let role_base = match role {
            RnsNativePublicPolynomialRoleV1::PublicA => 0_u64,
            RnsNativePublicPolynomialRoleV1::PublicB => 40,
            RnsNativePublicPolynomialRoleV1::CiphertextC0 => 80,
            RnsNativePublicPolynomialRoleV1::CiphertextC1 => 1_800,
        };
        let ordinal = role_base
            + record
                .map(|record| (record * TARGET_LIMBS_V2) as u64)
                .unwrap_or(0)
            + limb as u64;
        Some(fixture_digest_v2(b"artifact", ordinal))
    }
}

fn fixture_bridge_v2(
    calls: Rc<RefCell<Vec<ArtifactCallV2>>>,
    drops: Rc<Cell<u32>>,
    duplicate_artifacts: bool,
    panic_on_drop: bool,
) -> FixtureBridgeV2 {
    FixtureBridgeV2 {
        calls,
        drops,
        duplicate_artifacts,
        panic_on_drop,
    }
}

fn fixture_snapshot_v2(
    reads: Rc<Cell<u64>>,
    drops: Rc<Cell<u32>>,
    poisoned: Rc<Cell<bool>>,
    fault: FixtureSourceFaultV2,
) -> RepeatableFixtureSnapshotV2 {
    RepeatableFixtureSnapshotV2 {
        layout: fixture_layout_v2(),
        reads,
        drops,
        poisoned,
        fault,
    }
}

#[test]
fn exact_inventory_order_source_pass_and_transcript_context_are_owned_v2() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let bridge_drops = Rc::new(Cell::new(0));
    let source_reads = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let prepared = prepare_fixture_harness_v2(
        fixture_bridge_v2(Rc::clone(&calls), Rc::clone(&bridge_drops), false, false),
        fixture_snapshot_v2(
            Rc::clone(&source_reads),
            Rc::clone(&source_drops),
            Rc::new(Cell::new(false)),
            FixtureSourceFaultV2::None,
        ),
    )
    .expect("prepared authority");
    assert_eq!(source_reads.get(), 38_571);
    let calls = calls.borrow();
    assert_eq!(calls.len(), 3_520);
    for (limb, call) in calls[..40].iter().enumerate() {
        assert_eq!(
            *call,
            ArtifactCallV2 {
                role: RnsNativePublicPolynomialRoleV1::PublicA,
                record: None,
                limb,
            }
        );
    }
    for (limb, call) in calls[40..80].iter().enumerate() {
        assert_eq!(
            *call,
            ArtifactCallV2 {
                role: RnsNativePublicPolynomialRoleV1::PublicB,
                record: None,
                limb,
            }
        );
    }
    for (index, call) in calls[80..1_800].iter().enumerate() {
        assert_eq!(call.role, RnsNativePublicPolynomialRoleV1::CiphertextC0);
        assert_eq!(call.record, Some(index / TARGET_LIMBS_V2));
        assert_eq!(call.limb, index % TARGET_LIMBS_V2);
    }
    for (index, call) in calls[1_800..].iter().enumerate() {
        assert_eq!(call.role, RnsNativePublicPolynomialRoleV1::CiphertextC1);
        assert_eq!(call.record, Some(index / TARGET_LIMBS_V2));
        assert_eq!(call.limb, index % TARGET_LIMBS_V2);
    }
    drop(calls);

    let (mut started, transcript) = prepared.begin_transcript_v2().expect("transcript start");
    assert_eq!(
        transcript.governed_roster_digest(),
        started.facts.governed_roster_digest
    );
    assert_eq!(
        transcript.public_ciphertext_digest(),
        started.facts.public_bundle_digest
    );
    assert_eq!(
        transcript.source_binding_digest(),
        started.source.layout().source_binding_digest()
    );
    assert_eq!(
        transcript.main_snapshot_digest(),
        started.facts.receipt.main_snapshot_digest
    );
    assert_eq!(
        transcript.nonce_snapshot_digest(),
        started.facts.receipt.nonce_snapshot_digest
    );
    assert_eq!(
        transcript.source_receipt_digest(),
        started.facts.receipt.receipt_digest
    );
    let first = started
        .source
        .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)
        .expect("repeat read one");
    let second = started
        .source
        .read_slot(ZkAmsMkheRnsNativeSourceArenaV1::Main, 0)
        .expect("repeat read two");
    assert_eq!(first.as_slice(), second.as_slice());
    drop(started);
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn duplicate_inventory_fails_before_source_io_and_drops_every_owner_v2() {
    let bridge_drops = Rc::new(Cell::new(0));
    let source_reads = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let result = prepare_fixture_harness_v2(
        fixture_bridge_v2(
            Rc::new(RefCell::new(Vec::new())),
            Rc::clone(&bridge_drops),
            true,
            false,
        ),
        fixture_snapshot_v2(
            Rc::clone(&source_reads),
            Rc::clone(&source_drops),
            Rc::new(Cell::new(false)),
            FixtureSourceFaultV2::None,
        ),
    );
    assert!(matches!(
        result,
        Err(RnsNativePreTranscriptPublicStatementErrorV2::Source)
    ));
    assert_eq!(source_reads.get(), 0);
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn source_unwind_drops_bridge_and_snapshot_without_a_retry_owner_v2() {
    let bridge_drops = Rc::new(Cell::new(0));
    let source_reads = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = prepare_fixture_harness_v2(
            fixture_bridge_v2(
                Rc::new(RefCell::new(Vec::new())),
                Rc::clone(&bridge_drops),
                false,
                false,
            ),
            fixture_snapshot_v2(
                Rc::clone(&source_reads),
                Rc::clone(&source_drops),
                Rc::new(Cell::new(false)),
                FixtureSourceFaultV2::Panic,
            ),
        );
    }));
    assert!(result.is_err());
    assert_eq!(source_reads.get(), 1);
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn authenticated_source_failure_poisoning_consumes_every_fixture_owner_v2() {
    let bridge_drops = Rc::new(Cell::new(0));
    let source_reads = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let poisoned = Rc::new(Cell::new(false));
    let result = prepare_fixture_harness_v2(
        fixture_bridge_v2(
            Rc::new(RefCell::new(Vec::new())),
            Rc::clone(&bridge_drops),
            false,
            false,
        ),
        fixture_snapshot_v2(
            Rc::clone(&source_reads),
            Rc::clone(&source_drops),
            Rc::clone(&poisoned),
            FixtureSourceFaultV2::Authentication,
        ),
    );
    assert!(matches!(
        result,
        Err(RnsNativePreTranscriptPublicStatementErrorV2::Source)
    ));
    assert_eq!(source_reads.get(), 1);
    assert!(poisoned.get());
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn begin_error_consumes_and_drops_the_only_fixture_owner_v2() {
    let bridge_drops = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let mut prepared = prepare_fixture_harness_v2(
        fixture_bridge_v2(
            Rc::new(RefCell::new(Vec::new())),
            Rc::clone(&bridge_drops),
            false,
            false,
        ),
        fixture_snapshot_v2(
            Rc::new(Cell::new(0)),
            Rc::clone(&source_drops),
            Rc::new(Cell::new(false)),
            FixtureSourceFaultV2::None,
        ),
    )
    .expect("prepared fixture owner");
    prepared.facts.public_bundle_digest[0] ^= 1;
    assert!(matches!(
        prepared.begin_transcript_v2(),
        Err(RnsNativePreTranscriptPublicStatementErrorV2::Transcript)
    ));
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn begin_error_unwind_drops_snapshot_before_losing_fixture_authority_v2() {
    let bridge_drops = Rc::new(Cell::new(0));
    let source_drops = Rc::new(Cell::new(0));
    let mut prepared = prepare_fixture_harness_v2(
        fixture_bridge_v2(
            Rc::new(RefCell::new(Vec::new())),
            Rc::clone(&bridge_drops),
            false,
            true,
        ),
        fixture_snapshot_v2(
            Rc::new(Cell::new(0)),
            Rc::clone(&source_drops),
            Rc::new(Cell::new(false)),
            FixtureSourceFaultV2::None,
        ),
    )
    .expect("prepared fixture owner");
    prepared.facts.public_bundle_digest[0] ^= 1;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = prepared.begin_transcript_v2();
    }));
    assert!(result.is_err());
    assert_eq!(bridge_drops.get(), 1);
    assert_eq!(source_drops.get(), 1);
}

#[test]
fn production_surface_is_sealed_move_only_and_gate_closed_v2() {
    let source = include_str!("pretranscript_public_statement_v2.rs");
    for forbidden in [
        "impl Clone for RnsNativePreTranscript",
        "impl Copy for RnsNativePreTranscript",
        "into_raw_parts",
        "from_raw_parts",
        "pub fn facts",
        "pub(super) fn facts",
        "pub fn source",
        "pub(super) fn source",
        "pub fn bridge",
        "pub(super) fn bridge",
        "rewind",
        "SOURCE_PREFLIGHT_INTEGRATED_V2: bool = true",
        "REPEAT_READ_CONFORMANCE_QUALIFIED_V2: bool = true",
        "RESOURCE_EVIDENCE_QUALIFIED_V2: bool = true",
        "READINESS_V2: bool = true",
        "RELEASE_AUTHORIZED_V2: bool = true",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
    for authority in [
        "pub(super) struct RnsNativePreTranscriptPublicStatementV2",
        "pub(super) struct RnsNativeStartedPreTranscriptPublicStatementV2",
    ] {
        let declaration = source.find(authority).expect("authority declaration");
        let attribute_start = source[..declaration]
            .rfind("\n\n")
            .map_or(0, |position| position + 2);
        assert!(!source[attribute_start..declaration].contains("#[derive"));
    }
    assert!(source.contains("never: Infallible"));
    assert!(source.contains("pub(super) fn begin_transcript_v2("));
    assert!(source.contains("        self,"));
    assert!(source.contains("bridge: RnsNativeExistingReaderBridgeV2<K, P>,"));
    assert!(!source.contains("pub(super) trait RnsNativePreTranscriptBridgeV2"));
    assert!(!source.contains("RnsNativePublicArtifactViewV1"));
    assert!(!source.contains("pub(super) struct RnsNativePreTranscriptPublicStatementFactsV2"));
    assert!(source.contains("start_transcript_v2()"));
    assert!(source.contains("source: S,"));
    assert!(source.contains("facts: RnsNativePreTranscriptPublicStatementFactsV2,"));
}

#[test]
fn accounting_is_exact_and_scoped_v2() {
    let ledger = &RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2;
    assert_eq!(
        core::mem::size_of::<RnsNativePreTranscriptPublicStatementFactsV2>(),
        4_000
    );
    assert_eq!(ledger.authenticated_source_reads, 38_571);
    assert_eq!(ledger.source_plaintext_bytes, 315_622_752);
    assert_eq!(ledger.authenticated_backing_bytes, 316_239_888);
    assert_eq!(ledger.canonical_coefficient_checks, 5_636_096);
    assert_eq!(ledger.signed_coefficient_checks, 16_908_288);
    assert_eq!(ledger.retained_artifact_digests, 3_520);
    assert_eq!(ledger.retained_artifact_digest_bytes, 112_640);
    assert_eq!(ledger.inline_facts_bytes_current_target, 4_000);
    assert_eq!(ledger.pretranscript_public_alias_digests, 3_609);
    assert_eq!(ledger.pretranscript_public_alias_bytes, 115_488);
    assert_eq!(ledger.later_global_alias_digests, 3_754);
    assert_eq!(ledger.later_global_alias_bytes, 120_128);
    assert_eq!(ledger.preparation_public_digest_hash_bytes, 154_158);
    assert_eq!(ledger.begin_public_digest_hash_bytes, 138_248);
    assert_eq!(
        ledger.preparation_and_begin_public_digest_hash_bytes,
        292_406
    );
    assert_eq!(ledger.known_new_peak_bytes, 240_320);
}
