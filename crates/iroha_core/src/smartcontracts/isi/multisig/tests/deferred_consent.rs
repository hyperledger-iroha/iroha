// Selected consent uses the real proposal/approval/cancellation handlers and occurrence fixture.
fn select_deferred_consent_for_multisig_test(tx: &mut StateTransaction<'_, '_>) {
    let catalog = crate::runtime::release_catalog().unwrap();
    let before = tx.world.runtime_selection.get().clone();
    let change = catalog
        .activation(
            crate::runtime::catalog::additions::DEFERRED_CONSENT.module,
            &before,
            &crate::runtime::change_set::PublishedDefinitions(&tx.world.runtime_definitions),
            tx.network_id,
            tx.block_height(),
            [1; 32],
        )
        .unwrap();
    let applied = change
        .apply(
            &before,
            &crate::runtime::change_set::PublishedDefinitions(&tx.world.runtime_definitions),
        )
        .unwrap();
    crate::runtime::deferred_consent::check_transition(&*tx.world, &before, &applied.selection)
        .unwrap();
    // Publish the compiled catalog so the fixture can also select its Time successor below.
    for (id, descriptor) in &catalog.definitions {
        tx.world.runtime_definitions.insert(*id, descriptor.clone());
    }
    *tx.world.runtime_selection.get_mut() = applied.selection;
}

#[test]
fn multisig_deferred_consent_mismatch_requires_authorized_cancellation_and_new_occurrence() {
    with_occurrence_fixture(|tx, first, second, account, _| {
        select_deferred_consent_for_multisig_test(tx);
        let instructions = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "reviewed body".into(),
        ))];
        let hash = HashOf::new(&instructions);
        let nonce = MultisigProposalNonce::new([0x91; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        let original = MultisigPropose::new(account.clone(), nonce, instructions.clone(), None);
        execute_propose(tx, first, &original).unwrap();
        let key = multisig_proposal_state_key(account, &id);
        let original_bytes = tx.world.smart_contract_state.get(&key).unwrap().clone();
        let consent_key = crate::runtime::deferred_consent::multisig_key(account, id.as_bytes());
        let original_consent = tx
            .world
            .runtime_deferred_consents
            .get(&consent_key)
            .unwrap()
            .clone();
        let catalog = crate::runtime::release_catalog().unwrap();
        let changed = catalog
            .clone()
            .lifecycle_successor(tx.world.runtime_selection.get())
            .unwrap();
        assert_eq!(catalog.unsupported(&changed), None);
        *tx.world.runtime_selection.get_mut() = changed;
        assert!(
            execute_approve(tx, second, &MultisigApprove::new(account.clone(), id, hash)).is_err()
        );
        assert_eq!(
            tx.world.smart_contract_state.get(&key),
            Some(&original_bytes)
        );
        assert_eq!(
            tx.world.runtime_deferred_consents.get(&consent_key),
            Some(&original_consent)
        );
        assert!(
            execute_cancel(tx, first, &MultisigCancel::new(account.clone(), id, hash)).is_err()
        );
        let outsider = new_account_id(&checked_keypair());
        assert!(
            execute_approve(
                tx,
                &outsider,
                &MultisigApprove::new(account.clone(), id, hash)
            )
            .is_err()
        );
        let cancel = vec![InstructionBox::from(MultisigCancel::new(
            account.clone(),
            id,
            hash,
        ))];
        let cancel_hash = HashOf::new(&cancel);
        let cancel_nonce = MultisigProposalNonce::new([0x92; 32]).unwrap();
        let cancel_id = MultisigProposalId::derive(&tx.network_id, account, &cancel_nonce);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), cancel_nonce, cancel, None),
        )
        .unwrap();
        execute_approve(
            tx,
            second,
            &MultisigApprove::new(account.clone(), cancel_id, cancel_hash),
        )
        .unwrap();
        assert!(
            tx.world
                .runtime_deferred_consents
                .get(&consent_key)
                .is_none()
        );
        assert!(proposal_state(tx, account, &id).is_err());
        assert!(
            execute_propose(tx, first, &original).is_err(),
            "canceled occurrence remains terminal"
        );
        let terminal = tx
            .world
            .smart_contract_state
            .get(&multisig_proposal_terminal_state_key(account, &id))
            .unwrap()
            .clone();
        let renewed_nonce = MultisigProposalNonce::new([0x93; 32]).unwrap();
        let renewed_id = MultisigProposalId::derive(&tx.network_id, account, &renewed_nonce);
        assert_ne!(renewed_id, id);
        execute_propose(
            tx,
            first,
            &MultisigPropose::new(account.clone(), renewed_nonce, instructions, None),
        )
        .unwrap();
        let renewed = proposal_state(tx, account, &renewed_id).unwrap();
        assert_eq!(renewed.approvals, BTreeSet::from([first.clone()]));
        assert_ne!(
            tx.world
                .runtime_deferred_consents
                .get(&crate::runtime::deferred_consent::multisig_key(
                    account,
                    renewed_id.as_bytes()
                ))
                .unwrap(),
            &original_consent
        );
        execute_approve(
            tx,
            second,
            &MultisigApprove::new(account.clone(), renewed_id, hash),
        )
        .unwrap();
        assert!(tx.deferred_expectations.is_empty());
        assert_eq!(
            tx.world
                .smart_contract_state
                .get(&multisig_proposal_terminal_state_key(account, &id)),
            Some(&terminal)
        );
    });
}

#[test]
fn multisig_current_bindings_consent_preserves_authorization_and_normal_restrictions() {
    with_occurrence_fixture(|tx, first, second, account, _| {
        use iroha_data_model::{
            isi::runtime_deferred::ProposeCurrentBindingMultisigV1,
            runtime::{
                deferred::DeferredConsentV1,
                definition::{RestrictionEntryV1, RestrictionV1},
            },
        };
        select_deferred_consent_for_multisig_test(tx);
        let instructions = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "current body".into(),
        ))];
        let hash = HashOf::new(&instructions);
        let proposal = ProposeCurrentBindingMultisigV1 {
            account: account.clone(),
            proposal_nonce: [0xa1; 32],
            instructions: instructions.clone(),
            transaction_ttl_ms: None,
        };
        let outsider = new_account_id(&checked_keypair());
        assert!(proposal.clone().execute(&outsider, tx).is_err());
        assert!(tx.world.runtime_deferred_consents.iter().next().is_none());
        proposal.execute(first, tx).unwrap();
        let nonce = MultisigProposalNonce::new([0xa1; 32]).unwrap();
        let id = MultisigProposalId::derive(&tx.network_id, account, &nonce);
        let key = crate::runtime::deferred_consent::multisig_key(account, id.as_bytes());
        assert!(matches!(
            tx.world
                .runtime_deferred_consents
                .get(&key)
                .unwrap()
                .consent,
            DeferredConsentV1::CurrentBindings
        ));
        let catalog = crate::runtime::release_catalog().unwrap();
        let mut changed = catalog
            .clone()
            .lifecycle_successor(tx.world.runtime_selection.get())
            .unwrap();
        let wire = iroha_data_model::isi::instruction_wire_id(&instructions[0]).unwrap();
        let interface = changed
            .bindings
            .iter()
            .find(|binding| binding.wire_id == wire)
            .unwrap()
            .interface;
        changed.restrictions.push(RestrictionEntryV1 {
            definition: interface,
            restriction: RestrictionV1::Disabled(tx.block_height()),
        });
        changed.restrictions.sort_by_key(|entry| entry.definition);
        *tx.world.runtime_selection.get_mut() = changed;
        let refused_nonce = MultisigProposalNonce::new([0xa3; 32]).unwrap();
        let refused_id = MultisigProposalId::derive(&tx.network_id, account, &refused_nonce);
        assert!(
            execute_propose(
                tx,
                first,
                &MultisigPropose::new(account.clone(), refused_nonce, instructions.clone(), None)
            )
            .is_err()
        );
        assert!(
            tx.world
                .smart_contract_state
                .get(&multisig_proposal_state_key(account, &refused_id))
                .is_none()
        );
        assert!(
            tx.world
                .runtime_deferred_consents
                .get(&crate::runtime::deferred_consent::multisig_key(
                    account,
                    refused_id.as_bytes()
                ))
                .is_none()
        );
        ProposeCurrentBindingMultisigV1 {
            account: account.clone(),
            proposal_nonce: [0xa2; 32],
            instructions: instructions.clone(),
            transaction_ttl_ms: None,
        }
        .execute(first, tx)
        .expect("explicit current binding registration does not capture the restricted closure");
        assert!(
            crate::runtime::deferred_consent::check_multisig(
                tx,
                account,
                id.as_bytes(),
                &instructions
            )
            .unwrap()
            .is_none()
        );
        assert!(
            execute_approve(tx, second, &MultisigApprove::new(account.clone(), id, hash)).is_err(),
            "explicit opt-in does not bypass ordinary dispatch restrictions"
        );
        assert!(tx.deferred_expectations.is_empty());
    });
}
