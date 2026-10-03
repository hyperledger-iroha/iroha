//! Fixed temporal slots and normalized RFC event construction.

use super::*;

const TIME_ROLES: [ZkX509Rfc5280GrammarRoleV1; 5] = [
    ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore,
    ZkX509Rfc5280GrammarRoleV1::CertificateNotAfter,
    ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
    ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
    ZkX509Rfc5280GrammarRoleV1::CrlEntryTime,
];
const NODE_TIME_FLAGS: usize = CALENDAR_COLUMNS;
const NODE_TIME_INVERSES: usize = NODE_TIME_FLAGS + 5;

pub(super) fn populate_fixed_v1(
    fixed: &mut ZkX509Rfc5280StarkFixedRowV1,
    family: ZkX509Rfc5280StarkFamilyV1,
    ordinal: usize,
    shape: ZkX509Rfc5280StarkShapeV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    use ZkX509Rfc5280StarkFamilyV1 as Family;
    if matches!(family, Family::Calendar | Family::Decimal) {
        let width = if family == Family::Calendar {
            numeric::CALENDAR_PHASES_V1
        } else {
            numeric::DECIMAL_ROWS_PER_TIME_V1
        };
        let slot_index = ordinal / width;
        let position = ordinal % width;
        let slot = numeric::temporal_slot_v1(slot_index).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
        let identity = slot.identity_v1(F::ZERO);
        let with_certificate = slot.identity_v1(F::ONE);
        fixed[FIX_TEMPORAL_FAMILY] = F::ONE;
        fixed[FIX_TEMPORAL_DOCUMENT] = identity[0];
        fixed[FIX_TEMPORAL_CRL_DOCUMENT] = with_certificate[0].sub(identity[0]);
        fixed[FIX_TEMPORAL_ROLE] = identity[1];
        fixed[FIX_TEMPORAL_INSTANCE] = identity[2];
        fixed[FIX_TEMPORAL_SLOT] = F(slot_index as u64);
        fixed[FIX_NUMERIC_REQUIRED] = F(u64::from(slot_index < 8 && !matches!(slot_index, 2 | 5)));
        fixed[FIX_NUMERIC_CERT2] = F(u64::from(matches!(slot_index, 2 | 5)));
        fixed[FIX_LOCAL_FIRST] = F(u64::from(position == 0));
        fixed[FIX_LOCAL_LAST] = F(u64::from(position + 1 == width));
        fixed[FIX_TEMPORAL_CONTINUE] = F(u64::from(position + 1 != width));
        fixed[FIX_ACTIVATION_CONTINUE] = fixed[FIX_TEMPORAL_CONTINUE];
        if family == Family::Decimal {
            fixed[FIX_EXPECTED..FIX_EXPECTED + 10]
                .copy_from_slice(&temporal::fixed_cells_v1(position));
        } else {
            fixed[FIX_TIME_IDENTITY] = F(u64::from(position == 0));
            fixed[FIX_TIME_VALUE] = F(u64::from(position + 1 == width));
            fixed[FIX_TIME_THIS_UPDATE] = F(u64::from(position + 1 == width && slot_index == 6));
        }
    }
    if matches!(family, Family::Relation | Family::RangeByte) {
        let width = if family == Family::Relation {
            numeric::RELATION_PHASES_V1
        } else {
            numeric::RANGE_BYTES_PER_RELATION_V1
        };
        let slot =
            numeric::relation_slot_v1(ordinal / width).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
        let position = ordinal % width;
        fixed[FIX_EXPECTED] = F(u64::from(slot.relation));
        fixed[FIX_EXPECTED + 1] = F(u64::from(slot.instance));
        fixed[FIX_EXPECTED + 2] = F(u64::from(slot.strict));
        fixed[FIX_NUMERIC_REQUIRED] = F(u64::from(
            slot.activity == numeric::NumericActivityV1::Required,
        ));
        fixed[FIX_NUMERIC_CERT2] = F(u64::from(
            slot.activity == numeric::NumericActivityV1::CertificateTwo,
        ));
        fixed[FIX_ACTIVATION_CONTINUE] = F(u64::from(position + 1 != width));
        if family == Family::Relation {
            fixed[FIX_RELATION_CONTINUE] = F(u64::from(position == 0));
            fixed[FIX_NUMERIC_LEFT] = F(u64::from(position == 0));
            fixed[FIX_NUMERIC_RIGHT] = F(u64::from(position == 1));
            fixed[FIX_NUMERIC_ENTRY_COUNT] = F(u64::from(
                position == 1 && matches!(slot.activity, numeric::NumericActivityV1::Entry(_)),
            ));
            let operand = if position == 0 { slot.left } else { slot.right };
            match operand {
                numeric::NumericOperandV1::WindowStart | numeric::NumericOperandV1::WindowEnd => {
                    fixed[FIX_EXPECTED + 3] =
                        F(if operand == numeric::NumericOperandV1::WindowStart {
                            shape.presentation_not_before_unix_seconds
                        } else {
                            shape.presentation_not_after_unix_seconds
                        });
                    fixed[FIX_EXPECTED + 4] = F::ONE;
                }
                numeric::NumericOperandV1::Time { slot, add_seconds } => {
                    let identity = slot.identity_v1(F::ZERO);
                    fixed[FIX_TEMPORAL_DOCUMENT] = identity[0];
                    fixed[FIX_TEMPORAL_CRL_DOCUMENT] = slot.identity_v1(F::ONE)[0].sub(identity[0]);
                    fixed[FIX_TEMPORAL_ROLE] = identity[1];
                    fixed[FIX_TEMPORAL_INSTANCE] = identity[2];
                    fixed[FIX_EXPECTED + 5] = F(u64::from(add_seconds));
                    fixed[FIX_NUMERIC_QUERY] = F::ONE;
                }
            }
        } else {
            fixed[FIX_EXPECTED + 3] = F(position as u64);
            fixed[FIX_RANGE_CONTINUE] = F(u64::from(position + 1 != width));
            fixed[FIX_RANGE_LEADING_ZERO] = F(u64::from(position < 3));
            fixed[FIX_RANGE_HIGH_BITS_ZERO] = F(u64::from(position == 3));
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_node_classification_v1(row: &mut ZkX509Rfc5280StarkBaseRowV1) {
    for (index, role) in TIME_ROLES.into_iter().enumerate() {
        let difference = row[BASE_ROLE].sub(F(role as u64));
        row[NODE_TIME_FLAGS + index] = F(u64::from(difference == F::ZERO));
        // Role differences are canonical field values, including a matching zero.
        row[NODE_TIME_INVERSES + index] = difference.inverse_or_zero_canonical_v1();
    }
}

pub(super) fn normalize_v1<A: PolynomialAirFieldV1>(
    row: &mut ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) {
    let source_node = fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize];
    let decimal = fixed[ZkX509Rfc5280StarkFamilyV1::Decimal as usize];
    let active = row[BASE_ACTIVE];
    let node_is_time = row[NODE_TIME_FLAGS..NODE_TIME_FLAGS + 5]
        .iter()
        .copied()
        .fold(A::ZERO, A::add);
    row[BASE_NUMERIC_SOURCE] = source_node
        .mul(active)
        .mul(node_is_time)
        .add(fixed[FIX_TIME_VALUE].mul(active));
    row[BASE_NUMERIC_QUERY] = fixed[FIX_TIME_IDENTITY]
        .add(fixed[FIX_NUMERIC_QUERY])
        .mul(active);
    row[BASE_NUMERIC_MULTIPLICITY] = source_node
        .add(fixed[FIX_TIME_VALUE])
        .add(fixed[FIX_TIME_THIS_UPDATE].mul(A::ONE.add(row[BASE_ENTRY_COUNT])))
        .mul(active);
    row[BASE_DECIMAL_DIGIT_ACTIVE] = decimal.mul(active).mul(row[BASE_H]);
    let calendar_component = fixed
        [FIX_CALENDAR_PHASES + 1..FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1]
        .iter()
        .copied()
        .fold(A::ZERO, A::add)
        .mul(active);
    let decimal_component = decimal.mul(active).mul(row[BASE_STRICT]);
    row[BASE_COPY_START] = decimal_component
        .add(calendar_component)
        .mul(row[BASE_CONTENT_START]);
    row[BASE_COPY_GENERALIZED] = decimal_component
        .add(calendar_component)
        .mul(row[CALENDAR_COLUMNS + calendar::GENERALIZED]);
}

pub(super) fn numeric_lookup_event_v1<A: PolynomialAirFieldV1>(
    row: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
) -> numeric::NumericLookupEventV1<A> {
    let source_node = numeric::time_node_tuple_v1(
        [row[BASE_DOCUMENT], row[BASE_ROLE], row[BASE_INSTANCE]],
        row[BASE_NODE],
        row[BASE_TAG_NUMBER],
        row[BASE_CONTENT_START],
        row[BASE_CONTENT_END],
    );
    let calendar_node = numeric::time_node_tuple_v1(
        [
            row[BASE_DOCUMENT],
            row[BASE_PARENT],
            row[BASE_ENDPOINT_ROLE],
        ],
        row[BASE_NODE],
        row[BASE_TAG_NUMBER],
        row[BASE_CONTENT_START],
        row[BASE_CONTENT_END],
    );
    let timestamp = numeric::timestamp_tuple_v1(
        [
            row[BASE_DOCUMENT],
            row[BASE_PARENT],
            row[BASE_ENDPOINT_ROLE],
        ],
        row[BASE_G],
    );
    let tuple = core::array::from_fn(|index| {
        fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]
            .mul(source_node[index])
            .add(fixed[FIX_TIME_IDENTITY].mul(calendar_node[index]))
            .add(
                fixed[FIX_TIME_VALUE]
                    .add(fixed[FIX_NUMERIC_QUERY])
                    .mul(timestamp[index]),
            )
    });
    numeric::NumericLookupEventV1 {
        source: row[BASE_NUMERIC_SOURCE],
        query: row[BASE_NUMERIC_QUERY],
        multiplicity: row[BASE_NUMERIC_MULTIPLICITY],
        tuple,
    }
}

pub(super) fn append_residues_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509Rfc5280StarkBaseRowV1<A>,
    next: &ZkX509Rfc5280StarkBaseRowV1<A>,
    fixed: &ZkX509Rfc5280StarkFixedRowV1<A>,
    residues: &mut Vec<A>,
) {
    let start = residues.len();
    let active = current[BASE_ACTIVE];
    let source_node = active.mul(fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]);
    for (index, role) in TIME_ROLES.into_iter().enumerate() {
        push_reused_gated_zero_safe_inverse_v1(
            residues,
            source_node,
            current[BASE_ROLE].sub(A::from_base(F(role as u64))),
            current[NODE_TIME_FLAGS + index],
            current[NODE_TIME_INVERSES + index],
        );
    }
    residues.push(fixed[FIX_NUMERIC_REQUIRED].mul(active.sub(A::ONE)));
    residues.push(fixed[FIX_NUMERIC_CERT2].mul(active.sub(current[BASE_CERT2_ACTIVE])));
    let temporal = active.mul(fixed[FIX_TEMPORAL_FAMILY]);
    let generalized = current[CALENDAR_COLUMNS + calendar::GENERALIZED];
    push_boolean_v1(residues, temporal, generalized);
    for (actual, expected) in [
        (
            current[BASE_DOCUMENT],
            fixed[FIX_TEMPORAL_DOCUMENT]
                .add(fixed[FIX_TEMPORAL_CRL_DOCUMENT].mul(current[BASE_CERT2_ACTIVE])),
        ),
        (current[BASE_PARENT], fixed[FIX_TEMPORAL_ROLE]),
        (current[BASE_ENDPOINT_ROLE], fixed[FIX_TEMPORAL_INSTANCE]),
        (current[BASE_INSTANCE], fixed[FIX_TEMPORAL_SLOT]),
    ] {
        residues.push(temporal.mul(actual.sub(expected)));
    }
    for column in [
        BASE_ACTIVE,
        BASE_DOCUMENT,
        BASE_NODE,
        BASE_CONTENT_START,
        BASE_CONTENT_END,
        BASE_TAG_NUMBER,
        BASE_PARENT,
        BASE_INSTANCE,
        BASE_ENDPOINT_ROLE,
        CALENDAR_COLUMNS + calendar::GENERALIZED,
    ] {
        residues.push(fixed[FIX_TEMPORAL_CONTINUE].mul(next[column].sub(current[column])));
    }
    let identity = fixed[FIX_TIME_IDENTITY].mul(active);
    residues.push(
        identity.mul(
            current[BASE_TAG_NUMBER]
                .sub(A::from_base(F(23)))
                .sub(generalized),
        ),
    );
    residues.push(
        identity.mul(
            current[BASE_CONTENT_END]
                .sub(current[BASE_CONTENT_START])
                .sub(A::from_base(F(13)))
                .sub(generalized.mul(A::from_base(F(2)))),
        ),
    );
    let decimal = active.mul(fixed[ZkX509Rfc5280StarkFamilyV1::Decimal as usize]);
    let [group, offset, length, digit, terminator] =
        temporal::expected_v1(&fixed[FIX_EXPECTED..FIX_EXPECTED + 10], generalized);
    for (actual, expected) in [
        (current[BASE_ROLE], group),
        (current[BASE_OFFSET], offset),
        (current[BASE_B], length),
        (current[BASE_H], digit),
        (current[BASE_EQUAL], digit.add(terminator)),
    ] {
        residues.push(decimal.mul(actual.sub(expected)));
    }
    residues.push(
        decimal.mul(
            current[BASE_ADDRESS]
                .sub(current[BASE_CONTENT_START])
                .sub(fixed[FIX_EXPECTED]),
        ),
    );
    residues.push(
        decimal
            .mul(A::ONE.sub(current[BASE_EQUAL]))
            .mul(current[BASE_VALUE]),
    );
    residues.push(
        decimal
            .mul(current[BASE_EQUAL].sub(current[BASE_H]))
            .mul(current[BASE_VALUE].sub(A::from_base(F(90)))),
    );
    let nondigit = decimal.sub(current[BASE_DECIMAL_DIGIT_ACTIVE]);
    for column in [
        BASE_A,
        BASE_STATE_BEFORE,
        BASE_STATE_AFTER,
        BASE_IS_WRITE,
        BASE_STRICT,
        BASE_INVERSE,
        BASE_G,
        BASE_SMALL_BITS,
        BASE_SMALL_BITS + 1,
        BASE_SMALL_BITS + 2,
        BASE_SMALL_BITS + 3,
    ] {
        residues.push(nondigit.mul(current[column]));
    }
    // Decimal's BASE_ROLE is the component number, while its authenticated
    // grammar role is fixed by the slot and transferred through calendar's
    // node event. The document/start/encoding tuple closes the byte source.
    let relation = active.mul(fixed[ZkX509Rfc5280StarkFamilyV1::Relation as usize]);
    for (actual, expected) in [
        (current[BASE_ROLE], fixed[FIX_EXPECTED]),
        (current[BASE_INSTANCE], fixed[FIX_EXPECTED + 1]),
        (current[BASE_STRICT], fixed[FIX_EXPECTED + 2]),
    ] {
        residues.push(relation.mul(actual.sub(expected)));
    }
    let selected = fixed[FIX_NUMERIC_LEFT]
        .mul(current[BASE_A])
        .add(fixed[FIX_NUMERIC_RIGHT].mul(current[BASE_B]));
    residues.push(relation.mul(current[BASE_G].sub(selected).add(fixed[FIX_EXPECTED + 5])));
    residues.push(
        relation
            .mul(fixed[FIX_EXPECTED + 4])
            .mul(current[BASE_G].sub(fixed[FIX_EXPECTED + 3])),
    );
    for (actual, expected) in [
        (
            current[BASE_DOCUMENT],
            fixed[FIX_TEMPORAL_DOCUMENT]
                .add(fixed[FIX_TEMPORAL_CRL_DOCUMENT].mul(current[BASE_CERT2_ACTIVE])),
        ),
        (current[BASE_PARENT], fixed[FIX_TEMPORAL_ROLE]),
        (current[BASE_ENDPOINT_ROLE], fixed[FIX_TEMPORAL_INSTANCE]),
    ] {
        residues.push(
            current[BASE_NUMERIC_QUERY]
                .mul(fixed[ZkX509Rfc5280StarkFamilyV1::Relation as usize])
                .mul(actual.sub(expected)),
        );
    }
    for column in [
        BASE_ACTIVE,
        BASE_A,
        BASE_B,
        BASE_C,
        BASE_ROLE,
        BASE_INSTANCE,
        BASE_STRICT,
        BASE_INVERSE,
    ] {
        residues.push(fixed[FIX_RELATION_CONTINUE].mul(next[column].sub(current[column])));
    }
    let range = active.mul(fixed[ZkX509Rfc5280StarkFamilyV1::RangeByte as usize]);
    for (actual, expected) in [
        (current[BASE_ROLE], fixed[FIX_EXPECTED]),
        (current[BASE_INSTANCE], fixed[FIX_EXPECTED + 1]),
        (current[BASE_OFFSET], fixed[FIX_EXPECTED + 3]),
    ] {
        residues.push(range.mul(actual.sub(expected)));
    }
    residues.push(
        fixed[FIX_RANGE_CONTINUE].mul(next[BASE_STATE_BEFORE].sub(current[BASE_STATE_AFTER])),
    );
    residues.push(fixed[FIX_RANGE_LEADING_ZERO].mul(current[BASE_VALUE]));
    for bit in 6..8 {
        residues.push(fixed[FIX_RANGE_HIGH_BITS_ZERO].mul(current[BASE_BYTE_BITS + bit]));
    }
    let count = current[BASE_ENTRY_CENSUS];
    let delta = fixed[FIX_NUMERIC_ENTRY_COUNT].mul(active);
    residues.push(fixed[FIX_GLOBAL_FIRST].mul(count));
    residues.push(fixed[FIX_CONTINUE].mul(next[BASE_ENTRY_CENSUS].sub(count).sub(delta)));
    residues.push(fixed[FIX_GLOBAL_LAST].mul(count.add(delta).sub(current[BASE_ENTRY_COUNT])));
    residues.push(fixed[FIX_CONTINUE].mul(next[BASE_ENTRY_COUNT].sub(current[BASE_ENTRY_COUNT])));
    debug_assert_eq!(residues.len() - start, RESIDUES_V1);
}

pub(super) const RESIDUES_V1: usize = 85;

// The caller admits a digit row from one of the two closed time templates,
// so offset < length and both operands are canonical. The selected template
// can depend on the private DER time tag even at a fixed public position.
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn populate_decimal_inverses_v1(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    template: temporal::DecimalTemplateV1,
) {
    row[BASE_INVERSE] = F(template.offset).inverse_or_zero_canonical_v1();
    row[BASE_G] = F(template.length - template.offset - 1).inverse_or_zero_canonical_v1();
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn append_temporal_rows_v1(
    trace: &ZkX509Rfc5280TraceV1,
    family_rows: &mut [PrivateTableV1<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let decimal_family = ZkX509Rfc5280StarkFamilyV1::Decimal as usize;
    let calendar_family = ZkX509Rfc5280StarkFamilyV1::Calendar as usize;
    for (family, capacity) in [
        (decimal_family, FIXED_DECIMAL_ROWS_V1),
        (calendar_family, FIXED_CALENDAR_ROWS_V1),
    ] {
        family_rows[family]
            .try_reserve_exact(capacity)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    let cert2 = F(u64::from(trace.certificates.len() == 3));
    for slot_index in 0..numeric::TEMPORAL_SLOTS_V1 {
        let slot = numeric::temporal_slot_v1(slot_index).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
        let identity = slot.identity_v1(cert2);
        let mut nodes = canonical_time_nodes_v1(trace).filter(|node| {
            F(u64::from(node.document)) == identity[0]
                && F(node.role as u64) == identity[1]
                && F(u64::from(node.role_instance)) == identity[2]
        });
        let Some(node) = nodes.next() else {
            for _ in 0..numeric::DECIMAL_ROWS_PER_TIME_V1 {
                family_rows[decimal_family].push([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            }
            for _ in 0..numeric::CALENDAR_PHASES_V1 {
                family_rows[calendar_family].push([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            }
            continue;
        };
        if nodes.next().is_some() {
            return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
        }
        let cells = source_slice_v1(trace, node, true)?;
        let operands = calendar_operands_v1(&cells, node.tag_number)?;
        let set_metadata = |row: &mut ZkX509Rfc5280StarkBaseRowV1| {
            row[BASE_DOCUMENT] = identity[0];
            row[BASE_PARENT] = identity[1];
            row[BASE_ENDPOINT_ROLE] = identity[2];
            row[BASE_INSTANCE] = F(slot_index as u64);
            row[BASE_NODE] = F(u64::from(node.node));
            row[BASE_CONTENT_START] = F(u64::from(node.content_start));
            row[BASE_CONTENT_END] = F(u64::from(node.content_end));
            row[BASE_TAG_NUMBER] = F(u64::from(node.tag_number));
            row[CALENDAR_COLUMNS + calendar::GENERALIZED] = F(u64::from(operands.generalized));
        };
        let mut state = 0_u64;
        for position in 0..numeric::DECIMAL_ROWS_PER_TIME_V1 {
            let template = temporal::template_v1(position, operands.generalized);
            let value = cells.get(position).map_or(0, |cell| cell.value);
            let mut row = byte_row_v1(
                u64::from(node.document),
                u64::from(node.content_start) + position as u64,
                value,
            );
            set_metadata(&mut row);
            row[BASE_ROLE] = F(template.group);
            row[BASE_OFFSET] = F(template.offset);
            row[BASE_B] = F(template.length);
            row[BASE_H] = F(template.digit);
            row[BASE_EQUAL] = F(template.digit + template.terminator);
            if template.digit == 1 {
                let digit = value
                    .checked_sub(b'0')
                    .filter(|digit| *digit <= 9)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
                if template.offset == 0 {
                    state = 0;
                }
                row[BASE_A] = F(u64::from(digit));
                write_u8_bits_v1(&mut row, BASE_SMALL_BITS, digit);
                row[BASE_STATE_BEFORE] = F(state);
                state = state
                    .checked_mul(10)
                    .and_then(|state| state.checked_add(u64::from(digit)))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
                row[BASE_STATE_AFTER] = F(state);
                row[BASE_IS_WRITE] = F(u64::from(template.offset == 0));
                row[BASE_STRICT] = F(u64::from(template.offset + 1 == template.length));
                populate_decimal_inverses_v1(&mut row, template);
            }
            family_rows[decimal_family].push(row);
        }
        let mut calendar_row = calendar_row_v1(operands)?;
        set_metadata(&mut calendar_row);
        for _ in 0..numeric::CALENDAR_PHASES_V1 {
            family_rows[calendar_family].push(calendar_row);
        }
    }
    Ok(())
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn append_relation_rows_v1(
    trace: &ZkX509Rfc5280TraceV1,
    semantic: &ZkX509Rfc5280SemanticWitnessV1,
    family_rows: &mut [PrivateTableV1<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let relation_family = ZkX509Rfc5280StarkFamilyV1::Relation as usize;
    let range_family = ZkX509Rfc5280StarkFamilyV1::RangeByte as usize;
    for (family, capacity) in [
        (relation_family, FIXED_RELATION_ROWS_V1),
        (range_family, FIXED_RANGE_ROWS_V1),
    ] {
        family_rows[family]
            .try_reserve_exact(capacity)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    let cert2 = F(u64::from(trace.certificates.len() == 3));
    for index in 0..numeric::RELATION_SLOTS_V1 {
        let slot = numeric::relation_slot_v1(index).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
        let mut matching = semantic.numeric_relations.iter().filter(|relation| {
            relation.relation == slot.relation && relation.instance == slot.instance
        });
        let Some(relation) = matching.next() else {
            for _ in 0..numeric::RELATION_PHASES_V1 {
                family_rows[relation_family].push([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            }
            for _ in 0..numeric::RANGE_BYTES_PER_RELATION_V1 {
                family_rows[range_family].push([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
            }
            continue;
        };
        if matching.next().is_some() || relation.slack >= 1 << numeric::SLACK_BITS_V1 {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        for (operand, value) in [(slot.left, relation.left), (slot.right, relation.right)] {
            let mut row = active_zero_row_v1();
            row[BASE_A] = F(relation.left);
            row[BASE_B] = F(relation.right);
            row[BASE_C] = F(relation.slack);
            row[BASE_ROLE] = F(u64::from(relation.relation));
            row[BASE_INSTANCE] = F(u64::from(relation.instance));
            row[BASE_STRICT] = F(u64::from(relation.strict));
            row[BASE_INVERSE] = if relation.strict {
                F(relation.slack)
                    .inv()
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
            } else {
                F::ZERO
            };
            row[BASE_G] = F(value);
            if let numeric::NumericOperandV1::Time { slot, add_seconds } = operand {
                let identity = slot.identity_v1(cert2);
                row[BASE_DOCUMENT] = identity[0];
                row[BASE_PARENT] = identity[1];
                row[BASE_ENDPOINT_ROLE] = identity[2];
                row[BASE_G] = F(value
                    .checked_sub(u64::from(add_seconds))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?);
            }
            family_rows[relation_family].push(row);
        }
        let mut state = 0_u64;
        for (offset, value) in relation.slack.to_be_bytes().into_iter().enumerate() {
            let mut row = byte_row_v1(0, 0, value);
            row[BASE_ROLE] = F(u64::from(relation.relation));
            row[BASE_INSTANCE] = F(u64::from(relation.instance));
            row[BASE_OFFSET] = F(offset as u64);
            row[BASE_STATE_BEFORE] = F(state);
            state = state * 256 + u64::from(value);
            row[BASE_STATE_AFTER] = F(state);
            family_rows[range_family].push(row);
        }
    }
    Ok(())
}

#[cfg(test)]
mod private_inverse_tests {
    use super::*;

    fn time_cells(bytes: &[u8]) -> Vec<ZkX509Rfc5280SourceCellV1> {
        bytes
            .iter()
            .copied()
            .enumerate()
            .map(|(address, value)| ZkX509Rfc5280SourceCellV1 {
                document: 0,
                address: u16::try_from(address).expect("bounded time"),
                value,
            })
            .collect()
    }

    #[test]
    fn private_decimal_inverses_cover_both_admitted_time_templates() {
        for (tag, bytes) in [
            (23, b"700101000000Z".as_slice()),
            (23, b"491231235959Z".as_slice()),
            (24, b"20500101000000Z".as_slice()),
            (24, b"99991231235959Z".as_slice()),
        ] {
            let operands = calendar_operands_v1(&time_cells(bytes), tag).expect("canonical time");
            let mut saw_offset_zero = false;
            let mut saw_remaining_zero = false;
            let mut saw_both_nonzero = false;
            for position in 0..numeric::DECIMAL_ROWS_PER_TIME_V1 {
                let template = temporal::template_v1(position, operands.generalized);
                if template.digit == 0 {
                    continue;
                }
                let mut row = [F(37); ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
                let mut expected = row;
                let remaining = template.length - template.offset - 1;
                expected[BASE_INVERSE] = F(template.offset).inv().unwrap_or(F::ZERO);
                expected[BASE_G] = F(remaining).inv().unwrap_or(F::ZERO);
                populate_decimal_inverses_v1(&mut row, template);
                assert_eq!(row, expected, "tag {tag}, position {position}");
                saw_offset_zero |= template.offset == 0;
                saw_remaining_zero |= remaining == 0;
                saw_both_nonzero |= template.offset != 0 && remaining != 0;
            }
            assert!(saw_offset_zero && saw_remaining_zero);
            assert_eq!(saw_both_nonzero, operands.generalized);
        }
        for (tag, bytes) in [
            (23, b"690101000000Z".as_slice()),
            (24, b"20491231235959Z".as_slice()),
            (24, b"20500230000000Z".as_slice()),
            (24, b"20500101000000+".as_slice()),
            (23, b"70010100000Z".as_slice()),
            (25, b"20500101000000Z".as_slice()),
        ] {
            assert!(matches!(
                calendar_operands_v1(&time_cells(bytes), tag),
                Err(ZkX509Rfc5280StarkErrorV1::Semantic)
            ));
        }
    }

    #[test]
    fn actual_temporal_constructor_preserves_zero_and_nonzero_inverse_rows() {
        let trace = super::super::tests::canonical_trace_v1();
        let mut rows =
            core::array::from_fn(|_| PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1));
        append_temporal_rows_v1(&trace, &mut rows).expect("original trace time rows");
        let mut zeros = 0;
        let mut nonzeros = 0;
        for row in &*rows[ZkX509Rfc5280StarkFamilyV1::Decimal as usize] {
            if row[BASE_H] == F::ONE {
                assert_eq!(row[BASE_INVERSE], row[BASE_OFFSET].inv().unwrap_or(F::ZERO));
                assert_eq!(
                    row[BASE_G],
                    row[BASE_B]
                        .sub(row[BASE_OFFSET])
                        .sub(F::ONE)
                        .inv()
                        .unwrap_or(F::ZERO)
                );
                zeros += usize::from(row[BASE_INVERSE] == F::ZERO);
                nonzeros += usize::from(row[BASE_INVERSE] != F::ZERO);
            } else {
                assert_eq!((row[BASE_INVERSE], row[BASE_G]), (F::ZERO, F::ZERO));
            }
        }
        assert!(zeros > 0 && nonzeros > 0);
        let mut malformed = trace.clone();
        let node = malformed
            .semantic_provenance
            .iter_mut()
            .flat_map(|document| document.nodes.iter_mut())
            .find(|node| node.role == ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore)
            .expect("original time node");
        node.tag_number = 25;
        let mut rejected_rows =
            core::array::from_fn(|_| PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1));
        assert!(matches!(
            append_temporal_rows_v1(&malformed, &mut rejected_rows),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        ));
    }

    #[test]
    fn private_time_role_inverses_preserve_original_zero_cells_and_row_ownership() {
        // The source role is a bounded grammar enum. Include all byte values so
        // this also covers values outside the admitted role census.
        for role in 0_u64..=255 {
            let mut row = [F(7); ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
            row[BASE_ROLE] = F(role);
            let mut expected = row;
            for (index, time_role) in TIME_ROLES.into_iter().enumerate() {
                let difference = F(role).sub(F(time_role as u64));
                expected[NODE_TIME_FLAGS + index] = F(u64::from(difference == F::ZERO));
                expected[NODE_TIME_INVERSES + index] = difference.inv().unwrap_or(F::ZERO);
            }
            populate_node_classification_v1(&mut row);
            assert_eq!(row, expected, "role={role}");
        }
    }
}
