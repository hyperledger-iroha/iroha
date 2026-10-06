// Cryptographic and chain-identity primitives of the SCCP v1 TON contracts.
//
// Tolk 1.4.2 has no BLS, keccak, SHA-256-over-builders or GLOBALID builtins,
// so they are declared here as asm wrappers (specs/sccp.md §5.3.7). Every
// wrapper is pinned by an Acton emulator test in
// `tests/unit/sccp-vectors.test.tolk`.

/// `GLOBALID` (0xF835): the `global_id` of config parameter 19
/// (TON mainnet = -239).
@pure
fun sccpGlobalId(): int
    asm "GLOBALID"

/// Current destination time in milliseconds (`now() × 1000`, §0).
@pure
fun sccpNowMs(): int {
    return blockchain.now() * 1000;
}

/// Keccak-256 (`HASHEXT` hash id 3) of the concatenation of the byte-aligned
/// slices and builders in `parts` (at most 255 entries).
@pure
fun keccak256Parts(parts: tuple): uint256
    asm "255 PUSHINT" "EXPLODEVAR" "3 HASHEXT"

/// Keccak-256 of the data bits of one byte-aligned builder.
@pure
fun keccak256Builder(b: builder): uint256
    asm "1 PUSHINT" "3 HASHEXT"

/// Keccak-256 of two byte-aligned builders.
@pure
fun keccak256Builders2(a: builder, b: builder): uint256
    asm "2 PUSHINT" "3 HASHEXT"

/// Keccak-256 of two byte-aligned slices (`2 PUSHINT 3 HASHEXT`; the
/// checkpoint id over `XHead.bytes` and `XTail.bytes`, §5.3.4 item 3).
@pure
fun keccak256Slices2(a: slice, b: slice): uint256
    asm "2 PUSHINT" "3 HASHEXT"

/// SHA-256 (`HASHEXT` hash id 0) of the concatenation of the byte-aligned
/// slices and builders in `parts` (at most 255 entries).
@pure
fun sha256Parts(parts: tuple): uint256
    asm "255 PUSHINT" "EXPLODEVAR" "0 HASHEXT"

/// SHA-256 of the byte-aligned `parts` as a slice of exactly 256 bits and
/// no references (§5.3.4 item 2): `HASHEXTA` id 0 into an empty builder,
/// then `ENDC`, `CTOS`.
@pure
fun sha256PartsToSlice(parts: tuple): slice
    asm "NEWC" "SWAP" "255 PUSHINT" "EXPLODEVAR" "0 HASHEXTA" "ENDC" "CTOS"

/// `BLS_FASTAGGREGATEVERIFY` over the tuple `t = [k_1 … k_q, q, m, σ]`
/// (§5.3.4 item 5): `DUP TLEN UNTUPLEVAR` leaves `k_1 … k_q q m σ` with `σ`
/// on top, exactly the opcode's pop order. The opcode hashes `m` to G2 under
/// `DST_SIG`, aggregates the keys and checks the pairing; it returns false
/// for any malformed point.
@pure
fun sccpBlsFastAggregateVerify(t: tuple): bool
    asm "DUP" "TLEN" "UNTUPLEVAR" "BLS_FASTAGGREGATEVERIFY"

/// `SDLEXCMP`: −1, 0 or 1 as `a` is lexicographically below, equal to or
/// above `b` (data bits only).
@pure
fun sccpLexCompare(a: slice, b: slice): int
    asm "SDLEXCMP"
