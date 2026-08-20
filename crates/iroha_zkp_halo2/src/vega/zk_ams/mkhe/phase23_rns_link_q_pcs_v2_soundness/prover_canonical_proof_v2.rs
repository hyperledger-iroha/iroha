//! Exact canonical-section plan derived from the terminal-bound query owner.
use super::*;
const CANONICAL_SECTION_SHAPE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.qpcs.canonical-proof-section-shape\0";
const CANONICAL_SECTION_COUNT_V2: usize = 20;
const CANONICAL_INITIAL_LAYER_SENTINEL_V2: u8 = 0xff;
const CANONICAL_KAT_WIRE_BYTES_V2: usize = 27_196_704;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(in super::super) enum CanonicalProofTreeKindV2 {
    Initial = 1,
    OpeningQuotient = 2,
    Fri = 3,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct CanonicalProofSectionV2 {
    ordinal: u8,
    kind: CanonicalProofTreeKindV2,
    layer: u8,
    length: u32,
    opened: u32,
    authentication: u32,
}
impl CanonicalProofSectionV2 {
    #[cfg(test)]
    pub(in super::super) const fn test_only_v2(
        ordinal: u8,
        kind: u8,
        layer: u8,
        length: u32,
        opened: u32,
        authentication: u32,
    ) -> Self {
        let kind = match kind {
            1 => CanonicalProofTreeKindV2::Initial,
            2 => CanonicalProofTreeKindV2::OpeningQuotient,
            3 => CanonicalProofTreeKindV2::Fri,
            _ => panic!("invalid test-only canonical tree kind"),
        };
        section_v2(ordinal, kind, layer, length, opened, authentication)
    }
    pub(in super::super) const fn ordinal_v2(self) -> u8 {
        self.ordinal
    }
    pub(in super::super) const fn kind_v2(self) -> CanonicalProofTreeKindV2 {
        self.kind
    }
    pub(in super::super) const fn layer_v2(self) -> u8 {
        self.layer
    }
    pub(in super::super) const fn merkle_layer_v2(self) -> u8 {
        match self.kind {
            CanonicalProofTreeKindV2::Initial | CanonicalProofTreeKindV2::OpeningQuotient => 0,
            CanonicalProofTreeKindV2::Fri => self.layer,
        }
    }
    pub(in super::super) const fn length_v2(self) -> u32 {
        self.length
    }
    pub(in super::super) const fn opened_v2(self) -> u32 {
        self.opened
    }
    pub(in super::super) const fn authentication_v2(self) -> u32 {
        self.authentication
    }
}
const fn section_v2(
    ordinal: u8,
    kind: CanonicalProofTreeKindV2,
    layer: u8,
    length: u32,
    opened: u32,
    authentication: u32,
) -> CanonicalProofSectionV2 {
    CanonicalProofSectionV2 {
        ordinal,
        kind,
        layer,
        length,
        opened,
        authentication,
    }
}
const CANONICAL_SECTION_LAYOUTS_V2: [CanonicalProofSectionV2; CANONICAL_SECTION_COUNT_V2] = [
    section_v2(0, CanonicalProofTreeKindV2::Initial, 0xff, 524_288, 0, 0),
    section_v2(
        1,
        CanonicalProofTreeKindV2::OpeningQuotient,
        0xff,
        524_288,
        0,
        0,
    ),
    section_v2(2, CanonicalProofTreeKindV2::Fri, 0, 524_288, 0, 0),
    section_v2(3, CanonicalProofTreeKindV2::Fri, 1, 262_144, 0, 0),
    section_v2(4, CanonicalProofTreeKindV2::Fri, 2, 131_072, 0, 0),
    section_v2(5, CanonicalProofTreeKindV2::Fri, 3, 65_536, 0, 0),
    section_v2(6, CanonicalProofTreeKindV2::Fri, 4, 32_768, 0, 0),
    section_v2(7, CanonicalProofTreeKindV2::Fri, 5, 16_384, 0, 0),
    section_v2(8, CanonicalProofTreeKindV2::Fri, 6, 8_192, 0, 0),
    section_v2(9, CanonicalProofTreeKindV2::Fri, 7, 4_096, 0, 0),
    section_v2(10, CanonicalProofTreeKindV2::Fri, 8, 2_048, 0, 0),
    section_v2(11, CanonicalProofTreeKindV2::Fri, 9, 1_024, 0, 0),
    section_v2(12, CanonicalProofTreeKindV2::Fri, 10, 512, 0, 0),
    section_v2(13, CanonicalProofTreeKindV2::Fri, 11, 256, 0, 0),
    section_v2(14, CanonicalProofTreeKindV2::Fri, 12, 128, 0, 0),
    section_v2(15, CanonicalProofTreeKindV2::Fri, 13, 64, 0, 0),
    section_v2(16, CanonicalProofTreeKindV2::Fri, 14, 32, 0, 0),
    section_v2(17, CanonicalProofTreeKindV2::Fri, 15, 16, 0, 0),
    section_v2(18, CanonicalProofTreeKindV2::Fri, 16, 8, 0, 0),
    section_v2(19, CanonicalProofTreeKindV2::Fri, 17, 4, 0, 0),
];
struct CanonicalIndexSetV2 {
    values: [u32; 2 * QUERY_COUNT_V2],
    len: usize,
}
/// Move-only proof layout owner. The terminal query owner is consumed to make it.
pub(in super::super) struct ProverCanonicalProofPlanV2 {
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    queries: [u32; QUERY_COUNT_V2],
    sections: [CanonicalProofSectionV2; CANONICAL_SECTION_COUNT_V2],
    query_digest: [u8; 32],
    section_shape_digest: [u8; 32],
    exact_wire_bytes: usize,
}
fn query_digest_v2(queries: &[u32; QUERY_COUNT_V2]) -> [u8; 32] {
    let mut frame = [0_u8; QUERY_COUNT_V2 * 4];
    for (ordinal, query) in queries.iter().enumerate() {
        frame[ordinal * 4..ordinal * 4 + 4].copy_from_slice(&query.to_be_bytes());
    }
    keccak256(&frame)
}
fn canonical_indices_v2(queries: &[u32; QUERY_COUNT_V2], length: u32) -> CanonicalIndexSetV2 {
    let half = length / 2;
    let mut indices = CanonicalIndexSetV2 {
        values: [0; 2 * QUERY_COUNT_V2],
        len: 2 * QUERY_COUNT_V2,
    };
    for (ordinal, query) in queries.iter().copied().enumerate() {
        let base = query % half;
        indices.values[2 * ordinal] = base;
        indices.values[2 * ordinal + 1] = base + half;
    }
    indices.values.sort_unstable();
    let mut unique = 0;
    for position in 0..indices.len {
        if unique == 0 || indices.values[position] != indices.values[unique - 1] {
            indices.values[unique] = indices.values[position];
            unique += 1;
        }
    }
    indices.len = unique;
    indices
}
fn canonical_authentication_count_v2(
    indices: &CanonicalIndexSetV2,
    mut length: u32,
) -> Result<usize, SoundnessErrorV2> {
    let mut current = indices.values;
    let mut current_len = indices.len;
    let mut authentication = 0_usize;
    while length > 1 {
        let mut parents = [0_u32; 2 * QUERY_COUNT_V2];
        let mut parent_len = 0;
        for position in 0..current_len {
            let index = current[position];
            if current[..current_len].binary_search(&(index ^ 1)).is_err() {
                authentication = authentication
                    .checked_add(1)
                    .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
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
    }
    Ok(authentication)
}
fn derive_sections_v2(
    queries: &[u32; QUERY_COUNT_V2],
) -> Result<([CanonicalProofSectionV2; CANONICAL_SECTION_COUNT_V2], usize), SoundnessErrorV2> {
    for (ordinal, query) in queries.iter().copied().enumerate() {
        if query >= (DOMAIN_SIZE_V2 / 2) as u32 || queries[..ordinal].contains(&query) {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
    }
    let mut sections = CANONICAL_SECTION_LAYOUTS_V2;
    let mut fri_opened = 0_usize;
    let mut fri_authentication = 0_usize;
    let mut wire_bytes = FIXED_BEFORE_SECTIONS_V2
        .checked_add(SECTION_COUNT_V2 * SECTION_HEADER_BYTES_V2)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    for section in &mut sections {
        let indices = canonical_indices_v2(queries, section.length);
        let authentication = canonical_authentication_count_v2(&indices, section.length)?;
        section.opened =
            u32::try_from(indices.len).map_err(|_| SoundnessErrorV2::ArithmeticOverflow)?;
        section.authentication =
            u32::try_from(authentication).map_err(|_| SoundnessErrorV2::ArithmeticOverflow)?;
        if authentication > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        if section.ordinal < 2 {
            if indices.len > MAX_INITIAL_OPENED_LEAVES_V2
                || authentication > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2
            {
                return Err(SoundnessErrorV2::InvalidSectionCount);
            }
        } else {
            fri_opened = fri_opened
                .checked_add(indices.len)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            fri_authentication = fri_authentication
                .checked_add(authentication)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
        }
        wire_bytes = wire_bytes
            .checked_add(
                indices
                    .len
                    .checked_mul(LEAF_BYTES_V2)
                    .and_then(|bytes| {
                        authentication
                            .checked_mul(32)
                            .and_then(|authentication_bytes| {
                                bytes.checked_add(authentication_bytes)
                            })
                    })
                    .ok_or(SoundnessErrorV2::ArithmeticOverflow)?,
            )
            .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    }
    checked_fri_multiproof_bytes_v2(fri_opened, fri_authentication)?;
    if wire_bytes > MAX_PROOF_BYTES_V2 || wire_bytes >= GLOBAL_PROOF_CAP_BYTES_V2 {
        return Err(SoundnessErrorV2::ProofCapExceeded);
    }
    Ok((sections, wire_bytes))
}
fn section_shape_digest_v2(
    sections: &[CanonicalProofSectionV2; CANONICAL_SECTION_COUNT_V2],
) -> Result<[u8; 32], SoundnessErrorV2> {
    let mut frame = FrameV2::<512>::new();
    frame.push(CANONICAL_SECTION_SHAPE_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    for section in sections {
        frame.push(&[section.ordinal, section.kind as u8, section.layer])?;
        frame.push(&section.length.to_be_bytes())?;
        frame.push(&section.opened.to_be_bytes())?;
        frame.push(&section.authentication.to_be_bytes())?;
    }
    Ok(keccak256(frame.bytes()))
}
impl ProverFriQueriesV2 {
    pub(in super::super) fn into_canonical_proof_plan_v2(
        self,
    ) -> Result<ProverCanonicalProofPlanV2, SoundnessErrorV2> {
        if self.transcript == [0; 32]
            || self.batch_schedule_digest == [0; 32]
            || self.fold_schedule_digest == [0; 32]
        {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
        let query_digest = query_digest_v2(&self.queries);
        if query_digest == [0; 32] {
            return Err(SoundnessErrorV2::InvalidChallenge);
        }
        let (sections, exact_wire_bytes) = derive_sections_v2(&self.queries)?;
        Ok(ProverCanonicalProofPlanV2 {
            transcript: self.transcript,
            batch_schedule_digest: self.batch_schedule_digest,
            fold_schedule_digest: self.fold_schedule_digest,
            queries: self.queries,
            sections,
            query_digest,
            section_shape_digest: section_shape_digest_v2(&sections)?,
            exact_wire_bytes,
        })
    }
}
impl ProverCanonicalProofPlanV2 {
    pub(in super::super) const fn queries_v2(&self) -> &[u32; QUERY_COUNT_V2] {
        &self.queries
    }
    pub(in super::super) fn section_v2(
        &self,
        ordinal: usize,
    ) -> Result<CanonicalProofSectionV2, SoundnessErrorV2> {
        self.sections
            .get(ordinal)
            .copied()
            .ok_or(SoundnessErrorV2::InvalidSectionCount)
    }
    pub(in super::super) const fn query_digest_v2(&self) -> [u8; 32] {
        self.query_digest
    }
    pub(in super::super) const fn section_shape_digest_v2(&self) -> [u8; 32] {
        self.section_shape_digest
    }
    pub(in super::super) const fn exact_wire_bytes_v2(&self) -> usize {
        self.exact_wire_bytes
    }
    pub(in super::super) const fn transcript_context_v2(&self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            self.transcript,
            self.batch_schedule_digest,
            self.fold_schedule_digest,
        )
    }
}
const _: () = {
    assert!(CANONICAL_SECTION_COUNT_V2 == SECTION_COUNT_V2);
    assert!(CANONICAL_INITIAL_LAYER_SENTINEL_V2 == 0xff);
    assert!(CANONICAL_KAT_WIRE_BYTES_V2 <= MAX_PROOF_BYTES_V2);
    assert!(CANONICAL_KAT_WIRE_BYTES_V2 < GLOBAL_PROOF_CAP_BYTES_V2);
};
#[cfg(test)]
mod tests {
    use super::*;
    const SHAPE_DIGEST_KAT_V2: [u8; 32] = [
        0x03, 0xb8, 0x27, 0x20, 0x89, 0x43, 0xc7, 0x25, 0xf2, 0x02, 0x34, 0x24, 0x09, 0x0c, 0x5a,
        0x1a, 0x9a, 0x1a, 0xd1, 0x75, 0x20, 0x76, 0x43, 0x87, 0xd2, 0x90, 0xf9, 0xc4, 0x91, 0xa1,
        0xe1, 0x5d,
    ];
    fn kat_sections_v2() -> [CanonicalProofSectionV2; CANONICAL_SECTION_COUNT_V2] {
        let shapes = [
            (320, 3_096),
            (320, 3_096),
            (320, 3_096),
            (320, 2_824),
            (318, 2_484),
            (318, 2_162),
            (316, 1_850),
            (314, 1_532),
            (312, 1_246),
            (298, 934),
            (286, 664),
            (260, 390),
            (230, 194),
            (172, 64),
            (118, 10),
            (64, 0),
            (32, 0),
            (16, 0),
            (8, 0),
            (4, 0),
        ];
        let mut sections = CANONICAL_SECTION_LAYOUTS_V2;
        for (section, (opened, authentication)) in sections.iter_mut().zip(shapes) {
            section.opened = opened;
            section.authentication = authentication;
        }
        sections
    }
    #[test]
    fn exact_correlated_section_table_and_wire_size_are_frozen() {
        let sections = kat_sections_v2();
        assert_eq!(
            section_shape_digest_v2(&sections).unwrap(),
            SHAPE_DIGEST_KAT_V2
        );
        let opened: u32 = sections.iter().map(|shape| shape.opened).sum();
        let authentication: u32 = sections.iter().map(|shape| shape.authentication).sum();
        assert_eq!((opened, authentication), (4_346, 23_642));
        let wire = FIXED_BEFORE_SECTIONS_V2
            + SECTION_COUNT_V2 * SECTION_HEADER_BYTES_V2
            + opened as usize * LEAF_BYTES_V2
            + authentication as usize * 32;
        assert_eq!(wire, CANONICAL_KAT_WIRE_BYTES_V2);
        assert_eq!(sections[0].layer, 0xff);
        assert_eq!(sections[1].layer, 0xff);
        for (layer, shape) in sections[2..].iter().enumerate() {
            assert_eq!(shape.ordinal as usize, layer + 2);
            assert_eq!(shape.layer as usize, layer);
            assert_eq!(shape.length, (DOMAIN_SIZE_V2 >> layer) as u32);
        }
    }
}
