use super::*;

fn layout() -> ZkAmsMkheRnsNativeSourceLayoutV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        [0x31; 32],
        [0x32; 32],
        [0x33; 32],
    )
    .expect("source layout")
}

#[test]
fn exact_layout_binds_every_context_and_arena() {
    let baseline = layout();
    baseline.validate().expect("valid layout");
    assert_ne!(
        baseline.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Main),
        baseline.arena_context_digest(ZkAmsMkheRnsNativeSourceArenaV1::Nonce)
    );
    let changed = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        baseline.profile_digest(),
        baseline.topology_digest(),
        baseline.release_candidate_digest(),
        baseline.statement_digest(),
        [0x34; 32],
    )
    .expect("changed layout");
    assert_ne!(
        baseline.source_binding_digest(),
        changed.source_binding_digest()
    );
}

#[test]
fn rejects_zero_duplicate_and_foreign_profile_contexts() {
    let baseline = layout();
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            baseline.profile_digest(),
            baseline.topology_digest(),
            [0; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            baseline.profile_digest(),
            baseline.topology_digest(),
            [4; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
    assert_eq!(
        ZkAmsMkheRnsNativeSourceLayoutV1::new(
            [9; 32],
            baseline.topology_digest(),
            [3; 32],
            [4; 32],
            [5; 32],
        ),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::InvalidContext)
    );
}

#[test]
fn structural_receipt_is_exactly_layout_bound_and_nonzero() {
    let baseline = layout();
    let receipt =
        ZkAmsMkheRnsNativeSourceReceiptV1::new(baseline, [0x41; 32], [0x42; 32]).expect("receipt");
    receipt.validate(baseline).expect("receipt validates");
    let changed = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        baseline.profile_digest(),
        baseline.topology_digest(),
        baseline.release_candidate_digest(),
        baseline.statement_digest(),
        [0x35; 32],
    )
    .expect("changed layout");
    assert_eq!(
        receipt.validate(changed),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication)
    );
}

struct RepeatableSecretChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: Vec<u8>,
}

impl Drop for RepeatableSecretChunkV1 {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for RepeatableSecretChunkV1 {
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

struct RepeatableSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    mutate_main_repeat: bool,
    main_zero_reads: u8,
    reads: Vec<(ZkAmsMkheRnsNativeSourceArenaV1, u64)>,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for RepeatableSnapshotV1 {
    type Chunk = RepeatableSecretChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => [0x71; 32],
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => [0x72; 32],
        }
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        if slot >= arena.slot_count() {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::UnexpectedWrite);
        }
        self.reads.push((arena, slot));
        let mut value = (slot as u8).wrapping_add(arena as u8);
        if arena == ZkAmsMkheRnsNativeSourceArenaV1::Main && slot == 0 {
            self.main_zero_reads = self.main_zero_reads.saturating_add(1);
            if self.mutate_main_repeat && self.main_zero_reads > 1 {
                value = value.wrapping_add(1);
            }
        }
        Ok(RepeatableSecretChunkV1 {
            arena,
            bytes: vec![value; arena.plaintext_bytes() as usize],
        })
    }
}

impl ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1 for RepeatableSnapshotV1 {}

#[test]
fn repeatability_runner_emits_plaintext_free_stable_evidence() {
    let mut snapshot = RepeatableSnapshotV1 {
        layout: layout(),
        mutate_main_repeat: false,
        main_zero_reads: 0,
        reads: Vec::new(),
    };
    let receipt = snapshot.structural_receipt().expect("structural receipt");
    let evidence = verify_zk_ams_mkhe_rns_native_source_repeatability_v1(&mut snapshot)
        .expect("repeatability evidence");
    evidence
        .validate(snapshot.layout(), receipt)
        .expect("evidence validates");
    assert_eq!(
        snapshot.reads.as_slice(),
        &[
            (
                ZkAmsMkheRnsNativeSourceArenaV1::Main,
                ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_SLOTS_V1 - 1,
            ),
            (ZkAmsMkheRnsNativeSourceArenaV1::Main, 0),
            (ZkAmsMkheRnsNativeSourceArenaV1::Main, 0),
            (
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce,
                ZK_AMS_MKHE_RNS_NATIVE_SOURCE_NONCE_SLOTS_V1 - 1,
            ),
            (ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0),
            (ZkAmsMkheRnsNativeSourceArenaV1::Nonce, 0),
        ]
    );
    assert_ne!(evidence.evidence_digest(), [0; 32]);

    let mut replay = RepeatableSnapshotV1 {
        layout: layout(),
        mutate_main_repeat: false,
        main_zero_reads: 0,
        reads: Vec::new(),
    };
    let replay_evidence = verify_zk_ams_mkhe_rns_native_source_repeatability_v1(&mut replay)
        .expect("repeatability replay evidence");
    assert_eq!(evidence, replay_evidence);
}

#[test]
fn repeatability_runner_rejects_changed_authenticated_plaintext() {
    let mut snapshot = RepeatableSnapshotV1 {
        layout: layout(),
        mutate_main_repeat: true,
        main_zero_reads: 0,
        reads: Vec::new(),
    };
    assert_eq!(
        verify_zk_ams_mkhe_rns_native_source_repeatability_v1(&mut snapshot),
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Authentication)
    );
}
