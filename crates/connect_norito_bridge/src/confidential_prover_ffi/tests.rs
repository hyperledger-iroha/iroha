//! Native lifecycle, shape admission and actual canonical-proof controls.
use super::*;
use iroha_core::zk::confidential_v2::{
    compute_confidential_merkle_path_v3, default_confidential_diversifier_v2,
    derive_confidential_note_v2, derive_confidential_owner_tag_v2_with_diversifier,
};
use iroha_crypto::Hash;

fn fixture() -> (u64, [u8; 32], [u8; 32], [u8; 32]) {
    let asset = AssetDefinitionId::from_uuid_bytes([
        1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])
    .unwrap();
    let key = [7; 32];
    let diversifier = default_confidential_diversifier_v2();
    let rho = [9; 32];
    let owner = derive_confidential_owner_tag_v2_with_diversifier(&key, diversifier).unwrap();
    let leaf = derive_confidential_note_v2(&asset.to_string(), 42, rho, owner).unwrap();
    let id = create(
        Hash::new(b"mobile-wallet-prover-test").as_ref(),
        asset.to_string().as_bytes(),
        &key,
    )
    .unwrap();
    (id, leaf, rho, diversifier)
}
#[test]
fn accepted_job_retains_one_owner_after_close_and_handles_never_repeat() {
    let (id, _, _, _) = fixture();
    let weak = locked(|registry| Ok(Arc::downgrade(registry.provers.get(&id).unwrap()))).unwrap();
    let job = job_create(id, 0, &[1; 32], 0, 0).unwrap();
    close(id).unwrap();
    assert!(weak.upgrade().is_some());
    assert_eq!(job_create(id, 0, &[1; 32], 0, 0), Err(CLOSED));
    job_close(job).unwrap();
    assert!(weak.upgrade().is_none());
    assert_eq!(job_close(job), Err(CLOSED));
    let (second, _, _, _) = fixture();
    assert_ne!(second, id);
    close(second).unwrap();
}
#[test]
fn malformed_inputs_fail_before_work_and_failed_prove_consumes_job() {
    let (id, _, rho, diversifier) = fixture();
    let job = job_create(id, 1, &[1; 32], 42, 0).unwrap();
    assert_eq!(
        job_input(job, 42, 0, &rho[..31], &diversifier, 0),
        Err(INVALID)
    );
    assert_eq!(job_input(job, 42, 0, &rho, &diversifier, 65_536), Err(-15));
    job_input(job, 42, 0, &rho, &diversifier, 0).unwrap();
    assert_eq!(job_paths(job, &[0; 512], &[1; 16]), Err(-16));
    assert_eq!(job_paths(job, &[0; 1024], &[0; 32]), Err(-13));
    assert_eq!(job_prove(job), Err(INVALID));
    assert_eq!(job_prove(job), Err(CLOSED));
    close(id).unwrap();
    let mut output = 999;
    let code = unsafe {
        connect_norito_confidential_prover_create_v1(
            std::ptr::null(),
            32,
            std::ptr::null(),
            0,
            std::ptr::null(),
            32,
            &mut output,
        )
    };
    assert_eq!(code, INVALID);
    assert_eq!(output, 0);
}
#[test]
fn real_full_redemption_survives_context_close_and_exports_only_public_result() {
    let (id, leaf, rho, diversifier) = fixture();
    let path = compute_confidential_merkle_path_v3(&[leaf], 0).unwrap();
    let job = job_create(id, 1, &path.root, 42, 0).unwrap();
    job_input(job, 42, 0, &rho, &diversifier, 0).unwrap();
    let siblings = Zeroizing::new(
        path.siblings
            .iter()
            .flat_map(|word| word.iter().copied())
            .collect::<Vec<_>>(),
    );
    job_paths(job, &siblings, &path.directions).unwrap();
    close(id).unwrap();
    // The worker crosses the real native ownership boundary; no global lock is retained.
    let output = std::thread::spawn(move || {
        let mut pointer = std::ptr::null_mut();
        let mut length = 0;
        let status = unsafe {
            connect_norito_confidential_prover_job_prove_v1(job, &mut pointer, &mut length)
        };
        assert_eq!(status, 0);
        assert!(!pointer.is_null());
        let bytes = unsafe { std::slice::from_raw_parts(pointer, length as usize) }.to_vec();
        super::super::connect_norito_free(pointer);
        bytes
    })
    .join()
    .unwrap();
    assert!(output.len() < MAX_RESULT_BYTES);
    let value: norito::json::Value = norito::json::from_slice(&output).unwrap();
    assert_eq!(
        value["relation"].as_str(),
        Some("confidential_full_unshield")
    );
    assert!(!value["proof_hex"].as_str().unwrap().is_empty());
    assert_eq!(value["nullifiers_hex"].as_array().unwrap().len(), 1);
    assert!(
        value["output_commitments_hex"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(value.as_object().unwrap().len(), 6);
    assert_eq!(job_prove(job), Err(CLOSED));
}

#[test]
fn typed_job_builder_preserves_unsigned_limbs_and_rejects_extra_slots() {
    let (id, _, rho, diversifier) = fixture();
    let job = job_create(id, 0, &[1; 32], 0, 0).unwrap();
    job_input(job, u64::MAX, 1, &rho, &diversifier, 0).unwrap();
    job_output(job, u64::MAX, 1, &rho, &[4; 32]).unwrap();
    locked(|registry| {
        let job = registry.jobs.get(&job).unwrap();
        assert_eq!(job.inputs[0].amount, (1u128 << 65) - 1);
        assert_eq!(job.outputs[0].amount, job.inputs[0].amount);
        assert_eq!(job.outputs[0].owner_tag, [4; 32]);
        Ok(())
    })
    .unwrap();
    job_input(job, 1, 0, &rho, &diversifier, 1).unwrap();
    assert_eq!(job_input(job, 1, 0, &rho, &diversifier, 2), Err(-11));
    assert_eq!(job_commitments(job, &[0; 32]), Err(-15));
    job_commitments(job, &[0; 64]).unwrap();
    assert_eq!(job_output(job, 1, 0, &rho, &[4; 32]), Err(INVALID));
    job_close(job).unwrap();
    let change = job_create(id, 1, &[1; 32], 1, 0).unwrap();
    assert_eq!(job_output(change, 1, 0, &rho, &[4; 32]), Err(INVALID));
    job_output(change, 1, 0, &rho, &[]).unwrap();
    assert_eq!(job_output(change, 1, 0, &rho, &[]), Err(-22));
    job_close(change).unwrap();
    close(id).unwrap();
}

#[test]
fn second_path_failure_discards_first_owned_path_and_keeps_job_unbound() {
    let (id, _, rho, diversifier) = fixture();
    let job = job_create(id, 0, &[1; 32], 0, 0).unwrap();
    job_input(job, 1, 0, &rho, &diversifier, 0).unwrap();
    job_input(job, 1, 0, &rho, &diversifier, 1).unwrap();
    let siblings = Zeroizing::new(vec![7; 1024]);
    let mut directions = Zeroizing::new(vec![0; 32]);
    // The first private path is fully populated before the second index mismatch.
    assert_eq!(job_paths(job, &siblings, &directions), Err(-16));
    locked(|registry| {
        assert!(registry.jobs.get(&job).unwrap().evidence.is_none());
        Ok(())
    })
    .unwrap();
    directions[16] = 1;
    job_paths(job, &siblings, &directions).unwrap();
    job_close(job).unwrap();
    close(id).unwrap();
}

#[test]
fn unconserved_redemption_returns_stable_public_amount_error_and_clears_c_outputs() {
    let (id, leaf, rho, diversifier) = fixture();
    let job = job_create(id, 1, &[1; 32], 8, 0).unwrap();
    job_input(job, 7, 0, &rho, &diversifier, 0).unwrap();
    job_commitments(job, &leaf).unwrap();
    // Public conservation rejects before proving or testing this synthetic root.
    let mut pointer = std::ptr::dangling_mut::<u8>();
    let mut length = 99;
    let status =
        unsafe { connect_norito_confidential_prover_job_prove_v1(job, &mut pointer, &mut length) };
    assert_eq!(status, -21);
    assert!(pointer.is_null());
    assert_eq!(length, 0);
    assert_eq!(job_prove(job), Err(CLOSED));
    close(id).unwrap();
}
