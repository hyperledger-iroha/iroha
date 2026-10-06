//! Exact native Commit signing bytes and equal-weight quorum geometry.
//!
//! These components bind byte transcripts and signer selection. They do not
//! authenticate a committee, establish BLS verification, or authorize a Load.
//! The enclosing source must authenticate the epoch from genesis, bind these
//! same bytes to the hash-to-curve relation, and aggregate precisely the selected
//! keys from that epoch's ordered roster.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, U64, Uint, UintChip, Word, cells::low_u128};

/// The complete native Commit vote preimage, before W3f hash-to-curve framing.
#[derive(Clone, Debug)]
pub struct CommitVoteCells {
    bytes: [Word<Fp>; Self::BYTES],
    epoch: U64<Fp>,
    height: U64<Fp>,
    view: U64<Fp>,
}

impl CommitVoteCells {
    /// Exact native `sumeragi/sig || 0x03 || I || E || h || v || bh || R` length.
    pub const BYTES: usize = 165;

    /// Check the Commit domain and parse unsigned big-endian epoch, height and view.
    ///
    /// All bytes are range checked. The returned digest slices are the original
    /// signing tape, not separately supplied decoded fields. Height is at least
    /// two: genesis has no Commit quorum certificate.
    /// # Errors
    /// Returns layout errors. Wrong domain, byte range or height is unsatisfiable.
    pub fn from_bytes(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        bytes: &[Word<Fp>; Self::BYTES],
    ) -> Result<Self, Error> {
        for byte in bytes {
            uint.range_check::<8>(region, byte)?;
        }
        for (byte, expected) in bytes.iter().zip(b"sumeragi/sig\x03") {
            GlueChip::assert_constant(region, byte, Fp::from(u64::from(*expected)))?;
        }
        let epoch = read_be64(uint, region, &bytes[45..53])?;
        let height = read_be64(uint, region, &bytes[85..93])?;
        let view = read_be64(uint, region, &bytes[93..101])?;
        let two = uint.constant::<64>(region, 2)?;
        uint.assert_le(region, &two, &height)?;
        Ok(Self {
            bytes: bytes.clone(),
            epoch,
            height,
            view,
        })
    }

    /// Exact original message to feed into native W3f hash-to-curve framing.
    pub const fn bytes(&self) -> &[Word<Fp>; Self::BYTES] {
        &self.bytes
    }
    /// Instance identity, which the enclosing source must derive from its trust root.
    pub fn instance(&self) -> &[Word<Fp>] {
        &self.bytes[13..45]
    }
    /// Scheduling epoch number from the original signed bytes.
    pub const fn epoch(&self) -> &U64<Fp> {
        &self.epoch
    }
    /// Complete epoch context identity from the original signed bytes.
    pub fn context(&self) -> &[Word<Fp>] {
        &self.bytes[53..85]
    }
    /// Non-genesis block height from the original signed bytes.
    pub const fn height(&self) -> &U64<Fp> {
        &self.height
    }
    /// Commit view from the original signed bytes.
    pub const fn view(&self) -> &U64<Fp> {
        &self.view
    }
    /// Core block hash, not the application `BlockHeader` hash.
    pub fn block_hash(&self) -> &[Word<Fp>] {
        &self.bytes[101..133]
    }
    /// Native execution-result digest from the original signed bytes.
    pub fn result(&self) -> &[Word<Fp>] {
        &self.bytes[133..165]
    }
}

fn read_be64(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    bytes: &[Word<Fp>],
) -> Result<U64<Fp>, Error> {
    if bytes.len() != 8 {
        return Err(Error::Synthesis);
    }
    let mut result = uint.glue().constant(region, Fp::ZERO)?;
    for byte in bytes {
        result = uint.glue().linear(
            region,
            &[(Fp::from(256), &result), (Fp::ONE, byte)],
            Fp::ZERO,
        )?;
    }
    uint.range_check::<64>(region, &result)
}

/// Exact `n=3f+1`, `1<=f<=10` global committee and `q=2f+1` signer bitmap.
/// The four-byte tape is zero padded; `encoded_len` binds the original native
/// bitmap's actual byte length and spare bits must be zero.
#[derive(Clone, Debug)]
pub struct QuorumCells {
    members: Uint<Fp, 5>,
    faults: Uint<Fp, 4>,
    encoded_len: Uint<Fp, 3>,
    selected: [Bit<Fp>; 31],
}

impl QuorumCells {
    /// Constrain canonical LSB-first signer positions and an exact quorum.
    ///
    /// The enclosing source must bind `members` to its authenticated roster and
    /// `encoded_len` to the decoded QC, then use every selected bit in aggregation.
    /// Extra signatures, inactive positions, extra bytes and invalid committee
    /// sizes have no satisfying witness.
    /// # Errors
    /// Returns layout errors; invalid quorum geometry is unsatisfiable.
    pub fn from_bitmap(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        members: &Word<Fp>,
        faults: &Word<Fp>,
        encoded_len: &Word<Fp>,
        bitmap: &[Word<Fp>; 4],
    ) -> Result<Self, Error> {
        let members = uint.range_check::<5>(region, members)?;
        let faults = uint.range_check::<4>(region, faults)?;
        let encoded_len = uint.range_check::<3>(region, encoded_len)?;
        let one = uint.constant::<4>(region, 1)?;
        let ten = uint.constant::<4>(region, 10)?;
        uint.assert_le(region, &one, &faults)?;
        uint.assert_le(region, &faults, &ten)?;
        let count = uint
            .glue()
            .linear(region, &[(Fp::from(3), faults.word())], Fp::ONE)?;
        GlueChip::assert_equal(region, &count, members.word())?;
        let mut expected_len = uint.glue().constant(region, Fp::ZERO)?;
        for index in [0_u128, 8, 16, 24] {
            let index = uint.constant::<5>(region, index)?;
            let active = uint.lt(region, &index, &members)?;
            expected_len = uint.glue().add(region, &expected_len, active.word())?;
        }
        GlueChip::assert_equal(region, &expected_len, encoded_len.word())?;
        let mut selected = Vec::with_capacity(31);
        let mut signed = uint.glue().constant(region, Fp::ZERO)?;
        for (byte_index, byte) in bitmap.iter().enumerate() {
            let mut packed = uint.glue().constant(region, Fp::ZERO)?;
            for bit_index in 0..8 {
                let bit = uint.glue().boolean(
                    region,
                    byte.value()
                        .map(|byte| (low_u128(&byte) >> bit_index) & 1 == 1),
                )?;
                packed = uint.glue().linear(
                    region,
                    &[
                        (Fp::ONE, &packed),
                        (Fp::from(1_u64 << bit_index), bit.word()),
                    ],
                    Fp::ZERO,
                )?;
                let index = byte_index * 8 + bit_index;
                let position = uint.constant::<5>(region, index as u128)?;
                let active = uint.lt(region, &position, &members)?;
                let kept = uint.glue().mul(region, active.word(), bit.word())?;
                GlueChip::assert_equal(region, &kept, bit.word())?;
                signed = uint.glue().add(region, &signed, bit.word())?;
                if index < 31 {
                    selected.push(bit);
                }
            }
            GlueChip::assert_equal(region, &packed, byte)?;
        }
        let quorum = uint
            .glue()
            .linear(region, &[(Fp::from(2), faults.word())], Fp::ONE)?;
        GlueChip::assert_equal(region, &signed, &quorum)?;
        Ok(Self {
            members,
            faults,
            encoded_len,
            selected: selected.try_into().map_err(|_| Error::Synthesis)?,
        })
    }

    /// Authenticated-roster size which the caller must bind to its epoch source.
    pub const fn members(&self) -> &Uint<Fp, 5> {
        &self.members
    }
    /// Fault bound constrained by the exact committee geometry.
    pub const fn faults(&self) -> &Uint<Fp, 4> {
        &self.faults
    }
    /// Exact native bitmap byte length, before this component's zero padding.
    pub const fn encoded_len(&self) -> &Uint<Fp, 3> {
        &self.encoded_len
    }
    /// Exact ordered key-selection bits; inactive positions are always false.
    pub const fn selected(&self) -> &[Bit<Fp>; 31] {
        &self.selected
    }
}

#[cfg(test)]
mod tests;
