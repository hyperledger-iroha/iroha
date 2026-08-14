//! Move-only prover transcript and fold continuation for FRI layers B1 through B17.
use super::*;
const FIRST_CONTINUATION_LAYER_V2: u8 = 1;
const LAST_CONTINUATION_LAYER_V2: u8 = 17;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct ProverFriRoundContextV2 {
    pub(in super::super) layer: u8,
    pub(in super::super) pre_root_transcript: [u8; 32],
    pub(in super::super) post_root_transcript: [u8; 32],
    pub(in super::super) batch_schedule_digest: [u8; 32],
    pub(in super::super) prior_fold_schedule_digest: [u8; 32],
    pub(in super::super) fold_schedule_digest: [u8; 32],
    pub(in super::super) root: [u8; 32],
}
struct ProverFriRoundsLiveV2 {
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    next_layer: u8,
}
struct ProverFriRoundLiveV2 {
    continuation: ProverFriRoundsLiveV2,
    context: ProverFriRoundContextV2,
    alphas: [BatchValueV2; COORDINATE_COUNT_V2],
}
/// Move-only transcript owner ready to bind the next authenticated FRI root.
pub(in super::super) struct ProverFriRoundsReadyV2 {
    live: Option<ProverFriRoundsLiveV2>,
}
/// Move-only owner of all 380 challenges for one exact FRI round.
pub(in super::super) struct ProverFriRoundChallengesV2 {
    live: Option<ProverFriRoundLiveV2>,
    fields: [BatchFieldV2; LIMBS_V2],
    inverse_layer_roots: [BatchValueV2; LIMBS_V2],
    pair_blocks: u64,
    values_per_half: u16,
    next_pair_block: u64,
    next_column: u16,
}
/// Move-only transcript owner after one complete authenticated FRI fold.
pub(in super::super) struct ProverFriRoundCompleteV2 {
    live: Option<ProverFriRoundsLiveV2>,
    context: ProverFriRoundContextV2,
}
/// Move-only owner of the transcript after the equal terminal leaves are bound.
pub(in super::super) struct ProverFriTerminalBoundV2 {
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
}
/// Move-only owner of the exact 160 unique qPCS query indices.
pub(in super::super) struct ProverFriQueriesV2 {
    pub(super) transcript: [u8; 32],
    pub(super) batch_schedule_digest: [u8; 32],
    pub(super) fold_schedule_digest: [u8; 32],
    pub(super) queries: [u32; QUERY_COUNT_V2],
}
fn round_shape_v2(layer: u8) -> Result<(u64, u16), SoundnessErrorV2> {
    if !(FIRST_CONTINUATION_LAYER_V2..=LAST_CONTINUATION_LAYER_V2).contains(&layer) {
        return Err(SoundnessErrorV2::InvalidFriEquation);
    }
    let source_length = (DOMAIN_SIZE_V2 as u64) >> layer;
    let half = source_length / 2;
    let values_per_half =
        u16::try_from(half.min(1_024)).map_err(|_| SoundnessErrorV2::ArithmeticOverflow)?;
    let pair_blocks = if source_length > 1_024 {
        source_length / 2_048
    } else {
        1
    };
    Ok((pair_blocks, values_per_half))
}
pub(in super::super) fn validate_equal_terminal_v2(
    terminal: &[u8],
) -> Result<(), SoundnessErrorV2> {
    if terminal.len() != TERMINAL_BYTES_V2 {
        return Err(SoundnessErrorV2::InvalidTerminal);
    }
    validate_leaf_values_v2(terminal)?;
    if terminal[..LEAF_BYTES_V2] != terminal[LEAF_BYTES_V2..] {
        return Err(SoundnessErrorV2::InvalidTerminal);
    }
    Ok(())
}
fn bind_round_live_v2(
    continuation: ProverFriRoundsLiveV2,
    root: [u8; 32],
) -> Result<ProverFriRoundLiveV2, SoundnessErrorV2> {
    let layer = continuation.next_layer;
    if root == [0; 32]
        || !(FIRST_CONTINUATION_LAYER_V2..=LAST_CONTINUATION_LAYER_V2).contains(&layer)
    {
        return Err(SoundnessErrorV2::InvalidRoot);
    }
    let pre_root_transcript = continuation.transcript;
    let post_root_transcript =
        absorb_root_v2(FRI_ROOT_DOMAIN_V2, pre_root_transcript, layer, root)?;
    let prior_fold_schedule_digest = continuation.fold_schedule_digest;
    let mut fold_schedule_digest = prior_fold_schedule_digest;
    let mut alphas = [BatchValueV2::ZERO; COORDINATE_COUNT_V2];
    for limb in 0..LIMBS_V2 {
        for row in 0..ROWS_PER_LIMB_V2 {
            let alpha = derive_fq2_challenge_v2(
                FOLD_DOMAIN_V2,
                post_root_transcript,
                limb,
                row,
                0,
                usize::from(layer),
            )?;
            alphas[limb * ROWS_PER_LIMB_V2 + row] = BatchValueV2 {
                c0: alpha.c0,
                c1: alpha.c1,
            };
            fold_schedule_digest = absorb_schedule_value_v2(
                fold_schedule_digest,
                1,
                limb,
                row,
                0,
                usize::from(layer),
                alpha,
            )?;
        }
    }
    let context = ProverFriRoundContextV2 {
        layer,
        pre_root_transcript,
        post_root_transcript,
        batch_schedule_digest: continuation.batch_schedule_digest,
        prior_fold_schedule_digest,
        fold_schedule_digest,
        root,
    };
    Ok(ProverFriRoundLiveV2 {
        continuation: ProverFriRoundsLiveV2 {
            transcript: post_root_transcript,
            batch_schedule_digest: continuation.batch_schedule_digest,
            fold_schedule_digest,
            next_layer: layer + 1,
        },
        context,
        alphas,
    })
}
impl ProverFriLayer0FoldCompleteV2 {
    pub(in super::super) fn begin_fri_rounds_v2(
        self,
    ) -> Result<ProverFriRoundsReadyV2, SoundnessErrorV2> {
        if self.transcript == [0; 32]
            || self.batch_schedule_digest == [0; 32]
            || self.fold_schedule_digest == [0; 32]
            || self.layer0_root == [0; 32]
        {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        Ok(ProverFriRoundsReadyV2 {
            live: Some(ProverFriRoundsLiveV2 {
                transcript: self.transcript,
                batch_schedule_digest: self.batch_schedule_digest,
                fold_schedule_digest: self.fold_schedule_digest,
                next_layer: FIRST_CONTINUATION_LAYER_V2,
            }),
        })
    }
}
impl ProverFriRoundsReadyV2 {
    pub(in super::super) fn bind_next_root_v2(
        mut self,
        root: [u8; 32],
    ) -> Result<ProverFriRoundChallengesV2, SoundnessErrorV2> {
        let live = bind_round_live_v2(self.live.take().ok_or(SoundnessErrorV2::Poisoned)?, root)?;
        let layer = live.context.layer;
        let (pair_blocks, values_per_half) = round_shape_v2(layer)?;
        let first = BatchFieldV2::derive(RELEASE_MODULI_V1[0], DOMAIN_LOG_V2 as usize)
            .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
        let first_layer_root = first.pow(first.domain_root, 1_u128 << layer);
        let mut fields = [first; LIMBS_V2];
        let mut inverse_layer_roots = [first
            .inverse(first_layer_root)
            .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
            LIMBS_V2];
        for limb in 1..LIMBS_V2 {
            let field = BatchFieldV2::derive(RELEASE_MODULI_V1[limb], DOMAIN_LOG_V2 as usize)
                .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
            let layer_root = field.pow(field.domain_root, 1_u128 << layer);
            fields[limb] = field;
            inverse_layer_roots[limb] = field
                .inverse(layer_root)
                .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
        }
        Ok(ProverFriRoundChallengesV2 {
            live: Some(live),
            fields,
            inverse_layer_roots,
            pair_blocks,
            values_per_half,
            next_pair_block: 0,
            next_column: 0,
        })
    }
}
impl ProverFriRoundChallengesV2 {
    pub(in super::super) fn context_v2(&self) -> Result<ProverFriRoundContextV2, SoundnessErrorV2> {
        Ok(self
            .live
            .as_ref()
            .ok_or(SoundnessErrorV2::Poisoned)?
            .context)
    }
    pub(in super::super) const fn pair_blocks_v2(&self) -> u64 {
        self.pair_blocks
    }
    pub(in super::super) const fn values_per_half_v2(&self) -> u16 {
        self.values_per_half
    }
    pub(in super::super) fn fold_next_pair_v2(
        &mut self,
        pair_block: u64,
        column: u16,
        positive: &[u8],
        negative: &[u8],
        output: &mut [u8],
    ) -> Result<(), SoundnessErrorV2> {
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        let expected_bytes = usize::from(self.values_per_half) * FQ2_BYTES_V2;
        if pair_block != self.next_pair_block
            || column != self.next_column
            || pair_block >= self.pair_blocks
            || column >= COORDINATE_COUNT_V2 as u16
            || positive.len() != expected_bytes
            || negative.len() != expected_bytes
            || output.len() != expected_bytes
        {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        let coordinate = usize::from(column);
        let limb = coordinate / ROWS_PER_LIMB_V2;
        let field = self.fields[limb];
        let alpha = live.alphas[coordinate];
        let inverse_two = mod_pow_v1(2, field.modulus - 2, field.modulus);
        let layer_root = field.pow(field.domain_root, 1_u128 << live.context.layer);
        let exponent = u128::from(pair_block) * u128::from(self.values_per_half);
        let mut inverse_x = field
            .inverse(field.pow(layer_root, exponent))
            .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?;
        let inverse_root = self.inverse_layer_roots[limb];
        for ((positive, negative), next) in positive
            .chunks_exact(FQ2_BYTES_V2)
            .zip(negative.chunks_exact(FQ2_BYTES_V2))
            .zip(output.chunks_exact_mut(FQ2_BYTES_V2))
        {
            let decode = |value: &[u8]| -> Result<BatchValueV2, SoundnessErrorV2> {
                let c0 = read_u64_v2(value, 0)?;
                let c1 = read_u64_v2(value, 8)?;
                if c0 >= field.modulus || c1 >= field.modulus {
                    return Err(SoundnessErrorV2::NonCanonicalResidue);
                }
                Ok(BatchValueV2 { c0, c1 })
            };
            let positive = decode(positive)?;
            let negative = decode(negative)?;
            let even = field.scale(field.add(positive, negative), inverse_two);
            let odd = field.mul(
                field.sub(positive, negative),
                field.scale(inverse_x, inverse_two),
            );
            let value = field.add(even, field.mul(alpha, odd));
            next[..8].copy_from_slice(&value.c0.to_be_bytes());
            next[8..].copy_from_slice(&value.c1.to_be_bytes());
            inverse_x = field.mul(inverse_x, inverse_root);
        }
        self.next_column += 1;
        if self.next_column == COORDINATE_COUNT_V2 as u16 {
            self.next_column = 0;
            self.next_pair_block += 1;
        }
        self.live = Some(live);
        Ok(())
    }
    pub(in super::super) fn fold_terminal_column_in_place_v2(
        &mut self,
        column: u16,
        values: &mut [u8],
    ) -> Result<(), SoundnessErrorV2> {
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if live.context.layer != LAST_CONTINUATION_LAYER_V2
            || self.pair_blocks != 1
            || self.values_per_half != 2
            || self.next_pair_block != 0
            || column != self.next_column
            || column >= COORDINATE_COUNT_V2 as u16
            || values.len() != 4 * FQ2_BYTES_V2
        {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        let coordinate = usize::from(column);
        let limb = coordinate / ROWS_PER_LIMB_V2;
        let field = self.fields[limb];
        let alpha = live.alphas[coordinate];
        let inverse_two = mod_pow_v1(2, field.modulus - 2, field.modulus);
        let mut inverse_x = BatchValueV2::ONE;
        let inverse_root = self.inverse_layer_roots[limb];
        for lane in 0..2 {
            let positive_offset = lane * FQ2_BYTES_V2;
            let negative_offset = (lane + 2) * FQ2_BYTES_V2;
            let positive = BatchValueV2 {
                c0: read_u64_v2(values, positive_offset)?,
                c1: read_u64_v2(values, positive_offset + 8)?,
            };
            let negative = BatchValueV2 {
                c0: read_u64_v2(values, negative_offset)?,
                c1: read_u64_v2(values, negative_offset + 8)?,
            };
            if positive.c0 >= field.modulus
                || positive.c1 >= field.modulus
                || negative.c0 >= field.modulus
                || negative.c1 >= field.modulus
            {
                return Err(SoundnessErrorV2::NonCanonicalResidue);
            }
            let even = field.scale(field.add(positive, negative), inverse_two);
            let odd = field.mul(
                field.sub(positive, negative),
                field.scale(inverse_x, inverse_two),
            );
            let value = field.add(even, field.mul(alpha, odd));
            values[positive_offset..positive_offset + 8].copy_from_slice(&value.c0.to_be_bytes());
            values[positive_offset + 8..positive_offset + 16]
                .copy_from_slice(&value.c1.to_be_bytes());
            inverse_x = field.mul(inverse_x, inverse_root);
        }
        self.next_column += 1;
        if self.next_column == COORDINATE_COUNT_V2 as u16 {
            self.next_column = 0;
            self.next_pair_block = 1;
        }
        self.live = Some(live);
        Ok(())
    }
    pub(in super::super) fn complete_v2(
        mut self,
    ) -> Result<ProverFriRoundCompleteV2, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if self.next_pair_block != self.pair_blocks || self.next_column != 0 {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        live.alphas.fill(BatchValueV2::ZERO);
        Ok(ProverFriRoundCompleteV2 {
            context: live.context,
            live: Some(live.continuation),
        })
    }
}
impl ProverFriRoundCompleteV2 {
    pub(in super::super) const fn context_v2(&self) -> ProverFriRoundContextV2 {
        self.context
    }
    pub(in super::super) fn continue_v2(
        mut self,
    ) -> Result<ProverFriRoundsReadyV2, SoundnessErrorV2> {
        if self.context.layer >= LAST_CONTINUATION_LAYER_V2 {
            return Err(SoundnessErrorV2::InvalidFriEquation);
        }
        Ok(ProverFriRoundsReadyV2 {
            live: self.live.take(),
        })
    }
    pub(in super::super) fn bind_terminal_v2(
        mut self,
        terminal: &[u8],
    ) -> Result<ProverFriTerminalBoundV2, SoundnessErrorV2> {
        if self.context.layer != LAST_CONTINUATION_LAYER_V2 {
            return Err(SoundnessErrorV2::InvalidTerminal);
        }
        validate_equal_terminal_v2(terminal)?;
        let live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        Ok(ProverFriTerminalBoundV2 {
            transcript: absorb_terminal_v2(live.transcript, terminal)?,
            batch_schedule_digest: live.batch_schedule_digest,
            fold_schedule_digest: live.fold_schedule_digest,
        })
    }
}
impl ProverFriTerminalBoundV2 {
    pub(in super::super) fn derive_queries_v2(
        self,
    ) -> Result<ProverFriQueriesV2, SoundnessErrorV2> {
        Ok(ProverFriQueriesV2 {
            queries: derive_queries_v2(self.transcript)?,
            transcript: self.transcript,
            batch_schedule_digest: self.batch_schedule_digest,
            fold_schedule_digest: self.fold_schedule_digest,
        })
    }
}
impl ProverFriQueriesV2 {
    pub(in super::super) const fn context_v2(&self) -> ([u8; 32], [u8; 32], [u8; 32]) {
        (
            self.transcript,
            self.batch_schedule_digest,
            self.fold_schedule_digest,
        )
    }
    pub(in super::super) const fn queries_v2(&self) -> &[u32; QUERY_COUNT_V2] {
        &self.queries
    }
}
#[cfg(test)]
#[path = "prover_fri_rounds_v2_tests.rs"]
mod tests;
