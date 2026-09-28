//! Clearing owners for RFC semantic operands and temporary byte projections.

use super::*;

pub(super) fn zeroize_source_cells_v1(cells: &mut [ZkX509Rfc5280SourceCellV1]) {
    for cell in cells {
        zeroize_words_v1(core::slice::from_mut(&mut cell.document));
        zeroize_words_v1(core::slice::from_mut(&mut cell.address));
        zeroize_words_v1(core::slice::from_mut(&mut cell.value));
    }
}

pub(super) fn zeroize_fixed_bytes_v1(rows: &mut [ZkX509Rfc5280FixedByteV1]) {
    for row in rows {
        zeroize_source_cells_v1(core::slice::from_mut(&mut row.source));
        row.source_node.zeroize_private_v1();
        zeroize_words_v1(core::slice::from_mut(&mut row.expected));
        zeroize_words_v1(core::slice::from_mut(&mut row.purpose));
        zeroize_words_v1(core::slice::from_mut(&mut row.instance));
        zeroize_words_v1(core::slice::from_mut(&mut row.variant));
        zeroize_words_v1(core::slice::from_mut(&mut row.offset));
        zeroize_words_v1(core::slice::from_mut(&mut row.length));
    }
}

pub(super) fn zeroize_equal_bytes_v1(rows: &mut [ZkX509Rfc5280EqualByteV1]) {
    for row in rows {
        zeroize_source_cells_v1(core::slice::from_mut(&mut row.left));
        zeroize_source_cells_v1(core::slice::from_mut(&mut row.right));
        zeroize_words_v1(core::slice::from_mut(&mut row.purpose));
        zeroize_words_v1(core::slice::from_mut(&mut row.instance));
        zeroize_words_v1(core::slice::from_mut(&mut row.offset));
    }
}

pub(super) fn zeroize_numeric_relations_v1(rows: &mut [ZkX509Rfc5280NumericRelationV1]) {
    for row in rows {
        zeroize_words_v1(core::slice::from_mut(&mut row.relation));
        zeroize_words_v1(core::slice::from_mut(&mut row.instance));
        zeroize_words_v1(core::slice::from_mut(&mut row.left));
        zeroize_words_v1(core::slice::from_mut(&mut row.right));
        zeroize_words_v1(core::slice::from_mut(&mut row.slack));
        zeroize_words_v1(core::slice::from_mut(&mut row.strict));
    }
}

pub(super) fn zeroize_source_multiplicities_v1(rows: &mut [ZkX509Rfc5280SourceMultiplicityV1]) {
    for row in rows {
        zeroize_source_cells_v1(core::slice::from_mut(&mut row.source));
        zeroize_words_v1(core::slice::from_mut(&mut row.required_multiplicity));
    }
}

pub(super) fn zeroize_node_multiplicities_v1(rows: &mut [ZkX509Rfc5280SerialNodeMultiplicityV1]) {
    for row in rows {
        zeroize_words_v1(core::slice::from_mut(&mut row.document));
        zeroize_words_v1(core::slice::from_mut(&mut row.node));
        zeroize_words_v1(core::slice::from_mut(&mut row.required_multiplicity));
    }
}

/// Reserve through a clearing replacement, never by reallocating private cells.
/// The profile's complete native domain also bounds every semantic family.
pub(super) fn reserve_private_semantic_v1<T: Copy>(
    values: &mut Vec<T>,
    additional: usize,
    erase: fn(&mut [T]),
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let length = values
        .len()
        .checked_add(additional)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    if length > ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    if length <= values.capacity() {
        return Ok(());
    }
    let capacity = length
        .checked_next_power_of_two()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    let mut replacement = PrivateTableV1::new(Vec::new(), erase);
    replacement
        .try_reserve_exact(capacity)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    replacement.extend_from_slice(values);
    erase(values);
    *values = replacement.into_vec();
    Ok(())
}

pub(super) fn private_bytes_v1(
    source: impl ExactSizeIterator<Item = u8>,
) -> Result<PrivateTableV1<u8>, ZkX509Rfc5280StarkErrorV1> {
    let mut bytes = PrivateTableV1::new(Vec::new(), zeroize_words_v1);
    bytes
        .try_reserve_exact(source.len())
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    bytes.extend(source);
    Ok(bytes)
}

impl Drop for ZkX509Rfc5280SerialComparisonV1 {
    fn drop(&mut self) {
        zeroize_words_v1(core::slice::from_mut(&mut self.left_instance));
        zeroize_words_v1(core::slice::from_mut(&mut self.right_instance));
        zeroize_words_v1(&mut self.left);
        zeroize_words_v1(&mut self.right);
    }
}

impl Drop for ZkX509Rfc5280SerialSourceV1 {
    fn drop(&mut self) {
        zeroize_words_v1(core::slice::from_mut(&mut self.logical_id));
        self.node.zeroize_private_v1();
        zeroize_words_v1(&mut self.frame);
        zeroize_source_cells_v1(&mut self.encoded_contents);
    }
}

impl Drop for ZkX509Rfc5280SemanticWitnessV1 {
    fn drop(&mut self) {
        zeroize_fixed_bytes_v1(&mut self.fixed_bytes);
        zeroize_equal_bytes_v1(&mut self.equal_bytes);
        zeroize_source_cells_v1(&mut self.decimal_cells);
        zeroize_words_v1(&mut self.calendar_values);
        zeroize_numeric_relations_v1(&mut self.numeric_relations);
        for (purpose, instance, actual, expected) in &mut self.bit_flags {
            zeroize_words_v1(core::slice::from_mut(purpose));
            zeroize_words_v1(core::slice::from_mut(instance));
            zeroize_words_v1(core::slice::from_mut(actual));
            zeroize_words_v1(core::slice::from_mut(expected));
        }
        // Serial vectors own their nested cells through each element's Drop.
    }
}

impl core::fmt::Debug for ZkX509Rfc5280SerialComparisonV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509Rfc5280SerialComparisonV1 { <private material redacted> }")
    }
}

impl core::fmt::Debug for ZkX509Rfc5280SerialSourceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509Rfc5280SerialSourceV1 { <private material redacted> }")
    }
}

impl core::fmt::Debug for ZkX509Rfc5280SemanticWitnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509Rfc5280SemanticWitnessV1 { <private material redacted> }")
    }
}

#[cfg(test)]
#[path = "rfc5280_private_tests.rs"]
mod tests;
