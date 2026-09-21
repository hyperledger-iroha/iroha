//! Coherently rewritten first-use rows outside the selected adjacency still fail closed.
use super::*;
use shared::{ControlRecordParts, CustodyExecution};

fn configured_history<P: CustodyPurpose>() -> Fixture {
    let mut f = fixture();
    f.policy.binding.role = P::ROLE;
    f.policy.binding.purpose = P::purpose(DEPLOYMENT.into());
    for revision in 1_u8..=3 {
        // These valid same-key policy/handle changes must not be confused with scope changes.
        f.policy.binding.policy_revision = u64::from(revision);
        f.policy.binding.policy_digest = [revision + 40; 32];
        f.policy.binding.runtime_handle = format!("hsm://promotion/revision-{revision}");
        let bytes = shared::encode(&f.policy).unwrap();
        transact(&mut f.state, u64::from(revision) * 1_000, |tx| {
            let current = shared::read_control::<P>(tx.world(), DEPLOYMENT).unwrap();
            let before = retained(tx);
            let prepared = shared::prepare_control::<P>(
                tx,
                &f.manager,
                current.as_ref(),
                ControlTransition {
                    deployment: DEPLOYMENT,
                    expected_revision: current.as_ref().map_or(0, |row| row.index.revision),
                    expected_digest: current.as_ref().map_or([0; 32], |row| row.index.digest),
                    request_digest: [revision; 32],
                    action: ControlAction::Configure(&bytes),
                },
            )
            .unwrap();
            assert_eq!(retained(tx), before);
            let staged = shared::staging_fixture::staged_control::<P>(&prepared, DEPLOYMENT);
            // Publish the actual shared transition's complete staged write set in this fixture.
            for (path, bytes) in prepared {
                tx.world.smart_contract_state.insert(path, bytes);
            }
            let selected = shared::read_control::<P>(tx.world(), DEPLOYMENT)
                .unwrap()
                .unwrap();
            assert_eq!(selected.index, staged.index);
            assert_eq!(selected.state.policy, f.policy);
        });
    }
    f
}

fn reject_rewritten_first_use<P: CustodyPurpose>(change: &str) {
    let mut f = configured_history::<P>();
    transact(&mut f.state, 4_000, |tx| {
        let selected = shared::read_control::<P>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        assert_eq!(selected.index.revision, 3);
        let original = shared::read_control_record::<P>(tx.world(), DEPLOYMENT, 1).unwrap();
        assert!(original.index.revision + 1 < selected.index.revision);
        let fields = P::record_view(&original.record);
        let mut state = original.state.clone();
        let mut execution = fields.execution.view();
        let mut predecessor_digest = fields.predecessor_digest;
        let mut request_digest = fields.request_digest;
        match change {
            "purpose" => {
                let (role, purpose) = if P::ROLE == ReceiptPurpose::ROLE {
                    (
                        AccountPurpose::ROLE,
                        AccountPurpose::purpose(DEPLOYMENT.into()),
                    )
                } else {
                    (
                        ReceiptPurpose::ROLE,
                        ReceiptPurpose::purpose(DEPLOYMENT.into()),
                    )
                };
                state.policy.binding.role = role;
                state.policy.binding.purpose = purpose;
            }
            "network" => state.policy.binding.network_id[0] ^= 1,
            "chain" => state.policy.binding.chain_id.push('x'),
            "execution" => execution.recorded_at_unix_ms = 0,
            "request" => request_digest = [0; 32],
            "predecessor" => predecessor_digest = [70; 32],
            "state" | "height_missing" | "height_changed" => {}
            _ => unreachable!("closed corruption fixture"),
        }
        // Each changed policy remains intrinsically valid; its native provenance is the issue.
        state.validate().unwrap();
        let mut state_bytes = shared::encode(&state).unwrap();
        if change == "state" {
            state_bytes.push(0);
        }
        let record = P::build_record(ControlRecordParts {
            deployment: fields.deployment.to_owned(),
            revision: fields.revision,
            predecessor_digest,
            request_digest,
            execution: P::Execution::build(execution),
            control_state: state_bytes,
            enrollment: fields.enrollment.map(<[u8]>::to_vec),
        });
        let index = ControlIndexV1 {
            digest: shared::control_digest::<P>(&record).unwrap(),
            ..original.index
        };
        let height_path = shared::control_height_key::<P>(
            DEPLOYMENT,
            original.index.height,
            original.index.ordinal,
        )
        .unwrap();
        tx.world.smart_contract_state.insert(
            shared::control_record_key::<P>(DEPLOYMENT, 1).unwrap(),
            shared::encode(&record).unwrap(),
        );
        for (signer, key) in [
            (true, &original.state.policy.binding.public_key),
            (false, &original.state.policy.attester_public_key),
        ] {
            tx.world.smart_contract_state.insert(
                shared::key_path::<P>(DEPLOYMENT, signer, key).unwrap(),
                shared::encode(&index).unwrap(),
            );
        }
        match change {
            "height_missing" => {
                tx.world.smart_contract_state.remove(height_path);
            }
            "height_changed" => {
                let mut wrong = index;
                wrong.digest[0] ^= 1;
                tx.world
                    .smart_contract_state
                    .insert(height_path, shared::encode(&wrong).unwrap());
            }
            _ => {
                tx.world
                    .smart_contract_state
                    .insert(height_path, shared::encode(&index).unwrap());
            }
        }
        // Both key references and (except the explicit index negatives) the height index now
        // authenticate the rewritten bytes. The selected row and its predecessor are untouched.
        assert_eq!(
            tx.world().smart_contract_state().get(
                &shared::control_record_key::<P>(DEPLOYMENT, selected.index.revision).unwrap()
            ),
            Some(&shared::encode(&selected.record).unwrap()),
        );
        let before = retained(tx);
        assert!(
            matches!(
                shared::read_control::<P>(tx.world(), DEPLOYMENT),
                Err(HistoryError::CorruptHistory)
            ),
            "{change}"
        );
        assert_eq!(
            retained(tx),
            before,
            "rejected {change} read must not write"
        );
    });
}

#[test]
fn receipt_first_use_rows_require_full_provenance_and_original_height_index() {
    for change in [
        "purpose",
        "network",
        "chain",
        "execution",
        "request",
        "predecessor",
        "state",
        "height_missing",
        "height_changed",
    ] {
        reject_rewritten_first_use::<ReceiptPurpose>(change);
    }
}

#[test]
fn account_first_use_rows_require_full_provenance_and_original_height_index() {
    for change in [
        "purpose",
        "network",
        "chain",
        "execution",
        "request",
        "predecessor",
        "state",
        "height_missing",
        "height_changed",
    ] {
        reject_rewritten_first_use::<AccountPurpose>(change);
    }
}
