// Emits the canonical minter StateInit vectors of `fixtures/sccp/ton_stateinit_v1.json`
// computed with the contracts' own types and compiled code (§5.3.1).
//
// Run by `scripts/generate_ton_sccp_stateinit_golden.py`, which recomputes
// every value independently in Python and fails on any difference. Output
// lines are `sccp-stateinit <vector> <key>=<value>` with lowercase hex.

import "@acton/build"
import "@acton/io"
import "@contracts/sccp-addresses"
import "@contracts/sccp-cells"
import "@contracts/sccp-storage"
import "@contracts/sccp-verify"

/// Member `i` of a golden roster: 20 bytes of value `i`.
fun goldenMember(i: int): int {
    var value = 0;
    repeat (20) {
        value = (value << 8) | i;
    }
    return value;
}

fun printCode(name: string, key: string) {
    val code = build(name);
    println("sccp-stateinit code {}_code_hash={:064x}", key, code.hash());
    println("sccp-stateinit code {}_code_depth={}", key, code.depth());
}

fun emitVector(
    label: string,
    networkId: int,
    revision: int,
    maxSupply: int,
    generation: int,
    validFromMs: int,
    validUntilMs: int,
    members: array<int>,
) {
    val n = members.size();
    val t = sccpThreshold(n);
    val membersCell = sccpBuildMembers(members);
    val digest = sccpRosterDigest(
        networkId,
        generation,
        validFromMs,
        validUntilMs,
        n,
        t,
        membersCell,
    );
    val config: MinterConfig = {
        tairaNetworkId: networkId,
        routeRevision: revision,
        maxSupply,
        initialGeneration: generation,
        initialDigest: digest,
        walletCode: build("SccpTairaXorWallet"),
        bucketCode: build("SccpConsumedBucket"),
    };
    val roster: RosterState = {
        digest,
        generation,
        validFromMs,
        validUntilMs,
        n,
        t,
        members: membersCell,
    };
    val storage: MinterStorage = {
        initialized: false,
        totalSupply: 0,
        pendingSupply: 0,
        outboundNonce: 0,
        opCount: 0,
        controlNonce: 0,
        mintingPaused: false,
        deployedBuckets: 0,
        config: config.toCell(),
        roster: roster.toCell(),
        prevRoster: null,
    };
    val data = storage.toCell();
    val code = build("SccpTairaXorMinter");
    val stateInitHash = StateInit.calcHashCodeData(code, data);
    val deployment: AutoDeployAddress = { workchain: BASECHAIN, stateInit: { code, data } };
    val (workchain, accountId) = deployment.calculateAddress().getWorkchainAndHash();
    println("sccp-stateinit {} roster_digest={:064x}", label, digest);
    println("sccp-stateinit {} data_hash={:064x}", label, data.hash());
    println("sccp-stateinit {} data_depth={}", label, (data as cell).depth());
    println("sccp-stateinit {} state_init_hash={:064x}", label, stateInitHash);
    println("sccp-stateinit {} workchain={}", label, workchain);
    println("sccp-stateinit {} account_id={:064x}", label, accountId);

    // derived child addresses under this minter
    val minter = address.fromWorkchainAndHash(workchain, accountId);
    val owner = address.fromWorkchainAndHash(
        0,
        0xabababababababababababababababababababababababababababababababab,
    );
    val (_, walletId) = sccpWalletAddress(
        owner,
        minter,
        build("SccpTairaXorWallet"),
    ).getWorkchainAndHash();
    val (_, bucket0) = sccpBucketAddress(
        minter,
        0,
        build("SccpConsumedBucket"),
    ).getWorkchainAndHash();
    val (_, bucket1) = sccpBucketAddress(
        minter,
        1,
        build("SccpConsumedBucket"),
    ).getWorkchainAndHash();
    println("sccp-stateinit {} wallet_of_abab={:064x}", label, walletId);
    println("sccp-stateinit {} bucket_0={:064x}", label, bucket0);
    println("sccp-stateinit {} bucket_1={:064x}", label, bucket1);
}

fun main() {
    printCode("SccpTairaXorMinter", "minter");
    printCode("SccpTairaXorWallet", "wallet");
    printCode("SccpConsumedBucket", "bucket");
    val networkId = 0x1111111111111111111111111111111111111111111111111111111111111111;

    var four: array<int> = [];
    var i = 1;
    while (i <= 4) {
        four.push(goldenMember(i));
        i += 1;
    }
    emitVector(
        "n4",
        networkId,
        1,
        1000000000000000000,
        7,
        1800000000000,
        1801209600000,
        four,
    );

    var thirtyOne: array<int> = [0, 0, 0];
    i = 1;
    while (i <= 28) {
        thirtyOne.push(goldenMember(i));
        i += 1;
    }
    emitVector(
        "n31",
        networkId,
        2,
        0xffffffffffffffffffffffff,
        12,
        1800000000000,
        1801209600000,
        thirtyOne,
    );
}
