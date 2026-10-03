//! Exact original leaf Subject/OID/value provenance for disclosed attributes.
//!
//! The independent complete Name OID/value census constrains every original
//! attribute, including undisclosed values; release qualification remains separate.
use super::*;

pub(super) const FIX_DISCLOSURE: usize = projection_serial::FIX_END;
const FIX_BYTES: usize = FIX_DISCLOSURE + 1;
const FIX_BYTE_FIRST: usize = FIX_BYTES + 1;
const FIX_BYTE_CONTINUE: usize = FIX_BYTE_FIRST + 1;
const FIX_BYTE_LAST: usize = FIX_BYTE_CONTINUE + 1;
const FIX_LENGTH: usize = FIX_BYTE_LAST + 1;
pub(super) const FIX_LENGTH_FIRST: usize = FIX_LENGTH + 1;
const FIX_LENGTH_CONTINUE: usize = FIX_LENGTH_FIRST + 1;
const FIX_LENGTH_LAST: usize = FIX_LENGTH_CONTINUE + 1;
const FIX_LENGTH_ZERO: usize = FIX_LENGTH_LAST + 1;
const FIX_LENGTH_WEIGHT: usize = FIX_LENGTH_ZERO + 1;
const FIX_PAIR_CONTINUE: usize = FIX_LENGTH_WEIGHT + 1;
pub(super) const FIX_OID_QUERY: usize = FIX_PAIR_CONTINUE + 1;
const FIX_OID_OFFSET: usize = FIX_OID_QUERY + 1;
pub(super) const FIX_OID_VALUE: usize = FIX_OID_OFFSET + 1;
pub(super) const FIX_VALUE_FIRST: usize = FIX_OID_VALUE + 1;
const FIX_METADATA_CONTINUE: usize = FIX_VALUE_FIRST + 1;
pub(super) const FIX_END: usize = FIX_METADATA_CONTINUE + 1;
pub(super) const RESIDUES: usize = 88;
const METADATA: [usize; 11] = [
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
];
const OIDS: [[u8; 3]; 4] = [[0x55, 4, 6], [0x55, 4, 10], [0x55, 4, 11], [0x55, 4, 3]];

pub(super) fn slot(shape: ZkX509Rfc5280StarkShapeV1, channel: u32) -> Option<(usize, bool)> {
    let relative = channel.checked_sub(5)?;
    let slot = usize::try_from(relative / 2).ok()?;
    (slot < usize::from(shape.disclosed_attribute_count)).then_some((slot, relative % 2 == 0))
}

pub(super) fn populate_fixed(
    fixed: &mut ZkX509Rfc5280StarkFixedRowV1,
    shape: ZkX509Rfc5280StarkShapeV1,
    channel: u32,
    offset: usize,
    consumer: bool,
) {
    let Some((slot, length)) = slot(shape, channel).filter(|_| !consumer) else {
        return;
    };
    let oid = length && (1..=3).contains(&offset);
    fixed[FIX_DISCLOSURE] = F::ONE;
    fixed[FIX_BYTES] = F(u64::from(!length));
    fixed[FIX_BYTE_FIRST] = F(u64::from(!length && offset == 0));
    fixed[FIX_BYTE_CONTINUE] = F(u64::from(
        !length && offset + 1 < ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1,
    ));
    fixed[FIX_BYTE_LAST] = F(u64::from(
        !length && offset + 1 == ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1,
    ));
    fixed[FIX_LENGTH] = F(u64::from(length));
    fixed[FIX_LENGTH_FIRST] = F(u64::from(length && offset == 0));
    fixed[FIX_LENGTH_CONTINUE] = F(u64::from(length && offset < 7));
    fixed[FIX_LENGTH_LAST] = F(u64::from(length && offset == 7));
    fixed[FIX_LENGTH_ZERO] = F(u64::from(length && offset < 6));
    fixed[FIX_LENGTH_WEIGHT] = F(if length && offset == 6 {
        256
    } else {
        u64::from(length && offset == 7)
    });
    fixed[FIX_PAIR_CONTINUE] = F(u64::from(
        length || offset + 1 < ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1,
    ));
    fixed[FIX_OID_QUERY] = F(u64::from(oid));
    if oid {
        fixed[FIX_OID_OFFSET] = F((offset - 1) as u64);
        fixed[FIX_OID_VALUE] = F(u64::from(
            OIDS[usize::from(shape.disclosed_attribute_indices[slot])][offset - 1],
        ));
    }
    fixed[FIX_VALUE_FIRST] = F(u64::from(length && offset == 4));
    fixed[FIX_METADATA_CONTINUE] = F(u64::from(if length {
        matches!(offset, 1 | 2 | 4..=7)
    } else {
        offset + 1 < ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
    }));
}

pub(super) fn residues<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> [A; RESIDUES] {
    let disclosure = fixed[FIX_DISCLOSURE];
    let bytes = fixed[FIX_BYTES];
    let length = fixed[FIX_LENGTH];
    let subject = fixed[FIX_LENGTH_FIRST];
    let oid = fixed[FIX_OID_QUERY];
    let value = disclosure.sub(subject).sub(oid);
    let live = row[BASE_D];
    let half = A::from_base(F((GOLDILOCKS_MODULUS_V1 + 1) / 2));
    let oid_last = fixed[FIX_OID_OFFSET]
        .mul(fixed[FIX_OID_OFFSET].sub(A::ONE))
        .mul(half);
    let oid_first = oid.sub(fixed[FIX_OID_OFFSET]).add(oid_last);
    let boundary = oid_first.add(fixed[FIX_VALUE_FIRST]);
    let mut output = [A::ZERO; RESIDUES];
    let mut index = 0;
    let mut push = |residue| {
        output[index] = residue;
        index += 1;
    };
    push(disclosure.mul(row[BASE_DOCUMENT]));
    push(disclosure.mul(row[BASE_TAG_CLASS]));
    push(
        disclosure.mul(
            row[BASE_A]
                .sub(row[BASE_CONTENT_END])
                .add(row[BASE_CONTENT_START]),
        ),
    );
    push(length.mul(live).sub(oid));
    push(bytes.mul(live).mul(live.sub(A::ONE)));
    push(fixed[FIX_BYTE_FIRST].mul(live.sub(A::ONE)));
    push(
        fixed[FIX_BYTE_CONTINUE]
            .mul(next[BASE_D])
            .mul(A::ONE.sub(live)),
    );
    push(bytes.mul(row[BASE_C].sub(row[BASE_B]).sub(live)));
    push(fixed[FIX_BYTE_FIRST].mul(row[BASE_B]));
    push(fixed[FIX_BYTE_CONTINUE].mul(next[BASE_B].sub(row[BASE_C])));
    push(fixed[FIX_BYTE_LAST].mul(row[BASE_C].sub(row[BASE_PARENT])));
    push(
        length.mul(
            row[BASE_F]
                .sub(row[BASE_E])
                .sub(row[BASE_VALUE].mul(fixed[FIX_LENGTH_WEIGHT])),
        ),
    );
    push(subject.mul(row[BASE_E]));
    push(fixed[FIX_LENGTH_CONTINUE].mul(next[BASE_E].sub(row[BASE_F])));
    push(fixed[FIX_LENGTH_LAST].mul(row[BASE_F].sub(row[BASE_PARENT])));
    push(fixed[FIX_LENGTH_ZERO].mul(row[BASE_VALUE]));
    push(bytes.mul(A::ONE.sub(live)).mul(row[BASE_VALUE]));
    push(disclosure.mul(A::ONE.sub(live)).mul(row[BASE_ADDRESS]));
    push(
        bytes.mul(live).mul(
            row[BASE_ADDRESS]
                .sub(row[BASE_CONTENT_START])
                .sub(fixed[FIX_EXPECTED + 4]),
        ),
    );
    push(
        oid.mul(
            row[BASE_ADDRESS]
                .sub(row[BASE_CONTENT_START])
                .sub(fixed[FIX_OID_OFFSET]),
        ),
    );
    push(subject.mul(row[BASE_G].sub(A::from_base(F(
        ZkX509Rfc5280GrammarRoleV1::CertificateSubject as u64,
    )))));
    push(subject.mul(row[BASE_H].sub(A::ONE)));
    push(subject.mul(row[BASE_CONSTRUCTED].sub(A::ONE)));
    push(subject.mul(row[BASE_TAG_NUMBER].sub(A::from_base(F(16)))));
    push(subject.mul(row[BASE_DEPTH].sub(row[BASE_CONTENT_START])));
    push(subject.mul(row[BASE_CHILD].sub(row[BASE_CONTENT_END])));
    push(oid.mul(row[BASE_G].sub(A::from_base(F(
        ZkX509Rfc5280GrammarRoleV1::NameAttributeOid as u64,
    )))));
    push(oid.mul(row[BASE_CONSTRUCTED]));
    push(oid.mul(row[BASE_TAG_NUMBER].sub(A::from_base(F(6)))));
    push(oid.mul(row[BASE_A].sub(A::from_base(F(3)))));
    push(value.mul(row[BASE_G].sub(A::from_base(F(
        ZkX509Rfc5280GrammarRoleV1::NameAttributeValue as u64,
    )))));
    push(value.mul(row[BASE_CONSTRUCTED]));
    push(
        value
            .mul(row[BASE_TAG_NUMBER].sub(A::from_base(F(12))))
            .mul(row[BASE_TAG_NUMBER].sub(A::from_base(F(19)))),
    );
    push(value.mul(row[BASE_PARENT].sub(row[BASE_A])));
    push(oid_last.mul(next[BASE_START].sub(row[BASE_CONTENT_END])));
    push(oid_last.mul(next[BASE_H].sub(row[BASE_H])));
    let slack = (0..16).fold(A::ZERO, |sum, bit| {
        sum.add(row[BASE_SMALL_BITS + bit].mul_base(F(1 << bit)))
    });
    push(oid_first.mul(row[BASE_START].sub(row[BASE_DEPTH]).sub(slack)));
    push(fixed[FIX_VALUE_FIRST].mul(row[BASE_CHILD].sub(row[BASE_CONTENT_END]).sub(slack)));
    for bit in 0..16 {
        let bit = row[BASE_SMALL_BITS + bit];
        push(boundary.mul(bit).mul(bit.sub(A::ONE)));
    }
    for bit in 0..16 {
        push(disclosure.sub(boundary).mul(row[BASE_SMALL_BITS + bit]));
    }
    for column in [BASE_DEPTH, BASE_CHILD, BASE_PARENT] {
        push(fixed[FIX_PAIR_CONTINUE].mul(next[column].sub(row[column])));
    }
    for column in METADATA {
        push(fixed[FIX_METADATA_CONTINUE].mul(next[column].sub(row[column])));
    }
    push(length.mul(row[BASE_B]));
    push(length.mul(row[BASE_C]));
    push(bytes.mul(row[BASE_E]));
    push(bytes.mul(row[BASE_F]));
    assert_eq!(index, RESIDUES);
    output
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Copy)]
pub(super) struct Source<'a> {
    pub(super) subject: &'a ZkX509Rfc5280NodeProvenanceV1,
    pub(super) oid: &'a ZkX509Rfc5280NodeProvenanceV1,
    pub(super) value: &'a ZkX509Rfc5280NodeProvenanceV1,
    pub(super) bytes: &'a [u8],
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn sources(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<[Option<Source<'_>>; 4], ZkX509Rfc5280StarkErrorV1> {
    let leaf = trace
        .certificates
        .first()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    let nodes = &trace
        .semantic_provenance
        .first()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
        .nodes;
    let document = trace
        .documents
        .first()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    let mut subject_nodes = nodes.iter().filter(|node| {
        node.role == ZkX509Rfc5280GrammarRoleV1::CertificateSubject && node.role_instance == 1
    });
    let subject = subject_nodes
        .next()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    if subject_nodes.next().is_some()
        || subject.document != 0
        || subject.tag_class != 0
        || !subject.constructed
        || subject.tag_number != 16
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    let mut result = [None; 4];
    for (slot, &attribute) in trace
        .statement
        .disclosed_attribute_indices
        .iter()
        .enumerate()
    {
        let expected_oid = OIDS
            .get(usize::from(attribute))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        let bytes = leaf
            .subject
            .attributes
            .get(usize::from(attribute))
            .and_then(Option::as_deref)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        if bytes.is_empty() || bytes.len() > ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1 {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        let mut oids = nodes.iter().filter(|node| {
            node.role == ZkX509Rfc5280GrammarRoleV1::NameAttributeOid
                && node.document == 0
                && node.start >= subject.content_start
                && node.content_end <= subject.content_end
                && node.content_end.checked_sub(node.content_start) == Some(3)
                && document
                    .bytes
                    .get(usize::from(node.content_start)..usize::from(node.content_end))
                    .is_some_and(|cells| {
                        cells
                            .iter()
                            .zip(expected_oid)
                            .all(|(cell, &byte)| cell.value.value == F(u64::from(byte)))
                    })
        });
        let oid = oids.next().ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        if oids.next().is_some() || oid.tag_class != 0 || oid.constructed || oid.tag_number != 6 {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        let mut values = nodes.iter().filter(|node| {
            node.role == ZkX509Rfc5280GrammarRoleV1::NameAttributeValue
                && node.document == 0
                && node.role_instance == oid.role_instance
                && node.parent_node == oid.parent_node
                && node.start == oid.content_end
        });
        let value = values.next().ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        if values.next().is_some()
            || value.tag_class != 0
            || value.constructed
            || !matches!(value.tag_number, 12 | 19)
            || value.content_end > subject.content_end
            || value
                .content_end
                .checked_sub(value.content_start)
                .map(usize::from)
                != Some(bytes.len())
            || !document
                .bytes
                .get(usize::from(value.content_start)..usize::from(value.content_end))
                .is_some_and(|cells| {
                    cells
                        .iter()
                        .zip(bytes)
                        .all(|(cell, &byte)| cell.value.value == F(u64::from(byte)))
                })
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        *result
            .get_mut(slot)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Shape)? = Some(Source {
            subject,
            oid,
            value,
            bytes,
        });
    }
    // All metadata and private bytes stay borrowed from the existing clearing trace.
    Ok(result)
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_row(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    source: &Source<'_>,
    length: bool,
    offset: usize,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if offset
        >= if length {
            8
        } else {
            ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
        }
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    let oid = length && (1..=3).contains(&offset);
    let node = if length && offset == 0 {
        source.subject
    } else if oid {
        source.oid
    } else {
        source.value
    };
    let size = source.bytes.len();
    let expected = if length {
        (size as u64).to_be_bytes()[offset]
    } else {
        source.bytes.get(offset).copied().unwrap_or(0)
    };
    if row[BASE_VALUE] != F(u64::from(expected)) {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    row[BASE_A] = F(u64::from(
        node.content_end
            .checked_sub(node.content_start)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?,
    ));
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
    row[BASE_PARENT] = F(size as u64);
    row[BASE_DEPTH] = F(u64::from(source.subject.content_start));
    row[BASE_CHILD] = F(u64::from(source.subject.content_end));
    let live = !length && offset < size;
    row[BASE_D] = F(u64::from(live || oid));
    row[BASE_ADDRESS] = F(if oid {
        usize::from(node.content_start) + offset - 1
    } else if live {
        usize::from(node.content_start) + offset
    } else {
        0
    } as u64);
    if length {
        row[BASE_E] = F(if offset == 7 {
            (size & 0xff00) as u64
        } else {
            0
        });
        row[BASE_F] = F(if offset == 6 {
            (size & 0xff00) as u64
        } else if offset == 7 {
            size as u64
        } else {
            0
        });
    } else {
        row[BASE_B] = F(offset.min(size) as u64);
        row[BASE_C] = row[BASE_B].add(row[BASE_D]);
    }
    let slack = if length && offset == 1 {
        source.oid.start.checked_sub(source.subject.content_start)
    } else if length && offset == 4 {
        source
            .subject
            .content_end
            .checked_sub(source.value.content_end)
    } else {
        Some(0)
    }
    .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    write_u16_bits_v1(row, BASE_SMALL_BITS, slack);
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn byte_multiplicity(
    sources: &[Option<Source<'_>>; 4],
    document: usize,
    address: usize,
) -> usize {
    sources
        .iter()
        .flatten()
        .filter(|source| {
            document == 0
                && [source.oid, source.value].iter().any(|node| {
                    usize::from(node.content_start) <= address
                        && address < usize::from(node.content_end)
                })
        })
        .count()
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn node_multiplicity(
    sources: &[Option<Source<'_>>; 4],
    document: usize,
    node: usize,
) -> u16 {
    if document != 0 {
        return 0;
    }
    sources
        .iter()
        .flatten()
        .map(|source| {
            if node == usize::from(source.subject.node) {
                1
            } else if node == usize::from(source.oid.node) {
                3
            } else if node == usize::from(source.value.node) {
                source.value.content_end - source.value.content_start + 1
            } else {
                0
            }
        })
        .sum()
}
