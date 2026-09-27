//! SCCP v1 TON cells (spec §5.3.1, §5.3.2).
//!
//! A minimal ordinary-cell builder with TON representation hashes and depths, the standard
//! `SnakeBytes` format, the chunked `MemberChunk` and `HashChunk` lists, the canonical initial
//! data of the SCCP Jetton master (minter) and the workchain-0 `StateInit` addresses of the
//! minter, its wallets and its consumption buckets.
//!
//! A cell's representation hash depends on its children only through their hashes and depths,
//! so contract code is referenced by `(hash, depth)` alone ([`CellRef::opaque`]) and Taira can
//! recompute a deployment address from the registered code references (§4.14.3).
//!
//! TODO(ws3A): deduplicate the cell hashing with `crate::ton_native` once the retired TON
//! proof code is purged, and add bag-of-cells serialization for the message bodies.

use std::sync::Arc;

use iroha_data_model::sccp::deployment::SccpTonCodeRefV1;
use sha2::{Digest as _, Sha256};

use super::{
    constants::{TON_HASH_CHUNK_HASHES, TON_MEMBER_CHUNK_ADDRESSES, TON_SNAKE_CHUNK_BYTES},
    roster::RosterV1,
};

/// Maximum data bits of one cell.
pub const MAX_CELL_BITS: usize = 1023;
/// Maximum references of one cell.
pub const MAX_CELL_REFS: usize = 4;
/// Exclusive bound of a TON `Coins` value (`VarUInteger 16`: at most 15 value bytes).
pub const COINS_BOUND: u128 = 1 << 120;

unit_error! {
    /// TON cell construction and parsing errors.
    pub enum TonCellError {
        /// The cell would exceed 1023 data bits.
        BitOverflow => "a TON cell holds at most 1023 data bits",
        /// The cell would exceed 4 references.
        RefOverflow => "a TON cell holds at most 4 references",
        /// An unsigned value does not fit its bit width, or the width exceeds 128.
        ValueTooWide => "value does not fit the requested bit width",
        /// A `Coins` value is `2^120` or more.
        CoinsTooLarge => "TON Coins values must be below 2^120",
        /// A snake or chunk list would be empty.
        Empty => "snake bytes and chunk lists must not be empty",
        /// A snake cell has a shape other than the canonical one.
        BadSnake => "cell is not canonical SnakeBytes",
        /// Snake content exceeds the caller's bound.
        SnakeTooLong => "snake content exceeds the length bound",
        /// A child cell is known only by its hash and depth.
        OpaqueChild => "cell child is known only by hash and depth",
        /// The roster fields violate §3.7.
        BadRoster => "minter roster violates the roster rules",
    }
}

/// A reference to a child cell: its representation hash and depth, plus the cell when known.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellRef {
    hash: [u8; 32],
    depth: u16,
    cell: Option<Arc<Cell>>,
}

impl CellRef {
    /// A child known only by representation hash and depth (for example, contract code).
    #[must_use]
    pub const fn opaque(hash: [u8; 32], depth: u16) -> Self {
        Self {
            hash,
            depth,
            cell: None,
        }
    }

    /// Representation hash of the child.
    #[must_use]
    pub const fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Depth of the child.
    #[must_use]
    pub const fn depth(&self) -> u16 {
        self.depth
    }

    /// The child cell, unless it is opaque.
    #[must_use]
    pub fn cell(&self) -> Option<&Cell> {
        self.cell.as_deref()
    }
}

impl From<Cell> for CellRef {
    fn from(cell: Cell) -> Self {
        Self {
            hash: cell.hash,
            depth: cell.depth,
            cell: Some(Arc::new(cell)),
        }
    }
}

impl From<SccpTonCodeRefV1> for CellRef {
    fn from(code: SccpTonCodeRefV1) -> Self {
        Self::opaque(code.hash, code.depth)
    }
}

/// An ordinary (level-0) TON cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cell {
    data: Vec<u8>,
    bit_len: usize,
    refs: Vec<CellRef>,
    hash: [u8; 32],
    depth: u16,
}

impl Cell {
    /// Representation hash.
    #[must_use]
    pub const fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Depth (0 without references, else one more than the deepest child).
    #[must_use]
    pub const fn depth(&self) -> u16 {
        self.depth
    }

    /// Number of data bits.
    #[must_use]
    pub const fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// Data bits, left-aligned and zero-padded to whole bytes (no completion tag).
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Child references.
    #[must_use]
    pub fn refs(&self) -> &[CellRef] {
        &self.refs
    }

    /// The standard representation `d1 ‖ d2 ‖ augmented data ‖ child depths ‖ child hashes`.
    #[must_use]
    pub fn representation(&self) -> Vec<u8> {
        representation(&self.data, self.bit_len, &self.refs)
    }
}

fn representation(data: &[u8], bit_len: usize, refs: &[CellRef]) -> Vec<u8> {
    let full_bytes = bit_len / 8;
    let data_bytes = bit_len.div_ceil(8);
    let mut out = Vec::with_capacity(2 + data_bytes + refs.len() * 34);
    out.push(u8::try_from(refs.len()).expect("at most 4 references"));
    out.push(u8::try_from(full_bytes + data_bytes).expect("at most 256 data bytes"));
    out.extend_from_slice(&data[..data_bytes]);
    if bit_len % 8 != 0 {
        let last = out.len() - 1;
        out[last] |= 0x80 >> (bit_len % 8);
    }
    for child in refs {
        out.extend_from_slice(&child.depth.to_be_bytes());
    }
    for child in refs {
        out.extend_from_slice(&child.hash);
    }
    out
}

/// Builder of one ordinary cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellBuilder {
    data: Vec<u8>,
    bit_len: usize,
    refs: Vec<CellRef>,
}

impl CellBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Data bits stored so far.
    #[must_use]
    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    /// Append one bit.
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::BitOverflow`] past 1023 bits.
    pub fn store_bit(&mut self, bit: bool) -> Result<&mut Self, TonCellError> {
        if self.bit_len >= MAX_CELL_BITS {
            return Err(TonCellError::BitOverflow);
        }
        if self.bit_len % 8 == 0 {
            self.data.push(0);
        }
        if bit {
            let last = self.data.len() - 1;
            self.data[last] |= 0x80 >> (self.bit_len % 8);
        }
        self.bit_len += 1;
        Ok(self)
    }

    /// Append `value` as a big-endian unsigned integer of `bits` bits (at most 128).
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::ValueTooWide`] or [`TonCellError::BitOverflow`].
    pub fn store_uint(&mut self, value: u128, bits: u32) -> Result<&mut Self, TonCellError> {
        if bits > 128 || (bits < 128 && value >> bits != 0) {
            return Err(TonCellError::ValueTooWide);
        }
        self.ensure_room(bits as usize)?;
        for position in (0..bits).rev() {
            self.store_bit((value >> position) & 1 == 1)?;
        }
        Ok(self)
    }

    /// Append whole bytes (for example a `uint256` or `uint160` in big-endian form).
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::BitOverflow`].
    pub fn store_bytes(&mut self, bytes: &[u8]) -> Result<&mut Self, TonCellError> {
        self.ensure_room(bytes.len() * 8)?;
        for byte in bytes {
            self.store_uint(u128::from(*byte), 8)?;
        }
        Ok(self)
    }

    /// Append a `Coins` (`VarUInteger 16`) value: a 4-bit byte length, then the minimal
    /// big-endian bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::CoinsTooLarge`] or [`TonCellError::BitOverflow`].
    pub fn store_coins(&mut self, value: u128) -> Result<&mut Self, TonCellError> {
        if value >= COINS_BOUND {
            return Err(TonCellError::CoinsTooLarge);
        }
        let len = (128 - value.leading_zeros()).div_ceil(8);
        self.store_uint(u128::from(len), 4)?;
        self.store_uint(value, len * 8)
    }

    /// Append an `addr_std` without anycast: `10 ‖ 0 ‖ int8 workchain ‖ bits256 account`.
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::BitOverflow`].
    pub fn store_address_std(
        &mut self,
        workchain: i8,
        account: &[u8; 32],
    ) -> Result<&mut Self, TonCellError> {
        self.ensure_room(267)?;
        self.store_uint(0b100, 3)?;
        self.store_bytes(&workchain.to_be_bytes())?;
        self.store_bytes(account)
    }

    /// Append a reference.
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::RefOverflow`] past 4 references.
    pub fn store_ref(&mut self, child: impl Into<CellRef>) -> Result<&mut Self, TonCellError> {
        if self.refs.len() >= MAX_CELL_REFS {
            return Err(TonCellError::RefOverflow);
        }
        self.refs.push(child.into());
        Ok(self)
    }

    /// Append a `Maybe ^Cell`: bit 1 and the reference, or bit 0.
    ///
    /// # Errors
    ///
    /// Returns [`TonCellError::BitOverflow`] or [`TonCellError::RefOverflow`].
    pub fn store_maybe_ref(
        &mut self,
        child: Option<impl Into<CellRef>>,
    ) -> Result<&mut Self, TonCellError> {
        match child {
            Some(child) => {
                if self.refs.len() >= MAX_CELL_REFS {
                    return Err(TonCellError::RefOverflow);
                }
                self.store_bit(true)?;
                self.store_ref(child)
            }
            None => self.store_bit(false),
        }
    }

    /// Finish the cell and compute its representation hash and depth.
    #[must_use]
    pub fn build(&self) -> Cell {
        let representation = representation(&self.data, self.bit_len, &self.refs);
        let hash: [u8; 32] = Sha256::digest(&representation).into();
        let depth = self
            .refs
            .iter()
            .map(|child| child.depth.saturating_add(1))
            .max()
            .unwrap_or(0);
        Cell {
            data: self.data.clone(),
            bit_len: self.bit_len,
            refs: self.refs.clone(),
            hash,
            depth,
        }
    }

    fn ensure_room(&self, bits: usize) -> Result<(), TonCellError> {
        if self.bit_len + bits > MAX_CELL_BITS {
            Err(TonCellError::BitOverflow)
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------------------------
// SnakeBytes and chunked lists (§5.3.2)
// ---------------------------------------------------------------------------------------------

/// Canonical `SnakeBytes`: 127-byte chunks linked by the single reference, the last chunk
/// holding 1..=127 bytes.
///
/// # Errors
///
/// Returns [`TonCellError::Empty`] for no bytes.
pub fn snake_bytes(bytes: &[u8]) -> Result<Cell, TonCellError> {
    if bytes.is_empty() {
        return Err(TonCellError::Empty);
    }
    let mut chunks = bytes.chunks(TON_SNAKE_CHUNK_BYTES).rev();
    let last = chunks.next().expect("non-empty input has a chunk");
    let mut tail = CellBuilder::new().store_bytes(last)?.build();
    for chunk in chunks {
        let mut builder = CellBuilder::new();
        builder.store_bytes(chunk)?.store_ref(tail)?;
        tail = builder.build();
    }
    Ok(tail)
}

/// Parse canonical `SnakeBytes` of at most `max_bytes` bytes. Every chunk holds whole bytes; a
/// chunk with a continuation holds exactly 127 bytes and one reference; the last chunk holds
/// 1..=127 bytes and no reference.
///
/// # Errors
///
/// Returns [`TonCellError::BadSnake`], [`TonCellError::SnakeTooLong`] or
/// [`TonCellError::OpaqueChild`].
pub fn parse_snake_bytes(cell: &Cell, max_bytes: usize) -> Result<Vec<u8>, TonCellError> {
    let mut out = Vec::new();
    let mut current = cell;
    loop {
        if current.bit_len % 8 != 0 || current.refs.len() > 1 {
            return Err(TonCellError::BadSnake);
        }
        let len = current.bit_len / 8;
        let has_next = current.refs.len() == 1;
        if (has_next && len != TON_SNAKE_CHUNK_BYTES)
            || !(1..=TON_SNAKE_CHUNK_BYTES).contains(&len)
        {
            return Err(TonCellError::BadSnake);
        }
        if out.len() + len > max_bytes {
            return Err(TonCellError::SnakeTooLong);
        }
        out.extend_from_slice(&current.data[..len]);
        if !has_next {
            return Ok(out);
        }
        current = current.refs[0].cell().ok_or(TonCellError::OpaqueChild)?;
    }
}

fn chunk_list<T>(
    items: &[T],
    per_chunk: usize,
    store: impl Fn(&mut CellBuilder, &T) -> Result<(), TonCellError>,
) -> Result<Cell, TonCellError> {
    if items.is_empty() {
        return Err(TonCellError::Empty);
    }
    let mut next: Option<Cell> = None;
    for chunk in items.chunks(per_chunk).rev() {
        let mut builder = CellBuilder::new();
        for item in chunk {
            store(&mut builder, item)?;
        }
        builder.store_maybe_ref(next.take())?;
        next = Some(builder.build());
    }
    Ok(next.expect("non-empty input has a chunk"))
}

/// `MemberChunk`: maximal 6-address chunks of `uint160` members with a `Maybe ^MemberChunk`
/// continuation.
///
/// # Errors
///
/// Returns [`TonCellError::Empty`] for no members.
pub fn member_chunks(members: &[[u8; 20]]) -> Result<Cell, TonCellError> {
    chunk_list(members, TON_MEMBER_CHUNK_ADDRESSES, |builder, member| {
        builder.store_bytes(member).map(|_| ())
    })
}

/// `HashChunk`: maximal 3-hash chunks of `uint256` with a `Maybe ^HashChunk` continuation;
/// `None` for an empty path (`path:(Maybe ^HashChunk)`).
///
/// # Errors
///
/// Returns [`TonCellError::BitOverflow`] only on internal misuse.
pub fn hash_chunks(hashes: &[[u8; 32]]) -> Result<Option<Cell>, TonCellError> {
    if hashes.is_empty() {
        return Ok(None);
    }
    chunk_list(hashes, TON_HASH_CHUNK_HASHES, |builder, hash| {
        builder.store_bytes(hash).map(|_| ())
    })
    .map(Some)
}

// ---------------------------------------------------------------------------------------------
// Minter initial data and StateInit addresses (§5.3.1)
// ---------------------------------------------------------------------------------------------

/// Inputs of the canonical minter initial data for a deployment pinned to one generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TonMinterInitV1 {
    /// Live Taira `NetworkId` bytes.
    pub taira_network_id: [u8; 32],
    /// Route revision.
    pub route_revision: u32,
    /// Supply cap in token units (`Coins`, below `2^120`).
    pub max_supply: u128,
    /// The pinned Taira roster generation.
    pub roster: RosterV1,
    /// Jetton wallet code.
    pub wallet_code: SccpTonCodeRefV1,
    /// Consumption bucket code.
    pub bucket_code: SccpTonCodeRefV1,
}

/// The three cells under the minter data root, exposed for golden comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TonMinterDataV1 {
    /// `minter_data` root cell.
    pub root: Cell,
    /// `minter_config` cell.
    pub config: Cell,
    /// `roster_state` cell.
    pub roster: Cell,
    /// First `MemberChunk` cell.
    pub members: Cell,
    /// The pinned roster digest.
    pub roster_digest: [u8; 32],
}

/// Build the canonical initial minter data (§5.3.1): uninitialized, zero counters and supply,
/// not paused, no deployed buckets, no previous roster; the config holds the network id,
/// revision, cap, pinned generation and digest and both code references; the roster holds the
/// digest, generation, validity window, `n`, `t` and the members in maximal 6-address chunks.
///
/// # Errors
///
/// Returns [`TonCellError::BadRoster`] or [`TonCellError::CoinsTooLarge`].
pub fn minter_initial_data(init: &TonMinterInitV1) -> Result<TonMinterDataV1, TonCellError> {
    let roster_digest = init
        .roster
        .digest(&init.taira_network_id)
        .map_err(|_| TonCellError::BadRoster)?;
    let n = u8::try_from(init.roster.n()).map_err(|_| TonCellError::BadRoster)?;
    let t = u8::try_from(init.roster.threshold()).map_err(|_| TonCellError::BadRoster)?;

    let mut config = CellBuilder::new();
    config
        .store_bytes(&init.taira_network_id)?
        .store_uint(u128::from(init.route_revision), 32)?
        .store_coins(init.max_supply)?
        .store_uint(u128::from(init.roster.generation), 64)?
        .store_bytes(&roster_digest)?
        .store_ref(init.wallet_code)?
        .store_ref(init.bucket_code)?;
    let config = config.build();

    let members = member_chunks(&init.roster.members)?;
    let mut roster = CellBuilder::new();
    roster
        .store_bytes(&roster_digest)?
        .store_uint(u128::from(init.roster.generation), 64)?
        .store_uint(u128::from(init.roster.valid_from_ms), 64)?
        .store_uint(u128::from(init.roster.valid_until_ms), 64)?
        .store_uint(u128::from(n), 8)?
        .store_uint(u128::from(t), 8)?
        .store_ref(members.clone())?;
    let roster = roster.build();

    let mut root = CellBuilder::new();
    root.store_bit(false)? // initialized
        .store_coins(0)? // total_supply
        .store_coins(0)? // pending_supply
        .store_uint(0, 64)? // outbound_nonce
        .store_uint(0, 64)? // op_count
        .store_uint(0, 64)? // control_nonce
        .store_bit(false)? // minting_paused
        .store_uint(0, 64)? // deployed_buckets
        .store_ref(config.clone())?
        .store_ref(roster.clone())?
        .store_maybe_ref(None::<Cell>)?; // prev_roster
    Ok(TonMinterDataV1 {
        root: root.build(),
        config,
        roster,
        members,
        roster_digest,
    })
}

/// `StateInit` with code and data and no split depth, special flags or libraries.
///
/// # Errors
///
/// Never fails for the fixed 5-bit layout; the `Result` mirrors the builder.
pub fn state_init(
    code: impl Into<CellRef>,
    data: impl Into<CellRef>,
) -> Result<Cell, TonCellError> {
    let mut builder = CellBuilder::new();
    builder
        .store_bit(false)? // split_depth
        .store_bit(false)? // special
        .store_maybe_ref(Some(code))?
        .store_maybe_ref(Some(data))?
        .store_bit(false)?; // library
    Ok(builder.build())
}

/// Workchain-0 account id (`StateInit` hash) of the minter for `init` and its code.
///
/// # Errors
///
/// See [`minter_initial_data`].
pub fn minter_account_id(
    init: &TonMinterInitV1,
    minter_code: SccpTonCodeRefV1,
) -> Result<[u8; 32], TonCellError> {
    let data = minter_initial_data(init)?;
    Ok(*state_init(minter_code, data.root)?.hash())
}

/// Account id of the Jetton wallet of `owner` (zero balance) under the minter.
///
/// # Errors
///
/// Never fails for well-formed inputs; the `Result` mirrors the builder.
pub fn wallet_account_id(
    owner_workchain: i8,
    owner: &[u8; 32],
    minter_account: &[u8; 32],
    wallet_code: SccpTonCodeRefV1,
) -> Result<[u8; 32], TonCellError> {
    let mut data = CellBuilder::new();
    data.store_coins(0)?
        .store_address_std(owner_workchain, owner)?
        .store_address_std(0, minter_account)?;
    Ok(*state_init(wallet_code, data.build())?.hash())
}

/// Account id of consumption bucket `index` (all 512 flags clear) under the minter.
///
/// # Errors
///
/// Never fails for well-formed inputs; the `Result` mirrors the builder.
pub fn bucket_account_id(
    minter_account: &[u8; 32],
    index: u64,
    bucket_code: SccpTonCodeRefV1,
) -> Result<[u8; 32], TonCellError> {
    let mut data = CellBuilder::new();
    data.store_address_std(0, minter_account)?
        .store_uint(u128::from(index), 64)?
        .store_bytes(&[0; 64])?;
    Ok(*state_init(bucket_code, data.build())?.hash())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn from_hex(text: &str) -> [u8; 32] {
        let mut out = [0_u8; 32];
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&text[2 * index..2 * index + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn empty_cell_hash_is_the_known_constant() {
        let empty = CellBuilder::new().build();
        assert_eq!(
            hex(empty.hash()),
            "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7"
        );
        assert_eq!(empty.depth(), 0);
    }

    #[test]
    fn representation_uses_the_completion_tag() {
        let mut builder = CellBuilder::new();
        builder.store_uint(0b101, 3).unwrap();
        let cell = builder.build();
        assert_eq!(cell.representation(), vec![0, 1, 0b1011_0000]);
        assert_eq!(cell.data(), &[0b1010_0000]);
        let mut builder = CellBuilder::new();
        builder.store_uint(0xab, 8).unwrap();
        builder.store_ref(cell.clone()).unwrap();
        let parent = builder.build();
        let mut expected = vec![1, 2, 0xab, 0, 0];
        expected.extend_from_slice(cell.hash());
        assert_eq!(parent.representation(), expected);
        assert_eq!(parent.depth(), 1);
    }

    #[test]
    fn builder_limits() {
        let mut builder = CellBuilder::new();
        builder.store_bytes(&[0; 127]).unwrap();
        builder.store_uint(0, 7).unwrap();
        assert_eq!(builder.bit_len(), 1023);
        assert_eq!(builder.store_bit(true).unwrap_err(), TonCellError::BitOverflow);
        let mut builder = CellBuilder::new();
        assert_eq!(builder.store_uint(4, 2).unwrap_err(), TonCellError::ValueTooWide);
        assert_eq!(builder.store_uint(0, 129).unwrap_err(), TonCellError::ValueTooWide);
        assert!(builder.store_uint(u128::MAX, 128).is_ok());
        for _ in 0..4 {
            builder.store_ref(CellRef::opaque([0; 32], 0)).unwrap();
        }
        assert_eq!(
            builder.store_ref(CellRef::opaque([0; 32], 0)).unwrap_err(),
            TonCellError::RefOverflow
        );
        assert_eq!(
            CellBuilder::new().store_coins(COINS_BOUND).unwrap_err(),
            TonCellError::CoinsTooLarge
        );
    }

    #[test]
    fn coins_and_addresses() {
        let mut builder = CellBuilder::new();
        builder.store_coins(0).unwrap();
        assert_eq!(builder.bit_len(), 4);
        let mut builder = CellBuilder::new();
        builder.store_coins(1_000_000_000_000_000_000).unwrap();
        assert_eq!(builder.bit_len(), 4 + 64);
        assert_eq!(builder.build().data()[0], 0x80);
        let mut builder = CellBuilder::new();
        builder.store_address_std(-1, &[0xff; 32]).unwrap();
        assert_eq!(builder.bit_len(), 267);
        let cell = builder.build();
        assert_eq!(cell.data()[0], 0b1001_1111);
    }

    #[test]
    fn snake_bytes_shapes() {
        for len in [1_usize, 126, 127, 128, 254, 255, 1024] {
            let bytes: Vec<u8> = (0..len).map(|index| (index % 251) as u8).collect();
            let cell = snake_bytes(&bytes).unwrap();
            assert_eq!(parse_snake_bytes(&cell, 4096).unwrap(), bytes, "len {len}");
            let expected_depth = u16::try_from(len.div_ceil(127) - 1).unwrap();
            assert_eq!(cell.depth(), expected_depth);
        }
        assert_eq!(snake_bytes(&[]).unwrap_err(), TonCellError::Empty);
        let long = snake_bytes(&[1; 300]).unwrap();
        assert_eq!(
            parse_snake_bytes(&long, 299).unwrap_err(),
            TonCellError::SnakeTooLong
        );
        // Empty cell, short non-final chunk, non-byte bits, two refs and opaque continuations.
        assert_eq!(
            parse_snake_bytes(&CellBuilder::new().build(), 10).unwrap_err(),
            TonCellError::BadSnake
        );
        let tail = CellBuilder::new().store_bytes(&[1]).unwrap().build();
        let mut short = CellBuilder::new();
        short.store_bytes(&[1; 126]).unwrap().store_ref(tail.clone()).unwrap();
        assert_eq!(
            parse_snake_bytes(&short.build(), 4096).unwrap_err(),
            TonCellError::BadSnake
        );
        let mut odd_bits = CellBuilder::new();
        odd_bits.store_uint(1, 9).unwrap();
        assert_eq!(
            parse_snake_bytes(&odd_bits.build(), 4096).unwrap_err(),
            TonCellError::BadSnake
        );
        let mut two_refs = CellBuilder::new();
        two_refs
            .store_bytes(&[1; 127])
            .unwrap()
            .store_ref(tail.clone())
            .unwrap()
            .store_ref(tail)
            .unwrap();
        assert_eq!(
            parse_snake_bytes(&two_refs.build(), 4096).unwrap_err(),
            TonCellError::BadSnake
        );
        let mut opaque = CellBuilder::new();
        opaque
            .store_bytes(&[1; 127])
            .unwrap()
            .store_ref(CellRef::opaque([1; 32], 0))
            .unwrap();
        assert_eq!(
            parse_snake_bytes(&opaque.build(), 4096).unwrap_err(),
            TonCellError::OpaqueChild
        );
        let mut empty_tail = CellBuilder::new();
        empty_tail
            .store_bytes(&[1; 127])
            .unwrap()
            .store_ref(CellBuilder::new().build())
            .unwrap();
        assert_eq!(
            parse_snake_bytes(&empty_tail.build(), 4096).unwrap_err(),
            TonCellError::BadSnake
        );
    }

    #[test]
    fn chunk_lists() {
        let members: Vec<[u8; 20]> = (1..=13).map(|byte| [byte; 20]).collect();
        let head = member_chunks(&members).unwrap();
        assert_eq!(head.bit_len(), 6 * 160 + 1);
        assert_eq!(head.depth(), 2);
        let last = head.refs()[0].cell().unwrap().refs()[0].cell().unwrap();
        assert_eq!(last.bit_len(), 160 + 1);
        assert!(last.refs().is_empty());
        assert_eq!(member_chunks(&[]).unwrap_err(), TonCellError::Empty);
        assert_eq!(hash_chunks(&[]).unwrap(), None);
        let path = hash_chunks(&[[1; 32]; 4]).unwrap().unwrap();
        assert_eq!(path.bit_len(), 3 * 256 + 1);
        assert_eq!(path.refs()[0].cell().unwrap().bit_len(), 256 + 1);
    }

    fn fixture_n4() -> (TonMinterInitV1, SccpTonCodeRefV1) {
        let init = TonMinterInitV1 {
            taira_network_id: [0x11; 32],
            route_revision: 1,
            max_supply: 1_000_000_000_000_000_000,
            roster: RosterV1 {
                generation: 7,
                valid_from_ms: 1_800_000_000_000,
                valid_until_ms: 1_801_209_600_000,
                members: (1..=4).map(|byte| [byte; 20]).collect(),
            },
            wallet_code: SccpTonCodeRefV1 {
                hash: from_hex("ee3cf858ffdc0e230e9b97914a75c0d7d4ecd317881ecec0873d2abd875aaa90"),
                depth: 4,
            },
            bucket_code: SccpTonCodeRefV1 {
                hash: from_hex("e36819d3c7c4f489c9a0b1bc7ad520ff90f388263db19dafb4eab95c90e426b1"),
                depth: 6,
            },
        };
        let minter_code = SccpTonCodeRefV1 {
            hash: from_hex("d3afa6f2a007603bb32c5f7d34c0e537f8fb245a658a91e10a49695e523c02eb"),
            depth: 16,
        };
        (init, minter_code)
    }

    #[test]
    fn minter_data_and_addresses_match_the_contract_fixture() {
        // fixtures/sccp/ton_stateinit_v1.json, label "n4" (emitted by the Tolk contracts).
        let (init, minter_code) = fixture_n4();
        let data = minter_initial_data(&init).unwrap();
        assert_eq!(data.config.bit_len(), 676);
        assert_eq!(
            hex(data.config.hash()),
            "5b3619e6faf5c00f662c5e75485431d730e8820eb7446f8b0367b611a9033ce2"
        );
        assert_eq!(data.config.depth(), 7);
        assert_eq!(data.members.bit_len(), 641);
        assert_eq!(
            hex(data.members.hash()),
            "efdb0f63b51fe0addd82eb234ccb1c4fda247dd2bbe0f845258aa353a56d2a28"
        );
        assert_eq!(data.roster.bit_len(), 464);
        assert_eq!(
            hex(data.roster.hash()),
            "f0e8dc1f221010d8407e023420ce004d6259cc632652f397993a418e83b05ab1"
        );
        assert_eq!(data.root.bit_len(), 267);
        assert_eq!(data.root.depth(), 8);
        assert_eq!(
            hex(data.root.hash()),
            "bdc1e98dab57b453e3bbb2f2f5efbd69e6880579445c53c32b6aa96a54c4f862"
        );
        let minter = minter_account_id(&init, minter_code).unwrap();
        assert_eq!(
            hex(&minter),
            "492a5e8cc18da55f53963484998aa731f830c89a8ddd5eeea9b78018fd13cd75"
        );
        assert_eq!(
            hex(&bucket_account_id(&minter, 0, init.bucket_code).unwrap()),
            "37b5bb0f4fcda15fb878ae92ac79df24fbde27887e028ab4f6e5a75bf27c73fa"
        );
        assert_eq!(
            hex(&bucket_account_id(&minter, 1, init.bucket_code).unwrap()),
            "94a4f9868c368381bfb7eae16fd81204cc61bdcd91169aec671adebc3963322b"
        );
        assert_eq!(
            hex(&wallet_account_id(0, &[0xab; 32], &minter, init.wallet_code).unwrap()),
            "71c8c39c326c04e722b9a546f1a021d494439db770093a1efcd1a515436811f7"
        );
    }

    #[test]
    fn minter_data_rejects_bad_inputs() {
        let (mut init, _) = fixture_n4();
        init.roster.members.swap(0, 1);
        assert_eq!(
            minter_initial_data(&init).unwrap_err(),
            TonCellError::BadRoster
        );
        let (mut init, _) = fixture_n4();
        init.max_supply = COINS_BOUND;
        assert_eq!(
            minter_initial_data(&init).unwrap_err(),
            TonCellError::CoinsTooLarge
        );
    }

    #[test]
    fn cell_refs() {
        let code = SccpTonCodeRefV1 {
            hash: [3; 32],
            depth: 9,
        };
        let reference = CellRef::from(code);
        assert_eq!(reference.hash(), &[3; 32]);
        assert_eq!(reference.depth(), 9);
        assert!(reference.cell().is_none());
        let cell = CellBuilder::new().build();
        let reference = CellRef::from(cell.clone());
        assert_eq!(reference.cell(), Some(&cell));
        let state = state_init(code, cell).unwrap();
        assert_eq!(state.bit_len(), 5);
        assert_eq!(state.depth(), 10);
    }
}
