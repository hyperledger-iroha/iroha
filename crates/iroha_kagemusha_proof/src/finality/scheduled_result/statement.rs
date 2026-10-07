//! Compact native identities exported after both complete sources are verified.

use super::*;
use iroha_pasta::poseidon::hash_with_domain;
use iroha_plonk::frontend::Value;

/// Quorum-certified result under its parsed current native context.
/// This remains untrusted until its source proof and genesis-rooted history join.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScheduledResultStatement {
    /// Genesis-derived network from the exact parsed current epoch.
    pub network: [u8; 32],
    /// Original signed consensus instance, to bind to the selected global root.
    pub instance: [u8; 32],
    /// Scheduling epoch from the original signed Commit and current context.
    pub epoch: u64,
    /// Governed height from the original result, current context and Commit.
    pub height: u64,
    /// Native current context identifier, including its complete ordered roster.
    pub context: [u8; 32],
    /// Original signed native result digest.
    pub result: [u8; 32],
    /// Internal tape commitment to the complete original result frame.
    pub tape_root: Fp,
    /// Complete original result frame length, excluding the hash-domain prefix.
    pub frame_len: u32,
}
fn pack(bytes: &[u8]) -> Fp {
    bytes
        .iter()
        .rev()
        .fold(Fp::ZERO, |n, b| n * Fp::from(256) + Fp::from(u64::from(*b)))
}
impl ScheduledResultStatement {
    /// Canonical compact commitment; computing it supplies no proof authority.
    pub fn digest(&self) -> Fp {
        let mut words = Vec::with_capacity(12);
        for identity in [self.network, self.instance, self.context, self.result] {
            words.extend(identity.chunks_exact(16).map(pack));
        }
        words.extend([
            Fp::from(self.epoch),
            Fp::from(self.height),
            self.tape_root,
            Fp::from(u64::from(self.frame_len)),
        ]);
        hash_with_domain(CONTEXT_DOMAIN, &words)
    }
}

/// Original bounded statement cells. Their encoding alone grants no authority.
#[derive(Clone, Debug)]
pub struct ScheduledResultStatementCells {
    digest: Word<Fp>,
    network: [Word<Fp>; 32],
    instance: [Word<Fp>; 32],
    epoch: Uint<Fp, 64>,
    height: Uint<Fp, 64>,
    context: [Word<Fp>; 32],
    result: [Word<Fp>; 32],
    tape_root: Word<Fp>,
    frame_len: Uint<Fp, 32>,
}
/// Borrowed original cells used to derive the compact output without reassignment.
#[derive(Clone, Copy)]
pub(super) struct StatementInputs<'a> {
    pub network: &'a [Word<Fp>; 32],
    pub instance: &'a [Word<Fp>; 32],
    pub epoch: &'a Uint<Fp, 64>,
    pub height: &'a Uint<Fp, 64>,
    pub context: &'a [Word<Fp>; 32],
    pub result: &'a [Word<Fp>; 32],
    pub tape_root: &'a Word<Fp>,
    pub frame_len: &'a Uint<Fp, 32>,
}
impl ScheduledResultStatementCells {
    /// Assign an untrusted statement for exact source-output matching.
    /// # Errors
    /// Layout errors; byte ranges, height or result-length bounds fail.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Value<ScheduledResultStatement>,
    ) -> Result<Self, Error> {
        let mut identities: Vec<[Word<Fp>; 32]> = Vec::with_capacity(4);
        for at in 0..4 {
            let mut bytes = Vec::with_capacity(32);
            for i in 0..32 {
                bytes.push(chip.uint().glue().witness(
                    region,
                    value.map(|v| {
                        Fp::from(u64::from(
                            [v.network, v.instance, v.context, v.result][at][i],
                        ))
                    }),
                )?);
            }
            identities.push(bytes.try_into().map_err(|_| Error::Synthesis)?);
        }
        let identities: [[Word<Fp>; 32]; 4] =
            identities.try_into().map_err(|_| Error::Synthesis)?;
        let epoch = chip
            .uint()
            .assign::<64>(region, value.map(|v| u128::from(v.epoch)))?;
        let height = chip
            .uint()
            .assign::<64>(region, value.map(|v| u128::from(v.height)))?;
        let root = chip
            .uint()
            .glue()
            .witness(region, value.map(|v| v.tape_root))?;
        let len = chip
            .uint()
            .assign::<32>(region, value.map(|v| u128::from(v.frame_len)))?;
        Self::from_cells(
            chip,
            region,
            StatementInputs {
                network: &identities[0],
                instance: &identities[1],
                epoch: &epoch,
                height: &height,
                context: &identities[2],
                result: &identities[3],
                tape_root: &root,
                frame_len: &len,
            },
        )
    }
    pub(super) fn from_cells(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        inputs: StatementInputs<'_>,
    ) -> Result<Self, Error> {
        let StatementInputs {
            network,
            instance,
            epoch,
            height,
            context,
            result,
            tape_root,
            frame_len,
        } = inputs;
        let two = chip.uint().constant::<64>(region, 2)?;
        chip.uint().assert_le(region, &two, height)?;
        let maximum = chip.uint().constant::<32>(
            region,
            u128::from(crate::finality::result::MAX_RESULT_BYTES),
        )?;
        chip.uint().assert_le(region, frame_len, &maximum)?;
        let mut words = Vec::with_capacity(12);
        for identity in [network, instance, context, result] {
            for half in identity.chunks_exact(16) {
                let mut packed = chip.uint().glue().constant(region, Fp::ZERO)?;
                for byte in half.iter().rev() {
                    chip.uint().range_check::<8>(region, byte)?;
                    packed = chip.uint().glue().linear(
                        region,
                        &[(Fp::from(256), &packed), (Fp::ONE, byte)],
                        Fp::ZERO,
                    )?;
                }
                words.push(packed);
            }
        }
        words.extend([
            epoch.word().clone(),
            height.word().clone(),
            tape_root.clone(),
            frame_len.word().clone(),
        ]);
        let digest = chip.hash_words(region, CONTEXT_DOMAIN, &words)?;
        Ok(Self {
            digest,
            network: network.clone(),
            instance: instance.clone(),
            epoch: epoch.clone(),
            height: height.clone(),
            context: context.clone(),
            result: result.clone(),
            tape_root: tape_root.clone(),
            frame_len: frame_len.clone(),
        })
    }
    /// Exact commitment to match against a complete scheduled-result source.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.digest
    }
    /// Parsed native network identity.
    pub const fn network(&self) -> &[Word<Fp>; 32] {
        &self.network
    }
    /// Exact original signed consensus instance.
    pub const fn instance(&self) -> &[Word<Fp>; 32] {
        &self.instance
    }
    /// Exact original signed scheduling epoch.
    pub const fn epoch(&self) -> &Uint<Fp, 64> {
        &self.epoch
    }
    /// Exact original signed and executed height.
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Exact current native epoch identifier.
    pub const fn context(&self) -> &[Word<Fp>; 32] {
        &self.context
    }
    /// Exact original signed native result identity.
    pub const fn result(&self) -> &[Word<Fp>; 32] {
        &self.result
    }
    /// The same complete original result tape used by every parser.
    pub const fn tape_root(&self) -> &Word<Fp> {
        &self.tape_root
    }
    /// The same complete original result frame length.
    pub const fn frame_len(&self) -> &Uint<Fp, 32> {
        &self.frame_len
    }
}
