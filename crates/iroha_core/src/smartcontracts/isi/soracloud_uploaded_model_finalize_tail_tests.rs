#[test]
fn soracloud_uploaded_model_finalize_rejects_pin_digest_changed_after_register()
-> Result<(), eyre::Report> {
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();

    deploy_uploaded_model_service(&mut stx)?;
    let digest = ManifestDigest::new([0xF1; 32]);
    insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
    let bundle = sample_uploaded_model_bundle("portal", digest);
    isi::RegisterSoracloudUploadedModelBundle {
        bundle: bundle.clone(),
        provenance: uploaded_model_bundle_provenance(&bundle),
    }
    .execute(&ALICE_ID, &mut stx)?;
    insert_uploaded_model_pin_record(
        &mut stx,
        digest,
        ManifestDigest::new([0xF2; 32]),
        bundle.ciphertext_bytes,
        PinStatus::Approved(2),
    );

    let result = sample_uploaded_model_finalize_instruction(
        &bundle,
        "uploaded-artifact-mutated-pin-digest",
        bundle.bundle_root,
    )
    .execute(&ALICE_ID, &mut stx);
    assert!(result.is_err());
    assert!(
        stx.world
            .soracloud_model_artifacts
            .get(&(
                "portal".to_string(),
                "uploaded-artifact-mutated-pin-digest".to_string(),
            ))
            .is_none()
    );
    Ok(())
}

#[test]
fn soracloud_uploaded_model_finalize_rejects_provenance_signer_mismatch() -> Result<(), eyre::Report>
{
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();

    deploy_uploaded_model_service(&mut stx)?;
    let digest = ManifestDigest::new([0xDB; 32]);
    insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
    let bundle = sample_uploaded_model_bundle("portal", digest);
    isi::RegisterSoracloudUploadedModelBundle {
        bundle: bundle.clone(),
        provenance: uploaded_model_bundle_provenance(&bundle),
    }
    .execute(&ALICE_ID, &mut stx)?;

    let mut instruction = sample_uploaded_model_finalize_instruction(
        &bundle,
        "uploaded-artifact-signer-mismatch",
        bundle.bundle_root,
    );
    instruction.provenance = uploaded_model_finalize_provenance_for(
        &bundle.service_name,
        "vision_model",
        &bundle.model_id,
        "uploaded-artifact-signer-mismatch",
        &bundle.weight_version,
        bundle.bundle_root,
        instruction.weight_artifact_hash,
        &instruction.dataset_ref,
        instruction.training_config_hash,
        instruction.reproducibility_hash,
        instruction.provenance_attestation_hash,
        &BOB_KEYPAIR,
    );
    let result = instruction.execute(&ALICE_ID, &mut stx);
    assert!(result.is_err());
    assert!(
        stx.world
            .soracloud_model_artifacts
            .get(&(
                "portal".to_string(),
                "uploaded-artifact-signer-mismatch".to_string(),
            ))
            .is_none()
    );
    Ok(())
}

#[test]
fn soracloud_uploaded_model_finalize_rejects_retired_pin_after_register() -> Result<(), eyre::Report>
{
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_soracloud_permission(&kura)?;
    let block_header = ValidBlock::new_dummy(&checked_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    let mut stx = state_block.transaction();

    deploy_uploaded_model_service(&mut stx)?;
    let digest = ManifestDigest::new([0xD7; 32]);
    insert_uploaded_model_pin(&mut stx, digest, PinStatus::Approved(1));
    let bundle = sample_uploaded_model_bundle("portal", digest);
    isi::RegisterSoracloudUploadedModelBundle {
        bundle: bundle.clone(),
        provenance: uploaded_model_bundle_provenance(&bundle),
    }
    .execute(&ALICE_ID, &mut stx)?;
    insert_uploaded_model_pin(&mut stx, digest, PinStatus::Retired(12));

    let result = sample_uploaded_model_finalize_instruction(
        &bundle,
        "uploaded-artifact-retired",
        bundle.bundle_root,
    )
    .execute(&ALICE_ID, &mut stx);
    assert!(result.is_err());
    assert!(
        stx.world
            .soracloud_model_artifacts
            .get(&(
                "portal".to_string(),
                "uploaded-artifact-retired".to_string(),
            ))
            .is_none()
    );
    Ok(())
}
