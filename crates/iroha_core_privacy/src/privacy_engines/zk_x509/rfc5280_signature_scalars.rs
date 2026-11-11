//! Strict-DER `ECDSA-Sig-Value` parsing into the fixed 32-byte P-256 scalars.
//!
//! Each verifier-fixed group reads one document's `signatureValue` BIT STRING
//! node through the DER node lookup and its bytes through the DER byte lookup:
//! the six header bytes `30 len 02 Lr .. 02 Ls`, the first one or two content
//! bytes of each INTEGER, and the last 32 content bytes of each INTEGER as the
//! big-endian, left-zero-padded scalar. The 64 output rows of a group are joined
//! by MAIN to the matching P-256 binding sink's `r || s` real-input rows, so the
//! verified signature is the one in the document.
//!
//! Each length is `L = 1 + z + u` with `z = [first content byte = 0]` and `u` in
//! `[0, 31]`, so an INTEGER is minimal and positive exactly when its first byte
//! has its top bit clear and, when that byte is zero, the next byte has its top
//! bit set. A 33-byte INTEGER therefore always starts with its sign octet,
//! which lies outside the 32-byte window. The range `1 <= r, s < n` stays with
//! the P-256 relation.
use super::*;

/// Signatures in sink order 0, 1, 3, 2: the optional third certificate comes
/// last so that the active rows of the family are one prefix.
pub(super) const GROUPS: usize = 4;
pub(super) const ROWS_PER_GROUP: usize = 74;
pub(super) const ROWS: usize = GROUPS * ROWS_PER_GROUP;
const HEADER_ROWS: usize = 6;
const HEAD_ROWS: usize = 4;
/// Group offset of the first of the 64 output rows (`r` then `s`).
pub(super) const OUTPUT_OFFSET: usize = HEADER_ROWS + HEAD_ROWS;
pub(super) const SCALAR_BYTES: usize = 32;
const _: () = assert!(OUTPUT_OFFSET + 2 * SCALAR_BYTES == ROWS_PER_GROUP);
const MAX_INTEGER_BYTES: usize = SCALAR_BYTES + 1;

// These shared fixed cells have no other active interpretation on this family.
const ADDRESS_OFFSET: usize = FIX_EXPECTED;
const ADDRESS_R_LENGTH: usize = FIX_EXPECTED + 1;
const ADDRESS_S_LENGTH: usize = FIX_EXPECTED + 2;
const BYTE_CONSTANT: usize = FIX_EXPECTED + 3;
const BYTE_R_LENGTH: usize = FIX_EXPECTED + 4;
const BYTE_S_LENGTH: usize = FIX_EXPECTED + 5;
const OUTPUT: usize = FIX_EXPECTED + 6;
const INTEGER_FIRST: usize = FIX_EXPECTED + 7;
const INTEGER_SECOND: usize = FIX_EXPECTED + 8;
const DOCUMENT: usize = FIX_TEMPORAL_DOCUMENT;
const CRL_DOCUMENT: usize = FIX_TEMPORAL_CRL_DOCUMENT;
const ROLE: usize = FIX_TEMPORAL_ROLE;
const INSTANCE: usize = FIX_TEMPORAL_INSTANCE;
const OPTIONAL: usize = FIX_DOCUMENT_FIXED;

const R_LENGTH: usize = BASE_B;
const S_LENGTH: usize = BASE_C;
const QUERY: usize = BASE_D;
const ZERO: usize = BASE_E;
const SELECTED_LENGTH: usize = BASE_G;
const BITS: usize = BASE_SMALL_BITS;
const BIT_COUNT: usize = 6;

pub(super) const RESIDUES: usize = 33;

#[derive(Clone, Copy)]
struct GroupSpec {
    sink: usize,
    document: u64,
    cert2_coefficient: u64,
    optional: bool,
    role: ZkX509Rfc5280GrammarRoleV1,
    document_instance: bool,
}

const GROUP_SPECS: [GroupSpec; GROUPS] = {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    [
        GroupSpec {
            sink: 0,
            document: 0,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateSignatureValue,
            document_instance: true,
        },
        GroupSpec {
            sink: 1,
            document: 1,
            cert2_coefficient: 0,
            optional: false,
            role: R::CertificateSignatureValue,
            document_instance: true,
        },
        GroupSpec {
            sink: 3,
            document: 2,
            cert2_coefficient: 1,
            optional: false,
            role: R::CrlSignatureValue,
            document_instance: false,
        },
        GroupSpec {
            sink: 2,
            document: 2,
            cert2_coefficient: 0,
            optional: true,
            role: R::CertificateSignatureValue,
            document_instance: true,
        },
    ]
};

/// First family ordinal of the 64 output rows of P-256 sink `sink`.
pub(super) fn output_ordinal(sink: usize) -> Option<usize> {
    let group = GROUP_SPECS.iter().position(|spec| spec.sink == sink)?;
    Some(group * ROWS_PER_GROUP + OUTPUT_OFFSET)
}

/// Active rows: every group except the optional one, which needs slot two.
pub(super) const fn active_rows(certificate_slot_2: bool) -> usize {
    if certificate_slot_2 {
        ROWS
    } else {
        ROWS - ROWS_PER_GROUP
    }
}

pub(super) fn populate_fixed(fixed: &mut ZkX509Rfc5280StarkFixedRowV1, ordinal: usize) {
    if ordinal >= ROWS {
        return;
    }
    let spec = GROUP_SPECS[ordinal / ROWS_PER_GROUP];
    let offset = ordinal % ROWS_PER_GROUP;
    fixed[OPTIONAL] = F(u64::from(spec.optional));
    fixed[FIX_REQUIRED_ACTIVE] = F(u64::from(!spec.optional));
    fixed[DOCUMENT] = F(spec.document);
    fixed[CRL_DOCUMENT] = F(spec.cert2_coefficient);
    fixed[ROLE] = F(spec.role as u64);
    fixed[INSTANCE] = F(if spec.document_instance {
        spec.document
    } else {
        0
    });
    fixed[FIX_LOCAL_FIRST] = F(u64::from(offset == 0));
    fixed[FIX_LOCAL_LAST] = F(u64::from(offset + 1 == ROWS_PER_GROUP));
    // Row content offsets from the BIT STRING content start `cs`: the unused-bits
    // octet is cs, then 30 len 02 Lr r 02 Ls s.
    // Output row j of r reads cs + 5 + Lr - 32 + j, of s cs + 7 + Lr + Ls - 32 + j.
    let (address, r_length, s_length) = match offset {
        0..=3 => (F(offset as u64 + 1), 0, 0),
        4 | 5 => (F(offset as u64 + 1), 1, 0),
        6 | 7 => (F(5), 0, 0),
        8 | 9 => (F(7), 1, 0),
        _ if offset < OUTPUT_OFFSET + SCALAR_BYTES => {
            (F((offset - OUTPUT_OFFSET) as u64).sub(F(27)), 1, 0)
        }
        _ => (
            F((offset - OUTPUT_OFFSET - SCALAR_BYTES) as u64).sub(F(25)),
            1,
            1,
        ),
    };
    fixed[ADDRESS_OFFSET] = address;
    fixed[ADDRESS_R_LENGTH] = F(r_length);
    fixed[ADDRESS_S_LENGTH] = F(s_length);
    if offset < HEADER_ROWS {
        fixed[BYTE_CONSTANT] = F([0x30, 4, 2, 0, 2, 0][offset]);
        fixed[BYTE_R_LENGTH] = F([0, 1, 0, 1, 0, 0][offset]);
        fixed[BYTE_S_LENGTH] = F([0, 1, 0, 0, 0, 1][offset]);
    }
    fixed[INTEGER_FIRST] = F(u64::from(matches!(offset, 6 | 8)));
    fixed[INTEGER_SECOND] = F(u64::from(matches!(offset, 7 | 9)));
    fixed[OUTPUT] = F(u64::from(offset >= OUTPUT_OFFSET));
}

pub(super) fn byte_query_gate<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> A {
    fixed[ZkX509Rfc5280StarkFamilyV1::SignatureScalar as usize].mul(row[QUERY])
}

pub(super) fn node_query_gate<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> A {
    fixed[ZkX509Rfc5280StarkFamilyV1::SignatureScalar as usize].mul(row[BASE_IS_WRITE])
}

pub(super) fn append_residues<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
    output: &mut Vec<A>,
) {
    let before = output.len();
    let gate = fixed[ZkX509Rfc5280StarkFamilyV1::SignatureScalar as usize];
    let active = row[BASE_ACTIVE];
    let certificate_slot_2 = row[BASE_CERT2_ACTIVE];
    let continuation = A::ONE.sub(fixed[FIX_LOCAL_LAST]);
    let output_row = fixed[OUTPUT];
    let integer_first = fixed[INTEGER_FIRST];
    let integer_second = fixed[INTEGER_SECOND];
    let header = A::ONE
        .sub(output_row)
        .sub(integer_first)
        .sub(integer_second);
    let r_length = row[R_LENGTH];
    let s_length = row[S_LENGTH];
    let query = row[QUERY];
    let zero = row[ZERO];
    let value = row[BASE_VALUE];
    let top_bit = row[BASE_BYTE_BITS + 7];
    let bits = &row[BITS..BITS + BIT_COUNT];
    let mut push = |residue: A| output.push(gate.mul(residue));
    // The group's signature node: document, grammar role and instance, a
    // primitive universal BIT STRING whose contents are exactly the encoding.
    push(
        active
            .sub(fixed[FIX_REQUIRED_ACTIVE])
            .sub(fixed[OPTIONAL].mul(certificate_slot_2)),
    );
    push(
        row[BASE_DOCUMENT]
            .sub(fixed[DOCUMENT].mul(active))
            .sub(fixed[CRL_DOCUMENT].mul(certificate_slot_2)),
    );
    push(row[BASE_ROLE].sub(fixed[ROLE].mul(active)));
    push(row[BASE_INSTANCE].sub(fixed[INSTANCE].mul(active)));
    push(row[BASE_TAG_CLASS]);
    push(row[BASE_CONSTRUCTED]);
    push(row[BASE_TAG_NUMBER].sub(active.mul_base(F(3))));
    push(
        row[BASE_CONTENT_END]
            .sub(row[BASE_CONTENT_START])
            .sub(active.mul_base(F(7)))
            .sub(r_length)
            .sub(s_length),
    );
    push(
        row[BASE_A]
            .sub(row[BASE_CONTENT_END])
            .add(row[BASE_CONTENT_START]),
    );
    push(row[BASE_IS_WRITE].sub(fixed[FIX_LOCAL_FIRST].mul(active)));
    for column in [BASE_CONTENT_START, R_LENGTH, S_LENGTH] {
        push(continuation.mul(next[column].sub(row[column])));
    }
    // Every byte row reads cs + offset + coefficient lengths; a second INTEGER
    // byte row reads the byte after the sign octet, or the first byte again.
    push(
        row[BASE_ADDRESS]
            .sub(row[BASE_CONTENT_START])
            .sub(fixed[ADDRESS_OFFSET].mul(active))
            .sub(fixed[ADDRESS_R_LENGTH].mul(r_length))
            .sub(fixed[ADDRESS_S_LENGTH].mul(s_length))
            .sub(integer_second.mul(zero)),
    );
    push(query.mul(query.sub(A::ONE)));
    push(A::ONE.sub(output_row).mul(query.sub(active)));
    push(
        header
            .mul(value)
            .sub(fixed[BYTE_CONSTANT].mul(active))
            .sub(fixed[BYTE_R_LENGTH].mul(r_length))
            .sub(fixed[BYTE_S_LENGTH].mul(s_length)),
    );
    // Minimal positive INTEGER heads and L = 1 + z + u with u in [0, 31].
    push(zero.mul(zero.sub(A::ONE)));
    push(integer_first.mul(top_bit));
    push(integer_first.mul(zero).mul(value));
    push(integer_first.mul(value.mul(row[BASE_INVERSE]).sub(active).add(zero)));
    push(integer_first.mul(next[ZERO].sub(zero)));
    push(integer_second.mul(zero).mul(A::ONE.sub(top_bit)));
    push(
        integer_first.mul(
            r_length
                .add(fixed[ADDRESS_R_LENGTH].mul(s_length.sub(r_length)))
                .sub(active)
                .sub(zero)
                .sub(pack_bits_v1(&bits[..5])),
        ),
    );
    // Output row j of an INTEGER of length L is live exactly when L >= 32 - j:
    // w = live ? L - T : T - 1 - L lies in [0, 63] with T = 32 - j.
    let selected = row[SELECTED_LENGTH];
    push(
        selected
            .sub(output_row.mul(r_length))
            .sub(fixed[ADDRESS_S_LENGTH].mul(s_length.sub(r_length))),
    );
    let threshold = A::from_base(F(5))
        .add(fixed[ADDRESS_S_LENGTH].mul_base(F(2)))
        .sub(fixed[ADDRESS_OFFSET]);
    let window = query
        .mul(selected.sub(threshold).mul_base(F(2)).add(A::ONE))
        .add(threshold.sub(A::ONE).mul(active))
        .sub(selected);
    push(output_row.mul(window.sub(pack_bits_v1(bits))));
    push(output_row.mul(A::ONE.sub(query)).mul(value));
    for bit in bits {
        push(bit.mul(bit.sub(A::ONE)));
    }
    assert_eq!(output.len() - before, RESIDUES);
}

#[derive(Clone, Copy)]
pub(super) struct Source<'a> {
    node: &'a ZkX509Rfc5280NodeProvenanceV1,
    r_length: u8,
    s_length: u8,
    r_zero: bool,
    s_zero: bool,
}

impl Source<'_> {
    /// Address the row reads before any lookup selection. Output rows of a
    /// short INTEGER may lie before its first byte and are never queried.
    fn address(&self, offset: usize) -> F {
        let start = usize::from(self.node.content_start);
        let r = usize::from(self.r_length);
        let s = usize::from(self.s_length);
        let relative = match offset {
            0..=3 => start + offset + 1,
            4 | 5 => start + offset + 1 + r,
            6 => start + 5,
            7 => start + 5 + usize::from(self.r_zero),
            8 => start + 7 + r,
            9 => start + 7 + r + usize::from(self.s_zero),
            _ if offset < OUTPUT_OFFSET + SCALAR_BYTES => start + 5 + r + offset - OUTPUT_OFFSET,
            _ => start + 7 + r + s + offset - OUTPUT_OFFSET - SCALAR_BYTES,
        };
        let address = F(relative as u64);
        if offset >= OUTPUT_OFFSET {
            address.sub(F(SCALAR_BYTES as u64))
        } else {
            address
        }
    }
    /// INTEGER length an output row's window is measured against.
    fn selected_length(&self, offset: usize) -> Option<usize> {
        if offset < OUTPUT_OFFSET {
            None
        } else if offset < OUTPUT_OFFSET + SCALAR_BYTES {
            Some(usize::from(self.r_length))
        } else {
            Some(usize::from(self.s_length))
        }
    }
    fn threshold(offset: usize) -> usize {
        SCALAR_BYTES - (offset - OUTPUT_OFFSET) % SCALAR_BYTES
    }
    /// The address of the row's DER byte query, if it has one.
    fn query(&self, offset: usize) -> Option<usize> {
        if offset >= ROWS_PER_GROUP
            || self
                .selected_length(offset)
                .is_some_and(|length| length < Self::threshold(offset))
        {
            return None;
        }
        usize::try_from(self.address(offset).0).ok()
    }
}

fn document_byte(
    trace: &ZkX509Rfc5280TraceV1,
    node: &ZkX509Rfc5280NodeProvenanceV1,
    address: usize,
) -> Option<u8> {
    let byte = trace
        .documents
        .get(usize::from(node.document))?
        .bytes
        .get(address)?;
    u8::try_from(byte.value.value.0).ok()
}

/// Length of the minimal positive DER INTEGER starting at `tag`, or `None`.
fn integer_length(contents: &[u8], tag: usize) -> Option<usize> {
    let length = usize::from(*contents.get(tag + 1)?);
    let first = *contents.get(tag + 2)?;
    if contents[tag] != 0x02
        || !(1..=MAX_INTEGER_BYTES).contains(&length)
        || (first & 0x80) != 0
        || (first == 0 && (length < 2 || (*contents.get(tag + 3)? & 0x80) == 0))
        || (first != 0 && length > SCALAR_BYTES)
    {
        return None;
    }
    Some(length)
}

// The longest admitted contents: the unused-bits octet, the SEQUENCE header and
// two 33-byte INTEGERs with their headers. Cleared on every return.
struct ContentsFrame([u8; 3 + 2 * (2 + MAX_INTEGER_BYTES)]);

impl Drop for ContentsFrame {
    fn drop(&mut self) {
        zeroize_words_v1(&mut self.0);
    }
}

pub(super) fn sources(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<[Option<Source<'_>>; GROUPS], ZkX509Rfc5280StarkErrorV1> {
    if !(2..=3).contains(&trace.certificates.len()) {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let certificate_slot_2 = u64::from(trace.certificates.len() == 3);
    let mut result = [None; GROUPS];
    for (group, spec) in GROUP_SPECS.into_iter().enumerate() {
        if spec.optional && certificate_slot_2 == 0 {
            continue;
        }
        let document = usize::try_from(spec.document + spec.cert2_coefficient * certificate_slot_2)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        let instance = if spec.document_instance { document } else { 0 };
        let mut nodes = trace
            .semantic_provenance
            .get(document)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
            .nodes
            .iter()
            .filter(|node| node.role == spec.role && usize::from(node.role_instance) == instance);
        let node = nodes.next().ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        if nodes.next().is_some()
            || usize::from(node.document) != document
            || node.tag_class != 0
            || node.constructed
            || node.tag_number != 3
            || node.content_end < node.content_start
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Source);
        }
        let mut frame = ContentsFrame([0; 3 + 2 * (2 + MAX_INTEGER_BYTES)]);
        let length = usize::from(node.content_end - node.content_start);
        let contents = frame
            .0
            .get_mut(..length)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        for (index, byte) in contents.iter_mut().enumerate() {
            *byte = document_byte(trace, node, usize::from(node.content_start) + index)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
        }
        let contents = &frame.0[..length];
        let r = integer_length(contents, 3).ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        let s = integer_length(contents, 5 + r).ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        if contents[0] != 0
            || contents[1] != 0x30
            || usize::from(contents[2]) != 4 + r + s
            || length != 7 + r + s
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        result[group] = Some(Source {
            node,
            r_length: u8::try_from(r).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            s_length: u8::try_from(s).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            r_zero: contents[5] == 0,
            s_zero: contents[7 + r] == 0,
        });
    }
    Ok(result)
}

pub(super) fn byte_multiplicity(
    sources: &[Option<Source<'_>>; GROUPS],
    document: usize,
    address: usize,
) -> usize {
    sources
        .iter()
        .flatten()
        .filter(|source| usize::from(source.node.document) == document)
        .map(|source| {
            (0..ROWS_PER_GROUP)
                .filter(|&offset| source.query(offset) == Some(address))
                .count()
        })
        .sum()
}

pub(super) fn node_multiplicity(
    sources: &[Option<Source<'_>>; GROUPS],
    document: usize,
    node: usize,
) -> u16 {
    u16::try_from(
        sources
            .iter()
            .flatten()
            .filter(|source| {
                usize::from(source.node.document) == document
                    && usize::from(source.node.node) == node
            })
            .count(),
    )
    .expect("fixed signature group count fits u16")
}

pub(super) fn append_rows(
    trace: &ZkX509Rfc5280TraceV1,
    sources: &[Option<Source<'_>>; GROUPS],
    rows: &mut Vec<ZkX509Rfc5280StarkBaseRowV1>,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    for (group, source) in sources.iter().enumerate() {
        let Some(source) = source else {
            // Only the optional last group may be absent, keeping one prefix.
            if group + 1 != GROUPS {
                return Err(ZkX509Rfc5280StarkErrorV1::Shape);
            }
            continue;
        };
        let node = source.node;
        for offset in 0..ROWS_PER_GROUP {
            let mut row = active_zero_row_v1();
            row[BASE_A] = F(u64::from(node.content_end - node.content_start));
            row[R_LENGTH] = F(u64::from(source.r_length));
            row[S_LENGTH] = F(u64::from(source.s_length));
            row[BASE_DOCUMENT] = F(u64::from(node.document));
            row[BASE_NODE] = F(u64::from(node.node));
            row[BASE_START] = F(u64::from(node.start));
            row[BASE_CONTENT_START] = F(u64::from(node.content_start));
            row[BASE_CONTENT_END] = F(u64::from(node.content_end));
            row[BASE_ROLE] = F(node.role as u64);
            row[BASE_INSTANCE] = F(u64::from(node.role_instance));
            row[BASE_TAG_CLASS] = F(u64::from(node.tag_class));
            row[BASE_CONSTRUCTED] = F(u64::from(node.constructed));
            row[BASE_TAG_NUMBER] = F(u64::from(node.tag_number));
            row[BASE_IS_WRITE] = F(u64::from(offset == 0));
            row[BASE_ADDRESS] = source.address(offset);
            if (6..OUTPUT_OFFSET).contains(&offset) {
                let (zero, length) = if offset < 8 {
                    (source.r_zero, source.r_length)
                } else {
                    (source.s_zero, source.s_length)
                };
                row[ZERO] = F(u64::from(zero));
                if offset % 2 == 0 {
                    let unsigned = u64::from(length) - 1 - u64::from(zero);
                    for bit in 0..5 {
                        row[BITS + bit] = F((unsigned >> bit) & 1);
                    }
                }
            }
            if let Some(selected) = source.selected_length(offset) {
                let threshold = Source::threshold(offset);
                let window = if selected >= threshold {
                    selected - threshold
                } else {
                    threshold - 1 - selected
                } as u64;
                row[SELECTED_LENGTH] = F(selected as u64);
                for bit in 0..BIT_COUNT {
                    row[BITS + bit] = F((window >> bit) & 1);
                }
            }
            if let Some(address) = source.query(offset) {
                let byte =
                    document_byte(trace, node, address).ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
                row[QUERY] = F::ONE;
                row[BASE_VALUE] = F(u64::from(byte));
                write_u8_bits_v1(&mut row, BASE_BYTE_BITS, byte);
                if matches!(offset, 6 | 8) && byte != 0 {
                    row[BASE_INVERSE] = row[BASE_VALUE].inverse_or_zero_canonical_v1();
                }
            }
            push_family_row_v1(rows, row)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "rfc5280_signature_scalars_tests.rs"]
mod tests;
