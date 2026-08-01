    #[test]
    fn leader_wire_gate_reopens_volatile_terminal_and_verifies_durable_body_terminal() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-body-terminal.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 4;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: context
                .snapshot_bootstrap
                .map(|anchor| anchor.snapshot_block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"body terminal block")),
            payload_hash: Hash::new(b"body terminal payload"),
        };
        let durable_body = DurableBodyReceipt::for_test(
            context.id(),
            round,
            subject,
            HashOf::from_untyped_unchecked(Hash::new(b"body terminal manifest")),
        );
        let token = leader_wire_body_token(&context, &durable_body, 17, 109);
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 109).expect("runtime owner");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open gate");
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        let runtime = gate
            .mark_runtime(&token, runtime_owner)
            .expect("mark runtime");
        gate.mark_volatile_terminal(&runtime)
            .expect("mark volatile consumer departure");
        assert_eq!(
            gate.restore().expect("same-process restore").records()[0].status(),
            LeaderWireLifecycleStatus::VolatileTerminal
        );
        let retry = gate
            .reserve(leader_wire_body_token(&context, &durable_body, 18, 110))
            .expect("exact volatile retry coalesces");
        assert_eq!(retry.status(), LeaderWireLifecycleStatus::VolatileTerminal);
        assert_eq!(retry.token().admission_ordinal(), 17);
        assert_eq!(retry.token().scheduler_ordinal(), 109);

        let (reopened, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[durable_body.clone()],
        )
        .expect("volatile terminal always reopens");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Dormant
        );
        assert_eq!(restore.records()[0].runtime_owner(), Some(runtime_owner));
        let replay = reopened
            .reserve(token.clone())
            .expect("reactivate replay ingress");
        reopened
            .mark_ingress(replay.token())
            .expect("replay ingress");
        let runtime = reopened
            .mark_runtime(replay.token(), runtime_owner)
            .expect("rebind restored runtime");
        reopened
            .mark_terminal(
                &runtime,
                LeaderWireStableTerminalEvidence::DurableBody(
                    LeaderWireDurableBodyTerminalEvidence::from_receipt(
                        &durable_body,
                        OWNER_A,
                        runtime_owner,
                    ),
                ),
            )
            .expect("publish body-backed terminal");

        let (_, stable) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[durable_body],
        )
        .expect("independent body catalog verifies stable terminal");
        assert_eq!(
            stable.records()[0].status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert!(matches!(
            stable.records()[0].terminal_evidence(),
            Some(LeaderWireStableTerminalEvidence::DurableBody(_))
        ));

        assert!(
            LeaderWireLifecycleStoreGate::open(
                &wal,
                context.id(),
                context.height,
                OWNER_A,
                roster,
                capacity,
                max_chunks,
                leader_wire_recovery_authority(&context),
                &[],
                &[],
            )
            .is_err(),
            "a body terminal cannot authorize itself from the gate snapshot"
        );
    }

    #[test]
    fn leader_wire_gate_reconciles_body_first_terminal_crash() {
        let directory = TempDir::new().expect("temporary directory");
        let context = context();
        let wal = directory.path().join("leader-wire-body-first.wal");
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        let max_chunks = 4;
        let capacity = LeaderWireLifecycleStoreGate::derived_capacity(roster.len(), max_chunks)
            .expect("derived gate capacity");
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 5,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: context
                .snapshot_bootstrap
                .map(|anchor| anchor.snapshot_block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"body-first block")),
            payload_hash: Hash::new(b"body-first payload"),
        };
        let durable_body = DurableBodyReceipt::for_test(
            context.id(),
            round,
            subject,
            HashOf::from_untyped_unchecked(Hash::new(b"body-first manifest")),
        );
        let token = leader_wire_body_token(&context, &durable_body, 23, 151);
        let runtime_owner =
            LeaderWireRuntimeOwner::new(token.identity_hash(), 151).expect("runtime owner");
        let (gate, _) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster.clone(),
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[],
        )
        .expect("open gate");
        gate.reserve(token.clone()).expect("reserve");
        gate.mark_ingress(&token).expect("mark ingress");
        gate.mark_runtime(&token, runtime_owner)
            .expect("publish runtime before simulated crash");

        let (_, restore) = LeaderWireLifecycleStoreGate::open(
            &wal,
            context.id(),
            context.height,
            OWNER_A,
            roster,
            capacity,
            max_chunks,
            leader_wire_recovery_authority(&context),
            &[],
            &[durable_body],
        )
        .expect("independent body publication closes crash window");
        assert_eq!(
            restore.records()[0].status(),
            LeaderWireLifecycleStatus::Terminal
        );
        assert!(matches!(
            restore.records()[0].terminal_evidence(),
            Some(LeaderWireStableTerminalEvidence::DurableBody(_))
        ));
    }
