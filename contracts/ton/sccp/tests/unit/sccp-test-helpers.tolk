// Shared fixtures of the SCCP v1 TON emulator suite: test keys and signing,
// rosters, attestations, commitment and history trees, payloads, minter
// deployment and message senders.

import "@stdlib/gas-payments"
import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/sccp-addresses"
import "@contracts/sccp-cells"
import "@contracts/sccp-constants"
import "@contracts/sccp-crypto"
import "@contracts/sccp-errors"
import "@contracts/sccp-fees"
import "@contracts/sccp-messages"
import "@contracts/sccp-storage"
import "@contracts/sccp-verify"
import "@wrappers/SccpConsumedBucket.gen"
import "@wrappers/SccpTairaXorMinter.gen"
import "@wrappers/SccpTairaXorWallet.gen"
import "sccp-test-vectors.gen"

/// Fixed emulator time (seconds) at which every test starts.
const T0 = 1800000000
const DAY_MS = 86400000
const HOUR_MS = 3600000
const TEST_REVISION = 1
const TEST_MAX_SUPPLY = 1000000000000000000
const TEST_SENDER_LEN = 35

// ---------------------------------------------------------------------------
// Keys and signatures.

/// `(a + b) mod N` for `a, b < N` without leaving int257.
fun addModN(a: int, b: int): int {
    return a >= SECP256K1_N - b ? a - (SECP256K1_N - b) : a + b;
}

/// Signs `digest` with test key `key` using its fixed nonce; returns a
/// low-S `(r, s, v)` with `v ∈ {27, 28}`.
fun testSign(key: int, digest: int): (int, int, int) {
    val d = vecKeyPrivates().get(key);
    val r = vecKeyR().get(key);
    val kInverse = vecKeyKInverse().get(key);
    var recid = vecKeyParity().get(key);
    val (_, z) = divMod(digest, SECP256K1_N);
    val (_, rd) = mulDivMod(r, d, SECP256K1_N);
    val (_, s0) = mulDivMod(kInverse, addModN(z, rd), SECP256K1_N);
    var s = s0;
    if (s > SECP256K1_HALF_N) {
        s = SECP256K1_N - s;
        recid = recid ^ 1;
    }
    return (r, s, 27 + recid);
}

fun keyAddress(key: int): int {
    return vecKeyAddresses().get(key);
}

// ---------------------------------------------------------------------------
// Rosters.

/// A roster generation: members in §3.7 order and, per slot, the test key
/// (or -1 for a keyless zero slot).
struct TestRoster {
    generation: int
    validFromMs: int
    validUntilMs: int
    members: array<int>
    keys: array<int>
}

/// `zeros` keyless slots followed by test keys `firstKey .. firstKey + n − zeros − 1`.
fun makeRoster(
    generation: int,
    n: int,
    zeros: int,
    firstKey: int,
    validFromMs: int,
    validUntilMs: int,
): TestRoster {
    var members: array<int> = [];
    var keys: array<int> = [];
    var i = 0;
    while (i < n) {
        if (i < zeros) {
            members.push(0);
            keys.push(-1);
        } else {
            val key = firstKey + i - zeros;
            members.push(keyAddress(key));
            keys.push(key);
        }
        i += 1;
    }
    return {
        generation,
        validFromMs,
        validUntilMs,
        members,
        keys,
    };
}

fun TestRoster.n(self): int {
    return self.members.size();
}

fun TestRoster.t(self): int {
    return sccpThreshold(self.members.size());
}

fun TestRoster.membersCell(self): cell {
    return sccpBuildMembers(self.members);
}

fun TestRoster.digest(self, networkId: int): int {
    return sccpRosterDigest(
        networkId,
        self.generation,
        self.validFromMs,
        self.validUntilMs,
        self.n(),
        self.t(),
        self.membersCell(),
    );
}

fun TestRoster.spec(self): RosterSpec {
    return {
        generation: self.generation,
        validFromMs: self.validFromMs,
        validUntilMs: self.validUntilMs,
        n: self.n(),
        t: self.t(),
        members: self.membersCell(),
    };
}

/// Signature cell chain for explicit `(slot, r, s, v)` rows (ascending).
fun signatureCellsFromRows(bitmap: int, rows: array<[int, int, int]>): cell {
    var next: cell? = null;
    var i = rows.size() - 1;
    while (i >= 0) {
        val [r, s, v] = rows.get(i);
        next = beginCell()
            .storeUint(r, 256)
            .storeUint(s, 256)
            .storeUint(v, 8)
            .storeMaybeRef(next)
            .endCell();
        i -= 1;
    }
    return beginCell().storeUint(bitmap, 32).storeRef(next!).endCell();
}

/// Signature set over `digest` by the given roster slots (ascending).
fun signBySlots(roster: TestRoster, digest: int, slots: array<int>): cell {
    var bitmap = 0;
    var rows: array<[int, int, int]> = [];
    var i = 0;
    while (i < slots.size()) {
        val slot = slots.get(i);
        bitmap |= 1 << slot;
        val (r, s, v) = testSign(roster.keys.get(slot), digest);
        rows.push([r, s, v]);
        i += 1;
    }
    return signatureCellsFromRows(bitmap, rows);
}

/// The first `count` keyed slots of a roster.
fun keyedSlots(roster: TestRoster, count: int): array<int> {
    var slots: array<int> = [];
    var i = 0;
    while ((i < roster.n()) & (slots.size() < count)) {
        if (roster.keys.get(i) >= 0) {
            slots.push(i);
        }
        i += 1;
    }
    return slots;
}

/// Signature set by exactly `t` keyed members.
fun signByThreshold(roster: TestRoster, digest: int): cell {
    return signBySlots(roster, digest, keyedSlots(roster, roster.t()));
}

// ---------------------------------------------------------------------------
// Trees.

/// Promote-odd root (§3.4) of `leaves`.
fun treeRoot(leaves: array<int>): int {
    var level = leaves;
    while (level.size() > 1) {
        var next: array<int> = [];
        var i = 0;
        while (i + 1 < level.size()) {
            next.push(sccpNode(level.get(i), level.get(i + 1)));
            i += 2;
        }
        if (level.size() % 2 == 1) {
            next.push(level.get(level.size() - 1));
        }
        level = next;
    }
    return level.get(0);
}

/// Sibling path of leaf `index`.
fun treePath(leaves: array<int>, index: int): array<int> {
    var path: array<int> = [];
    var level = leaves;
    var i = index;
    while (level.size() > 1) {
        val sibling = i ^ 1;
        if (sibling < level.size()) {
            path.push(level.get(sibling));
        }
        var next: array<int> = [];
        var j = 0;
        while (j + 1 < level.size()) {
            next.push(sccpNode(level.get(j), level.get(j + 1)));
            j += 2;
        }
        if (level.size() % 2 == 1) {
            next.push(level.get(level.size() - 1));
        }
        level = next;
        i = i >> 1;
    }
    return path;
}

/// Deterministic filler leaf `i`.
fun fillerLeaf(i: int): int {
    return keccak256Builder(beginCell().storeUint(0xF1, 8).storeUint(i, 32));
}

/// `count` filler leaves with `leaf` at `index`.
fun leavesWith(leaf: int, index: int, count: int): array<int> {
    var leaves: array<int> = [];
    var i = 0;
    while (i < count) {
        leaves.push(i == index ? leaf : fillerLeaf(i));
        i += 1;
    }
    return leaves;
}

// ---------------------------------------------------------------------------
// Attestations.

struct TestStatement {
    height: int
    epoch: int
    timestampMs: int
    blockHash: int
    sccpRoot: int
    messageCount: int
    historyRoot: int
    historySize: int
    rosterDigest: int
    nextRosterDigest: int
}

fun TestStatement.attestation(self): Attestation {
    val tail: AttestationTail = {
        historyRoot: self.historyRoot,
        rosterDigest: self.rosterDigest,
        nextRosterDigest: self.nextRosterDigest,
    };
    return {
        height: self.height,
        epoch: self.epoch,
        timestampMs: self.timestampMs,
        messageCount: self.messageCount,
        historySize: self.historySize,
        blockHash: self.blockHash,
        sccpRoot: self.sccpRoot,
        tail: tail.toCell(),
    };
}

fun TestStatement.toCell(self): Cell<Attestation> {
    return self.attestation().toCell();
}

fun TestStatement.digest(self, networkId: int): int {
    val a = self.attestation();
    return sccpAttestationDigest(networkId, a, a.tail.load());
}

/// Statement of a block whose own leaves are `leaves`; the history is just
/// this block (size 1).
fun blockStatement(height: int, leaves: array<int>, rosterDigest: int): TestStatement {
    val root = treeRoot(leaves);
    return {
        height,
        epoch: 1,
        timestampMs: T0 * 1000 - 60000,
        blockHash: keccak256Builder(beginCell().storeUint(0xB0, 8).storeUint(height, 64)),
        sccpRoot: root,
        messageCount: leaves.size(),
        historyRoot: sccpHistoryLeaf(height, root, leaves.size()),
        historySize: 1,
        rosterDigest,
        nextRosterDigest: 0,
    };
}

/// History of `blocks` SCCP-bearing blocks where block `index` carries
/// `(root, count)` and the others are fillers; returns the history leaves.
fun historyLeaves(blocks: int, index: int, height: int, root: int, count: int): array<int> {
    var leaves: array<int> = [];
    var i = 0;
    while (i < blocks) {
        leaves.push(
            i == index
                ? sccpHistoryLeaf(height, root, count)
                : sccpHistoryLeaf(1000 + i, fillerLeaf(5000 + i), 1),
        );
        i += 1;
    }
    return leaves;
}

// ---------------------------------------------------------------------------
// Payloads.

/// Taira→TON payload (§3.2) with a 35-byte sender.
fun inboundPayloadCell(
    nonce: int,
    revision: int,
    deadlineMs: int,
    amount: int,
    recipientHash: int,
): cell {
    return inboundPayloadWithSender(
        nonce,
        revision,
        deadlineMs,
        amount,
        recipientHash,
        TEST_SENDER_LEN,
    );
}

/// Taira→TON payload (§3.2) with a `senderLen`-byte sender (1..=1024).
fun inboundPayloadWithSender(
    nonce: int,
    revision: int,
    deadlineMs: int,
    amount: int,
    recipientHash: int,
    senderLen: int,
): cell {
    var w = SnakeWriter.create();
    w.writeUint(SCCP_PAYLOAD_KIND_TRANSFER, 1);
    w.writeUint(SCCP_PAYLOAD_VERSION, 1);
    w.writeUint(SCCP_DOMAIN_TAIRA, 4);
    w.writeUint(SCCP_DOMAIN_TON, 4);
    w.writeUint(nonce, 8);
    w.writeUint(revision, 4);
    w.writeUint(deadlineMs, 8);
    w.writeUint(SCCP_DOMAIN_TAIRA, 4);
    w.writeUint(SCCP_CODEC_CANONICAL_TEXT, 1);
    w.writeUint(SCCP_ASSET_ID_XOR_LEN, 2);
    w.writeUint(SCCP_ASSET_ID_XOR, SCCP_ASSET_ID_XOR_LEN);
    w.writeUint(amount, 16);
    w.writeUint(SCCP_CODEC_TAIRA_ACCOUNT, 1);
    w.writeUint(senderLen, 2);
    var i = 1;
    while (i <= senderLen) {
        w.writeUint(i & 0xFF, 1);
        i += 1;
    }
    w.writeUint(SCCP_CODEC_TON_ACCOUNT36, 1);
    w.writeUint(SCCP_TON_ACCOUNT36_LEN, 2);
    w.writeUint(0, 4);
    w.writeUint(recipientHash, 32);
    w.writeUint(SCCP_CODEC_CANONICAL_TEXT, 1);
    w.writeUint(SCCP_ROUTE_ID_TON_LEN, 2);
    w.writeUint(SCCP_ROUTE_ID_TON, SCCP_ROUTE_ID_TON_LEN);
    return sccpSnakeCellFromChunks(w.chunks());
}

/// Snake cell of the concatenation of byte pieces.
fun snakeFromPieces(pieces: array<slice>): cell {
    var w = SnakeWriter.create();
    var i = 0;
    while (i < pieces.size()) {
        w.writeSlice(pieces.get(i));
        i += 1;
    }
    return sccpSnakeCellFromChunks(w.chunks());
}

/// Snake cell of `len` bytes `0x5A`.
fun snakeOfLength(len: int): cell {
    var w = SnakeWriter.create();
    repeat (len) {
        w.writeUint(0x5A, 1);
    }
    return sccpSnakeCellFromChunks(w.chunks());
}

/// `message_id`, leaf and parsed fields of an inbound payload for `minter`.
fun inboundLeaf(networkId: int, payload: cell, minter: address): int {
    val parsed = sccpParseInboundPayload(payload, TEST_REVISION, minter.accountId());
    return sccpTransferLeaf(
        sccpInboundMessageId(networkId, parsed.payloadHash),
        minter.accountId(),
    );
}

fun inboundMessageId(networkId: int, payload: cell, minter: address): int {
    val parsed = sccpParseInboundPayload(payload, TEST_REVISION, minter.accountId());
    return sccpInboundMessageId(networkId, parsed.payloadHash);
}

// ---------------------------------------------------------------------------
// Deployment.

struct Env {
    minter: SccpTairaXorMinter
    deployer: address
    networkId: int
    roster: TestRoster
}

fun Env.digest(self): int {
    return self.roster.digest(self.networkId);
}

/// Canonical initial data (§5.3.1) for `roster`.
fun canonicalStorage(
    networkId: int,
    revision: int,
    maxSupply: int,
    roster: TestRoster,
): MinterStorage {
    val digest = roster.digest(networkId);
    val config: MinterConfig = {
        tairaNetworkId: networkId,
        routeRevision: revision,
        maxSupply,
        initialGeneration: roster.generation,
        initialDigest: digest,
        walletCode: build("SccpTairaXorWallet"),
        bucketCode: build("SccpConsumedBucket"),
    };
    val state: RosterState = {
        digest,
        generation: roster.generation,
        validFromMs: roster.validFromMs,
        validUntilMs: roster.validUntilMs,
        n: roster.n(),
        t: roster.t(),
        members: roster.membersCell(),
    };
    return {
        initialized: false,
        totalSupply: 0,
        pendingSupply: 0,
        outboundNonce: 0,
        opCount: 0,
        controlNonce: 0,
        mintingPaused: false,
        deployedBuckets: 0,
        config: config.toCell(),
        roster: state.toCell(),
        prevRoster: null,
    };
}

/// The default generation-7 roster valid from one hour before `T0` for 14 days.
fun defaultRoster(n: int, zeros: int): TestRoster {
    return makeRoster(
        7,
        n,
        zeros,
        0,
        T0 * 1000 - HOUR_MS,
        T0 * 1000 + 14 * DAY_MS,
    );
}

/// Deploys `storage` with `sccp_init` in the deploy message.
fun deployWithInit(
    storage: MinterStorage,
    deployer: address,
    value: coins,
): (SccpTairaXorMinter, SendResultList) {
    val minter = SccpTairaXorMinter.fromStorage(storage);
    val res = net.send(
        deployer,
        createMessage({
            bounce: BounceMode.NoBounce,
            value,
            dest: { workchain: BASECHAIN, stateInit: minter.stateInit! },
            body: SccpInit { queryId: 0 },
        }),
    );
    return (minter, res);
}

/// Deployed and initialized minter with an n-member roster (`zeros` keyless).
fun setupEnvWith(n: int, zeros: int, maxSupply: int): Env {
    testing.setNow(T0);
    val deployer = testing.treasury("deployer");
    val roster = defaultRoster(n, zeros);
    val (minter, res) = deployWithInit(
        canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, maxSupply, roster),
        deployer.address,
        grams("10"),
    );
    expect(res).toHaveSuccessfulDeploy({ to: minter.address });
    expect(minter.getSccpState().initialized).toBeTrue();
    return {
        minter,
        deployer: deployer.address,
        networkId: VEC_NETWORK_ID,
        roster,
    };
}

fun setupEnv(): Env {
    return setupEnvWith(4, 0, TEST_MAX_SUPPLY);
}

// ---------------------------------------------------------------------------
// Finalize / void scenarios.

/// A payload with its single-block attestation and proof.
struct Delivery {
    payload: cell
    statement: TestStatement
    signatures: cell
    proof: MessageProof
    messageId: int
}

/// Builds a delivery of a Taira→TON transfer at `leafIndex` of a block of
/// `leafCount` leaves, signed by `t` members of `roster`.
fun deliver(
    env: Env,
    roster: TestRoster,
    nonce: int,
    amount: int,
    recipient: address,
    deadlineMs: int,
    leafIndex: int,
    leafCount: int,
): Delivery {
    val payload = inboundPayloadCell(
        nonce,
        TEST_REVISION,
        deadlineMs,
        amount,
        recipient.accountId(),
    );
    val leaf = inboundLeaf(env.networkId, payload, env.minter.address);
    val leaves = leavesWith(leaf, leafIndex, leafCount);
    val statement = blockStatement(100 + nonce, leaves, roster.digest(env.networkId));
    val path = treePath(leaves, leafIndex);
    return {
        payload,
        statement,
        signatures: signByThreshold(roster, statement.digest(env.networkId)),
        proof: {
            leafIndex,
            pathLen: path.size(),
            payload,
            path: sccpBuildHashPath(path),
        },
        messageId: inboundMessageId(env.networkId, payload, env.minter.address),
    };
}

/// Default delivery: leaf 1 of 3, deadline one day after `T0`.
fun deliverDefault(env: Env, nonce: int, amount: int, recipient: address): Delivery {
    return deliver(
        env,
        env.roster,
        nonce,
        amount,
        recipient,
        T0 * 1000 + DAY_MS,
        1,
        3,
    );
}

fun sendFinalize(env: Env, from: address, d: Delivery, value: coins): SendResultList {
    return net.send(
        from,
        createMessage({
            bounce: BounceMode.Only256BitsOfBody,
            value,
            dest: env.minter.address,
            body: SccpFinalize {
                queryId: 7,
                response: from,
                attestation: d.statement.toCell(),
                signatures: d.signatures,
                message: d.proof.toCell(),
            },
        }),
    );
}

fun finalizeValue(env: Env, nonce: int): coins {
    return env.minter.finalizeRequiredValue(nonce);
}

fun sendVoidExpired(
    env: Env,
    from: address,
    nonce: int,
    d: Delivery,
    value: coins,
): SendResultList {
    return net.send(
        from,
        createMessage({
            bounce: BounceMode.Only256BitsOfBody,
            value,
            dest: env.minter.address,
            body: SccpVoidExpired {
                queryId: 8,
                response: from,
                nonce,
                attestation: d.statement.toCell(),
                signatures: d.signatures,
                message: d.proof.toCell(),
            },
        }),
    );
}

/// Wallet of `owner` under `env.minter`.
fun walletOf(env: Env, owner: address): SccpTairaXorWallet {
    return SccpTairaXorWallet.fromAddress(env.minter.getWalletAddress(owner));
}

fun jettonBalanceOf(env: Env, owner: address): int {
    val wallet = walletOf(env, owner);
    if (!testing.isDeployed(wallet.address)) {
        return 0;
    }
    return wallet.getWalletData().jettonBalance;
}

fun bucketOf(env: Env, index: int): SccpConsumedBucket {
    return SccpConsumedBucket.fromAddress(env.minter.getBucketAddress(index));
}

fun isConsumedAt(env: Env, nonce: int): bool {
    val bucket = bucketOf(env, nonce >> SCCP_BUCKET_SHIFT);
    if (!testing.isDeployed(bucket.address)) {
        return false;
    }
    return bucket.isConsumed(nonce);
}

// ---------------------------------------------------------------------------
// Rotations.

/// The next generation after `current`, valid from `validFromMs` for 14 days,
/// with test keys starting at `firstKey`.
fun nextRoster(current: TestRoster, n: int, firstKey: int, validFromMs: int): TestRoster {
    return makeRoster(
        current.generation + 1,
        n,
        0,
        firstKey,
        validFromMs,
        validFromMs + 14 * DAY_MS,
    );
}

/// Rotation statement of a boundary block signed by `signer` announcing `next`.
fun rotationStatement(env: Env, signer: TestRoster, next: TestRoster, height: int): TestStatement {
    return {
        height,
        epoch: 3,
        timestampMs: next.validFromMs,
        blockHash: keccak256Builder(beginCell().storeUint(0xB1, 8).storeUint(height, 64)),
        sccpRoot: 0,
        messageCount: 0,
        historyRoot: sccpHistoryLeaf(1, fillerLeaf(1), 1),
        historySize: 1,
        rosterDigest: signer.digest(env.networkId),
        nextRosterDigest: next.digest(env.networkId),
    };
}

fun sendRotate(
    env: Env,
    statement: TestStatement,
    signatures: cell,
    next: RosterSpec,
): SendResultList {
    return env
        .minter
        .sendSccpRotate(
            env.deployer,
            4,
            env.deployer,
            statement.attestation(),
            signatures,
            next,
            { value: grams("1"), bounce: true },
        );
}

/// Rotates `env` to `next` with a statement signed by the current roster.
fun rotateTo(env: Env, next: TestRoster, height: int): SendResultList {
    val statement = rotationStatement(env, env.roster, next, height);
    val res = sendRotate(
        env,
        statement,
        signByThreshold(env.roster, statement.digest(env.networkId)),
        next.spec(),
    );
    expect(res).toHaveSuccessfulTx<SccpRotate>({ to: env.minter.address });
    return res;
}

// ---------------------------------------------------------------------------
// Controls.

/// §3.4 control leaf for an arbitrary target, destination and network.
fun controlLeafFor(
    networkId: int,
    targetTag: int,
    targetWord: int,
    destination: int,
    revision: int,
    nonce: int,
    paused: bool,
): int {
    return keccak256Builder(
        beginCell()
            .storeUint(SCCP_TAG_CONTROL_V1, SCCP_TAG_CONTROL_V1_BITS)
            .storeUint(SCCP_TAG_TAIRA, 8)
            .storeUint(networkId, 256)
            .storeUint(targetTag, 8)
            .storeUint(targetWord, 256)
            .storeUint(destination, 256)
            .storeUint(revision, 32)
            .storeUint(nonce, 64)
            .storeUint(paused ? 1 : 0, 8),
    );
}

/// A control proof with its single-block attestation.
struct ControlDelivery {
    statement: TestStatement
    signatures: cell
    proof: ControlProof
}

/// Control `(nonce, paused)` whose committed leaf is `leaf`, at leaf 0 of a
/// block of `count` leaves.
fun deliverControlLeaf(env: Env, leaf: int, nonce: int, paused: bool, count: int): ControlDelivery {
    val leaves = leavesWith(leaf, 0, count);
    val statement = blockStatement(500 + nonce, leaves, env.digest());
    val path = treePath(leaves, 0);
    return {
        statement,
        signatures: signByThreshold(env.roster, statement.digest(env.networkId)),
        proof: {
            controlNonce: nonce,
            paused,
            leafIndex: 0,
            pathLen: path.size(),
            path: sccpBuildHashPath(path),
        },
    };
}

/// A genuine control for `env.minter`.
fun deliverControl(env: Env, nonce: int, paused: bool): ControlDelivery {
    val leaf = sccpControlLeaf(
        env.networkId,
        env.minter.address.accountId(),
        TEST_REVISION,
        nonce,
        paused,
    );
    return deliverControlLeaf(env, leaf, nonce, paused, 2);
}

fun sendApplyControl(env: Env, c: ControlDelivery): SendResultList {
    return env
        .minter
        .sendSccpApplyControl(
            env.deployer,
            5,
            env.deployer,
            c.statement.attestation(),
            c.signatures,
            c.proof,
            { value: grams("1"), bounce: true },
        );
}

fun applyControlOk(env: Env, nonce: int, paused: bool): SendResultList {
    val res = sendApplyControl(env, deliverControl(env, nonce, paused));
    expect(res).toHaveSuccessfulTx<SccpApplyControl>({ to: env.minter.address });
    val state = env.minter.getSccpState();
    expect(state.mintingPaused).toEqual(paused);
    expect(state.controlNonce).toEqual(nonce);
    return res;
}

// ---------------------------------------------------------------------------
// Burns.

/// Sends an SCCP burn of `amount` from `owner`'s wallet to `recipient` bytes.
fun sendBurn(env: Env, owner: address, amount: int, recipient: cell, value: coins): SendResultList {
    val payload: SccpBurnToTaira = { recipient };
    return walletOf(env, owner).sendAskToBurn(
        owner,
        11,
        amount,
        owner,
        payload.toCell(),
        { value, bounce: true },
    );
}

fun burnValue(env: Env, owner: address): coins {
    return walletOf(env, owner).burnRequiredValue() + 1;
}

/// The body a `BounceMode.Only256BitsOfBody` bounce of `body` carries.
fun bouncedBodyOf<T>(body: T): cell {
    val s = body.toCell().beginParse();
    val bits = min(256, s.remainingBitsCount());
    return beginCell().storeUint(0xFFFFFFFF, 32).storeSlice(s.preloadBits(bits)).endCell();
}

/// A bounced message with `body`'s bounce body, delivered to `dest`.
fun bouncedMessage<T>(body: T, dest: address, value: coins): OutMessage {
    return createMessage({
        bounce: BounceMode.NoBounce,
        value,
        dest,
        body: bouncedBodyOf(body),
    }).bounced();
}

/// Re-sends the `index`-th out message of a single processed step.
fun forwardOut(step: SendResult, index: int): OutMessage {
    return { messageCell: step.outMessages.atCell(index) };
}

/// Finalizes nonce `nonce` for `recipient` and checks the mint.
fun finalizeOk(env: Env, nonce: int, amount: int, recipient: address): SendResultList {
    val before = jettonBalanceOf(env, recipient);
    val d = deliverDefault(env, nonce, amount, recipient);
    val res = sendFinalize(env, env.deployer, d, finalizeValue(env, nonce));
    expect(res).toHaveSuccessfulTx<SccpFinalize>({ to: env.minter.address });
    expect(res).toHaveSuccessfulTx<SccpConsumed>({ to: env.minter.address });
    expect(jettonBalanceOf(env, recipient)).toEqual(before + amount);
    return res;
}
