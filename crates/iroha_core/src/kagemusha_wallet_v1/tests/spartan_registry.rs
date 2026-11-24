//! Governed World entry points. All State, keys and balances in these cases are local fixtures.
use super::*;
use iroha_data_model::isi::kagemusha_wallet::{
    KagemushaWalletLedgerV1,
    spartan::{
        ActivateKagemushaSpartanV1 as Activate, InstallKagemushaSpartanProgramsV1 as Install,
        UnloadKagemushaSpartanV1 as Unload,
    },
};
use iroha_data_model::runtime::definition::RootSelectionV1;

#[path = "spartan_retirement.rs"]
mod retirement;

fn selected(state: &State) -> RootSelectionV1 {
    let before = state.view().world.runtime_selection().clone();
    let catalog = crate::runtime::catalog::release_catalog().unwrap();
    catalog
        .activation(
            crate::runtime::catalog::additions::KAGEMUSHA_SPARTAN.module,
            &before,
            &catalog.definitions,
            state.network_id,
            25,
            [9; 32],
        )
        .unwrap()
        .apply(&before, &catalog.definitions)
        .unwrap()
        .selection
}

fn select_published(tx: &mut StateTransaction<'_, '_>, selection: RootSelectionV1) {
    let catalog = crate::runtime::catalog::release_catalog().unwrap();
    for id in selection.active.iter().chain(&selection.installed_storage) {
        let descriptor = catalog
            .definitions
            .get(id)
            .expect("selected published definition");
        tx.world.runtime_definitions.insert(*id, descriptor.clone());
    }
    *tx.world.runtime_selection.get_mut() = selection;
}

fn specimen(memory: &Memory) -> Install {
    let manifest = vec![2, 3, 4];
    Install {
        scheme: memory.registration.scheme.scheme_id(),
        asset: memory.registration.asset.asset_digest(),
        manifest_digest: kagemusha_wallet_poseidon_bytes_v1(
            u64::from_le_bytes(*b"kgwparm1"),
            &manifest,
        ),
        manifest,
        signature: [7; 64],
        programs: vec![1, 2, 3],
    }
}

#[test]
fn governed_spartan_requires_the_complete_selection_reserve_and_originals() {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    let unselected = state.view().world.runtime_selection().clone();
    let active = selected(&state);
    spartan::validate_cutover(&unselected, &active, 25).unwrap();
    let restricted = {
        use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
        let mut restricted = unselected.clone();
        let source = restricted
            .bindings
            .iter()
            .find(|b| b.wire_id == KagemushaWalletLedgerV1::WIRE_ID)
            .unwrap()
            .implementation;
        restricted.restrictions.push(RestrictionEntryV1 {
            definition: source,
            restriction: RestrictionV1::Disabled(20),
        });
        restricted
    };
    assert!(spartan::validate_cutover(&restricted, &active, 25).is_err());
    let mut partial = active.clone();
    partial
        .bindings
        .retain(|binding| binding.wire_id != Unload::WIRE_ID);
    let mut missing_activate = active.clone();
    missing_activate
        .bindings
        .retain(|binding| binding.wire_id != Activate::WIRE_ID);
    assert!(spartan::validate_cutover(&unselected, &missing_activate, 25).is_err());
    let install = specimen(&memory);
    register(&mut state, &memory, 1).unwrap();
    let scope = routing::Operation::from_instruction(&install)
        .unwrap()
        .dataspace(state.view().world())
        .unwrap();
    assert_eq!(scope, iroha_model_base::topology::DataSpaceId::UNIVERSAL);
    let mut wrong_asset = install.clone();
    wrong_asset.asset[1] ^= 1;
    assert!(
        routing::Operation::from_instruction(&wrong_asset)
            .unwrap()
            .dataspace(state.view().world())
            .is_err()
    );
    let submit = |state: &mut State, who: &AccountId| {
        transact(state, 2, |tx| spartan::install(tx, who, &install))
    };
    assert!(matches!(
        submit(&mut state, &memory.registration.reserve),
        Err(Error::Binding)
    ));
    for (height, selection) in [(2, partial), (3, active.clone())] {
        transact(&mut state, height, |tx| {
            select_published(tx, selection);
            Ok(())
        })
        .unwrap();
        assert!(submit(&mut state, &memory.authority).is_err());
    }
    assert!(matches!(
        submit(&mut state, &memory.authority),
        Err(Error::Execution(_))
    ));
    assert!(submit(&mut state, &memory.registration.reserve).is_err());
    assert!(
        state
            .view()
            .world
            .kagemusha_spartan_programs()
            .iter()
            .next()
            .is_none()
    );

    // Even a damaged pre-existing historical installation is occupied. Refuse before
    // interpreting its public programs or the candidate's manifest.
    assert!(matches!(
        transact(&mut state, 4, |tx| {
            tx.world
                .kagemusha_wallet_ledger
                .insert(artifacts::key(install.scheme), vec![0xff]);
            spartan::install(tx, &memory.registration.reserve, &install)
        }),
        Err(Error::Conflict)
    ));
    // A restored original under a different reserve or a dual verifier selection is refused.
    transact(&mut state, 4, |tx| {
        tx.world.kagemusha_spartan_programs.insert(install.scheme, install.clone());
        assert!(spartan::validate_snapshot(&*tx.world).is_err());
        assert!(spartan::check_transition(&*tx.world, &unselected).is_err());
        let old = KagemushaWalletLedgerV1::new(install.scheme,
            iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerActionV1::InstallVerifierPack {
                asset: install.asset, manifest_digest: install.manifest_digest, pack: vec![1]
            });
        assert!(crate::smartcontracts::isi::kagemusha_wallet::dispatch_spartan_ledger(
            &old.into(), &memory.registration.reserve, tx).unwrap().is_err());
        let malformed = Unload {scheme:install.scheme,asset:install.asset,manifest_digest:[0;32],claim:vec![1]};
        assert!(matches!(spartan::unload(tx, &memory.authority, &malformed), Err(Error::Binding)));
        Ok(())
    }).unwrap();
}

#[test]
fn spartan_typed_wire_roundtrips_and_unknown_profile_is_inactive() {
    let memory = Memory::new();
    let state = world_state(&memory, true);
    let install = specimen(&memory);
    let unload = Unload {
        scheme: install.scheme,
        asset: install.asset,
        manifest_digest: install.manifest_digest,
        claim: vec![2, 3, 4],
    };
    let baseline = state.view().world.runtime_selection().clone();
    for (wire, bytes) in [
        (Install::WIRE_ID, norito::to_bytes(&install).unwrap()),
        (
            Activate::WIRE_ID,
            norito::to_bytes(&Activate {
                scheme: install.scheme,
                asset: install.asset,
                manifest_digest: install.manifest_digest,
                wallet: [4; 32],
                activation: vec![1],
            })
            .unwrap(),
        ),
        (Unload::WIRE_ID, norito::to_bytes(&unload).unwrap()),
    ] {
        let instruction = iroha_data_model::isi::decode_instruction_envelope(wire, &bytes).unwrap();
        assert!(
            crate::runtime::envelope::refuse_unexecutable_envelope(&baseline, 10, &instruction)
                .is_err()
        );
        assert!(
            crate::runtime::envelope::refuse_unexecutable_envelope(
                &selected(&state),
                30,
                &instruction
            )
            .is_ok()
        );
        assert!(
            iroha_data_model::isi::decode_instruction_envelope(wire, &bytes[..bytes.len() - 1])
                .unwrap()
                .as_opaque()
                .is_some()
        );
    }
}

fn world_for_spartan_size_limits() -> (Memory, State) {
    let memory = Memory::new();
    let mut state = world_state(&memory, true);
    register(&mut state, &memory, 1).unwrap();
    let active = selected(&state);
    transact(&mut state, 2, |tx| {
        select_published(tx, active);
        Ok(())
    })
    .unwrap();
    (memory, state)
}

#[test]
fn governed_spartan_installation_checks_the_exact_framed_limit() {
    let (memory, mut state) = world_for_spartan_size_limits();
    let mut installation = specimen(&memory);
    let encoded_len = |original: &Install| {
        let _layout = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
        norito::core::encoded_frame_len(original).unwrap()
    };
    installation.programs.resize(Install::MAX_BYTES, 1);
    let overhead = encoded_len(&installation) - Install::MAX_BYTES;
    installation
        .programs
        .truncate(Install::MAX_BYTES - overhead);
    assert_eq!(encoded_len(&installation), Install::MAX_BYTES);
    let mut submit = |original: &Install| {
        transact(&mut state, 3, |tx| {
            spartan::install(tx, &memory.registration.reserve, original)
        })
    };
    // At the ceiling, the intentionally unsigned specimen reaches authentication.
    assert!(matches!(submit(&installation), Err(Error::Proof)));
    installation.programs.push(1);
    assert_eq!(encoded_len(&installation), Install::MAX_BYTES + 1);
    assert!(matches!(submit(&installation), Err(Error::Binding)));
}

#[test]
fn governed_spartan_installation_checks_the_exact_manifest_limit() {
    let (memory, mut state) = world_for_spartan_size_limits();
    let mut installation = specimen(&memory);
    installation.manifest.resize(Install::MAX_MANIFEST_BYTES, 1);
    let mut submit = |original: &mut Install| {
        original.manifest_digest = kagemusha_wallet_poseidon_bytes_v1(
            u64::from_le_bytes(*b"kgwparm1"),
            &original.manifest,
        );
        transact(&mut state, 3, |tx| {
            spartan::install(tx, &memory.registration.reserve, original)
        })
    };
    assert!(matches!(submit(&mut installation), Err(Error::Proof)));
    installation.manifest.push(1);
    assert!(matches!(submit(&mut installation), Err(Error::Binding)));
}

#[test]
fn governed_spartan_activation_checks_the_exact_claim_limit() {
    let (memory, mut state) = world_for_spartan_size_limits();
    let registration = specimen(&memory);
    let mut request = Activate {
        scheme: registration.scheme,
        asset: registration.asset,
        manifest_digest: registration.manifest_digest,
        wallet: [4; 32],
        activation: vec![1; Activate::MAX_ACTIVATION_BYTES],
    };
    let mut submit = |original: &Activate| {
        transact(&mut state, 3, |tx| {
            spartan::activate(tx, &memory.authority, original)
        })
    };
    // The registered scheme has no installed program. That lookup follows size admission.
    assert!(matches!(submit(&request), Err(Error::Unavailable)));
    request.activation.push(1);
    assert!(matches!(submit(&request), Err(Error::Proof)));
}

fn published_install() -> Install {
    let wire = std::fs::read(std::env::var_os("KAGEMUSHA_SPARTAN_WORLD_INSTALL").unwrap()).unwrap();
    assert!(wire.len() <= Install::MAX_BYTES);
    let install: Install =
        norito::decode_from_bytes_with_limits(&wire, norito::canonical_decode_limits(wire.len()))
            .unwrap();
    let _fixed = norito::core::WireLayoutGuard::enter(norito::core::WireLayout::Fixed);
    assert!(
        norito::to_bytes(&install).unwrap() == wire,
        "noncanonical fixed-layout published installation"
    );
    install
}

fn install_under_fixture_governance(
    fixture: &iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture,
    memory: &Memory,
    install: &Install,
) -> State {
    use iroha_data_model::block::SharedSignedBlock;
    let reserve = memory.registration.reserve.clone();
    let mut state = world_state(memory, true);
    state.chain_id = fixture.chain_id().parse().unwrap();
    let genesis = SharedSignedBlock::try_new(
        fixture.genesis().clone(),
        &iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
    )
    .unwrap();
    state.kura().store_block(genesis).unwrap();
    state.push_block_hash_for_testing(fixture.genesis().hash());
    let activation = selected(&state);
    transact(&mut state, 2, |tx| {
        for original in fixture.genesis().external_transactions() {
            for instruction in original.instructions().explicit_instructions() {
                if let Some(parameter) = instruction
                    .as_any()
                    .downcast_ref::<iroha_data_model::isi::SetParameter>()
                {
                    tx.world.set_parameter_for_testing(&parameter.0).unwrap();
                }
            }
        }
        assert_eq!(
            crate::sumeragi::lanes::routing::committed_root_scope(&*tx.world),
            Some(fixture.root_scope())
        );
        select_published(tx, activation);
        wsv::WsvLedger::new(tx, &reserve)?.register(&memory.registration)
    })
    .unwrap();
    transact(&mut state, 3, |tx| {
        install
            .clone()
            .execute(&reserve, tx)
            .map_err(Error::Execution)
    })
    .unwrap();
    assert_eq!(
        state
            .view()
            .world
            .kagemusha_spartan_programs()
            .get(&install.scheme),
        Some(install)
    );
    spartan::validate_snapshot(&state.view().world).unwrap();
    transact(&mut state, 4, |tx| {
        install
            .clone()
            .execute(&reserve, tx)
            .map_err(Error::Execution)
    })
    .unwrap();
    state
}

#[test]
#[ignore = "requires a real true-genesis v4 installation and independently selected manifest digest"]
fn governed_spartan_installs_exact_published_source_and_validates_its_snapshot() {
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let install = published_install();
    let selected: [u8; 32] =
        hex::decode(std::env::var("KAGEMUSHA_SPARTAN_WORLD_MANIFEST_DIGEST").unwrap())
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(install.manifest_digest, selected);
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let mut memory = Memory::new();
    memory.registration.scheme.network_id = *fixture.network_id().as_bytes();
    memory.registration.scheme.relation_id = [0x33; 32];
    assert_eq!(memory.registration.scheme.scheme_id(), install.scheme);
    assert_eq!(memory.registration.asset.asset_digest(), install.asset);
    install_under_fixture_governance(&fixture, &memory, &install);
}

/// The independent fixture operator supplies the selected digest and the original charged claim.
#[test]
#[ignore = "requires a fresh v4 installation and genuine charged Unload issued on the fixture's actual signed genesis"]
fn governed_spartan_installs_and_pays_the_genesis_scoped_signed_original() {
    use iroha_core_zk::kagemusha_spartan_native::native::{AuthenticatedParameters, NativeEngine};
    use iroha_data_model::sumeragi_finality::{
        SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture,
    };
    let dir =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_WORLD_PROGRAMS").unwrap());
    let read = |name| std::fs::read(dir.join(name)).unwrap();
    let claim = std::fs::read(std::env::var_os("KAGEMUSHA_SPARTAN_WORLD_CLAIM").unwrap()).unwrap();
    let expected: [u8; 32] =
        hex::decode(std::env::var("KAGEMUSHA_SPARTAN_WORLD_MANIFEST_DIGEST").unwrap())
            .unwrap()
            .try_into()
            .unwrap();
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let native = fixture.verifier();
    let network = iroha_data_model::NetworkId::from_genesis_hash(fixture.genesis().hash());
    let mut memory = Memory::new();
    memory.registration.scheme.network_id = *network.as_bytes();
    memory.registration.scheme.relation_id = [0x33; 32];
    let scheme = memory.registration.scheme.scheme_id();
    let asset = memory.registration.asset.asset_digest();
    let install = Install {
        scheme,
        asset,
        manifest_digest: expected,
        manifest: read("parameter-manifest.norito"),
        signature: read("parameter-signature.bin").try_into().unwrap(),
        programs: read("program-bundle.norito"),
    };
    let published = published_install();
    assert!(
        install == published,
        "World must execute the independently published registration source"
    );
    let params = AuthenticatedParameters::install(
        &install.programs,
        &install.manifest,
        install.signature,
        &memory.registration.scheme,
        SumeragiCommitVerifierV1::new(&native).unwrap(),
    )
    .unwrap();
    assert_eq!(params.manifest_digest(), &expected);
    let verified = NativeEngine::default()
        .verify_unload_claim(&claim, &params, &scheme, &asset)
        .unwrap();
    let account = verified.destinations().account.clone();
    let beneficiary = verified
        .destinations()
        .charge
        .beneficiary()
        .unwrap()
        .clone();
    assert_eq!(
        (
            verified.amount(),
            verified.account_payout(),
            verified.online_charge()
        ),
        (4, 3, 1)
    );
    let key = KagemushaWalletPayoutKeyV1::Unload(*verified.nullifier()).ledger_key(scheme);
    drop(verified);
    drop(params);
    let reserve = memory.registration.reserve.clone();
    memory.balances.insert(account.clone(), 0);
    memory.balances.insert(beneficiary.clone(), 0);
    memory.balances.insert(reserve.clone(), 4);
    let mut state = install_under_fixture_governance(&fixture, &memory, &install);
    let request = Unload {
        scheme,
        asset,
        manifest_digest: expected,
        claim,
    };
    use iroha_data_model::runtime::definition::RestrictionV1;
    let implementation = state
        .view()
        .world
        .runtime_selection()
        .bindings
        .iter()
        .find(|binding| binding.wire_id == Unload::WIRE_ID)
        .unwrap()
        .implementation;
    let invalid = Unload {
        claim: vec![1],
        ..request.clone()
    };
    let rejected = transact(&mut state, 5, |tx| {
        let restricted = retirement::restrict(
            tx.world.runtime_selection(),
            implementation,
            RestrictionV1::AdmissionClosed(5),
        );
        select_published(tx, restricted);
        crate::smartcontracts::isi::execute_borrowed_instruction(
            &invalid.into(),
            &memory.authority,
            tx,
        )
        .map_err(Error::Execution)
    })
    .unwrap_err();
    assert!(
        rejected.to_string().contains(&Error::Proof.to_string()),
        "{rejected}"
    );
    for (height, restriction) in [
        (5, RestrictionV1::AdmissionClosed(5)),
        (6, RestrictionV1::CompletionOnly(6)),
    ] {
        transact(&mut state, height, |tx| {
            let current = tx.world.runtime_selection().clone();
            let restricted = retirement::restrict(&current, implementation, restriction);
            spartan::check_transition(&*tx.world, &restricted)?;
            select_published(tx, restricted);
            crate::smartcontracts::isi::execute_borrowed_instruction(
                &request.clone().into(),
                &memory.authority,
                tx,
            )
            .map_err(Error::Execution)
        })
        .unwrap();
    }
    let amount = |n: u128| {
        Quantity::from_canonical_numeric(Numeric::new(n, memory.registration.asset.scale)).unwrap()
    };
    assert_eq!(balance(&state, &memory, &reserve), amount(0));
    assert_eq!(balance(&state, &memory, &account), amount(3));
    assert_eq!(balance(&state, &memory, &beneficiary), amount(1));
    let view = state.view();
    let row: KagemushaWalletPayoutRecordV1 = storage::decode(
        view.world.kagemusha_wallet_ledger().get(&key).unwrap(),
        storage::PAYOUT_CAP,
    )
    .unwrap();
    assert_eq!(row.transaction, *Hash::new(5u64.to_le_bytes()).as_ref());
}

/// Replay the isolated fixture payment key's public Bootstrap and its first ordinary Load.
#[test]
#[ignore = "requires a signed true-genesis Spartan Bootstrap claim and selected public installation"]
fn governed_spartan_activates_an_enrolled_wallet_before_its_first_load() {
    use iroha_core_zk::kagemusha_spartan_native::native::NativeEngine;
    use iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerActionV1 as Action;
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
    let dir =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_ACTIVATION_FIXTURE").unwrap());
    let install = published_install();
    let selected: [u8; 32] =
        hex::decode(std::env::var("KAGEMUSHA_SPARTAN_WORLD_MANIFEST_DIGEST").unwrap())
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(install.manifest_digest, selected);
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let mut memory = Memory::new();
    memory.registration.scheme.network_id = *fixture.network_id().as_bytes();
    memory.registration.scheme.relation_id = [0x33; 32];
    let account_bytes = std::fs::read(dir.join("activation-account.norito")).unwrap();
    let account: AccountId = norito::decode_canonical_with_limits(
        &account_bytes,
        norito::canonical_decode_limits(account_bytes.len()),
    )
    .unwrap();
    let credential = std::fs::read(dir.join("activation-credential.bin")).unwrap();
    let fields = NativeEngine::credential_original_fields(&credential).unwrap();
    let activate = Activate {
        scheme: install.scheme,
        asset: install.asset,
        manifest_digest: selected,
        wallet: fields.wallet_id,
        activation: std::fs::read(dir.join("signed-activation-claim.norito")).unwrap(),
    };
    let package = NativeEngine::activation_relay_package(
        &activate.activation,
        &memory.registration.scheme,
        &install.asset,
        &credential,
        &account,
    )
    .unwrap();
    let load = KagemushaWalletLedgerV1::new(
        install.scheme,
        Action::IssueLoad {
            wallet: fields.wallet_id,
            asset: install.asset,
            ordinal: 0,
            request_id: [0x88; 32],
            amount: 4,
            charge: None,
        },
    );
    memory.balances.insert(account.clone(), 9);
    let mut state = install_under_fixture_governance(&fixture, &memory, &install);
    assert!(routing::dataspace(&state.view().world, &load).is_err());
    use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
    use iroha_data_model::transaction::{
        FeePaymentIntent, TransactionBuilder, TransactionEntrypoint,
    };
    // Public RFC 8032 fixture signer for this isolated enrollment.
    let signer = PrivateKey::from_bytes(
        Algorithm::Ed25519,
        &hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60").unwrap(),
    )
    .unwrap();
    assert_eq!(
        AccountId::new(
            KeyPair::from_private_key(signer.clone())
                .unwrap()
                .public_key()
                .clone()
        ),
        account
    );
    let signed = TransactionBuilder::new(
        fixture.network_id(),
        account.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([load.clone()])
    .sign(&signer);
    let entrypoint = TransactionEntrypoint::External(signed);
    // The FASTPQ producer and committed Load must name the same signed invocation.
    let invocation = Hash::from(entrypoint.execution_call_hash());
    let issue = |state: &mut State, height| {
        transact_with_invocation(state, height, invocation, |tx| {
            crate::state::seed_committed_transaction_context(tx, &entrypoint, 0);
            tx.current_direct_kagemusha_load_instruction_index = Some(0);
            load.clone().execute(&account, tx).map_err(Error::Execution)
        })
    };
    assert!(issue(&mut state, 5).is_err());
    let wrong = &memory.registration.reserve;
    assert_ne!(&account, wrong);
    assert!(
        transact(&mut state, 5, |tx| activate
            .clone()
            .execute(wrong, tx)
            .map_err(Error::Execution))
        .is_err()
    );
    let mut wrong_pin = activate.clone();
    wrong_pin.manifest_digest[0] ^= 1;
    assert!(
        transact(&mut state, 5, |tx| wrong_pin
            .execute(&account, tx)
            .map_err(Error::Execution))
        .is_err()
    );
    transact(&mut state, 5, |tx| {
        activate
            .clone()
            .execute(&account, tx)
            .map_err(Error::Execution)
    })
    .unwrap();
    assert!(routing::dataspace(&state.view().world, &load).is_ok());
    issue(&mut state, 6).unwrap();
    transact(&mut state, 7, |tx| {
        activate
            .clone()
            .execute(&account, tx)
            .map_err(Error::Execution)?;
        let ledger = wsv::WsvLedger::new(tx, &account)?;
        let issuance = ledger
            .issuance(&install.scheme, &fields.wallet_id, &[0x88; 32])?
            .unwrap();
        assert_eq!(issuance.payer, account);
        assert_eq!((issuance.command.ordinal, issuance.command.amount), (0, 4));
        assert_eq!(issuance.body.wallet_id, fields.wallet_id);
        assert_eq!(issuance.body.scheme_id, install.scheme);
        let record = ledger.wallet(&install.scheme, &fields.wallet_id)?.unwrap();
        assert_eq!(
            (
                record.phase,
                record.activation,
                record.asset,
                record.next_load
            ),
            (Phase::Active, package, install.asset, 1)
        );
        Ok(())
    })
    .unwrap();
    let amount = |n: u128| {
        Quantity::from_canonical_numeric(Numeric::new(n, memory.registration.asset.scale)).unwrap()
    };
    assert_eq!(balance(&state, &memory, &account), amount(5));
    let reserve = &memory.registration.reserve;
    assert_eq!(
        balance(&state, &memory, reserve),
        amount(memory.balances[reserve] + 4)
    );
}
