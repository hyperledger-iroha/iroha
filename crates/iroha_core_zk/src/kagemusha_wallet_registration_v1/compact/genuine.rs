//! Actual-artifact differential gate. No fixture or DATA constructor supplies a proof grant.
use super::*;
use ff::Field;

fn pinned_input(directory: &PrivateDirectory, role: &str, maximum: usize) -> Vec<u8> {
    let digest = std::env::var(format!("KAGEMUSHA_{role}_SHA256")).expect("exact original SHA pin");
    assert_eq!(digest.len(), 64);
    let bytes: [u8; 32] = hex::decode(&digest).unwrap().try_into().unwrap();
    assert_eq!(hex::encode(bytes), digest, "canonical lowercase SHA pin");
    let length = std::env::var(format!("KAGEMUSHA_{role}_BYTES"))
        .expect("exact original byte-length pin")
        .parse()
        .unwrap();
    read_original(
        directory,
        BlobV1 {
            sha256: bytes,
            bytes: length,
        },
        maximum,
        &mut || false,
    )
    .unwrap()
}

fn same_registration(
    native: &FinalizedKagemushaWalletRegistrationV1,
    compact: &FinalizedKagemushaWalletRegistrationV1,
) {
    assert_eq!(native.scheme_original(), compact.scheme_original());
    assert_eq!(native.asset_original(), compact.asset_original());
    assert_eq!(native.reserve(), compact.reserve());
    assert_eq!(native.height(), compact.height());
    assert_eq!(native.block_hash(), compact.block_hash());
    assert_eq!(native.transaction_hash(), compact.transaction_hash());
    assert_eq!(native.instruction_index(), compact.instruction_index());
}

#[test]
#[ignore = "requires pinned signed complete inventory, genuine terminal Register history proof and native source originals"]
fn genuine_native_and_compact_registration_agree_and_reject_mutations() {
    let directory = PrivateDirectory::open_exact(
        std::env::var_os("KAGEMUSHA_REGISTRATION_DIFFERENTIAL_ORIGINALS")
            .expect("private content-addressed differential originals"),
    )
    .unwrap();
    let native_bytes = pinned_input(
        &directory,
        "NATIVE_REGISTRATION",
        REGISTRATION_SOURCE_MAX_BYTES_V1,
    );
    let compact_bytes = pinned_input(
        &directory,
        "COMPACT_REGISTRATION",
        COMPACT_REGISTRATION_MAX_BYTES_V1,
    );
    let native_source = RegistrationSourceV1::decode_canonical(&native_bytes).unwrap();
    let original = CompactRegistrationOriginalV1::decode_canonical(&compact_bytes).unwrap();
    assert_eq!(native_source.inventory.asset_digest, original.asset_digest);
    assert_eq!(
        native_source.inventory.instruction_index,
        original.instruction_index
    );
    assert_eq!(
        native_source.inventory.committed,
        BlobV1::of(&original.committed)
    );

    // This facade reauthenticates exact persisted signed originals and runs the ordinary
    // complete descriptor/VK graph qualification. It neither imports wallet PKs nor
    // constructs a wallet-open grant. Missing inputs fail; there is no component fallback.
    let output = tempfile::tempdir().unwrap();
    let (installed, graph, genesis, _originals) =
        crate::kagemusha_wallet_artifacts_v1::producer_inventory::open_pinned_engineering_finality_sources(
            &output.path().join("finality-admission")
        );
    let scheme = KagemushaWalletSchemeV1::decode_canonical(
        &installed.originals().scheme,
        &graph.installation().0,
    )
    .unwrap();
    let native =
        verify_registration_source_v1(&native_source, &genesis, &scheme, || false).unwrap();
    let budget = MemoryBudget::DEFAULT;
    let verify = |value: &CompactRegistrationOriginalV1| {
        verify_compact_registration_v1(value, &graph, &genesis, &scheme, budget, None)
    };
    let compact = verify(&original).unwrap();
    same_registration(&native, &compact);
    assert_eq!(compact.height(), native_source.inventory.proof_count);

    let prefix = HistoryOriginalV1::decode_canonical(&original.history)
        .unwrap()
        .restore_qualified(&graph, budget, None)
        .unwrap();
    let block = decode_framed_signed_block(&original.block).unwrap();
    let committed = decode(&original.committed).unwrap();
    let rebuilt = CompactRegistrationOriginalV1::from_terminal(
        &prefix,
        &block,
        &committed,
        &scheme,
        original.asset_digest,
        original.instruction_index,
        None,
    )
    .unwrap();
    assert_eq!(rebuilt.encode_canonical().unwrap(), compact_bytes);
    same_registration(&native, &verify(&rebuilt).unwrap());

    // Actual original proof plus genuine state: each modification reaches the full
    // qualified verifier, including independently changed Pallas and Vesta claims.
    for change in 0..10 {
        let mut state = *prefix.state();
        let mut evidence = prefix.evidence().clone();
        match change {
            0 => state.next_height += 1,
            1 => state.result[0] ^= 1,
            2 => state.tape_root += iroha_pasta::Fp::ONE,
            3 => state.frame_len += 1,
            4 => state.current.context[0] ^= 1,
            5 => state.following.context[0] ^= 1,
            6 => evidence.endpoints[5] += iroha_pasta::Fp::ONE,
            7 => evidence.proof[0] ^= 1,
            8 => {
                let mut challenges = *evidence.pallas.challenges();
                challenges[0] = -challenges[0];
                evidence.pallas =
                    iroha_plonk_recursion::AccumulatorT::new(*evidence.pallas.g(), challenges)
                        .unwrap();
            }
            _ => {
                let mut challenges = *evidence.vesta.challenges();
                challenges[0] = -challenges[0];
                evidence.vesta =
                    iroha_plonk_recursion::AccumulatorT::new(*evidence.vesta.g(), challenges)
                        .unwrap();
            }
        }
        assert!(
            graph
                .restore_history(&state, evidence, budget, None)
                .is_err(),
            "proof mutation {change}"
        );
    }
    for change in 0..7 {
        let mut changed = original.clone();
        match change {
            0 => changed.asset_digest[0] ^= 1,
            1 => changed.instruction_index = u32::MAX,
            2 => changed.result[0] ^= 1,
            3 => changed.result.push(0),
            4 => changed.block.push(0),
            5 => changed.committed.push(0),
            _ => changed.history.push(0),
        }
        assert!(verify(&changed).is_err(), "original mutation {change}");
    }
    let foreign = iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture::start_with_explicit_parameters("foreign-compact-registration").verifier();
    assert_ne!(derive_history_anchor(&foreign).unwrap(), *graph.anchor());
    assert!(
        verify_compact_registration_v1(&original, &graph, &foreign, &scheme, budget, None).is_err()
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        verify_compact_registration_v1(
            &original,
            &graph,
            &genesis,
            &scheme,
            budget,
            Some(&cancellation)
        ),
        Err(RegistrationErrorV1::Cancelled)
    ));
    assert!(
        CompactRegistrationOriginalV1::from_terminal(
            &prefix,
            &block,
            &committed,
            &scheme,
            original.asset_digest,
            original.instruction_index,
            Some(&cancellation)
        )
        .is_err()
    );
    same_registration(&native, &verify(&original).unwrap());
    directory.revalidate().unwrap();
}
