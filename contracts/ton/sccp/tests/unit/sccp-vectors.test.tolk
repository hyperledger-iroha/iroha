// Golden vectors: the asm wrappers of §5.3.7 (GLOBALID, ECRECOVER, HASHEXT
// keccak) and every §3 encoding against values computed by the independent
// Python implementation (`sccp-test-vectors.gen.tolk`), plus the canonical
// cell shapes of §5.3.2 and their negatives.

import "@stdlib/exotic-cells"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/sccp-cells"
import "@contracts/sccp-constants"
import "@contracts/sccp-crypto"
import "@contracts/sccp-errors"
import "@contracts/sccp-messages"
import "@contracts/sccp-verify"
import "sccp-test-helpers"
import "sccp-test-vectors.gen"

get fun `test globalid is ton mainnet in the emulator`() {
    expect(sccpGlobalId()).toEqual(SCCP_TON_GLOBAL_ID);
}

get fun `test keccak256 wrappers match ethereum keccak`() {
    expect(
        keccak256Parts([]),
    ).toEqual(0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470);
    expect(keccak256Builder(beginCell().storeUint(0x53434350, 32))).toEqual(SCCP_NAME_HASH);
    expect(keccak256Parts(["SC".literalSlice(), beginCell().storeUint(0x4350, 16)])).toEqual(
        SCCP_NAME_HASH,
    );
    expect(
        keccak256Builders2(beginCell().storeUint(0x5343, 16), beginCell().storeUint(0x4350, 16)),
    ).toEqual(SCCP_NAME_HASH);
    expect(keccak256Builder(beginCell().storeUint(0x31, 8))).toEqual(SCCP_VERSION_HASH);
}

get fun `test ecrecover wrapper recovers an independent python signature`() {
    expect(sccpRecoverSigner(VEC_ECDSA_DIGEST, VEC_ECDSA_R, VEC_ECDSA_S, VEC_ECDSA_V)).toEqual(
        keyAddress(0),
    );
    // The raw opcode takes the recovery id, not the Ethereum v.
    val (h, x, y, ok) = ecrecoverRaw(VEC_ECDSA_DIGEST, VEC_ECDSA_V - 27, VEC_ECDSA_R, VEC_ECDSA_S);
    expect(ok).toBeTrue();
    expect(h!).toEqual(4);
    expect(sccpAddressOfPoint(x!, y!)).toEqual(keyAddress(0));
    // The failure branch is normalized to four values.
    val (h2, x2, y2, ok2) = ecrecoverRaw(1, 0, 0, 0);
    expect(ok2).toBeFalse();
    expect(h2 == null).toBeTrue();
    expect(x2 == null).toBeTrue();
    expect(y2 == null).toBeTrue();
}

get fun `test signature rules reject malformed signatures`() {
    val d = VEC_ECDSA_DIGEST;
    // v outside {27, 28}
    expect(sccpRecoverSigner(d, VEC_ECDSA_R, VEC_ECDSA_S, VEC_ECDSA_V - 27)).toEqual(0);
    expect(sccpRecoverSigner(d, VEC_ECDSA_R, VEC_ECDSA_S, 29)).toEqual(0);
    // high-S twin of a valid signature
    val flippedV = VEC_ECDSA_V == 27 ? 28 : 27;
    expect(sccpRecoverSigner(d, VEC_ECDSA_R, SECP256K1_N - VEC_ECDSA_S, flippedV)).toEqual(0);
    // r and s bounds
    expect(sccpRecoverSigner(d, 0, VEC_ECDSA_S, VEC_ECDSA_V)).toEqual(0);
    expect(sccpRecoverSigner(d, SECP256K1_N, VEC_ECDSA_S, VEC_ECDSA_V)).toEqual(0);
    expect(sccpRecoverSigner(d, VEC_ECDSA_R, 0, VEC_ECDSA_V)).toEqual(0);
    // a different digest recovers a different address
    expect(sccpRecoverSigner(d + 1, VEC_ECDSA_R, VEC_ECDSA_S, VEC_ECDSA_V)).toNotEqual(
        keyAddress(0),
    );
}

get fun `test fixed-nonce test signing recovers every test key`() {
    var key = 0;
    while (key < VEC_KEY_COUNT) {
        val digest = keccak256Builder(beginCell().storeUint(key, 32));
        val (r, s, v) = testSign(key, digest);
        expect(s <= SECP256K1_HALF_N).toBeTrue();
        expect(sccpRecoverSigner(digest, r, s, v)).toEqual(keyAddress(key));
        key += 1;
    }
    // keys are sorted by address
    var i = 1;
    while (i < VEC_KEY_COUNT) {
        expect(keyAddress(i) > keyAddress(i - 1)).toBeTrue();
        i += 1;
    }
}

get fun `test eip712 domain and attestation digest`() {
    expect(sccpDomainSeparator(VEC_NETWORK_ID)).toEqual(VEC_DOMAIN_SEPARATOR);
    val f = vecAttestationFields();
    val statement: TestStatement = {
        height: f.get(0),
        epoch: f.get(1),
        timestampMs: f.get(2),
        blockHash: f.get(3),
        sccpRoot: f.get(4),
        messageCount: f.get(5),
        historyRoot: f.get(6),
        historySize: f.get(7),
        rosterDigest: f.get(8),
        nextRosterDigest: f.get(9),
    };
    expect(statement.digest(VEC_NETWORK_ID)).toEqual(VEC_ATTESTATION_DIGEST);
}

get fun `test roster digests with keyless slots and multiple chunks`() {
    val r4 = makeRoster(
        7,
        4,
        0,
        0,
        1700000000000,
        1701209600000,
    );
    expect(r4.members).toEqual(vecRoster4Members());
    expect(r4.digest(VEC_NETWORK_ID)).toEqual(VEC_ROSTER4_DIGEST);
    val r7 = makeRoster(
        8,
        7,
        2,
        0,
        1700000000000,
        1701209600000,
    );
    expect(r7.members).toEqual(vecRoster7Members());
    expect(r7.digest(VEC_NETWORK_ID)).toEqual(VEC_ROSTER7_DIGEST);
    val r31 = makeRoster(
        9,
        31,
        3,
        0,
        1700000000000,
        1701209600000,
    );
    expect(r31.digest(VEC_NETWORK_ID)).toEqual(VEC_ROSTER31_DIGEST);
    val (members, parts) = sccpLoadMembers(r31.membersCell(), 31);
    expect(members).toEqual(r31.members);
    expect(parts.size()).toEqual(6);
}

get fun `test roster digest rejects bad n, t, order and validity`() {
    val base = makeRoster(
        7,
        4,
        0,
        0,
        1700000000000,
        1701209600000,
    );
    val members = base.membersCell();
    try {
        sccpRosterDigest(
            VEC_NETWORK_ID,
            7,
            1,
            2,
            4,
            2,
            members,
        );
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadRoster as int);
    }
    try {
        sccpRosterDigest(
            VEC_NETWORK_ID,
            7,
            2,
            2,
            4,
            3,
            members,
        );
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadRoster as int);
    }
    try {
        sccpRosterDigest(
            VEC_NETWORK_ID,
            0,
            1,
            2,
            4,
            3,
            members,
        );
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadRoster as int);
    }
    // n = 3 is below the minimum
    try {
        sccpRosterDigest(
            VEC_NETWORK_ID,
            7,
            1,
            2,
            3,
            3,
            sccpBuildMembers([keyAddress(0), keyAddress(1), keyAddress(2)]),
        );
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadRoster as int);
    }
    // descending, duplicate and zero-after-nonzero orders
    val orders: array<array<int>> = [
        [keyAddress(1), keyAddress(0), keyAddress(2), keyAddress(3)],
        [keyAddress(0), keyAddress(0), keyAddress(2), keyAddress(3)],
        [keyAddress(0), 0, keyAddress(2), keyAddress(3)],
    ];
    var i = 0;
    while (i < orders.size()) {
        try {
            sccpRosterDigest(
                VEC_NETWORK_ID,
                7,
                1,
                2,
                4,
                3,
                sccpBuildMembers(orders.get(i)),
            );
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadRoster as int);
        }
        i += 1;
    }
}

get fun `test member chunks reject non-canonical shapes`() {
    // five members in two chunks (3 + 2) instead of one
    val second = beginCell()
        .storeUint(keyAddress(3), 160)
        .storeUint(keyAddress(4), 160)
        .storeMaybeRef(null)
        .endCell();
    val first = beginCell()
        .storeUint(keyAddress(0), 160)
        .storeUint(keyAddress(1), 160)
        .storeUint(keyAddress(2), 160)
        .storeMaybeRef(second)
        .endCell();
    try {
        sccpLoadMembers(first, 5);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadSnake as int);
    }
    // a trailing reference on the last chunk
    val extra = beginCell()
        .storeUint(keyAddress(0), 160)
        .storeUint(keyAddress(1), 160)
        .storeUint(keyAddress(2), 160)
        .storeUint(keyAddress(3), 160)
        .storeMaybeRef(createEmptyCell())
        .endCell();
    try {
        sccpLoadMembers(extra, 4);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadSnake as int);
    }
}

/// Seven members in canonical chunks (6 + 1) whose second chunk is a library
/// reference to the canonical last chunk.
fun sevenMembersWithLibraryTail(): cell {
    val tail = beginCell().storeUint(keyAddress(6), 160).storeMaybeRef(null).endCell();
    testing.registerLibrary(tail);
    var head = beginCell();
    var i = 0;
    while (i < SCCP_MEMBERS_PER_CHUNK) {
        head.storeUint(keyAddress(i), 160);
        i += 1;
    }
    return head.storeMaybeRef(tail.toLibraryReference()).endCell();
}

get fun `test canonical parsers reject library cells with canonical content`() {
    // the ordinary encodings parse
    val members4 = sccpBuildMembers([keyAddress(0), keyAddress(1), keyAddress(2), keyAddress(3)]);
    expect(sccpLoadMembers(members4, 4).0.size()).toEqual(4);
    val hashes: array<int> = [1, 2, 3, 4];
    val path = sccpBuildHashPath(hashes)!;
    expect(sccpLoadHashPath(path, 4, 9)).toEqual(hashes);
    val snake = snakeOfLength(200);
    expect(sccpParseSnakeBytes(snake, 4096, SccpErrors.BadPayload).1).toEqual(200);
    // a published library with the same content is refused at every depth
    testing.registerLibrary(members4);
    testing.registerLibrary(path);
    testing.registerLibrary(snake);
    val pathTail = path.beginParse().preloadRef();
    testing.registerLibrary(pathTail);
    val snakeTail = snake.beginParse().preloadRef();
    testing.registerLibrary(snakeTail);
    val memberCases: array<[cell, int]> = [
        [members4.toLibraryReference(), 4],
        [sevenMembersWithLibraryTail(), 7],
    ];
    var i = 0;
    while (i < memberCases.size()) {
        val [c, n] = memberCases.get(i);
        try {
            sccpLoadMembers(c, n);
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadSnake as int);
        }
        i += 1;
    }
    val pathCases: array<cell> = [
        path.toLibraryReference(),
        beginCell()
            .storeUint(1, 256)
            .storeUint(2, 256)
            .storeUint(3, 256)
            .storeMaybeRef(pathTail.toLibraryReference())
            .endCell(),
    ];
    i = 0;
    while (i < pathCases.size()) {
        try {
            sccpLoadHashPath(pathCases.get(i), 4, 9);
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadSnake as int);
        }
        i += 1;
    }
    val snakeCases: array<cell> = [
        snake.toLibraryReference(),
        beginCell()
            .storeSlice(snake.beginParse().preloadBits(SCCP_SNAKE_CHUNK_BITS))
            .storeRef(snakeTail.toLibraryReference())
            .endCell(),
    ];
    i = 0;
    while (i < snakeCases.size()) {
        try {
            sccpParseSnakeBytes(snakeCases.get(i), 4096, SccpErrors.BadPayload);
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadSnake as int);
        }
        i += 1;
    }
}

get fun `test control leaves for the ton target`() {
    val destination = 0x2222222222222222222222222222222222222222222222222222222222222222;
    expect(sccpControlLeaf(VEC_NETWORK_ID, destination, 1, 1, true)).toEqual(VEC_CONTROL_LEAF_1);
    expect(sccpControlLeaf(VEC_NETWORK_ID, destination, 1, 2, false)).toEqual(VEC_CONTROL_LEAF_2);
}

get fun `test inbound payload hash, message id and transfer leaf`() {
    val payload = snakeFromPieces(vecInboundPayloadPieces());
    val parsed = sccpParseInboundPayload(payload, 1, 0x1234);
    expect(parsed.nonce).toEqual(77);
    expect(parsed.deadlineMs).toEqual(1700000600000);
    expect(parsed.amount).toEqual(123456789000);
    expect(
        parsed.recipientHash,
    ).toEqual(0x3333333333333333333333333333333333333333333333333333333333333333);
    expect(parsed.payloadHash).toEqual(VEC_INBOUND_PAYLOAD_HASH);
    val messageId = sccpInboundMessageId(VEC_NETWORK_ID, parsed.payloadHash);
    expect(messageId).toEqual(VEC_INBOUND_MESSAGE_ID);
    val destination = 0x2222222222222222222222222222222222222222222222222222222222222222;
    expect(sccpTransferLeaf(messageId, destination)).toEqual(VEC_INBOUND_LEAF);
}

get fun `test outbound payload of a burn`() {
    val recipient = snakeFromPieces(vecOutboundRecipientPieces());
    val (chunks, len) = sccpParseSnakeBytes(
        recipient,
        SCCP_MAX_TAIRA_ACCOUNT_BYTES,
        SccpErrors.BadRecipient,
    );
    expect(len).toEqual(40);
    val built = sccpBuildOutboundPayload(
        5,
        1,
        987654321,
        0x4444444444444444444444444444444444444444444444444444444444444444,
        chunks,
        len,
    );
    expect(built.payloadHash).toEqual(VEC_OUTBOUND_PAYLOAD_HASH);
    expect(built.cell.hash()).toEqual(snakeFromPieces(vecOutboundPayloadPieces()).hash());
    expect(sccpOutboundMessageId(VEC_NETWORK_ID, built.payloadHash)).toEqual(
        VEC_OUTBOUND_MESSAGE_ID,
    );
}

get fun `test merkle roots, promoted nodes and history leaves`() {
    val leaves = vecMerkleLeaves();
    expect(treeRoot(leaves)).toEqual(VEC_MERKLE_ROOT5);
    expect(treePath(leaves, 3)).toEqual(vecMerklePath5Index3());
    expect(treePath(leaves, 4)).toEqual(vecMerklePath5Index4());
    expect(sccpMerkleRoot(leaves.get(3), 3, 5, vecMerklePath5Index3())).toEqual(VEC_MERKLE_ROOT5);
    expect(sccpMerkleRoot(leaves.get(4), 4, 5, vecMerklePath5Index4())).toEqual(VEC_MERKLE_ROOT5);
    expect(sccpMerkleRoot(leaves.get(0), 0, 1, [])).toEqual(leaves.get(0));
    expect(
        sccpHistoryLeaf(42, 0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee, 3),
    ).toEqual(VEC_HISTORY_LEAF);
    // every count 1..=17 and every index round-trips
    var count = 1;
    while (count <= 17) {
        var all: array<int> = [];
        var i = 0;
        while (i < count) {
            all.push(fillerLeaf(i));
            i += 1;
        }
        val root = treeRoot(all);
        i = 0;
        while (i < count) {
            expect(sccpMerkleRoot(all.get(i), i, count, treePath(all, i))).toEqual(root);
            i += 1;
        }
        count += 1;
    }
}

get fun `test merkle verification rejects bad index, count and path length`() {
    val leaves = vecMerkleLeaves();
    val path = vecMerklePath5Index3();
    val bad: array<[int, int]> = [[5, 5], [3, 0], [3, 4], [-1, 5]];
    var i = 0;
    while (i < bad.size()) {
        val [index, count] = bad.get(i);
        try {
            val root = sccpMerkleRoot(leaves.get(3), index, count, path);
            expect(root == VEC_MERKLE_ROOT5).toBeFalse();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadProof as int);
        }
        i += 1;
    }
    var longer = path;
    longer.push(1);
    try {
        sccpMerkleRoot(leaves.get(3), 3, 5, longer);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadProof as int);
    }
}

get fun `test hash paths are canonical`() {
    val hashes: array<int> = [1, 2, 3, 4, 5];
    val path = sccpBuildHashPath(hashes);
    expect(sccpLoadHashPath(path, 5, 9)).toEqual(hashes);
    expect(sccpLoadHashPath(null, 0, 9)).toEqual([]);
    // wrong length, missing and superfluous cells, over-long paths
    val cases: array<[cell?, int, int]> = [
        [path, 4, 9],
        [path, 6, 9],
        [null, 1, 9],
        [path, 0, 9],
        [path, 5, 4],
    ];
    var i = 0;
    while (i < cases.size()) {
        val [c, len, max] = cases.get(i);
        try {
            sccpLoadHashPath(c, len, max);
            expect(false).toBeTrue();
        } catch (e) {
            expect(
                (e == SccpErrors.BadSnake as int) | (e == SccpErrors.BadProof as int),
            ).toBeTrue();
        }
        i += 1;
    }
    // two hashes in the first chunk instead of three
    val nonMaximal = beginCell()
        .storeUint(1, 256)
        .storeUint(2, 256)
        .storeMaybeRef(
            beginCell().storeUint(3, 256).storeUint(4, 256).storeMaybeRef(null).endCell(),
        )
        .endCell();
    try {
        sccpLoadHashPath(nonMaximal, 4, 9);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadSnake as int);
    }
}

get fun `test snake bytes accept only the canonical shape`() {
    val canonical = snakeOfLength(300);
    val (chunks, len) = sccpParseSnakeBytes(canonical, 4096, SccpErrors.BadPayload);
    expect(len).toEqual(300);
    expect(chunks.size()).toEqual(3);
    // 1 byte and exactly 127 bytes are single chunks
    expect(sccpParseSnakeBytes(snakeOfLength(1), 4096, 1).1).toEqual(1);
    expect(sccpParseSnakeBytes(snakeOfLength(127), 4096, 1).0.size()).toEqual(1);
    val negatives: array<cell> = [
        // empty root
        createEmptyCell(),
        // non-final chunk shorter than 127 bytes
        beginCell().storeUint(0xAB, 8).storeRef(beginCell().storeUint(0xCD, 8).endCell()).endCell(),
        // empty final chunk
        beginCell()
            .storeSlice(snakeOfLength(127).beginParse())
            .storeRef(createEmptyCell())
            .endCell(),
        // non-byte-aligned final chunk
        beginCell().storeUint(1, 9).endCell(),
        // two references
        beginCell()
            .storeUint(0xAB, 8)
            .storeRef(createEmptyCell())
            .storeRef(createEmptyCell())
            .endCell(),
    ];
    var i = 0;
    while (i < negatives.size()) {
        try {
            sccpParseSnakeBytes(negatives.get(i), 4096, SccpErrors.BadPayload);
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(SccpErrors.BadSnake as int);
        }
        i += 1;
    }
    // the length bound uses the caller's error
    try {
        sccpParseSnakeBytes(snakeOfLength(1025), 1024, SccpErrors.BadRecipient);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadRecipient as int);
    }
    expect(sccpParseSnakeBytes(snakeOfLength(1024), 1024, SccpErrors.BadRecipient).1).toEqual(1024);
}

get fun `test inbound payload rules`() {
    val minter = 0x77;
    val good = inboundPayloadCell(1, 1, 5, 10, 0x33);
    expect(sccpParseInboundPayload(good, 1, minter).nonce).toEqual(1);
    // revision mismatch, zero deadline, zero amount, amount 2^96, recipient cases
    val cases: array<[cell, int]> = [
        [inboundPayloadCell(1, 2, 5, 10, 0x33), SccpErrors.BadPayload as int],
        [inboundPayloadCell(1, 1, 0, 10, 0x33), SccpErrors.BadPayload as int],
        [inboundPayloadCell(1, 1, 5, 0, 0x33), SccpErrors.BadAmount as int],
        [inboundPayloadCell(1, 1, 5, SCCP_TON_AMOUNT_LIMIT, 0x33), SccpErrors.BadAmount as int],
        [inboundPayloadCell(1, 1, 5, 10, 0), SccpErrors.BadRecipient as int],
        [inboundPayloadCell(1, 1, 5, 10, minter), SccpErrors.BadRecipient as int],
    ];
    var i = 0;
    while (i < cases.size()) {
        val [payload, code] = cases.get(i);
        try {
            sccpParseInboundPayload(payload, 1, minter);
            expect(false).toBeTrue();
        } catch (e) {
            expect(e).toEqual(code);
        }
        i += 1;
    }
    // trailing byte and truncation
    var w = SnakeWriter.create();
    val (chunks, _) = sccpParseSnakeBytes(good, 4096, 1);
    var j = 0;
    while (j < chunks.size()) {
        w.writeSlice(chunks.get(j));
        j += 1;
    }
    w.writeUint(0, 1);
    try {
        sccpParseInboundPayload(sccpSnakeCellFromChunks(w.chunks()), 1, minter);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadPayload as int);
    }
    try {
        sccpParseInboundPayload(snakeOfLength(20), 1, minter);
        expect(false).toBeTrue();
    } catch (e) {
        expect(e).toEqual(SccpErrors.BadPayload as int);
    }
}
