//! Original leaf INTEGER provenance for Projection's canonical serial magnitude.
//!
//! TODO: disclosed subject attributes still need exact source and parser-equivalence
//! constraints; this serial repair does not activate the credential profile.
use super::*;

pub(super) const FIX_SERIAL: usize = variable_output::FIX_END;
const FIX_BYTES: usize = FIX_SERIAL + 1;
const FIX_BYTE_FIRST: usize = FIX_BYTES + 1;
const FIX_BYTE_CONTINUE: usize = FIX_BYTE_FIRST + 1;
const FIX_BYTE_LAST: usize = FIX_BYTE_CONTINUE + 1;
const FIX_LENGTH: usize = FIX_BYTE_LAST + 1;
const FIX_LENGTH_FIRST: usize = FIX_LENGTH + 1;
const FIX_LENGTH_CONTINUE: usize = FIX_LENGTH_FIRST + 1;
pub(super) const FIX_LENGTH_LAST: usize = FIX_LENGTH_CONTINUE + 1;
const FIX_LENGTH_ZERO: usize = FIX_LENGTH_LAST + 1;
const FIX_PAIR_CONTINUE: usize = FIX_LENGTH_ZERO + 1;
pub(super) const FIX_END: usize = FIX_PAIR_CONTINUE + 1;
pub(super) const RESIDUES: usize = 47;
const METADATA: [usize; 13] = [
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
    BASE_STRICT,
];

pub(super) fn slot(channel: u32) -> Option<bool> {
    match channel {
        3 => Some(true),
        4 => Some(false),
        _ => None,
    }
}

pub(super) fn populate_fixed(
    fixed: &mut ZkX509Rfc5280StarkFixedRowV1,
    channel: u32,
    offset: usize,
    consumer: bool,
) {
    let Some(length) = slot(channel).filter(|_| !consumer) else {
        return;
    };
    fixed[FIX_SERIAL] = F::ONE;
    fixed[FIX_BYTES] = F(u64::from(!length));
    fixed[FIX_BYTE_FIRST] = F(u64::from(!length && offset == 0));
    fixed[FIX_BYTE_CONTINUE] = F(u64::from(
        !length && offset + 1 < ZK_X509_MAX_SERIAL_BYTES_V1,
    ));
    fixed[FIX_BYTE_LAST] = F(u64::from(
        !length && offset + 1 == ZK_X509_MAX_SERIAL_BYTES_V1,
    ));
    fixed[FIX_LENGTH] = F(u64::from(length));
    fixed[FIX_LENGTH_FIRST] = F(u64::from(length && offset == 0));
    fixed[FIX_LENGTH_CONTINUE] = F(u64::from(length && offset < 7));
    fixed[FIX_LENGTH_LAST] = F(u64::from(length && offset == 7));
    fixed[FIX_LENGTH_ZERO] = F(u64::from(length && offset < 7));
    fixed[FIX_PAIR_CONTINUE] = F(u64::from(
        length || offset + 1 < ZK_X509_MAX_SERIAL_BYTES_V1,
    ));
}

pub(super) fn residues<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> [A; RESIDUES] {
    let serial = fixed[FIX_SERIAL];
    let bytes = fixed[FIX_BYTES];
    let length = fixed[FIX_LENGTH];
    let first = fixed[FIX_BYTE_FIRST];
    let live = row[BASE_D];
    let sign = row[BASE_STRICT];
    let mut output = [A::ZERO; RESIDUES];
    let mut index = 0;
    let mut push = |r| {
        output[index] = r;
        index += 1;
    };
    push(serial.mul(sign).mul(sign.sub(A::ONE)));
    push(serial.mul(live).mul(live.sub(A::ONE)));
    push(fixed[FIX_LENGTH_FIRST].mul(live.sub(sign)));
    push(length.sub(fixed[FIX_LENGTH_FIRST]).mul(live));
    push(first.mul(live.sub(A::ONE)));
    push(
        fixed[FIX_BYTE_CONTINUE]
            .mul(next[BASE_D])
            .mul(A::ONE.sub(live)),
    );
    push(bytes.mul(row[BASE_C].sub(row[BASE_B]).sub(live)));
    push(first.mul(row[BASE_B]));
    push(fixed[FIX_BYTE_CONTINUE].mul(next[BASE_B].sub(row[BASE_C])));
    push(fixed[FIX_BYTE_LAST].mul(row[BASE_C].sub(row[BASE_PARENT])));
    push(
        length.mul(
            row[BASE_F]
                .sub(row[BASE_E])
                .sub(row[BASE_VALUE].mul(fixed[FIX_LENGTH_LAST])),
        ),
    );
    push(fixed[FIX_LENGTH_FIRST].mul(row[BASE_E]));
    push(fixed[FIX_LENGTH_CONTINUE].mul(next[BASE_E].sub(row[BASE_F])));
    push(fixed[FIX_LENGTH_LAST].mul(row[BASE_F].sub(row[BASE_PARENT])));
    push(fixed[FIX_LENGTH_ZERO].mul(row[BASE_VALUE]));
    push(bytes.mul(A::ONE.sub(live)).mul(row[BASE_VALUE]));
    push(first.mul(row[BASE_VALUE].mul(row[BASE_INVERSE]).sub(A::ONE)));
    push(first.mul(row[BASE_BYTE_BITS + 7].sub(sign)));
    push(serial.sub(first).mul(row[BASE_INVERSE]));
    push(serial.mul(row[BASE_DOCUMENT]));
    push(serial.mul(row[BASE_G].sub(A::from_base(F(
        ZkX509Rfc5280GrammarRoleV1::CertificateSerial as u64,
    )))));
    push(serial.mul(row[BASE_H]));
    push(serial.mul(row[BASE_TAG_CLASS]));
    push(serial.mul(row[BASE_CONSTRUCTED]));
    push(serial.mul(row[BASE_TAG_NUMBER].sub(A::from_base(F(2)))));
    push(
        serial.mul(
            row[BASE_A]
                .sub(row[BASE_CONTENT_END])
                .add(row[BASE_CONTENT_START]),
        ),
    );
    push(serial.mul(row[BASE_A].sub(row[BASE_PARENT]).sub(sign)));
    push(
        bytes.mul(live).mul(
            row[BASE_ADDRESS]
                .sub(row[BASE_CONTENT_START])
                .sub(sign)
                .sub(fixed[FIX_EXPECTED + 4]),
        ),
    );
    push(
        fixed[FIX_LENGTH_FIRST]
            .mul(live)
            .mul(row[BASE_ADDRESS].sub(row[BASE_CONTENT_START])),
    );
    push(serial.mul(A::ONE.sub(live)).mul(row[BASE_ADDRESS]));
    push(bytes.mul(row[BASE_E]));
    push(bytes.mul(row[BASE_F]));
    push(length.mul(row[BASE_B]));
    push(length.mul(row[BASE_C]));
    for column in METADATA {
        push(fixed[FIX_PAIR_CONTINUE].mul(next[column].sub(row[column])));
    }
    assert_eq!(index, RESIDUES);
    output
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Copy)]
pub(super) struct Source<'a> {
    pub(super) node: &'a ZkX509Rfc5280NodeProvenanceV1,
    pub(super) magnitude: &'a [u8],
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn source(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<Source<'_>, ZkX509Rfc5280StarkErrorV1> {
    let magnitude = trace
        .certificates
        .first()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?
        .serial
        .as_slice();
    if magnitude.is_empty() || magnitude.len() > ZK_X509_MAX_SERIAL_BYTES_V1 || magnitude[0] == 0 {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    let mut nodes = trace
        .semantic_provenance
        .first()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
        .nodes
        .iter()
        .filter(|node| {
            node.role == ZkX509Rfc5280GrammarRoleV1::CertificateSerial && node.role_instance == 0
        });
    let node = nodes.next().ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    let sign = usize::from(magnitude[0] & 0x80 != 0);
    if nodes.next().is_some()
        || node.document != 0
        || node.tag_class != 0
        || node.constructed
        || node.tag_number != 2
        || node.content_start < node.start
        || usize::from(node.content_start) + magnitude.len() + sign != usize::from(node.content_end)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    let contents = trace
        .documents
        .first()
        .and_then(|document| {
            document
                .bytes
                .get(usize::from(node.content_start)..usize::from(node.content_end))
        })
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    if contents.iter().enumerate().any(|(index, byte)| {
        byte.value.value
            != F(u64::from(if index < sign {
                0
            } else {
                magnitude[index - sign]
            }))
    }) {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    // All private bytes remain borrowed from the existing clearing trace owner.
    Ok(Source { node, magnitude })
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_row(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    source: &Source<'_>,
    length_row: bool,
    offset: usize,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if offset
        >= if length_row {
            8
        } else {
            ZK_X509_MAX_SERIAL_BYTES_V1
        }
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    let length = source.magnitude.len();
    let node = source.node;
    let sign = usize::from(source.magnitude[0] & 0x80 != 0);
    let live = !length_row && offset < length;
    let query = live || (length_row && offset == 0 && sign == 1);
    let expected = if length_row {
        (length as u64).to_be_bytes()[offset]
    } else {
        source.magnitude.get(offset).copied().unwrap_or(0)
    };
    if row[BASE_VALUE] != F(u64::from(expected)) {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
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
    row[BASE_PARENT] = F(length as u64);
    row[BASE_STRICT] = F(sign as u64);
    row[BASE_D] = F(u64::from(query));
    row[BASE_ADDRESS] = F(if live {
        usize::from(node.content_start) + sign + offset
    } else if query {
        usize::from(node.content_start)
    } else {
        0
    } as u64);
    if length_row {
        row[BASE_F] = F(if offset == 7 { length as u64 } else { 0 });
    } else {
        row[BASE_B] = F(offset.min(length) as u64);
        row[BASE_C] = row[BASE_B].add(row[BASE_D]);
    }
    if !length_row && offset == 0 {
        row[BASE_INVERSE] = row[BASE_VALUE]
            .inv()
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
    }
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn byte_multiplicity(source: &Source<'_>, document: usize, address: usize) -> usize {
    usize::from(
        document == 0
            && usize::from(source.node.content_start) <= address
            && address < usize::from(source.node.content_end),
    )
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn node_multiplicity(source: &Source<'_>, document: usize, node: usize) -> u16 {
    if document == 0 && node == usize::from(source.node.node) {
        source.node.content_end - source.node.content_start + 1
    } else {
        0
    }
}
