// Parsed-owner and partial-construction erasure regression controls.

fn assert_der_cells_cleared(
    observations: &[crate::privacy_engines::zk_x509::private_table::inspection::ErasureObservationV1],
) {
    assert!(observations.iter().any(|item| item.nonzero_before > 0));
    assert!(observations.iter().all(|item| item.nonzero_after == 0));
}

#[test]
fn der_partial_compiler_and_signature_errors_clear_initialized_owners() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    // The first child has already emitted private rows when the second child
    // fails its claimed length. The caller's input remains independently owned.
    let malformed = sequence(&[octet_string(b"private-content"), vec![0x02, 0x02, 0x01]]);
    let (result, observations) =
        inspection::observe_v1(|| build_strict_der_document_trace_v1(&malformed));
    assert!(result.is_err());
    assert_der_cells_cleared(&observations);
    let malformed_signature = sequence(&[integer(17), integer(0)]);
    let (result, observations) =
        inspection::observe_v1(|| parse_signature_v1(&malformed_signature));
    assert!(result.is_err());
    assert_der_cells_cleared(&observations);
}

#[test]
fn der_parsed_precursors_clear_without_a_complete_rfc_owner() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    let (chain, crl, _) = rfc5280_fixture(2, &[10, 11]);
    let cert_trace = build_strict_der_document_trace_v1(&chain[0]).unwrap();
    let crl_trace = build_strict_der_document_trace_v1(&crl).unwrap();
    for unwind in [false, true] {
        let certificate = parse_certificate_document_v1(&cert_trace).unwrap();
        let crl = parse_crl_document_v1(&crl_trace).unwrap();
        // Begin observing only after parsing, so nested parser trace cleanup
        // cannot conceal a missing parsed-certificate or CRL destructor.
        let (result, observations) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let _owners = (certificate, crl);
                assert!(!unwind, "injected pre-RFC-owner unwind");
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_der_cells_cleared(&observations);
        assert!(
            observations
                .iter()
                .map(|item| item.nonzero_before)
                .sum::<usize>()
                > 100
        );
    }
}

#[test]
fn der_byte_reconstruction_clears_partial_output_on_range_error() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    let mut trace = build_strict_der_document_trace_v1(&octet_string(b"secret")).unwrap();
    trace.bytes[3].value.value = F(256);
    let (result, observations) = inspection::observe_v1(|| trace_bytes_v1(&trace));
    assert!(matches!(result, Err(ZkX509DerAirErrorV1::Range)));
    assert_eq!(observations.iter().map(|item| item.cells).sum::<usize>(), 3);
    assert_der_cells_cleared(&observations);
}

#[test]
fn der_private_growth_preserves_values_and_clears_displaced_allocation() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    let mut rows = PrivateTableV1::new(vec![F(11), F(12)], zeroize_fields_v1);
    let previous_capacity = rows.capacity();
    let (result, observations) = inspection::observe_v1(|| {
        reserve_private_rows_v1(&mut rows, previous_capacity, zeroize_fields_v1)
    });
    result.unwrap();
    assert_eq!(rows.as_slice(), &[F(11), F(12)]);
    assert_eq!(observations.iter().map(|item| item.cells).sum::<usize>(), 2);
    assert_eq!(
        observations
            .iter()
            .map(|item| item.nonzero_before)
            .sum::<usize>(),
        2
    );
    assert_der_cells_cleared(&observations);
    assert!(reserve_private_rows_v1(&mut rows, usize::MAX, zeroize_fields_v1).is_err());
}

#[test]
fn der_incomplete_provenance_and_path_rows_clear_on_late_failure() {
    use crate::privacy_engines::zk_x509::private_table::inspection;
    let trace = build_strict_der_document_trace_v1(&sequence(&[integer(3), integer(4)])).unwrap();
    let (result, observations) = inspection::observe_v1(|| {
        let mut builder = SemanticProvenanceBuilderV1::new(&trace, 2).unwrap();
        builder
            .assign(0, None, 0, ZkX509Rfc5280GrammarRoleV1::Certificate, 0)
            .unwrap();
        // Nodes 1 and 2 remain absent: finish must clear the already transferred root.
        builder.finish(ZkX509Rfc5280DocumentKindV1::Certificate, None, None)
    });
    assert!(matches!(result, Err(ZkX509DerAirErrorV1::Topology)));
    assert_der_cells_cleared(&observations);
    let (chain, crl, statement) = rfc5280_fixture(2, &[10]);
    let mut owner = build_zk_x509_rfc5280_trace_v1(&chain, &crl, statement).unwrap();
    owner.certificates[1].not_after = 0;
    let (result, observations) =
        inspection::observe_v1(|| build_path_rows_v1(&owner.statement, &owner.certificates));
    assert!(matches!(result, Err(ZkX509DerAirErrorV1::Input)));
    assert_der_cells_cleared(&observations);
}

#[test]
fn der_borrowed_encoded_and_extension_spans_preserve_exact_bytes() {
    let encoded = extension(OID_AUTHORITY_KEY_IDENTIFIER_V1, false, &aki_inner(&[7; 20]));
    let trace = build_strict_der_document_trace_v1(&encoded).unwrap();
    let bytes = trace_bytes_v1(&trace).unwrap();
    let whole = node_encoded_v1(&trace, &bytes, 0).unwrap();
    assert_eq!(whole, encoded);
    assert_eq!(whole.as_ptr(), bytes.as_ptr());
    let (oid, critical, value) = parse_extension_v1(&trace, &bytes, 0).unwrap();
    assert_eq!(oid, OID_AUTHORITY_KEY_IDENTIFIER_V1);
    assert!(!critical);
    assert_eq!(value, aki_inner(&[7; 20]));
    assert!(
        (bytes.as_ptr() as usize..bytes.as_ptr() as usize + bytes.len())
            .contains(&(value.as_ptr() as usize))
    );
}

#[test]
fn der_header_borrows_identifier_and_long_length_before_validation() {
    let encoded = tlv(&[0x9f, 0x81, 0x01], &[7; 128]);
    let header = parse_header_v1(&encoded, 0, encoded.len()).unwrap();
    assert_eq!(header.identifier, &[0x9f, 0x81, 0x01]);
    assert_eq!(header.identifier.as_ptr(), encoded.as_ptr());
    assert_eq!(header.length, &[0x81, 0x80]);
    assert_eq!(header.length.as_ptr(), encoded[3..].as_ptr());
    assert_eq!(header.content_len, 128);
    assert_eq!(header.end, encoded.len());
    // A claimed length beyond the enclosing value must fail without making an
    // owned copy of the identifier or partially validated length encoding.
    assert!(matches!(
        parse_header_v1(&encoded, 0, encoded.len() - 1),
        Err(ZkX509DerAirErrorV1::Topology)
    ));
}
