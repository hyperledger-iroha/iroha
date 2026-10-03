//! Original DER provenance for padded TBS, CRL and signature output pairs.
//!
//! TODO: bind the remaining disclosed-attribute projection source bytes;
//! this incremental source repair does not activate the credential profile.

use super::*;

pub(super) const FIX_VARIABLE: usize = FIX_OUTPUT_SOURCE_SPKI + 1;
const FIX_BYTES: usize = FIX_VARIABLE + 1;
const FIX_FIRST: usize = FIX_BYTES + 1;
const FIX_BYTE_CONTINUE: usize = FIX_FIRST + 1;
const FIX_BYTE_LAST: usize = FIX_BYTE_CONTINUE + 1;
const FIX_PAIR_CONTINUE: usize = FIX_BYTE_LAST + 1;
const FIX_LENGTH: usize = FIX_PAIR_CONTINUE + 1;
const FIX_LENGTH_FIRST: usize = FIX_LENGTH + 1;
const FIX_LENGTH_CONTINUE: usize = FIX_LENGTH_FIRST + 1;
const FIX_LENGTH_LAST: usize = FIX_LENGTH_CONTINUE + 1;
const FIX_LENGTH_WEIGHT: usize = FIX_LENGTH_LAST + 1;
const FIX_LENGTH_ZERO: usize = FIX_LENGTH_WEIGHT + 1;
const FIX_SIGNATURE: usize = FIX_LENGTH_ZERO + 1;
const FIX_ROLE: usize = FIX_SIGNATURE + 1;
const FIX_DOCUMENT_INSTANCE: usize = FIX_ROLE + 1;
pub(super) const FIX_END: usize = FIX_DOCUMENT_INSTANCE + 1;
pub(super) const RESIDUES: usize = 50;
const SIGNATURE_BYTES: usize = 72;
const METADATA: [usize; 12] = [
    BASE_A,
    BASE_G,
    BASE_H,
    BASE_DOCUMENT,
    BASE_NODE,
    BASE_START,
    BASE_CONTENT_START,
    BASE_CONTENT_END,
    BASE_TAG_CLASS,
    BASE_CONSTRUCTED,
    BASE_TAG_NUMBER,
    BASE_PARENT,
];

#[derive(Clone, Copy)]
struct SourceSpec {
    channel: u32,
    document: u64,
    cert2_coefficient: u64,
    optional: bool,
    role: ZkX509Rfc5280GrammarRoleV1,
    signature: bool,
    document_instance: bool,
}

const SOURCES: [SourceSpec; 9] = {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    [
        SourceSpec {
            channel: 5,
            document: 0,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateTbs,
            signature: false,
            document_instance: true,
        },
        SourceSpec {
            channel: 7,
            document: 1,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateTbs,
            signature: false,
            document_instance: true,
        },
        SourceSpec {
            channel: 9,
            document: 2,
            cert2_coefficient: 0,
            optional: true,
            role: R::CertificateTbs,
            signature: false,
            document_instance: true,
        },
        SourceSpec {
            channel: 21,
            document: 2,
            cert2_coefficient: 1,
            optional: false,
            role: R::CrlTbs,
            signature: false,
            document_instance: false,
        },
        SourceSpec {
            channel: 23,
            document: 2,
            cert2_coefficient: 1,
            optional: false,
            role: R::Crl,
            signature: false,
            document_instance: false,
        },
        SourceSpec {
            channel: 12,
            document: 0,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateSignatureValue,
            signature: true,
            document_instance: true,
        },
        SourceSpec {
            channel: 15,
            document: 1,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateSignatureValue,
            signature: true,
            document_instance: true,
        },
        SourceSpec {
            channel: 18,
            document: 2,
            cert2_coefficient: 0,
            optional: true,
            role: R::CertificateSignatureValue,
            signature: true,
            document_instance: true,
        },
        SourceSpec {
            channel: 25,
            document: 2,
            cert2_coefficient: 1,
            optional: false,
            role: R::CrlSignatureValue,
            signature: true,
            document_instance: false,
        },
    ]
};

const _: () = assert!(ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1 < 1 << 16);
const _: () = assert!(
    MAX_SERIAL_SOURCE_ROWS_V1
        + 5 * ZK_X509_UNCOMPRESSED_P256_BYTES_V1
        + 5 * SPKI_OUTPUT_BYTES_V1
        + 5 * ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
        + 4 * SIGNATURE_BYTES
        < u16::MAX as usize
);

fn capacity(spec: SourceSpec) -> usize {
    if spec.signature {
        SIGNATURE_BYTES
    } else {
        ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
    }
}

pub(super) fn slot(shape: ZkX509Rfc5280StarkShapeV1, channel: u32) -> Option<(usize, bool)> {
    let shift = u32::from(shape.disclosed_attribute_count) * 2;
    SOURCES.iter().enumerate().find_map(|(index, spec)| {
        if channel == spec.channel + shift {
            Some((index, false))
        } else if channel == spec.channel + shift + 1 {
            Some((index, true))
        } else {
            None
        }
    })
}

pub(super) fn populate_fixed(
    fixed: &mut ZkX509Rfc5280StarkFixedRowV1,
    shape: ZkX509Rfc5280StarkShapeV1,
    channel: u32,
    offset: usize,
    consumer: bool,
) {
    let Some((index, length)) = slot(shape, channel).filter(|_| !consumer) else {
        return;
    };
    let spec = SOURCES[index];
    let width = capacity(spec);
    fixed[FIX_VARIABLE] = F::ONE;
    fixed[FIX_BYTES] = F(u64::from(!length));
    fixed[FIX_FIRST] = F(u64::from(!length && offset == 0));
    fixed[FIX_BYTE_CONTINUE] = F(u64::from(!length && offset + 1 < width));
    fixed[FIX_BYTE_LAST] = F(u64::from(!length && offset + 1 == width));
    fixed[FIX_PAIR_CONTINUE] = F(u64::from(!length || offset + 1 < 8));
    fixed[FIX_LENGTH] = F(u64::from(length));
    fixed[FIX_LENGTH_FIRST] = F(u64::from(length && offset == 0));
    fixed[FIX_LENGTH_CONTINUE] = F(u64::from(length && offset + 1 < 8));
    fixed[FIX_LENGTH_LAST] = F(u64::from(length && offset + 1 == 8));
    fixed[FIX_LENGTH_WEIGHT] = F(if length && offset >= 6 {
        1 << (8 * (7 - offset))
    } else {
        0
    });
    fixed[FIX_LENGTH_ZERO] = F(u64::from(length && offset < 6));
    fixed[FIX_SIGNATURE] = F(u64::from(spec.signature));
    fixed[FIX_ROLE] = F(spec.role as u64);
    fixed[FIX_DOCUMENT_INSTANCE] = F(u64::from(spec.document_instance));
    fixed[FIX_EXPECTED + 7] = F(u64::from(spec.optional));
    // Optional document2 is zero when absent; live row zero is the anchor.
    fixed[FIX_EXPECTED + 8] = F(if spec.optional { 0 } else { spec.document });
    fixed[FIX_EXPECTED + 9] = F(if spec.optional {
        spec.document
    } else {
        spec.cert2_coefficient
    });
}

pub(super) fn residues<A: PolynomialAirFieldV1>(
    current: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> [A; RESIDUES] {
    let variable = fixed[FIX_VARIABLE];
    let bytes = fixed[FIX_BYTES];
    let length = fixed[FIX_LENGTH];
    let optional = fixed[FIX_EXPECTED + 7];
    let present = A::ONE
        .sub(optional)
        .add(optional.mul(current[BASE_CERT2_ACTIVE]));
    let live = current[BASE_D];
    let signature = fixed[FIX_SIGNATURE];
    let mut output = [A::ZERO; RESIDUES];
    let mut index = 0;
    let mut push = |value| {
        output[index] = value;
        index += 1;
    };
    push(variable.mul(live).mul(live.sub(A::ONE)));
    push(fixed[FIX_FIRST].mul(live.sub(present)));
    push(length.mul(live));
    push(
        fixed[FIX_BYTE_CONTINUE]
            .mul(next[BASE_D])
            .mul(A::ONE.sub(live)),
    );
    push(variable.mul(current[BASE_C].sub(current[BASE_B]).sub(live)));
    push(fixed[FIX_FIRST].mul(current[BASE_B]));
    push(fixed[FIX_PAIR_CONTINUE].mul(next[BASE_B].sub(current[BASE_C])));
    push(
        fixed[FIX_BYTE_LAST].mul(
            current[BASE_C]
                .sub(current[BASE_CONTENT_END])
                .add(current[BASE_PARENT]),
        ),
    );
    push(
        length.mul(
            current[BASE_F]
                .sub(current[BASE_E])
                .sub(current[BASE_VALUE].mul(fixed[FIX_LENGTH_WEIGHT])),
        ),
    );
    push(fixed[FIX_LENGTH_FIRST].mul(current[BASE_E]));
    push(fixed[FIX_LENGTH_CONTINUE].mul(next[BASE_E].sub(current[BASE_F])));
    push(fixed[FIX_LENGTH_LAST].mul(current[BASE_F].sub(current[BASE_C])));
    push(fixed[FIX_LENGTH_ZERO].mul(current[BASE_VALUE]));
    push(bytes.mul(A::ONE.sub(live)).mul(current[BASE_VALUE]));
    push(variable.mul(A::ONE.sub(live)).mul(current[BASE_ADDRESS]));
    // A committed start helper avoids degree5 at a live optional-signature query.
    push(
        variable.mul(
            current[BASE_PARENT]
                .sub(A::ONE.sub(signature).mul(current[BASE_START]))
                .sub(signature.mul(current[BASE_CONTENT_START].add(present))),
        ),
    );
    push(
        variable.mul(
            current[BASE_DOCUMENT]
                .sub(fixed[FIX_EXPECTED + 8])
                .sub(fixed[FIX_EXPECTED + 9].mul(current[BASE_CERT2_ACTIVE])),
        ),
    );
    push(variable.mul(current[BASE_G].sub(fixed[FIX_ROLE].mul(present))));
    push(
        variable.mul(current[BASE_H].sub(fixed[FIX_DOCUMENT_INSTANCE].mul(current[BASE_DOCUMENT]))),
    );
    push(variable.mul(current[BASE_TAG_CLASS]));
    push(variable.mul(current[BASE_CONSTRUCTED].sub(A::ONE.sub(signature).mul(present))));
    push(
        variable.mul(
            current[BASE_TAG_NUMBER].sub(
                A::from_base(F(16))
                    .sub(signature.mul(A::from_base(F(13))))
                    .mul(present),
            ),
        ),
    );
    push(
        variable.mul(
            current[BASE_CONTENT_END]
                .sub(current[BASE_CONTENT_START])
                .sub(current[BASE_A]),
        ),
    );
    push(bytes.mul(current[BASE_E]));
    push(bytes.mul(current[BASE_F]));
    for column in METADATA {
        push(fixed[FIX_PAIR_CONTINUE].mul(next[column].sub(current[column])));
        push(variable.mul(A::ONE.sub(present)).mul(current[column]));
    }
    push(
        bytes.mul(live).mul(
            current[BASE_ADDRESS]
                .sub(current[BASE_PARENT])
                .sub(fixed[FIX_EXPECTED + 4]),
        ),
    );
    assert_eq!(index, RESIDUES);
    output
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
fn span(
    node: &ZkX509Rfc5280NodeProvenanceV1,
    spec: SourceSpec,
) -> Result<(usize, usize), ZkX509Rfc5280StarkErrorV1> {
    let start = if spec.signature {
        usize::from(node.content_start) + 1
    } else {
        usize::from(node.start)
    };
    let length = usize::from(node.content_end)
        .checked_sub(start)
        .filter(|&n| n > 0 && n <= capacity(spec))
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    Ok((start, length))
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn nodes(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<[Option<&ZkX509Rfc5280NodeProvenanceV1>; 9], ZkX509Rfc5280StarkErrorV1> {
    if !(2..=3).contains(&trace.certificates.len()) {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let cert2 = u64::from(trace.certificates.len() == 3);
    let mut output = [None; 9];
    for (index, spec) in SOURCES.into_iter().enumerate() {
        if spec.optional && cert2 == 0 {
            continue;
        }
        let document = (spec.document + spec.cert2_coefficient * cert2) as usize;
        let role_instance = if spec.document_instance { document } else { 0 };
        let provenance = trace
            .semantic_provenance
            .get(document)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
        let mut matches = provenance.nodes.iter().filter(|node| {
            node.role == spec.role && usize::from(node.role_instance) == role_instance
        });
        let node = matches.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
        if matches.next().is_some()
            || usize::from(node.document) != document
            || node.tag_class != 0
            || node.constructed == spec.signature
            || node.tag_number != if spec.signature { 3 } else { 16 }
            || node.content_start < node.start
            || node.content_end < node.content_start
            || usize::from(node.content_end)
                > trace
                    .documents
                    .get(document)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
                    .bytes
                    .len()
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        span(node, spec)?;
        output[index] = Some(node);
    }
    Ok(output)
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_row(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    trace: &ZkX509Rfc5280TraceV1,
    node: Option<&ZkX509Rfc5280NodeProvenanceV1>,
    index: usize,
    length_row: bool,
    offset: usize,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let spec = *SOURCES
        .get(index)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Output)?;
    if offset >= if length_row { 8 } else { capacity(spec) } {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    let absent = spec.optional && trace.certificates.len() == 2;
    if !(2..=3).contains(&trace.certificates.len()) || node.is_none() != absent {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    let (start, length) = node
        .map(|node| span(node, spec))
        .transpose()?
        .unwrap_or((0, 0));
    let live = !length_row && offset < length;
    let expected = if length_row {
        (length as u64).to_be_bytes()[offset]
    } else if live {
        let byte = trace
            .documents
            .get(usize::from(
                node.ok_or(ZkX509Rfc5280StarkErrorV1::Source)?.document,
            ))
            .and_then(|document| document.bytes.get(start + offset))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        u8::try_from(byte.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?
    } else {
        0
    };
    if row[BASE_VALUE] != F(u64::from(expected)) {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    row[BASE_D] = F(u64::from(live));
    row[BASE_B] = F(if length_row {
        length
    } else {
        offset.min(length)
    } as u64);
    row[BASE_C] = row[BASE_B].add(row[BASE_D]);
    if length_row {
        row[BASE_E] = F(match offset {
            0..=6 => 0,
            _ => length as u64 & !255,
        });
        row[BASE_F] = F(match offset {
            0..=5 => 0,
            6 => length as u64 & !255,
            _ => length as u64,
        });
    }
    if let Some(node) = node {
        row[BASE_A] = F(u64::from(node.content_end - node.content_start));
        row[BASE_G] = F(node.role as u64);
        row[BASE_H] = F(u64::from(node.role_instance));
        row[BASE_DOCUMENT] = F(u64::from(node.document));
        row[BASE_NODE] = F(u64::from(node.node));
        row[BASE_START] = F(u64::from(node.start));
        row[BASE_CONTENT_START] = F(u64::from(node.content_start));
        row[BASE_CONTENT_END] = F(u64::from(node.content_end));
        row[BASE_TAG_CLASS] = F(u64::from(node.tag_class));
        row[BASE_CONSTRUCTED] = F(u64::from(node.constructed));
        row[BASE_TAG_NUMBER] = F(u64::from(node.tag_number));
        row[BASE_PARENT] = F(start as u64);
        row[BASE_ADDRESS] = F(if live { start + offset } else { 0 } as u64);
    }
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn byte_multiplicity(
    nodes: &[Option<&ZkX509Rfc5280NodeProvenanceV1>; 9],
    document: usize,
    address: usize,
) -> usize {
    nodes
        .iter()
        .zip(SOURCES)
        .filter(|(node, spec)| {
            node.is_some_and(|node| {
                let start = if spec.signature {
                    usize::from(node.content_start) + 1
                } else {
                    usize::from(node.start)
                };
                usize::from(node.document) == document
                    && start <= address
                    && address < usize::from(node.content_end)
            })
        })
        .count()
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn node_multiplicity(
    nodes: &[Option<&ZkX509Rfc5280NodeProvenanceV1>; 9],
    document: usize,
    ordinal: usize,
) -> u16 {
    let count: usize = nodes
        .iter()
        .zip(SOURCES)
        .filter_map(|(node, spec)| {
            node.filter(|node| {
                usize::from(node.document) == document && usize::from(node.node) == ordinal
            })
            .map(|node| span(node, spec).expect("validated borrowed output node").1)
        })
        .sum();
    u16::try_from(count).expect("public source caps fit the existing multiplicity owner")
}
