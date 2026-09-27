//! Identifiers, keys, signatures, signer bitmaps, committees, quorum math and per-height
//! configuration (spec §1.1–§1.2, §3.1, §10.1, §12.4).

use core::fmt;

/// Length in bytes of every signature and aggregate signature.
///
/// Production uses BLS12-381 in the min-pk setting (`BlsNormal`), whose signatures and aggregates
/// are compressed G2 points of 96 bytes; the simulator's fake scheme uses the same length.
// SPEC: §3.1 leaves the signature length to the scheme. The length is fixed here because
// `tc_digest` places `agg_sig` before a variable-length `opt(..)` field: with variable-length
// aggregates two different TCs could share a digest (and a cached verification verdict).
// (Appendix E, E9)
pub const SIGNATURE_LEN: usize = 96;

/// Largest committee the core accepts anywhere (bounds bitmaps, TC entries and header key lists).
// SPEC: the spec has no hard committee bound (it plans for n ≤ 31); 1024 bounds decoding.
// (Appendix E, E10)
pub const MAX_COMMITTEE_SIZE: usize = 1024;

/// Largest raw public key in bytes (production: 48-byte compressed G1 point; simulator: 32).
pub const MAX_PUBLIC_KEY_LEN: usize = 128;

/// Local monotonic milliseconds.
pub type Millis = u64;

/// Canonical index of a member in `C_h` (§3.1). Never an index into a topology.
pub type ValidatorIndex = u32;

/// A 32-byte hash or execution commitment (`Hash32 = [u8; 32]`, §3.1).
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, norito::Encode, norito::Decode,
)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    /// The all-zero hash.
    pub const ZERO: Self = Self([0; 32]);

    /// The raw bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(f, &self.0[..4])?;
        f.write_str("..")
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_hex(f, &self.0)
    }
}

fn write_hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for byte in bytes {
        write!(f, "{byte:02x}")?;
    }
    Ok(())
}

/// A consensus public key as raw canonical bytes (opaque to the core).
///
/// The total order is ascending by `kb(pk) = be16(len(raw)) ‖ raw` (§3.1): shorter keys first,
/// then lexicographic. Committees are kept in this canonical order.
#[derive(Clone, PartialEq, Eq, Hash, norito::Encode, norito::Decode)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    /// Wrap raw key bytes; fails unless `1 ≤ len ≤ MAX_PUBLIC_KEY_LEN`.
    ///
    /// # Errors
    /// [`KeyError`] if the length is out of range.
    pub fn new(raw: Vec<u8>) -> Result<Self, KeyError> {
        let key = Self(raw);
        if key.is_well_formed() {
            Ok(key)
        } else {
            Err(KeyError)
        }
    }

    /// The raw key bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// A key without the length check (tests of malformed input only).
    #[cfg(test)]
    pub(crate) fn unchecked(raw: Vec<u8>) -> Self {
        Self(raw)
    }

    /// Whether the length is within `1..=MAX_PUBLIC_KEY_LEN` (decoded keys are checked with this).
    pub fn is_well_formed(&self) -> bool {
        (1..=MAX_PUBLIC_KEY_LEN).contains(&self.0.len())
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.cmp(&other.0))
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("pk:")?;
        write_hex(f, self.0.get(..4).unwrap_or(&self.0))
    }
}

/// Error for a public key of invalid length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyError;

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("public key length out of range")
    }
}

impl std::error::Error for KeyError {}

/// An individual signature (canonical compressed encoding, [`SIGNATURE_LEN`] bytes).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, norito::Encode, norito::Decode)]
pub struct Signature(pub [u8; SIGNATURE_LEN]);

/// An aggregate signature (canonical compressed encoding, [`SIGNATURE_LEN`] bytes).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, norito::Encode, norito::Decode)]
pub struct AggregateSignature(pub [u8; SIGNATURE_LEN]);

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sig:")?;
        write_hex(f, &self.0[..4])
    }
}

impl fmt::Debug for AggregateSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("agg:")?;
        write_hex(f, &self.0[..4])
    }
}

/// Signer bitmap of a certificate: exactly `ceil(n/8)` bytes, bit `i` = canonical index `i`,
/// spare bits zero (§3.4).
// SPEC: §3.4 does not fix the bit order inside a byte. Bit `i` is `(bytes[i / 8] >> (i % 8)) & 1`
// (least significant bit first) (Appendix E, E11).
#[derive(Clone, PartialEq, Eq, Hash, Default, norito::Encode, norito::Decode)]
pub struct Bitmap(Vec<u8>);

impl Bitmap {
    /// An empty bitmap sized for a committee of `n` members.
    pub fn new(n: usize) -> Self {
        Self(vec![0; n.div_ceil(8)])
    }

    /// A bitmap for `n` members with the given indices set; `None` if an index is `≥ n`.
    pub fn from_indices(
        n: usize,
        indices: impl IntoIterator<Item = ValidatorIndex>,
    ) -> Option<Self> {
        let mut bitmap = Self::new(n);
        for index in indices {
            if usize_of(index) >= n || !bitmap.set(index) {
                return None;
            }
        }
        Some(bitmap)
    }

    /// Wrap raw bytes (no validation; see [`Bitmap::is_well_formed`]).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Whether bit `index` is set (`false` when out of range).
    pub fn get(&self, index: ValidatorIndex) -> bool {
        let index = usize_of(index);
        self.0
            .get(index / 8)
            .is_some_and(|byte| (byte >> (index % 8)) & 1 == 1)
    }

    /// Set bit `index`; returns `false` (and changes nothing) when it is out of range.
    pub fn set(&mut self, index: ValidatorIndex) -> bool {
        let index = usize_of(index);
        self.0.get_mut(index / 8).is_some_and(|byte| {
            *byte |= 1 << (index % 8);
            true
        })
    }

    /// Number of set bits.
    pub fn count_ones(&self) -> usize {
        self.0.iter().map(|byte| byte.count_ones() as usize).sum()
    }

    /// Set indices in ascending order.
    pub fn ones(&self) -> impl Iterator<Item = ValidatorIndex> + '_ {
        self.0.iter().enumerate().flat_map(|(byte_index, byte)| {
            (0..8u32)
                .filter(move |bit| (byte >> bit) & 1 == 1)
                .map(move |bit| index_of(byte_index * 8).saturating_add(bit))
        })
    }

    /// Exact length `ceil(n/8)` and all spare bits (indices `≥ n`) zero.
    pub fn is_well_formed(&self, n: usize) -> bool {
        if self.0.len() != n.div_ceil(8) {
            return false;
        }
        match self.0.last() {
            Some(last) if !n.is_multiple_of(8) => last >> (n % 8) == 0,
            _ => true,
        }
    }
}

impl fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.ones()).finish()
    }
}

/// `usize` of a validator index (lossless on every supported target).
pub(crate) fn usize_of(index: ValidatorIndex) -> usize {
    usize::try_from(index).unwrap_or(usize::MAX)
}

/// Validator index of a `usize` position (saturating; committees are ≤ [`MAX_COMMITTEE_SIZE`]).
pub(crate) fn index_of(position: usize) -> ValidatorIndex {
    ValidatorIndex::try_from(position).unwrap_or(ValidatorIndex::MAX)
}

/// Fault threshold `f = floor((n − 1) / 3)` (§1.2); `0` for `n = 0`.
pub const fn fault_threshold(n: usize) -> usize {
    n.saturating_sub(1) / 3
}

/// Quorum `q = n − f` (§1.2). This is the only quorum formula used anywhere in the crate.
pub const fn quorum(n: usize) -> usize {
    if cfg!(sumeragi_mutation = "MS13") {
        return 2 * fault_threshold(n) + 1;
    }
    n - fault_threshold(n)
}

/// Committee `C_h`: distinct consensus keys in canonical order (ascending by `kb(pk)`, §1.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Committee {
    members: Vec<PublicKey>,
}

impl Committee {
    /// Build a committee from keys in any order; the result is sorted canonically.
    ///
    /// # Errors
    /// Empty, larger than [`MAX_COMMITTEE_SIZE`], duplicate or malformed keys.
    pub fn new(mut keys: Vec<PublicKey>) -> Result<Self, CommitteeError> {
        if keys.is_empty() {
            return Err(CommitteeError::Empty);
        }
        if keys.len() > MAX_COMMITTEE_SIZE {
            return Err(CommitteeError::TooLarge);
        }
        if !keys.iter().all(PublicKey::is_well_formed) {
            return Err(CommitteeError::MalformedKey);
        }
        keys.sort();
        if keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(CommitteeError::Duplicate);
        }
        Ok(Self { members: keys })
    }

    /// Committee size `n`.
    pub fn n(&self) -> usize {
        self.members.len()
    }

    /// Fault threshold `f` of this committee.
    pub fn f(&self) -> usize {
        fault_threshold(self.n())
    }

    /// Quorum `q = n − f` of this committee.
    pub fn q(&self) -> usize {
        quorum(self.n())
    }

    /// Members in canonical order.
    pub fn members(&self) -> &[PublicKey] {
        &self.members
    }

    /// The member with canonical index `index`.
    pub fn get(&self, index: ValidatorIndex) -> Option<&PublicKey> {
        self.members.get(usize_of(index))
    }

    /// Canonical index of `key`, if it is a member.
    pub fn index_of(&self, key: &PublicKey) -> Option<ValidatorIndex> {
        self.members.binary_search(key).ok().map(index_of)
    }

    /// Whether `key` is a member.
    pub fn contains(&self, key: &PublicKey) -> bool {
        self.index_of(key).is_some()
    }

    /// Keys of the set bits of `bitmap`, or `None` if the bitmap is not well formed for `n`.
    pub fn keys_of(&self, bitmap: &Bitmap) -> Option<Vec<&PublicKey>> {
        if !bitmap.is_well_formed(self.n()) {
            return None;
        }
        bitmap.ones().map(|index| self.get(index)).collect()
    }
}

/// Error building a [`Committee`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitteeError {
    /// No members.
    Empty,
    /// More than [`MAX_COMMITTEE_SIZE`] members.
    TooLarge,
    /// The same key twice.
    Duplicate,
    /// A key of invalid length.
    MalformedKey,
}

impl fmt::Display for CommitteeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for CommitteeError {}

/// Chain parameters (§12.4). They come from committed state (§10.1) and MUST be identical on all
/// validators of an instance. The demotion window `W` is not a chain parameter: it is a genesis
/// constant passed in [`crate::api::Init`] (§2.1, §10.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainParams {
    /// Target block time under load.
    pub block_time: Millis,
    /// Heartbeat interval of an idle chain.
    pub idle_block_interval: Millis,
    /// Execution budget `E_max`.
    pub e_max: Millis,
    /// Apply budget `A_max`.
    pub a_max: Millis,
    /// Largest payload a block may carry.
    pub max_block_bytes: u32,
    /// Fresh blocks from this view on must be `EMPTY` (§6.2 step 6).
    pub empty_after_views: u64,
    /// Epoch length in heights (§11.7).
    pub epoch_length: u64,
}

impl Default for ChainParams {
    /// §9.3 defaults (1 s block time).
    // SPEC: §9.3 gives no default `epoch_length`; 3600 heights (one hour at 1 s) is used.
    // (Appendix E, E12)
    fn default() -> Self {
        Self {
            block_time: 1_000,
            idle_block_interval: 5_000,
            e_max: 4_000,
            a_max: 1_000,
            max_block_bytes: 4 * 1024 * 1024,
            empty_after_views: 2,
            epoch_length: 3_600,
        }
    }
}

/// Height configuration: committee and chain parameters of one height (§10.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HeightConfig {
    /// `C_h`.
    pub committee: Committee,
    /// Chain parameters of `h`.
    pub params: ChainParams,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(bytes: &[u8]) -> PublicKey {
        PublicKey::new(bytes.to_vec()).unwrap()
    }

    #[test]
    fn quorum_math_n_1_to_31() {
        for n in 1..=31usize {
            let f = fault_threshold(n);
            let q = quorum(n);
            assert_eq!(f, (n - 1) / 3, "n={n}");
            assert_eq!(q, n - f, "n={n}");
            // Two quorums intersect in at least f + 1 members (one honest).
            assert!(2 * q >= n + f + 1, "n={n}");
            // At least f + 1 honest members inside any quorum.
            assert!(q > f, "n={n}");
            // A quorum is reachable with f members silent.
            assert!(n - f >= q, "n={n}");
            // q honest + f faulty never exceed n.
            assert!(q + f <= n, "n={n}");
        }
        let table = [
            (1, 0, 1),
            (2, 0, 2),
            (3, 0, 3),
            (4, 1, 3),
            (5, 1, 4),
            (6, 1, 5),
            (7, 2, 5),
            (20, 6, 14),
            (22, 7, 15),
            (31, 10, 21),
        ];
        for (n, f, q) in table {
            assert_eq!((fault_threshold(n), quorum(n)), (f, q), "n={n}");
        }
        assert_eq!((fault_threshold(0), quorum(0)), (0, 0));
    }

    #[test]
    fn det_s13_quorum_n5() {
        // n = 5: 2f + 1 = 3 would be wrong; q = n − f = 4.
        assert_eq!(quorum(5), 4);
        assert_ne!(quorum(5), 2 * fault_threshold(5) + 1);
    }

    #[test]
    fn hash32_formatting() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xab;
        bytes[31] = 0x01;
        let hash = Hash32(bytes);
        assert_eq!(format!("{hash:?}"), "ab000000..");
        let text = hash.to_string();
        assert_eq!(text.len(), 64);
        assert!(text.starts_with("ab00") && text.ends_with("01"));
        assert_eq!(Hash32::ZERO.as_bytes(), &[0; 32]);
    }

    #[test]
    fn public_key_bounds_and_canonical_order() {
        assert!(PublicKey::new(vec![]).is_err());
        assert!(PublicKey::new(vec![0; MAX_PUBLIC_KEY_LEN + 1]).is_err());
        assert!(PublicKey::new(vec![0; MAX_PUBLIC_KEY_LEN]).is_ok());
        // kb order: shorter first, then lexicographic.
        assert!(key(&[0xff]) < key(&[0x00, 0x00]));
        assert!(key(&[0x01, 0x02]) < key(&[0x01, 0x03]));
        assert_eq!(key(&[1, 2]).as_bytes(), &[1, 2]);
        assert_eq!(
            format!("{:?}", key(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee])),
            "pk:aabbccdd"
        );
        assert_eq!(format!("{:?}", key(&[0xaa])), "pk:aa");
        assert_eq!(KeyError.to_string(), "public key length out of range");
    }

    #[test]
    fn signature_debug() {
        assert_eq!(
            format!("{:?}", Signature([0x12; SIGNATURE_LEN])),
            "sig:12121212"
        );
        assert_eq!(
            format!("{:?}", AggregateSignature([0x34; SIGNATURE_LEN])),
            "agg:34343434"
        );
    }

    #[test]
    fn bitmap_operations() {
        for n in 1..=40usize {
            let mut bitmap = Bitmap::new(n);
            assert_eq!(bitmap.as_bytes().len(), n.div_ceil(8));
            assert!(bitmap.is_well_formed(n));
            assert_eq!(bitmap.count_ones(), 0);
            let last = index_of(n - 1);
            assert!(bitmap.set(0));
            assert!(bitmap.set(last));
            assert!(bitmap.get(0) && bitmap.get(last));
            assert!(bitmap.is_well_formed(n));
            assert_eq!(bitmap.ones().collect::<Vec<_>>(), {
                let mut v = vec![0, last];
                v.dedup();
                v
            });
            // Out of range positions within the last byte are spare bits.
            if n % 8 != 0 {
                let mut spare = bitmap.clone();
                assert!(spare.set(index_of(n)));
                assert!(!spare.is_well_formed(n));
            }
            assert!(!bitmap.set(index_of(n.div_ceil(8) * 8)));
            assert!(!bitmap.get(index_of(n.div_ceil(8) * 8)));
        }
        // LSB-first layout.
        let bitmap = Bitmap::from_indices(10, [0, 3, 9]).unwrap();
        assert_eq!(bitmap.as_bytes(), &[0b0000_1001, 0b0000_0010]);
        assert_eq!(bitmap.count_ones(), 3);
        assert_eq!(format!("{bitmap:?}"), "{0, 3, 9}");
        assert!(Bitmap::from_indices(10, [10]).is_none());
        // Wrong length.
        assert!(!Bitmap::from_bytes(vec![0, 0, 0]).is_well_formed(10));
        assert!(!Bitmap::from_bytes(vec![0]).is_well_formed(10));
        assert!(Bitmap::from_bytes(vec![0xff]).is_well_formed(8));
        assert!(Bitmap::from_bytes(vec![]).is_well_formed(0));
        assert_eq!(Bitmap::default().count_ones(), 0);
    }

    #[test]
    fn committee_canonical_and_errors() {
        let committee = Committee::new(vec![key(&[3, 0]), key(&[9]), key(&[1, 5])]).unwrap();
        assert_eq!(
            committee.members(),
            &[key(&[9]), key(&[1, 5]), key(&[3, 0])]
        );
        assert_eq!((committee.n(), committee.f(), committee.q()), (3, 0, 3));
        assert_eq!(committee.index_of(&key(&[3, 0])), Some(2));
        assert_eq!(committee.index_of(&key(&[4])), None);
        assert!(committee.contains(&key(&[9])));
        assert_eq!(committee.get(1), Some(&key(&[1, 5])));
        assert_eq!(committee.get(3), None);
        let bitmap = Bitmap::from_indices(3, [0, 2]).unwrap();
        assert_eq!(
            committee.keys_of(&bitmap).unwrap(),
            vec![&key(&[9]), &key(&[3, 0])]
        );
        assert!(committee.keys_of(&Bitmap::new(9)).is_none());

        assert_eq!(Committee::new(vec![]), Err(CommitteeError::Empty));
        assert_eq!(
            Committee::new(vec![key(&[1]), key(&[1])]),
            Err(CommitteeError::Duplicate)
        );
        assert_eq!(
            Committee::new(vec![PublicKey(vec![])]),
            Err(CommitteeError::MalformedKey)
        );
        let many = (0..=MAX_COMMITTEE_SIZE)
            .map(|i| key(&u32::try_from(i).unwrap().to_be_bytes()))
            .collect();
        assert_eq!(Committee::new(many), Err(CommitteeError::TooLarge));
        assert_eq!(CommitteeError::Empty.to_string(), "Empty");
    }

    #[test]
    fn chain_params_defaults() {
        let params = ChainParams::default();
        assert_eq!(params.block_time, 1_000);
        assert_eq!(params.idle_block_interval, 5_000);
        assert_eq!(params.max_block_bytes, 4 << 20);
        assert_eq!(params.empty_after_views, 2);
        assert_eq!((params.e_max, params.a_max), (4_000, 1_000));
    }

    #[test]
    fn index_conversions() {
        assert_eq!(usize_of(7), 7);
        assert_eq!(index_of(7), 7);
    }
}
