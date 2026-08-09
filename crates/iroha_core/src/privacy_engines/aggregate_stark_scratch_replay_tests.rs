#[test]
fn encrypted_field_scratch_rejects_authenticated_noncanonical_fields() {
    let mut scratch = encrypted_scratch_fixture();
    let record_index = 0_u64;
    let nonce = encrypted_field_scratch_nonce_v1(&scratch.nonce_prefix, record_index);
    let aad = encrypted_field_scratch_record_aad_v1(
        scratch.rows,
        scratch.width,
        scratch.chunk_rows,
        record_index,
    )
    .expect("aad");
    let mut plaintext = Zeroizing::new(Vec::new());
    for _ in 0..scratch.chunk_rows {
        plaintext.extend_from_slice(&u64::MAX.to_be_bytes());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(scratch.key.as_ref()).expect("fixed key length");
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_slice(),
                aad: &aad,
            },
        )
        .expect("test encryption");
    assert_eq!(ciphertext.len(), scratch.ciphertext_chunk_bytes);
    scratch
        .file
        .seek(std::io::SeekFrom::Start(0))
        .expect("seek");
    scratch.file.write_all(&ciphertext).expect("replace record");
    assert!(scratch.read_chunk(0).is_err());
}

#[test]
fn replayed_masked_trace_spill_matches_exact_masked_lde_rows() {
    let native_log2 = 3;
    let lde_log2 = 6;
    let native_columns = [
        (0..1_usize << native_log2)
            .map(|index| F(u64::try_from(index + 1).expect("small")))
            .collect::<Vec<_>>(),
        (0..1_usize << native_log2)
            .map(|index| F(u64::try_from(index * 7 + 3).expect("small")))
            .collect::<Vec<_>>(),
    ];
    let mut rng = StdRng::seed_from_u64(0x5C12_A7C4);
    let (_, masks) = commit_masked_trace_columns_v1(
        DOMAINS.base_leaf,
        DOMAINS.base_node,
        0,
        native_log2,
        lde_log2,
        native_columns.len(),
        7,
        &[],
        &mut rng,
        |column| Ok(native_columns[column].clone()),
    )
    .expect("masked commitment");
    let expected = masks
        .masks
        .iter()
        .zip(&native_columns)
        .map(|(mask, native)| {
            masked_trace_lde_column_with_mask_v1(native, native_log2, lde_log2, mask.coefficients())
                .expect("masked LDE")
        })
        .collect::<Vec<_>>();
    let mut scratch =
        spill_replayed_masked_trace_columns_v1(&masks, |column| Ok(native_columns[column].clone()))
            .expect("spill");
    assert_eq!(scratch.chunk_count(), 1);
    let block = scratch.read_chunk(0).expect("block");
    for row in 0..1_usize << lde_log2 {
        assert_eq!(
            block.row(row).expect("row"),
            &[expected[0][row], expected[1][row]]
        );
    }
}
