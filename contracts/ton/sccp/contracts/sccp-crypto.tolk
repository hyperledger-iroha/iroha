// Cryptographic and chain-identity primitives of the SCCP v1 TON contracts.
//
// Tolk 1.4.2 has no ECRECOVER, keccak or GLOBALID builtins, so they are
// declared here as asm wrappers (specs/sccp.md §5.3.7). Every wrapper is
// pinned by an Acton emulator test in `tests/unit/sccp-crypto.test.tolk`.

import "sccp-constants"

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

/// Raw `ECRECOVER` (0xF912). Stack in `hash v r s`, out `h x y -1` or `0`;
/// the two `NULLSWAPIFNOT` steps normalize the failure branch to four values.
/// `v` is the recovery id (0 or 1), not the Ethereum `v`.
@pure
fun ecrecoverRaw(hash: uint256, v: int, r: uint256, s: uint256): (int?, int?, int?, bool)
    asm "ECRECOVER" "NULLSWAPIFNOT2" "NULLSWAPIFNOT"

/// 20-byte address of an uncompressed secp256k1 point:
/// `keccak256(X ‖ Y)[12..32]` as the low 160 bits (§3.8).
@inline
fun sccpAddressOfPoint(x: uint256, y: uint256): int {
    return keccak256Builder(beginCell().storeUint(x, 256).storeUint(y, 256)) & SCCP_ADDRESS_MASK;
}

/// Recovers the signer address of a §3.8 signature over `digest`.
///
/// Returns 0 when the signature is malformed (`v ∉ {27, 28}`, `r ∉ [1, N)`,
/// `s ∉ [1, HALF_N]`) or recovery fails. The contract passes `v − 27` to
/// `ECRECOVER`, so it does not depend on the global-version rule that also
/// accepts 27/28 (§5.3.7). The result is masked to 160 bits.
@inline
fun sccpRecoverSigner(digest: uint256, r: int, s: int, v: int): int {
    if ((v != 27) & (v != 28)) {
        return 0;
    }
    if ((r < 1) | (r >= SECP256K1_N)) {
        return 0;
    }
    if ((s < 1) | (s > SECP256K1_HALF_N)) {
        return 0;
    }
    val (_, x, y, ok) = ecrecoverRaw(digest, v - 27, r, s);
    if (!ok) {
        return 0;
    }
    return sccpAddressOfPoint(x!, y!);
}
