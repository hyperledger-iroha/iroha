// Native controls for the strict-DER signature scalar family.
use super::*;
use p256::ecdsa::Signature;

type Row = ZkX509Rfc5280StarkBaseRowV1;

// Positions in `append_residues`.
const ACTIVITY: usize = 0;
const CONTENT_LENGTH: usize = 7;
const HEADER: usize = 16;
const FIRST_TOP_BIT: usize = 18;
const ZERO_FLAG_BYTE: usize = 19;
const NONZERO_INVERSE: usize = 20;
const SECOND_TOP_BIT: usize = 22;
const LENGTH_DECOMPOSITION: usize = 23;
const CONTENT_START: usize = 300;

fn fixed_row(ordinal: usize) -> ZkX509Rfc5280StarkFixedRowV1 {
    let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
    fixed[ZkX509Rfc5280StarkFamilyV1::SignatureScalar as usize] = F::ONE;
    populate_fixed(&mut fixed, ordinal);
    fixed
}

/// Rows a prover commits for the leaf certificate's signature group when every
/// DER byte lookup is answered by `contents`, the BIT STRING contents from the
/// unused-bits octet, for claimed INTEGER lengths and zero flags. `None` when
/// a queried byte lies outside the contents, which the DER lookup rejects.
/// Addresses, windows and helpers are computed here independently.
fn group_rows(contents: &[u8], lengths: [usize; 2], zero: [bool; 2]) -> Option<Vec<Row>> {
    let [r, s] = lengths;
    let [zero_r, zero_s] = zero.map(usize::from);
    let mut rows = Vec::new();
    for offset in 0..ROWS_PER_GROUP {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(contents.len() as u64);
        row[R_LENGTH] = F(r as u64);
        row[S_LENGTH] = F(s as u64);
        row[BASE_NODE] = F(9);
        row[BASE_START] = F((CONTENT_START - 2) as u64);
        row[BASE_CONTENT_START] = F(CONTENT_START as u64);
        row[BASE_CONTENT_END] = F((CONTENT_START + contents.len()) as u64);
        row[BASE_ROLE] = F(ZkX509Rfc5280GrammarRoleV1::CertificateSignatureValue as u64);
        row[BASE_TAG_NUMBER] = F(3);
        row[BASE_IS_WRITE] = F(u64::from(offset == 0));
        // Offset from the content start plus 32, so that the output rows that
        // lie before a short INTEGER stay unsigned.
        let (shifted, length, zero_flag) = match offset {
            0..=3 => (offset + 33, None, None),
            4 | 5 => (offset + 33 + r, None, None),
            6 => (37, Some(r), Some(zero[0])),
            7 => (37 + zero_r, None, Some(zero[0])),
            8 => (39 + r, Some(s), Some(zero[1])),
            9 => (39 + r + zero_s, None, Some(zero[1])),
            _ if offset < OUTPUT_OFFSET + 32 => (5 + r + offset - OUTPUT_OFFSET, None, None),
            _ => (7 + r + s + offset - OUTPUT_OFFSET - 32, None, None),
        };
        row[BASE_ADDRESS] = F((CONTENT_START + shifted) as u64).sub(F(32));
        if let Some(zero_flag) = zero_flag {
            row[ZERO] = F(u64::from(zero_flag));
        }
        let mut queried = offset < OUTPUT_OFFSET;
        if offset >= OUTPUT_OFFSET {
            let selected = if offset < OUTPUT_OFFSET + 32 { r } else { s };
            let threshold = 32 - (offset - OUTPUT_OFFSET) % 32;
            queried = selected >= threshold;
            let window = if queried {
                selected - threshold
            } else {
                threshold - 1 - selected
            };
            row[SELECTED_LENGTH] = F(selected as u64);
            for bit in 0..BIT_COUNT {
                row[BITS + bit] = F(((window >> bit) & 1) as u64);
            }
        }
        if queried {
            let byte = *contents.get(shifted.checked_sub(32)?)?;
            row[QUERY] = F::ONE;
            row[BASE_VALUE] = F(u64::from(byte));
            write_u8_bits_v1(&mut row, BASE_BYTE_BITS, byte);
            if let (Some(length), Some(zero_flag)) = (length, zero_flag) {
                row[BASE_INVERSE] = row[BASE_VALUE].inverse_or_zero_canonical_v1();
                // L - 1 - z modulo 32: a length outside [1 + z, 32 + z] has
                // no admissible decomposition and is written wrapped.
                let unsigned = (length + 63 - usize::from(zero_flag)) % 32;
                for bit in 0..5 {
                    row[BITS + bit] = F(((unsigned >> bit) & 1) as u64);
                }
            }
        }
        rows.push(row);
    }
    Some(rows)
}

fn nonzero_residues(rows: &[Row]) -> Vec<(usize, usize)> {
    let zero = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
    let mut nonzero = Vec::new();
    for (offset, row) in rows.iter().enumerate() {
        let mut residues = Vec::new();
        append_residues(
            row,
            rows.get(offset + 1).unwrap_or(&zero),
            &fixed_row(offset),
            &mut residues,
        );
        assert_eq!(residues.len(), RESIDUES);
        nonzero.extend(
            residues
                .iter()
                .enumerate()
                .filter(|(_, residue)| **residue != F::ZERO)
                .map(|(position, _)| (offset, position)),
        );
    }
    nonzero
}

/// Minimal positive DER INTEGER contents of a 32-byte big-endian scalar.
fn integer(scalar: &[u8; 32]) -> Vec<u8> {
    let first = scalar.iter().position(|byte| *byte != 0).unwrap_or(31);
    let mut out = Vec::new();
    if scalar[first] & 0x80 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&scalar[first..]);
    out
}

fn encoding(r: &[u8], s: &[u8], sequence_adjust: i8, trailing: &[u8]) -> Vec<u8> {
    let sequence = 4 + r.len() + s.len() + trailing.len();
    let mut out = vec![
        0,
        0x30,
        u8::try_from(sequence)
            .unwrap()
            .wrapping_add_signed(sequence_adjust),
        2,
        u8::try_from(r.len()).unwrap(),
    ];
    out.extend_from_slice(r);
    out.extend([2, u8::try_from(s.len()).unwrap()]);
    out.extend_from_slice(s);
    out.extend_from_slice(trailing);
    out
}

fn integer_lengths(contents: &[u8]) -> [usize; 2] {
    let r = usize::from(contents[4]);
    [r, usize::from(contents[6 + r])]
}

const R_SCALAR: [u8; 32] = [
    0x8f, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
];

#[test]
fn fixed_layout_reads_the_encoding_and_the_two_scalar_windows() {
    assert_eq!(ROWS, 296);
    assert_eq!(active_rows(false), 222);
    assert_eq!(active_rows(true), 296);
    assert_eq!(
        (0..GROUPS)
            .map(|sink| output_ordinal(sink).unwrap())
            .collect::<Vec<_>>(),
        [10, 84, 232, 158]
    );
    assert_eq!(output_ordinal(4), None);
    let cell = |fixed: &ZkX509Rfc5280StarkFixedRowV1, column| fixed[column];
    for ordinal in 0..ROWS {
        let fixed = fixed_row(ordinal);
        let offset = ordinal % ROWS_PER_GROUP;
        let spec = GROUP_SPECS[ordinal / ROWS_PER_GROUP];
        assert_eq!(cell(&fixed, OPTIONAL), F(u64::from(spec.optional)));
        assert_eq!(
            cell(&fixed, FIX_REQUIRED_ACTIVE),
            F(u64::from(!spec.optional))
        );
        assert_eq!(cell(&fixed, FIX_LOCAL_FIRST), F(u64::from(offset == 0)));
        assert_eq!(cell(&fixed, FIX_LOCAL_LAST), F(u64::from(offset == 73)));
        // Exactly one row kind: header, INTEGER first byte, INTEGER second
        // byte or scalar output.
        let kinds = [OUTPUT, INTEGER_FIRST, INTEGER_SECOND]
            .map(|column| cell(&fixed, column))
            .into_iter()
            .fold(F::ZERO, F::add);
        assert_eq!(kinds, F(u64::from(offset >= HEADER_ROWS)));
        // Every row's constant offset from the BIT STRING content start.
        let expected = match offset {
            0..=5 => F(offset as u64 + 1),
            6 | 7 => F(5),
            8 | 9 => F(7),
            _ if offset < 42 => F((offset - 10) as u64).sub(F(27)),
            _ => F((offset - 42) as u64).sub(F(25)),
        };
        assert_eq!(cell(&fixed, ADDRESS_OFFSET), expected);
        assert_eq!(
            cell(&fixed, ADDRESS_S_LENGTH),
            F(u64::from(offset >= OUTPUT_OFFSET + SCALAR_BYTES))
        );
    }
    // The four MAIN joins start at the first output row of each sink's group.
    let shape = ZkX509Rfc5280StarkShapeV1::default();
    let (rows, column) = zk_x509_rfc_signature_scalar_join_rows_v1(shape).unwrap();
    let start = ZkX509Rfc5280StarkShapeV1::family_counts()
        [..ZkX509Rfc5280StarkFamilyV1::SignatureScalar as usize]
        .iter()
        .sum::<usize>();
    assert_eq!(column, BASE_VALUE);
    assert_eq!(rows, [10, 84, 232, 158].map(|ordinal| start + ordinal));
}

#[test]
fn canonical_encodings_satisfy_every_residue_and_output_the_padded_scalars() {
    let mut short = [0_u8; 32];
    short[2..].copy_from_slice(&R_SCALAR[2..]);
    short[2] = 0x3a;
    let mut signed = [0_u8; 32];
    signed[1..].copy_from_slice(&R_SCALAR[1..]);
    signed[1] = 0x9a;
    let one = {
        let mut one = [0_u8; 32];
        one[31] = 1;
        one
    };
    for (r, s) in [
        (R_SCALAR, short),
        (short, R_SCALAR),
        (signed, R_SCALAR),
        (one, signed),
        (R_SCALAR, R_SCALAR),
    ] {
        let contents = encoding(&integer(&r), &integer(&s), 0, &[]);
        let lengths = integer_lengths(&contents);
        let honest = [r, s].map(|scalar| integer(&scalar)[0] == 0);
        let rows = group_rows(&contents, lengths, honest).unwrap();
        assert_eq!(nonzero_residues(&rows), Vec::new(), "r={r:02x?} s={s:02x?}");
        let output = rows[OUTPUT_OFFSET..]
            .iter()
            .map(|row| u8::try_from(row[BASE_VALUE].0).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(output, [r, s].concat());
        // The zero flags are determined: every other choice is rejected.
        for zero in [[false, false], [false, true], [true, false], [true, true]] {
            if zero != honest {
                assert!(
                    group_rows(&contents, lengths, zero)
                        .is_none_or(|rows| !nonzero_residues(&rows).is_empty())
                );
            }
        }
    }
}

/// Every zero-flag choice is rejected, by the lookup or by the residues; the
/// returned positions are those of the choices that reach the residues.
fn rejected(contents: &[u8], lengths: [usize; 2]) -> Vec<Vec<(usize, usize)>> {
    let mut failures = Vec::new();
    for zero in [[false, false], [false, true], [true, false], [true, true]] {
        if let Some(rows) = group_rows(contents, lengths, zero) {
            let nonzero = nonzero_residues(&rows);
            assert!(!nonzero.is_empty(), "zero flags {zero:?} accepted");
            failures.push(nonzero);
        }
    }
    failures
}

fn fails(failures: &[Vec<(usize, usize)>], position: (usize, usize)) -> bool {
    failures.iter().all(|nonzero| nonzero.contains(&position))
}

#[test]
fn non_canonical_signature_encodings_fail_the_parsing_residues() {
    let r = integer(&R_SCALAR);
    let s = {
        let mut s = R_SCALAR;
        s[0] = 0x3a;
        integer(&s)
    };
    // An extra leading zero before a byte whose top bit is clear: a 33-byte
    // INTEGER, and the two-byte encoding 00 01 of one.
    let mut padded = vec![0];
    padded.extend_from_slice(&s);
    for (r_bytes, s_bytes, row) in [
        (r.clone(), padded.clone(), 9),
        (r.clone(), vec![0, 1], 9),
        (padded.clone(), s.clone(), 7),
    ] {
        let contents = encoding(&r_bytes, &s_bytes, 0, &[]);
        let failures = rejected(&contents, integer_lengths(&contents));
        assert_eq!(failures.len(), 4);
        // Claiming the sign octet fails the second byte's top bit; claiming no
        // sign octet fails the first byte's inverse.
        assert!(failures.iter().all(|nonzero| {
            nonzero.contains(&(row, SECOND_TOP_BIT))
                || nonzero.contains(&(row - 1, NONZERO_INVERSE))
        }));
    }
    // A missing leading zero: the first byte's top bit is set.
    let contents = encoding(&R_SCALAR, &s, 0, &[]);
    let failures = rejected(&contents, integer_lengths(&contents));
    assert!(fails(&failures, (6, FIRST_TOP_BIT)));
    // A 33-byte INTEGER whose first byte is not zero exceeds 2^256.
    let mut wide = vec![1];
    wide.extend_from_slice(&R_SCALAR);
    let contents = encoding(&wide, &s, 0, &[]);
    let failures = rejected(&contents, integer_lengths(&contents));
    assert!(failures.iter().all(|nonzero| {
        nonzero.contains(&(6, LENGTH_DECOMPOSITION)) || nonzero.contains(&(6, ZERO_FLAG_BYTE))
    }));
    // A wrong SEQUENCE length fails its header byte, and lengths adjusted to
    // match it fail the INTEGER length byte or the content length.
    for adjust in [-1, 1] {
        let contents = encoding(&r, &s, adjust, &[]);
        let lengths = integer_lengths(&contents);
        assert!(fails(&rejected(&contents, lengths), (1, HEADER)));
        let shifted = [
            lengths[0],
            lengths[1].checked_add_signed(isize::from(adjust)).unwrap(),
        ];
        let failures = rejected(&contents, shifted);
        assert!(failures.iter().all(|nonzero| {
            nonzero.contains(&(0, CONTENT_LENGTH)) || nonzero.contains(&(5, HEADER))
        }));
    }
    // A trailing byte inside the BIT STRING, with a SEQUENCE length that
    // covers it or not, fails the content length.
    for adjust in [0, -1] {
        let contents = encoding(&r, &s, adjust, &[0]);
        assert!(fails(
            &rejected(&contents, integer_lengths(&contents)),
            (0, CONTENT_LENGTH)
        ));
    }
    // A zero-length INTEGER has no admissible length decomposition. A
    // zero-length final INTEGER is also rejected by the lookup: its first byte
    // would lie past the contents.
    let contents = encoding(&[], &s, 0, &[]);
    let failures = rejected(&contents, integer_lengths(&contents));
    assert_eq!(failures.len(), 4);
    assert!(fails(&failures, (6, LENGTH_DECOMPOSITION)));
    let contents = encoding(&r, &[], 0, &[]);
    assert!(rejected(&contents, integer_lengths(&contents)).is_empty());
}

#[test]
fn an_absent_third_certificate_group_is_zero_and_a_present_one_is_active() {
    let rows = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
    let mut slot_two = rows;
    slot_two[BASE_CERT2_ACTIVE] = F::ONE;
    let optional = 3 * ROWS_PER_GROUP;
    for offset in 0..ROWS_PER_GROUP {
        for (row, accepted) in [(rows, true), (slot_two, false)] {
            let mut residues = Vec::new();
            append_residues(&row, &rows, &fixed_row(optional + offset), &mut residues);
            assert_eq!(residues[ACTIVITY] == F::ZERO, accepted);
            if accepted {
                assert!(residues.iter().all(|residue| *residue == F::ZERO));
            }
        }
        // A required group cannot be inactive.
        let mut residues = Vec::new();
        append_residues(&rows, &rows, &fixed_row(offset), &mut residues);
        assert_ne!(residues[ACTIVITY], F::ZERO);
    }
}

fn depth_three_trace() -> ZkX509Rfc5280TraceV1 {
    use crate::privacy_engines::zk_x509::{
        der_air::build_zk_x509_rfc5280_trace_v1,
        relation::release_fixture::{
            build_zk_x509_release_fixture_v1, reference_statement_context_v1,
        },
        verifier_profile::rfc_statement_with_crl_number_v1,
    };
    let fixture = build_zk_x509_release_fixture_v1(reference_statement_context_v1(), true).unwrap();
    build_zk_x509_rfc5280_trace_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
        rfc_statement_with_crl_number_v1(
            &fixture.statement,
            fixture.authoritative_state.crl_record().crl_number,
        ),
    )
    .unwrap()
}

/// The 32-byte r and s of each signature in sink order, parsed natively from
/// the document's `signatureValue`; `None` for an absent third certificate.
fn native_scalars(trace: &ZkX509Rfc5280TraceV1) -> [Option<[u8; 64]>; GROUPS] {
    let slot_two = trace.certificates.len() == 3;
    let found = sources(trace).unwrap();
    let mut result = [None; GROUPS];
    for (group, source) in found.iter().enumerate() {
        let Some(source) = source else {
            assert!(!slot_two && group == 3);
            continue;
        };
        let node = source.node;
        let der = (usize::from(node.content_start) + 1..usize::from(node.content_end))
            .map(|address| document_byte(trace, node, address).unwrap())
            .collect::<Vec<_>>();
        let signature = Signature::from_der(&der).unwrap();
        assert_eq!(signature.to_der().as_bytes(), der.as_slice());
        let mut scalars = [0_u8; 64];
        scalars[..32].copy_from_slice(&signature.r().to_bytes());
        scalars[32..].copy_from_slice(&signature.s().to_bytes());
        result[GROUP_SPECS[group].sink] = Some(scalars);
    }
    result
}

#[test]
fn actual_traces_join_each_documents_scalars_and_reject_a_malleated_sink() {
    use crate::privacy_engines::zk_x509::p256_ecdsa_air::P256EcdsaWitnessV1;
    use crate::privacy_engines::zk_x509::p256_external_binding_air::p256_input_selection_byte_v1;
    for trace in [
        super::super::tests::canonical_trace_v1(),
        depth_three_trace(),
    ] {
        let material = build_zk_x509_rfc5280_stark_base_material_v1(&trace).unwrap();
        let (rows, column) =
            zk_x509_rfc_signature_scalar_join_rows_v1(material.schedule.shape).unwrap();
        let scalars = native_scalars(&trace);
        assert_eq!(
            scalars.iter().flatten().count(),
            usize::from(material.private_shape.chain_depth) + 1
        );
        for (sink, expected) in scalars.iter().enumerate() {
            let joined = (0..2 * SCALAR_BYTES)
                .map(|index| material.base_row(rows[sink] + index).unwrap()[column])
                .collect::<Vec<_>>();
            let Some(expected) = expected else {
                // An absent third certificate leaves zero rows; its inactive
                // sink holds zero r and s.
                assert!(joined.iter().all(|value| *value == F::ZERO));
                continue;
            };
            assert_eq!(
                joined,
                expected.map(|byte| F(u64::from(byte))).to_vec(),
                "sink {sink}"
            );
            // The sink rows that the scalar block joins hold r || s; the key
            // and digest blocks join the rows before and after them.
            let witness = |s: [u8; 32]| P256EcdsaWitnessV1 {
                public_key_x_be: [0x11; 32],
                public_key_y_be: [0x22; 32],
                r_be: expected[..32].try_into().unwrap(),
                s_be: s,
                digest_be: [0x33; 32],
            };
            let sink_bytes = |witness: &P256EcdsaWitnessV1| {
                (0..5 * 32)
                    .map(|byte| p256_input_selection_byte_v1(witness, byte).unwrap())
                    .collect::<Vec<_>>()
            };
            let honest = sink_bytes(&witness(expected[32..].try_into().unwrap()));
            assert_eq!(&honest[64..128], expected.as_slice());
            // (r, n - s) is another valid signature on the same digest under
            // the same key, and certificate sinks do not require low s.
            let signature = Signature::from_scalars(
                <[u8; 32]>::try_from(&expected[..32]).unwrap(),
                <[u8; 32]>::try_from(&expected[32..]).unwrap(),
            )
            .unwrap();
            let malleated: [u8; 32] = (-*signature.s()).to_bytes().into();
            assert!(Signature::from_scalars(signature.r().to_bytes(), malleated).is_ok());
            let forged = sink_bytes(&witness(malleated));
            assert_eq!(forged[..64], honest[..64], "key block rows unchanged");
            assert_eq!(forged[128..], honest[128..], "digest block rows unchanged");
            assert_ne!(
                forged[64..128]
                    .iter()
                    .map(|byte| F(u64::from(*byte)))
                    .collect::<Vec<_>>(),
                joined,
                "only the scalar join of sink {sink} differs"
            );
        }
    }
}
