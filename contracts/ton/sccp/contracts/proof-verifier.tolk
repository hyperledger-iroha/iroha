import "@stdlib/exotic-cells"
import "constants"
import "errors"
import "sccp-codec"

/// BLST-compressed Groth16 proof points (G1/G2/G1).
struct Groth16Bls12381Proof {
    a: cell
    b: cell
    c: cell
}

/// Immutable verification-key tail. `ic` must contain exactly keys `0..11`.
struct Groth16Bls12381VerificationKeyTail {
    deltaG2: cell
    ic: map<uint8, cell>
}

/// Immutable governed BLS12-381 Groth16 verification key.
struct Groth16Bls12381VerificationKey {
    version: uint8
    publicSignalCount: uint8
    alphaG1: cell
    betaG2: cell
    gammaG2: cell
    tail: Cell<Groth16Bls12381VerificationKeyTail>
}

// Eleven signals are split so each ordinary cell stays below 1023 bits.
struct DestinationPublicSignals3 {
    routeConfigurationHash: uint256
    soraFinalityAnchorHash: uint256
}

struct DestinationPublicSignals2 {
    sourceDomain: uint256
    statementHash: uint256
    destinationBindingHash: uint256
    next: Cell<DestinationPublicSignals3>
}

struct DestinationPublicSignals1 {
    commitmentRoot: uint256
    finalityHeight: uint256
    finalityBlockHash: uint256
    next: Cell<DestinationPublicSignals2>
}

struct DestinationPublicSignals0 {
    messageId: uint256
    payloadHash: uint256
    targetDomain: uint256
    next: Cell<DestinationPublicSignals1>
}

/// Immutable inputs bound into the TON destination proof.
struct DestinationProofPolicy {
    messageId: uint256
    payloadHash: uint256
    statementHash: uint256
    routeConfigurationHash: uint256
    destinationBindingHash: uint256
    semanticProofProfileHash: uint256
    soraFinalityAnchorHash: uint256
    /// SHA-256 of canonical `version || alpha || beta || gamma || delta || IC[0..11]` bytes.
    verifyingKeyHash: uint256
    /// TON representation hash of the immutable serialized key cell.
    verifyingKeyCellHash: uint256
}

/// # Safety
/// The verifier length-checks this BLST-compressed G1 slice before invoking the opcode.
@pure
fun blsG1InGroup(x: slice): bool
    asm "BLS_G1_INGROUP"

/// # Safety
/// The verifier length-checks this BLST-compressed G2 slice before invoking the opcode.
@pure
fun blsG2InGroup(x: slice): bool
    asm "BLS_G2_INGROUP"

/// # Safety
/// The verifier supplies an exact 384-bit G1 encoding.
@pure
fun blsG1IsZero(x: slice): bool
    asm "BLS_G1_ISZERO"

/// # Safety
/// The verifier supplies an exact 768-bit G2 encoding.
@pure
fun blsG2IsZero(x: slice): bool
    asm "BLS_G2_ISZERO"

/// # Safety
/// The verifier first checks curve/subgroup membership and non-identity.
@pure
fun blsG1Neg(x: slice): slice
    asm "BLS_G1_NEG"

/// Native six-term G1 multiexponentiation. Two calls cover IC[0] plus eleven signals while
/// respecting Tolk 1.4.1's sixteen-argument limit for assembler built-ins.
/// # Safety
/// Every point is an exact, subgroup-checked G1 encoding and every scalar is canonical Fr.
@pure
fun blsG1MultiExp6(
    p0: slice,
    s0: int,
    p1: slice,
    s1: int,
    p2: slice,
    s2: int,
    p3: slice,
    s3: int,
    p4: slice,
    s4: int,
    p5: slice,
    s5: int,
): slice
    asm "6 PUSHINT BLS_G1_MULTIEXP"

/// # Safety
/// Both inputs are outputs of checked G1 operations.
@pure
fun blsG1Add(a: slice, b: slice): slice
    asm "BLS_G1_ADD"

/// Four-pair Groth16 equation:
/// `e(-A,B) * e(alpha,beta) * e(vk_x,gamma) * e(C,delta) == 1`.
/// # Safety
/// All alternating G1/G2 points are exact, subgroup-checked, non-identity encodings.
@pure
fun blsPairingCheck4(
    a1: slice,
    a2: slice,
    b1: slice,
    b2: slice,
    c1: slice,
    c2: slice,
    d1: slice,
    d2: slice,
): bool
    asm "4 PUSHINT BLS_PAIRING"

fun ordinaryPointSlice(value: cell, expectedBits: int): slice {
    val (point, exotic) = value.beginParseSpecial();
    assert (!exotic) throw SccpErrors.InvalidProofShape;
    assert (
        point.remainingBitsCount() == expectedBits && point.remainingRefsCount() == 0
    ) throw SccpErrors.InvalidProofShape;
    return point;
}

fun validG1(value: slice): bool {
    return value.remainingBitsCount() == 384 && value.remainingRefsCount() == 0 &&
    blsG1InGroup(value) &&
    !blsG1IsZero(value);
}

fun validG2(value: slice): bool {
    return value.remainingBitsCount() == 768 && value.remainingRefsCount() == 0 &&
    blsG2InGroup(value) &&
    !blsG2IsZero(value);
}

/// Hash exactly 17 byte slices without exposing them as 17 Tolk function parameters.
/// # Safety
/// Callers construct a 17-element array of byte-aligned, reference-free slices.
@pure
fun sha256Seventeen(parts: array<slice>): uint256
    asm "17 PUSHINT" "UNTUPLEVAR" "17 PUSHINT" "0 HASHEXT"

/// Reproduce the Rust/data-model verification-key commitment over canonical compressed bytes.
fun canonicalBls12381VerifyingKeyHash(verifyingKeyCell: cell): uint256 {
    val key = Groth16Bls12381VerificationKey.fromCell(verifyingKeyCell);
    assert (
        key.version == 1 && key.publicSignalCount == SCCP_GROTH16_PUBLIC_SIGNAL_COUNT
    ) throw SccpErrors.InvalidProofShape;
    val tail = key.tail.load();
    var parts: array<slice> = [];
    parts.push(beginCell().storeUint(key.version, 8).endCell().beginParse());
    val alpha = ordinaryPointSlice(key.alphaG1, 384);
    val beta = ordinaryPointSlice(key.betaG2, 768);
    val gamma = ordinaryPointSlice(key.gammaG2, 768);
    val delta = ordinaryPointSlice(tail.deltaG2, 768);
    assert (
        validG1(alpha) && validG2(beta) && validG2(gamma) && validG2(delta)
    ) throw SccpErrors.InvalidProofShape;
    parts.push(alpha);
    parts.push(beta);
    parts.push(gamma);
    parts.push(delta);
    var remainingIc = tail.ic;
    var index = 0;
    while (index < SCCP_GROTH16_VK_IC_COUNT) {
        assert (remainingIc.exists(index as uint8)) throw SccpErrors.InvalidProofShape;
        val point = ordinaryPointSlice(remainingIc.mustGet(index as uint8), 384);
        assert (validG1(point)) throw SccpErrors.InvalidProofShape;
        parts.push(point);
        remainingIc.delete(index as uint8);
        index += 1;
    }
    assert (remainingIc.isEmpty() && parts.size() == 17) throw SccpErrors.InvalidProofShape;
    return sha256Seventeen(parts);
}

/// Domain-separate one raw SCCP role into the BLS12-381 Fr public-signal schema.
/// This intentionally does not reuse the BN254 destination mapping.
fun bls12381SignalWord(label: slice, raw: uint256): uint256 {
    val labelHash = sha256One(label);
    val preimage = beginCell().storeUint(labelHash, 256).storeUint(raw, 256).endCell();
    return sha256One(preimage.beginParse()) % SCCP_BLS12_381_SCALAR_FIELD;
}

fun signalMessageId(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:message-id:v1".literalSlice(),
        raw,
    );
}

fun signalPayloadHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:payload-hash:v1".literalSlice(),
        raw,
    );
}

fun signalTargetDomain(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:target-domain:v1".literalSlice(),
        raw,
    );
}

fun signalCommitmentRoot(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:commitment-root:v1".literalSlice(),
        raw,
    );
}

fun signalFinalityHeight(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:finality-height:v1".literalSlice(),
        raw,
    );
}

fun signalFinalityBlockHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:finality-block-hash:v1".literalSlice(),
        raw,
    );
}

fun signalSourceDomain(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:source-domain:v1".literalSlice(),
        raw,
    );
}

fun signalStatementHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:statement-hash:v1".literalSlice(),
        raw,
    );
}

fun signalDestinationBindingHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:destination-binding-hash:v1".literalSlice(),
        raw,
    );
}

fun signalRouteConfigurationHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:route-config-hash:v1".literalSlice(),
        raw,
    );
}

fun signalSoraFinalityAnchorHash(raw: uint256): uint256 {
    return bls12381SignalWord(
        "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1".literalSlice(),
        raw,
    );
}

/// Full standalone verification, including the canonical byte commitment of
/// the key. Canonical StateInit construction performs this once.
fun verifyTairaDestinationProof(
    policy: DestinationProofPolicy,
    publicSignalsCell: cell,
    proofCell: cell,
    verifyingKeyCell: cell,
): bool {
    if (canonicalBls12381VerifyingKeyHash(verifyingKeyCell) != policy.verifyingKeyHash) {
        return false;
    }
    return verifyTairaDestinationProofWithBoundKey(
        policy,
        publicSignalsCell,
        proofCell,
        verifyingKeyCell,
    );
}

/// Runtime proof verification for a key whose exact representation and
/// canonical byte hash are already immutable in the governed StateInit.
fun verifyTairaDestinationProofWithBoundKey(
    policy: DestinationProofPolicy,
    publicSignalsCell: cell,
    proofCell: cell,
    verifyingKeyCell: cell,
): bool {
    if (
        policy.messageId == 0 || policy.payloadHash == 0 || policy.statementHash == 0 ||
        policy.routeConfigurationHash == 0 ||
        policy.destinationBindingHash == 0 ||
        policy.semanticProofProfileHash == 0 ||
        policy.soraFinalityAnchorHash == 0 ||
        policy.verifyingKeyHash == 0 ||
        policy.verifyingKeyCellHash == 0 ||
        verifyingKeyCell.hash() != policy.verifyingKeyCellHash
    ) {
        return false;
    }

    val signals0 = DestinationPublicSignals0.fromCell(publicSignalsCell);
    val signals1 = signals0.next.load();
    val signals2 = signals1.next.load();
    val signals3 = signals2.next.load();
    if (
        signals0.messageId != signalMessageId(policy.messageId) ||
        signals0.payloadHash != signalPayloadHash(policy.payloadHash) ||
        signals0.targetDomain != signalTargetDomain(SCCP_DOMAIN_TON) ||
        signals2.sourceDomain != signalSourceDomain(SCCP_DOMAIN_SORA) ||
        signals2.statementHash != signalStatementHash(policy.statementHash) ||
        signals2.destinationBindingHash !=
        signalDestinationBindingHash(policy.destinationBindingHash) ||
        signals3.routeConfigurationHash !=
        signalRouteConfigurationHash(policy.routeConfigurationHash) ||
        signals3.soraFinalityAnchorHash !=
        signalSoraFinalityAnchorHash(policy.soraFinalityAnchorHash)
    ) {
        return false;
    }

    // All eleven wrapper inputs are canonical BLS12-381 scalars. The commitment/finality fields
    // not directly interpreted by TVM remain bound by the audited wrapper circuit.
    if (
        signals0.messageId >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals0.payloadHash >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals0.targetDomain >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals1.commitmentRoot >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals1.finalityHeight >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals1.finalityBlockHash >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals2.sourceDomain >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals2.statementHash >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals2.destinationBindingHash >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals3.routeConfigurationHash >= SCCP_BLS12_381_SCALAR_FIELD ||
        signals3.soraFinalityAnchorHash >= SCCP_BLS12_381_SCALAR_FIELD
    ) {
        return false;
    }

    val key = Groth16Bls12381VerificationKey.fromCell(verifyingKeyCell);
    if (key.version != 1 || key.publicSignalCount != SCCP_GROTH16_PUBLIC_SIGNAL_COUNT) {
        return false;
    }
    val keyTail = key.tail.load();
    val alpha = ordinaryPointSlice(key.alphaG1, 384);
    val beta = ordinaryPointSlice(key.betaG2, 768);
    val gamma = ordinaryPointSlice(key.gammaG2, 768);
    val delta = ordinaryPointSlice(keyTail.deltaG2, 768);
    // Canonical StateInit construction already checked every immutable key
    // point for subgroup membership/non-identity before committing both key
    // hashes. Repeating sixteen native subgroup checks here exceeds TON's
    // per-transaction gas ceiling; only untrusted proof points are rechecked.

    val ic0 = ordinaryPointSlice(keyTail.ic.mustGet(0), 384);
    val ic1 = ordinaryPointSlice(keyTail.ic.mustGet(1), 384);
    val ic2 = ordinaryPointSlice(keyTail.ic.mustGet(2), 384);
    val ic3 = ordinaryPointSlice(keyTail.ic.mustGet(3), 384);
    val ic4 = ordinaryPointSlice(keyTail.ic.mustGet(4), 384);
    val ic5 = ordinaryPointSlice(keyTail.ic.mustGet(5), 384);
    val ic6 = ordinaryPointSlice(keyTail.ic.mustGet(6), 384);
    val ic7 = ordinaryPointSlice(keyTail.ic.mustGet(7), 384);
    val ic8 = ordinaryPointSlice(keyTail.ic.mustGet(8), 384);
    val ic9 = ordinaryPointSlice(keyTail.ic.mustGet(9), 384);
    val ic10 = ordinaryPointSlice(keyTail.ic.mustGet(10), 384);
    val ic11 = ordinaryPointSlice(keyTail.ic.mustGet(11), 384);
    val proof = Groth16Bls12381Proof.fromCell(proofCell);
    val proofA = ordinaryPointSlice(proof.a, 384);
    val proofB = ordinaryPointSlice(proof.b, 768);
    val proofC = ordinaryPointSlice(proof.c, 384);
    if (!validG1(proofA) || !validG2(proofB) || !validG1(proofC)) {
        return false;
    }

    val vkXFirst = blsG1MultiExp6(
        ic0,
        1,
        ic1,
        signals0.messageId,
        ic2,
        signals0.payloadHash,
        ic3,
        signals0.targetDomain,
        ic4,
        signals1.commitmentRoot,
        ic5,
        signals1.finalityHeight,
    );
    val vkXSecond = blsG1MultiExp6(
        ic6,
        signals1.finalityBlockHash,
        ic7,
        signals2.sourceDomain,
        ic8,
        signals2.statementHash,
        ic9,
        signals2.destinationBindingHash,
        ic10,
        signals3.routeConfigurationHash,
        ic11,
        signals3.soraFinalityAnchorHash,
    );
    val vkX = blsG1Add(vkXFirst, vkXSecond);
    return blsPairingCheck4(
        blsG1Neg(proofA),
        proofB,
        alpha,
        beta,
        vkX,
        gamma,
        proofC,
        delta,
    );
}
