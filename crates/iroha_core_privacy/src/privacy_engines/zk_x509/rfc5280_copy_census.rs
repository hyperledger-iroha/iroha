//! Complete original-source equality and embedded-extension copy census.
//!
//! Fixed private endpoint slots authenticate one original node each and query
//! every byte of its nonempty span. Typed slot/offset/value tuples join both
//! endpoints through the existing four-lane copy products. No private length,
//! document count or new terminal enters the public statement or transcript.
use super::*;

pub(super) const NAME_BYTES: usize = 836;
pub(super) const IDENTIFIER_BYTES: usize = 64;
pub(super) const EMBEDDED_BYTES: usize = 256;
pub(super) const EQUALITY_ROWS: usize = 4 * (NAME_BYTES + IDENTIFIER_BYTES);
pub(super) const EMBEDDED_ROWS: usize = 15 * EMBEDDED_BYTES;
pub(super) const CONSUMER_ROWS: usize = EQUALITY_ROWS + EMBEDDED_ROWS;
pub(super) const RESIDUES: usize = 40;
const EQUALITY_DOMAIN: u64 = 103;
const EMBEDDED_DOMAIN: u64 = 104;
const _: () = assert!(EQUALITY_ROWS <= FIXED_EQUAL_BYTE_ROWS_V1);
const _: () = assert!(EMBEDDED_ROWS <= FIXED_EMBEDDED_COPY_ROWS_V1);
const _: () = assert!(CONSUMER_ROWS <= FIXED_SEMANTIC_CONSUMER_ROWS_V1);

/// Public endpoint identity, reconstructed without reading witness cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Slot {
    pub(super) embedded: bool,
    pub(super) consumer: bool,
    pub(super) id: usize,
    pub(super) offset: usize,
    pub(super) capacity: usize,
    document: u64,
    certificate_coefficient: u64,
    role: ZkX509Rfc5280GrammarRoleV1,
    instance: u64,
    tag_class: u64,
    constructed: bool,
    tag_number: u64,
    contents_only: bool,
    optional: bool,
}

fn equality_slot(id: usize, offset: usize, consumer: bool) -> Option<Slot> {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    let (document, certificate_coefficient, role, instance) = match (id, consumer) {
        (0, false) => (0, 0, R::CertificateIssuer, 0),
        (0, true) => (1, 0, R::CertificateSubject, 1),
        (1, false) => (3, 1, R::EmbeddedAkiIdentifier, 0),
        (1, true) => (9, 1, R::EmbeddedSki, 0),
        (2, false) => (1, 0, R::CertificateIssuer, 0),
        (2, true) => (1, 1, R::CertificateSubject, 1),
        (3, false) => (8, 1, R::EmbeddedAkiIdentifier, 0),
        (3, true) => (9, 5, R::EmbeddedSki, 0),
        (4, false) => (2, 0, R::CertificateIssuer, 0),
        (4, true) => (2, 0, R::CertificateSubject, 1),
        (5, false) => (13, 0, R::EmbeddedAkiIdentifier, 0),
        (5, true) => (14, 0, R::EmbeddedSki, 0),
        (6, false) => (2, 1, R::CrlIssuer, 2),
        (6, true) => (1, 0, R::CertificateSubject, 1),
        (7, false) => (12, 5, R::EmbeddedAkiIdentifier, 0),
        (7, true) => (9, 1, R::EmbeddedSki, 0),
        _ => return None,
    };
    let identifier = id % 2 == 1;
    let capacity = if identifier {
        IDENTIFIER_BYTES
    } else {
        NAME_BYTES
    };
    if offset >= capacity {
        return None;
    }
    Some(Slot {
        embedded: false,
        consumer,
        id,
        offset,
        capacity,
        document,
        certificate_coefficient,
        role,
        instance,
        tag_class: u64::from(identifier && !consumer) * 2,
        constructed: !identifier,
        tag_number: if identifier {
            if consumer { 4 } else { 0 }
        } else {
            16
        },
        contents_only: identifier,
        optional: matches!(id, 4 | 5),
    })
}

fn embedded_slot(id: usize, offset: usize, consumer: bool) -> Option<Slot> {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    if id >= 15 || offset >= EMBEDDED_BYTES {
        return None;
    }
    let (parent, parent_coefficient, extension, embedded, embedded_coefficient, crl) = match id {
        0..=4 => (0, 0, id, 3 + id, 1, false),
        5..=8 => (1, 0, id - 5, 8 + id - 5, 1, false),
        9..=12 => (2, 0, id - 9, 13 + id - 9, 0, false),
        _ => (2, 1, id - 13, 12 + id - 13, 5, true),
    };
    let root = if crl {
        [R::EmbeddedAki, R::EmbeddedCrlNumber][extension]
    } else {
        [
            R::EmbeddedAki,
            R::EmbeddedSki,
            R::EmbeddedKeyUsage,
            R::EmbeddedBasicConstraints,
            R::EmbeddedEku,
        ][extension]
    };
    let (constructed, tag_number) = match root {
        R::EmbeddedSki => (false, 4),
        R::EmbeddedKeyUsage => (false, 3),
        R::EmbeddedCrlNumber => (false, 2),
        _ => (true, 16),
    };
    Some(Slot {
        embedded: true,
        consumer,
        id,
        offset,
        capacity: EMBEDDED_BYTES,
        document: if consumer { embedded as u64 } else { parent },
        certificate_coefficient: if consumer {
            embedded_coefficient
        } else {
            parent_coefficient
        },
        role: if consumer {
            root
        } else if crl {
            R::CrlExtensionValue
        } else {
            R::CertificateExtensionValue
        },
        instance: if consumer { 0 } else { extension as u64 },
        tag_class: 0,
        constructed: consumer && constructed,
        tag_number: if consumer { tag_number } else { 4 },
        contents_only: !consumer,
        optional: matches!(id, 9..=12),
    })
}

pub(super) fn slot(family: ZkX509Rfc5280StarkFamilyV1, mut ordinal: usize) -> Option<Slot> {
    use ZkX509Rfc5280StarkFamilyV1 as Family;
    let consumer = family == Family::SemanticConsumer;
    let embedded = if consumer {
        if ordinal >= EQUALITY_ROWS {
            ordinal -= EQUALITY_ROWS;
            true
        } else {
            false
        }
    } else if family == Family::EmbeddedCopy {
        true
    } else if family == Family::EqualByte {
        false
    } else {
        return None;
    };
    if embedded {
        return embedded_slot(ordinal / EMBEDDED_BYTES, ordinal % EMBEDDED_BYTES, consumer);
    }
    let pair = ordinal / (NAME_BYTES + IDENTIFIER_BYTES);
    let position = ordinal % (NAME_BYTES + IDENTIFIER_BYTES);
    if position < NAME_BYTES {
        equality_slot(2 * pair, position, consumer)
    } else {
        equality_slot(2 * pair + 1, position - NAME_BYTES, consumer)
    }
}

pub(super) fn rows(family: ZkX509Rfc5280StarkFamilyV1) -> usize {
    match family {
        ZkX509Rfc5280StarkFamilyV1::EqualByte => EQUALITY_ROWS,
        ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy => EMBEDDED_ROWS,
        ZkX509Rfc5280StarkFamilyV1::SemanticConsumer => CONSUMER_ROWS,
        _ => 0,
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn active_rows(family: ZkX509Rfc5280StarkFamilyV1, certificate_two: bool) -> usize {
    let equality = (3 + usize::from(certificate_two)) * (NAME_BYTES + IDENTIFIER_BYTES);
    let embedded = (11 + 4 * usize::from(certificate_two)) * EMBEDDED_BYTES;
    match family {
        ZkX509Rfc5280StarkFamilyV1::EqualByte => equality,
        ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy => embedded,
        ZkX509Rfc5280StarkFamilyV1::SemanticConsumer => equality + embedded,
        _ => 0,
    }
}

pub(super) fn populate_fixed(
    fixed: &mut ZkX509Rfc5280StarkFixedRowV1,
    family: ZkX509Rfc5280StarkFamilyV1,
    ordinal: usize,
) {
    if rows(family) == 0 {
        return;
    }
    fixed[FIX_LOCAL_FIRST] = F::ONE;
    fixed[FIX_LOCAL_LAST] = F::ONE;
    fixed[FIX_ACTIVATION_CONTINUE] = F::ZERO;
    if let Some(slot) = slot(family, ordinal) {
        fixed[FIX_EXPECTED..FIX_EXPECTED + 10].copy_from_slice(&[
            F(slot.document),
            F(slot.certificate_coefficient),
            F(slot.role as u64),
            F(slot.instance),
            F(slot.tag_class),
            F(u64::from(slot.constructed)),
            F(slot.tag_number),
            F(u64::from(slot.contents_only)),
            F(slot.id as u64),
            F(slot.offset as u64),
        ]);
        fixed[FIX_ADDRESS_FIXED] = F(u64::from(slot.embedded));
        fixed[FIX_DOCUMENT_FIXED] = F(u64::from(slot.optional));
        fixed[FIX_REQUIRED_ACTIVE] = F(u64::from(!slot.optional));
        fixed[FIX_LOCAL_FIRST] = F(u64::from(slot.offset == 0));
        fixed[FIX_LOCAL_LAST] = F(u64::from(slot.offset + 1 == slot.capacity));
        fixed[FIX_ACTIVATION_CONTINUE] = F::ONE.sub(fixed[FIX_LOCAL_LAST]);
    }
}

fn family_gate<A: PolynomialAirFieldV1>(fixed: &ZkX509Rfc5280StarkFixedRowV1<A>) -> A {
    fixed[ZkX509Rfc5280StarkFamilyV1::EqualByte as usize]
        .add(fixed[ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy as usize])
        .add(fixed[ZkX509Rfc5280StarkFamilyV1::SemanticConsumer as usize])
}

pub(super) fn node_query_gate<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> A {
    family_gate(fixed).mul(row[BASE_IS_WRITE])
}

pub(super) fn normalize<A: PolynomialAirFieldV1>(
    row: &mut ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) {
    let gate = family_gate(fixed).mul(row[BASE_D]);
    let source = fixed[ZkX509Rfc5280StarkFamilyV1::EqualByte as usize]
        .add(fixed[ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy as usize])
        .mul(row[BASE_D]);
    let consumer = fixed[ZkX509Rfc5280StarkFamilyV1::SemanticConsumer as usize].mul(row[BASE_D]);
    row[BASE_SERIAL_BYTE_QUERY_ACTIVE] = row[BASE_SERIAL_BYTE_QUERY_ACTIVE].add(gate);
    row[BASE_SERIAL_BYTE_QUERY_VALUE] =
        row[BASE_SERIAL_BYTE_QUERY_VALUE].add(gate.mul(row[BASE_VALUE]));
    row[BASE_COPY_SOURCE_ACTIVE] = row[BASE_COPY_SOURCE_ACTIVE].add(source);
    row[BASE_COPY_CONSUMER_ACTIVE] = row[BASE_COPY_CONSUMER_ACTIVE].add(consumer);
    row[BASE_COPY_DOMAIN] = row[BASE_COPY_DOMAIN].add(
        gate.mul(
            A::from_base(F(EQUALITY_DOMAIN))
                .add(fixed[FIX_ADDRESS_FIXED].mul_base(F(EMBEDDED_DOMAIN - EQUALITY_DOMAIN))),
        ),
    );
    row[BASE_COPY_KEY_1] = row[BASE_COPY_KEY_1].add(gate.mul(row[BASE_H]));
    row[BASE_COPY_KEY_2] = row[BASE_COPY_KEY_2].add(gate.mul(row[BASE_OFFSET]));
    row[BASE_COPY_VALUE] = row[BASE_COPY_VALUE].add(gate.mul(row[BASE_VALUE]));
}

pub(super) fn residues<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> [A; RESIDUES] {
    let family = family_gate(fixed);
    let active = row[BASE_ACTIVE];
    let live = row[BASE_D];
    let first = fixed[FIX_LOCAL_FIRST];
    let last = fixed[FIX_LOCAL_LAST];
    let continuation = A::ONE.sub(last);
    let mut output = [A::ZERO; RESIDUES];
    let mut index = 0;
    let mut push = |value: A| {
        output[index] = family.mul(value);
        index += 1;
    };
    push(
        active
            .sub(fixed[FIX_REQUIRED_ACTIVE])
            .sub(fixed[FIX_DOCUMENT_FIXED].mul(row[BASE_CERT2_ACTIVE])),
    );
    push(row[BASE_DOCUMENT].sub(
        active.mul(fixed[FIX_EXPECTED].add(fixed[FIX_EXPECTED + 1].mul(row[BASE_CERT2_ACTIVE]))),
    ));
    for (column, expected) in [
        (BASE_ROLE, 2),
        (BASE_INSTANCE, 3),
        (BASE_TAG_CLASS, 4),
        (BASE_CONSTRUCTED, 5),
        (BASE_TAG_NUMBER, 6),
        (BASE_H, 8),
        (BASE_OFFSET, 9),
    ] {
        push(row[column].sub(active.mul(fixed[FIX_EXPECTED + expected])));
    }
    push(
        row[BASE_A]
            .sub(row[BASE_CONTENT_END])
            .add(row[BASE_CONTENT_START]),
    );
    push(
        row[BASE_F]
            .sub(row[BASE_START])
            .sub(fixed[FIX_EXPECTED + 7].mul(row[BASE_CONTENT_START].sub(row[BASE_START]))),
    );
    push(row[BASE_B].sub(row[BASE_CONTENT_END]).add(row[BASE_F]));
    push(live.mul(live.sub(A::ONE)));
    push(live.mul(A::ONE.sub(active)));
    push(row[BASE_E].sub(row[BASE_C]).sub(live));
    push(first.mul(row[BASE_C]));
    push(continuation.mul(next[BASE_C].sub(row[BASE_E])));
    push(last.mul(row[BASE_E].sub(row[BASE_B])));
    push(continuation.mul(next[BASE_D]).mul(A::ONE.sub(live)));
    push(row[BASE_ADDRESS].sub(live.mul(row[BASE_F].add(row[BASE_OFFSET]))));
    push(A::ONE.sub(live).mul(row[BASE_VALUE]));
    push(row[BASE_IS_WRITE].sub(active.mul(first)));
    push(row[BASE_B].mul(row[BASE_G]).sub(active));
    for column in [
        BASE_A,
        BASE_B,
        BASE_F,
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
        BASE_ROLE,
        BASE_INSTANCE,
    ] {
        push(continuation.mul(next[column].sub(row[column])));
    }
    // Only the embedded consumer endpoint is the root of its normalized DER document.
    let root =
        fixed[ZkX509Rfc5280StarkFamilyV1::SemanticConsumer as usize].mul(fixed[FIX_ADDRESS_FIXED]);
    output[index] = root.mul(row[BASE_NODE]);
    index += 1;
    output[index] = root.mul(row[BASE_START]);
    index += 1;
    assert_eq!(index, RESIDUES);
    output
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) struct Sources<'a> {
    nodes: [Option<&'a ZkX509Rfc5280NodeProvenanceV1>; 46],
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl<'a> Sources<'a> {
    pub(super) fn new(trace: &'a ZkX509Rfc5280TraceV1) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        let depth = trace.certificates.len();
        if !(2..=3).contains(&depth) || trace.semantic_provenance.len() != 5 * depth + 4 {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        let mut result = Self { nodes: [None; 46] };
        for index in 0..46 {
            let consumer = index % 2 == 1;
            let slot = if index < 16 {
                equality_slot(index / 2, 0, consumer)
            } else {
                embedded_slot((index - 16) / 2, 0, consumer)
            }
            .ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
            if slot.optional && depth == 2 {
                continue;
            }
            let document = slot.document + slot.certificate_coefficient * u64::from(depth == 3);
            let source = trace
                .semantic_provenance
                .get(document as usize)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
            let mut matching = source.nodes.iter().filter(|node| {
                u64::from(node.document) == document
                    && node.role == slot.role
                    && u64::from(node.role_instance) == slot.instance
            });
            let node = matching.next().ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
            if matching.next().is_some()
                || u64::from(node.tag_class) != slot.tag_class
                || node.constructed != slot.constructed
                || u64::from(node.tag_number) != slot.tag_number
                || (slot.embedded && slot.consumer && (node.node != 0 || node.start != 0))
            {
                return Err(ZkX509Rfc5280StarkErrorV1::Source);
            }
            result.nodes[index] = Some(node);
        }
        Ok(result)
    }
    fn node(&self, slot: Slot) -> Option<&'a ZkX509Rfc5280NodeProvenanceV1> {
        self.nodes[if slot.embedded { 16 } else { 0 } + 2 * slot.id + usize::from(slot.consumer)]
    }
    pub(super) fn node_multiplicity(&self, document: usize, node: usize) -> u16 {
        u16::try_from(
            self.nodes
                .iter()
                .flatten()
                .filter(|source| {
                    usize::from(source.document) == document && usize::from(source.node) == node
                })
                .count(),
        )
        .expect("at most46 borrowed endpoint nodes")
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_rows(
    trace: &ZkX509Rfc5280TraceV1,
    sources: &Sources<'_>,
    family: ZkX509Rfc5280StarkFamilyV1,
    output: &mut Vec<ZkX509Rfc5280StarkBaseRowV1>,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if !output.is_empty() || rows(family) == 0 {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    for ordinal in 0..rows(family) {
        let slot = slot(family, ordinal).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
        let Some(node) = sources.node(slot) else {
            if !slot.optional || trace.certificates.len() != 2 {
                return Err(ZkX509Rfc5280StarkErrorV1::Source);
            }
            push_family_row_v1(output, [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1])?;
            continue;
        };
        let start = if slot.contents_only {
            node.content_start
        } else {
            node.start
        };
        let length = node
            .content_end
            .checked_sub(start)
            .map(usize::from)
            .filter(|length| *length > 0 && *length <= slot.capacity)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        let live = slot.offset < length;
        let address = if live {
            usize::from(start) + slot.offset
        } else {
            0
        };
        let source = source_documents_v1(trace)
            .nth(usize::from(node.document))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        let value = if live {
            u8::try_from(
                source
                    .bytes
                    .get(address)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
                    .value
                    .value
                    .0,
            )
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?
        } else {
            0
        };
        let mut row = active_zero_row_v1();
        row[BASE_VALUE] = F(u64::from(value));
        write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
        row[BASE_A] = F(u64::from(
            node.content_end
                .checked_sub(node.content_start)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?,
        ));
        row[BASE_B] = F(length as u64);
        row[BASE_C] = F(slot.offset.min(length) as u64);
        row[BASE_D] = F(u64::from(live));
        row[BASE_E] = F((slot.offset + 1).min(length) as u64);
        row[BASE_F] = F(u64::from(start));
        row[BASE_G] = F(length as u64).inverse_or_zero_canonical_v1();
        row[BASE_H] = F(slot.id as u64);
        row[BASE_DOCUMENT] = F(u64::from(node.document));
        row[BASE_ADDRESS] = F(address as u64);
        row[BASE_NODE] = F(u64::from(node.node));
        row[BASE_START] = F(u64::from(node.start));
        row[BASE_CONTENT_START] = F(u64::from(node.content_start));
        row[BASE_CONTENT_END] = F(u64::from(node.content_end));
        row[BASE_TAG_CLASS] = F(u64::from(node.tag_class));
        row[BASE_CONSTRUCTED] = F(u64::from(node.constructed));
        row[BASE_TAG_NUMBER] = F(u64::from(node.tag_number));
        row[BASE_ROLE] = F(node.role as u64);
        row[BASE_INSTANCE] = F(u64::from(node.role_instance));
        row[BASE_OFFSET] = F(slot.offset as u64);
        row[BASE_IS_WRITE] = F(u64::from(slot.offset == 0));
        push_family_row_v1(output, row)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "rfc5280_copy_census_tests.rs"]
mod tests;
