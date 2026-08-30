//! One-pass canonical multiproof emission with a bounded transposition window.
use super::*;
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::{
    CanonicalProofSectionV2, CanonicalProofTreeKindV2, ProverCanonicalProofPlanV2,
};
use core::{array, sync::atomic};
const CANONICAL_LEAF_BYTES_V2: usize = 6_080;
const CANONICAL_COLUMNS_V2: usize = 380;
const CANONICAL_MAX_OPENED_V2: usize = 320;
const CANONICAL_MAX_HEIGHT_V2: usize = 20;
const CANONICAL_MAX_WINDOW_BYTES_V2: usize = 6_225_920;
const CANONICAL_MAX_AUTH_HEAP_BYTES_V2: usize = 108_544;
const CANONICAL_MAX_CHUNK_BYTES_V2: usize = 16_384;
const CANONICAL_REPLAY_PEAK_HEAP_BYTES_V2: usize = 6_350_848;
#[derive(Clone, Copy)]
struct SelectedIndicesV2 {
    values: [u32; CANONICAL_MAX_OPENED_V2],
    len: usize,
}
#[derive(Clone, Copy)]
struct ProofFrontierNodeV2 {
    digest: [u8; 32],
    selected: bool,
}
const EMPTY_PROOF_NODE_V2: ProofFrontierNodeV2 = ProofFrontierNodeV2 {
    digest: [0; 32],
    selected: false,
};
struct ZeroizingCanonicalWindowV2 {
    bytes: Vec<u8>,
}
impl ZeroizingCanonicalWindowV2 {
    fn new_v2(values_per_block: u16) -> Result<Self, ProverPrerequisiteErrorV2> {
        let bytes_len = usize::from(values_per_block)
            .checked_mul(CANONICAL_LEAF_BYTES_V2)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        if bytes_len == 0 || bytes_len > CANONICAL_MAX_WINDOW_BYTES_V2 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(bytes_len)
            .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
        if bytes.capacity() != bytes_len {
            return Err(ProverPrerequisiteErrorV2::Allocation);
        }
        bytes.resize(bytes_len, 0);
        Ok(Self { bytes })
    }
    fn scatter_column_v2(
        &mut self,
        column: usize,
        chunk: &[u8],
        values_per_block: u16,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        if column >= CANONICAL_COLUMNS_V2 || chunk.len() != usize::from(values_per_block) * 16 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let modulus = RELEASE_MODULI_V1[column / 10];
        for (lane, value) in chunk.chunks_exact(16).enumerate() {
            let c0 = u64::from_be_bytes(
                value[..8]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidCanonicalProof)?,
            );
            let c1 = u64::from_be_bytes(
                value[8..]
                    .try_into()
                    .map_err(|_| ProverPrerequisiteErrorV2::InvalidCanonicalProof)?,
            );
            if c0 >= modulus || c1 >= modulus {
                return Err(ProverPrerequisiteErrorV2::NonCanonicalResidue);
            }
            let start = lane * CANONICAL_LEAF_BYTES_V2 + column * 16;
            self.bytes[start..start + 16].copy_from_slice(value);
        }
        Ok(())
    }
    fn leaf_v2(&self, lane: usize) -> Result<&[u8], ProverPrerequisiteErrorV2> {
        let start = lane
            .checked_mul(CANONICAL_LEAF_BYTES_V2)
            .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
        self.bytes
            .get(start..start + CANONICAL_LEAF_BYTES_V2)
            .ok_or(ProverPrerequisiteErrorV2::InvalidCanonicalProof)
    }
}
impl Drop for ZeroizingCanonicalWindowV2 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        atomic::compiler_fence(atomic::Ordering::SeqCst);
    }
}
struct AuthenticationBucketsV2 {
    nodes: [Vec<[u8; 32]>; CANONICAL_MAX_HEIGHT_V2],
    expected: [usize; CANONICAL_MAX_HEIGHT_V2],
}
impl AuthenticationBucketsV2 {
    fn new_v2(
        expected: [usize; CANONICAL_MAX_HEIGHT_V2],
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let mut nodes: [Vec<[u8; 32]>; CANONICAL_MAX_HEIGHT_V2] = array::from_fn(|_| Vec::new());
        for (bucket, count) in nodes.iter_mut().zip(expected) {
            bucket
                .try_reserve_exact(count)
                .map_err(|_| ProverPrerequisiteErrorV2::Allocation)?;
            if bucket.capacity() != count {
                return Err(ProverPrerequisiteErrorV2::Allocation);
            }
        }
        Ok(Self { nodes, expected })
    }
    fn push_v2(
        &mut self,
        height: usize,
        digest: [u8; 32],
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let bucket = self
            .nodes
            .get_mut(height)
            .ok_or(ProverPrerequisiteErrorV2::InvalidCanonicalProof)?;
        if bucket.len() >= self.expected[height] {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        bucket.push(digest);
        Ok(())
    }
    fn validate_v2(&self) -> Result<(), ProverPrerequisiteErrorV2> {
        if self
            .nodes
            .iter()
            .zip(self.expected)
            .any(|(bucket, expected)| bucket.len() != expected)
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        Ok(())
    }
}
struct CanonicalMerkleFrontierV2 {
    nodes: [ProofFrontierNodeV2; CANONICAL_MAX_HEIGHT_V2],
    occupied: u32,
    leaves: usize,
    parameter_digest: [u8; 32],
    section: CanonicalProofSectionV2,
    authentication: AuthenticationBucketsV2,
}
fn selected_indices_v2(
    queries: &[u32; 160],
    length: u32,
) -> Result<SelectedIndicesV2, ProverPrerequisiteErrorV2> {
    if length < 2 || !length.is_power_of_two() {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let half = length / 2;
    let mut result = SelectedIndicesV2 {
        values: [0; CANONICAL_MAX_OPENED_V2],
        len: CANONICAL_MAX_OPENED_V2,
    };
    for (ordinal, query) in queries.iter().copied().enumerate() {
        let base = query % half;
        result.values[2 * ordinal] = base;
        result.values[2 * ordinal + 1] = base + half;
    }
    result.values.sort_unstable();
    let mut unique = 0;
    for index in 0..result.len {
        if unique == 0 || result.values[index] != result.values[unique - 1] {
            result.values[unique] = result.values[index];
            unique += 1;
        }
    }
    result.len = unique;
    Ok(result)
}
fn authentication_counts_v2(
    selected: &SelectedIndicesV2,
    mut length: u32,
) -> Result<[usize; CANONICAL_MAX_HEIGHT_V2], ProverPrerequisiteErrorV2> {
    let mut counts = [0_usize; CANONICAL_MAX_HEIGHT_V2];
    let mut current = selected.values;
    let mut current_len = selected.len;
    let mut height = 0;
    while length > 1 {
        let mut parents = [0_u32; CANONICAL_MAX_OPENED_V2];
        let mut parent_len = 0;
        for position in 0..current_len {
            let index = current[position];
            if current[..current_len].binary_search(&(index ^ 1)).is_err() {
                counts[height] = counts[height]
                    .checked_add(1)
                    .ok_or(ProverPrerequisiteErrorV2::ArithmeticOverflow)?;
            }
            let parent = index / 2;
            if parent_len == 0 || parents[parent_len - 1] != parent {
                parents[parent_len] = parent;
                parent_len += 1;
            }
        }
        current = parents;
        current_len = parent_len;
        length /= 2;
        height += 1;
    }
    Ok(counts)
}
fn canonical_leaf_hash_v2(
    parameter_digest: [u8; 32],
    section: CanonicalProofSectionV2,
    values: &[u8],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if values.len() != CANONICAL_LEAF_BYTES_V2 {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[section.kind_v2() as u8, section.merkle_layer_v2()]);
    hash.update(&section.length_v2().to_be_bytes());
    hash.update(&(CANONICAL_COLUMNS_V2 as u16).to_be_bytes());
    hash.update(values);
    Ok(hash.finalize())
}
fn canonical_node_hash_v2(
    parameter_digest: [u8; 32],
    section: CanonicalProofSectionV2,
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&parameter_digest);
    hash.update(&[
        section.kind_v2() as u8,
        section.merkle_layer_v2(),
        u8::try_from(height).map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?,
    ]);
    hash.update(&left);
    hash.update(&right);
    Ok(hash.finalize())
}
impl CanonicalMerkleFrontierV2 {
    fn new_v2(
        parameter_digest: [u8; 32],
        section: CanonicalProofSectionV2,
        authentication: AuthenticationBucketsV2,
    ) -> Self {
        Self {
            nodes: [EMPTY_PROOF_NODE_V2; CANONICAL_MAX_HEIGHT_V2],
            occupied: 0,
            leaves: 0,
            parameter_digest,
            section,
            authentication,
        }
    }
    fn push_v2(
        &mut self,
        digest: [u8; 32],
        selected: bool,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        let mut node = ProofFrontierNodeV2 { digest, selected };
        let mut height = 0;
        let mut prior = self.leaves;
        while prior & 1 == 1 {
            let left = self.nodes[height];
            if left.selected != node.selected {
                self.authentication.push_v2(
                    height,
                    if left.selected {
                        node.digest
                    } else {
                        left.digest
                    },
                )?;
            }
            node = ProofFrontierNodeV2 {
                digest: canonical_node_hash_v2(
                    self.parameter_digest,
                    self.section,
                    height + 1,
                    left.digest,
                    node.digest,
                )?,
                selected: left.selected || node.selected,
            };
            self.nodes[height] = EMPTY_PROOF_NODE_V2;
            self.occupied &= !(1_u32 << height);
            prior >>= 1;
            height += 1;
        }
        if height >= self.nodes.len() {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        self.nodes[height] = node;
        self.occupied |= 1_u32 << height;
        self.leaves += 1;
        Ok(())
    }
    fn finish_v2(
        self,
        expected_root: [u8; 32],
    ) -> Result<AuthenticationBucketsV2, ProverPrerequisiteErrorV2> {
        let length = self.section.length_v2() as usize;
        let height = length.ilog2() as usize;
        if self.leaves != length
            || self.occupied != 1_u32 << height
            || self.nodes[height].digest != expected_root
            || !self.nodes[height].selected
        {
            return Err(ProverPrerequisiteErrorV2::InvalidMerkleRoot);
        }
        self.authentication.validate_v2()?;
        Ok(self.authentication)
    }
}
fn emit_merkle_section_with_queries_v2<R, S>(
    mut replay: R,
    queries: &[u32; 160],
    section: CanonicalProofSectionV2,
    parameter_digest: [u8; 32],
    expected_root: [u8; 32],
    writer: &mut CanonicalProofSinkWriterV2<S>,
) -> Result<R::Owner, ProverPrerequisiteErrorV2>
where
    R: CanonicalTreeReplayV2,
    S: BatchFriCanonicalProofSinkV2,
{
    let shape = replay.shape_v2()?;
    if shape.length != section.length_v2()
        || shape.columns != CANONICAL_COLUMNS_V2 as u16
        || shape.values_per_block == 0
        || shape.length % u32::from(shape.values_per_block) != 0
    {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let selected = selected_indices_v2(queries, shape.length)?;
    let authentication_counts = authentication_counts_v2(&selected, shape.length)?;
    let authentication_count: usize = authentication_counts.iter().sum();
    if selected.len != section.opened_v2() as usize
        || authentication_count != section.authentication_v2() as usize
    {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut header = [0_u8; 8];
    header[..4].copy_from_slice(&section.opened_v2().to_be_bytes());
    header[4..].copy_from_slice(&section.authentication_v2().to_be_bytes());
    writer.write_v2(&header)?;
    let authentication = AuthenticationBucketsV2::new_v2(authentication_counts)?;
    let mut frontier = CanonicalMerkleFrontierV2::new_v2(parameter_digest, section, authentication);
    let mut window = ZeroizingCanonicalWindowV2::new_v2(shape.values_per_block)?;
    let blocks = shape.length / u32::from(shape.values_per_block);
    let mut opened_cursor = 0;
    let mut leaf_index = 0_u32;
    for _block in 0..blocks {
        for column in 0..CANONICAL_COLUMNS_V2 {
            let chunk = replay.read_next_column_v2()?;
            window.scatter_column_v2(column, chunk.bytes_v2(), shape.values_per_block)?;
        }
        for lane in 0..usize::from(shape.values_per_block) {
            let values = window.leaf_v2(lane)?;
            let is_selected =
                opened_cursor < selected.len && selected.values[opened_cursor] == leaf_index;
            if is_selected {
                writer.write_v2(values)?;
                opened_cursor += 1;
            }
            frontier.push_v2(
                canonical_leaf_hash_v2(parameter_digest, section, values)?,
                is_selected,
            )?;
            leaf_index += 1;
        }
    }
    if opened_cursor != selected.len || leaf_index != shape.length {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let authentication = frontier.finish_v2(expected_root)?;
    for bucket in authentication.nodes {
        for digest in bucket {
            writer.write_v2(&digest)?;
        }
    }
    replay.complete_v2()
}
pub(super) fn emit_merkle_section_v2<R, S>(
    replay: R,
    plan: &ProverCanonicalProofPlanV2,
    section: CanonicalProofSectionV2,
    parameter_digest: [u8; 32],
    expected_root: [u8; 32],
    writer: &mut CanonicalProofSinkWriterV2<S>,
) -> Result<R::Owner, ProverPrerequisiteErrorV2>
where
    R: CanonicalTreeReplayV2,
    S: BatchFriCanonicalProofSinkV2,
{
    emit_merkle_section_with_queries_v2(
        replay,
        plan.queries_v2(),
        section,
        parameter_digest,
        expected_root,
        writer,
    )
}
const _: () = {
    assert!(CANONICAL_MAX_WINDOW_BYTES_V2 == 1_024 * CANONICAL_LEAF_BYTES_V2);
    assert!(CANONICAL_MAX_AUTH_HEAP_BYTES_V2 == 3_392 * 32);
    assert!(
        CANONICAL_REPLAY_PEAK_HEAP_BYTES_V2
            == CANONICAL_MAX_WINDOW_BYTES_V2
                + CANONICAL_MAX_AUTH_HEAP_BYTES_V2
                + CANONICAL_MAX_CHUNK_BYTES_V2
    );
};
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::confidential_spool::ConfidentialSpoolChunkV1;
    struct VecSinkV2 {
        bytes: Vec<u8>,
        expected: usize,
    }
    impl BatchFriCanonicalProofSinkV2 for VecSinkV2 {
        type Output = Vec<u8>;
        fn begin_exact_v2(&mut self, exact_bytes: usize) -> Result<(), ProverPrerequisiteErrorV2> {
            self.expected = exact_bytes;
            Ok(())
        }
        fn write_next_v2(&mut self, bytes: &[u8]) -> Result<(), ProverPrerequisiteErrorV2> {
            self.bytes.extend_from_slice(bytes);
            Ok(())
        }
        fn finish_exact_v2(self) -> Result<Self::Output, ProverPrerequisiteErrorV2> {
            if self.bytes.len() != self.expected {
                return Err(ProverPrerequisiteErrorV2::CanonicalProofSink);
            }
            Ok(self.bytes)
        }
    }
    struct TinyReplayV2 {
        next: u16,
        noncanonical: bool,
        malformed_chunk: bool,
    }
    impl CanonicalTreeReplayV2 for TinyReplayV2 {
        type Owner = ();
        fn shape_v2(&self) -> Result<CanonicalTreeReplayShapeV2, ProverPrerequisiteErrorV2> {
            Ok(CanonicalTreeReplayShapeV2 {
                length: 4,
                columns: 380,
                values_per_block: 4,
            })
        }
        fn read_next_column_v2(
            &mut self,
        ) -> Result<AuthenticatedReplayChunkV2, ProverPrerequisiteErrorV2> {
            if self.next >= 380 {
                return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
            }
            let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(64)?;
            if self.malformed_chunk && self.next == 0 {
                chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(48)?;
            }
            if self.noncanonical && self.next == 0 {
                chunk.as_mut_slice_v1()[..8].copy_from_slice(&RELEASE_MODULI_V1[0].to_be_bytes());
            }
            self.next += 1;
            Ok(AuthenticatedReplayChunkV2 { chunk })
        }
        fn complete_v2(self) -> Result<Self::Owner, ProverPrerequisiteErrorV2> {
            if self.next != 380 {
                return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
            }
            Ok(())
        }
    }
    fn tiny_root_v2(section: CanonicalProofSectionV2) -> [u8; 32] {
        let leaf = canonical_leaf_hash_v2([0x11; 32], section, &[0; 6_080]).unwrap();
        let parent = canonical_node_hash_v2([0x11; 32], section, 1, leaf, leaf).unwrap();
        canonical_node_hash_v2([0x11; 32], section, 2, parent, parent).unwrap()
    }
    #[test]
    fn one_pass_emits_sorted_values_then_height_major_minimal_authentication() {
        let section = CanonicalProofSectionV2::test_only_v2(2, 3, 0, 4, 2, 2);
        let exact = 8 + 2 * 6_080 + 2 * 32;
        let sink = VecSinkV2 {
            bytes: Vec::new(),
            expected: 0,
        };
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(sink, exact).unwrap();
        emit_merkle_section_with_queries_v2(
            TinyReplayV2 {
                next: 0,
                noncanonical: false,
                malformed_chunk: false,
            },
            &[0; 160],
            section,
            [0x11; 32],
            tiny_root_v2(section),
            &mut writer,
        )
        .unwrap();
        let bytes = writer.finish_v2().unwrap();
        assert_eq!(&bytes[..4], &2_u32.to_be_bytes());
        assert_eq!(&bytes[4..8], &2_u32.to_be_bytes());
        assert_eq!(bytes.len(), exact);
    }
    #[test]
    fn wrong_root_and_noncanonical_replay_fail_closed() {
        let section = CanonicalProofSectionV2::test_only_v2(2, 3, 0, 4, 2, 2);
        let exact = 8 + 2 * 6_080 + 2 * 32;
        let make_writer = || {
            CanonicalProofSinkWriterV2::begin_v2(
                VecSinkV2 {
                    bytes: Vec::new(),
                    expected: 0,
                },
                exact,
            )
            .unwrap()
        };
        let mut wrong_root = make_writer();
        assert!(
            emit_merkle_section_with_queries_v2(
                TinyReplayV2 {
                    next: 0,
                    noncanonical: false,
                    malformed_chunk: false,
                },
                &[0; 160],
                section,
                [0x11; 32],
                [0x99; 32],
                &mut wrong_root,
            )
            .is_err()
        );
        let mut noncanonical = make_writer();
        assert!(matches!(
            emit_merkle_section_with_queries_v2(
                TinyReplayV2 {
                    next: 0,
                    noncanonical: true,
                    malformed_chunk: false,
                },
                &[0; 160],
                section,
                [0x11; 32],
                tiny_root_v2(section),
                &mut noncanonical,
            ),
            Err(ProverPrerequisiteErrorV2::NonCanonicalResidue)
        ));
    }
    #[test]
    fn hostile_replay_with_malformed_authenticated_chunk_fails_closed() {
        let section = CanonicalProofSectionV2::test_only_v2(2, 3, 0, 4, 2, 2);
        let mut writer = CanonicalProofSinkWriterV2::begin_v2(
            VecSinkV2 {
                bytes: Vec::new(),
                expected: 0,
            },
            8 + 2 * 6_080 + 2 * 32,
        )
        .unwrap();
        assert!(matches!(
            emit_merkle_section_with_queries_v2(
                TinyReplayV2 {
                    next: 0,
                    noncanonical: false,
                    malformed_chunk: true,
                },
                &[0; 160],
                section,
                [0x11; 32],
                tiny_root_v2(section),
                &mut writer,
            ),
            Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof)
        ));
    }
}
