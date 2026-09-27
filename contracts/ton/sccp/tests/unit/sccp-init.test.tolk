// Deployment with canonical initial data and `sccp_init` (§5.3.1): the
// recomputed digest, generation and validity bounds, the chain identity, the
// initial immutables and counters, and every operation throwing before init.

import "@stdlib/exotic-cells"
import "@acton/build"
import "@acton/emulation/config"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/sccp-cells"
import "@contracts/sccp-constants"
import "@contracts/sccp-errors"
import "@contracts/sccp-messages"
import "@contracts/sccp-storage"
import "@wrappers/SccpTairaXorMinter.gen"
import "sccp-test-helpers"
import "sccp-test-vectors.gen"

/// Deploys `storage` without init (plain top-up) and returns the wrapper.
fun deployUninitialized(storage: MinterStorage): SccpTairaXorMinter {
    val deployer = testing.treasury("deployer");
    val minter = SccpTairaXorMinter.fromStorage(storage);
    val res = net.send(
        deployer.address,
        createMessage({
            bounce: BounceMode.NoBounce,
            value: grams("5"),
            dest: { workchain: BASECHAIN, stateInit: minter.stateInit! },
            body: TopUpTons { queryId: 0 },
        }),
    );
    expect(res).toHaveSuccessfulDeploy({ to: minter.address });
    return minter;
}

fun sendInit(minter: SccpTairaXorMinter): SendResultList {
    val deployer = testing.treasury("deployer");
    return minter.sendSccpInit(deployer.address, 0, { value: grams("1"), bounce: true });
}

get fun `test init records immutables and zero counters`() {
    val env = setupEnv();
    val state = env.minter.getSccpState();
    expect(state.initialized).toBeTrue();
    expect(state.tairaNetworkId).toEqual(VEC_NETWORK_ID);
    expect(state.routeRevision).toEqual(TEST_REVISION);
    expect(state.initialGeneration).toEqual(7);
    expect(state.initialDigest).toEqual(env.digest());
    expect(state.digest).toEqual(env.digest());
    expect(state.generation).toEqual(7);
    expect(state.validUntilMs).toEqual(env.roster.validUntilMs);
    expect(state.prevDigest).toEqual(0);
    expect(state.prevValidUntilMs).toEqual(0);
    expect(state.outboundNonce).toEqual(0);
    expect(state.opCount).toEqual(0);
    expect(state.controlNonce).toEqual(0);
    expect(state.mintingPaused).toBeFalse();
    expect(state.totalSupply).toEqual(0);
    expect(state.pendingSupply).toEqual(0);
    expect(state.maxSupply).toEqual(TEST_MAX_SUPPLY);
    expect(state.deployedBuckets).toEqual(0);
    expect(env.minter.getSccpMembers()).toEqual(env.roster.members);
    val jetton = env.minter.getJettonData();
    expect(jetton.totalSupply).toEqual(0);
    expect(jetton.mintable).toBeTrue();
    expect(jetton.adminAddress == null).toBeTrue();
    expect(jetton.jettonWalletCode.hash()).toEqual(build("SccpTairaXorWallet").hash());
}

get fun `test init works for a 31-member roster with keyless slots`() {
    val env = setupEnvWith(31, 3, TEST_MAX_SUPPLY);
    expect(env.minter.getSccpMembers()).toEqual(env.roster.members);
}

get fun `test init is permissionless and runs once`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    val minter = deployUninitialized(
        canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster),
    );
    expect(minter.getSccpState().initialized).toBeFalse();
    val stranger = testing.treasury("stranger");
    val first = minter.sendSccpInit(stranger.address, 1, { value: grams("1"), bounce: true });
    expect(first).toHaveSuccessfulTx<SccpInit>({ to: minter.address });
    expect(minter.getSccpState().initialized).toBeTrue();
    val second = sendInit(minter);
    expect(
        second,
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.AlreadyInitialized as int });
}

get fun `test operations throw before init`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    val minter = deployUninitialized(
        canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster),
    );
    val user = testing.treasury("user");
    val deploy = minter.sendSccpDeployBuckets(
        user.address,
        0,
        user.address,
        1,
        { value: grams("2"), bounce: true },
    );
    expect(
        deploy,
    ).toHaveFailedTx<SccpDeployBuckets>({ to: minter.address, exitCode: SccpErrors.NotInitialized as int });
    val frozen = minter.sendSccpVoidFrozen(
        user.address,
        0,
        user.address,
        0,
        1,
        { value: grams("2"), bounce: true },
    );
    expect(
        frozen,
    ).toHaveFailedTx<SccpVoidFrozen>({ to: minter.address, exitCode: SccpErrors.NotInitialized as int });
    // finalize with a well-formed but irrelevant body
    val env: Env = {
        minter,
        deployer: user.address,
        networkId: VEC_NETWORK_ID,
        roster,
    };
    val d = deliverDefault(env, 0, 1000, user.address);
    val fin = sendFinalize(env, user.address, d, grams("2"));
    expect(
        fin,
    ).toHaveFailedTx<SccpFinalize>({ to: minter.address, exitCode: SccpErrors.NotInitialized as int });
    // top-ups are accepted before init
    val topUp = minter.sendTopUpTons(user.address, 0, { value: grams("1") });
    expect(topUp).toHaveSuccessfulTx<TopUpTons>({ to: minter.address });
}

get fun `test init rejects a digest that does not match the stored roster`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    // config.initial_digest differs from the recomputed digest
    var config = storage.config.load();
    config.initialDigest = config.initialDigest ^ 1;
    storage.config = config.toCell();
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadRoster as int });
}

get fun `test init rejects a roster digest field that does not match the members`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    var state = storage.roster.load();
    state.members = sccpBuildMembers([keyAddress(0), keyAddress(1), keyAddress(2), keyAddress(4)]);
    storage.roster = state.toCell();
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadRoster as int });
}

get fun `test init rejects a library-cell member list`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    var state = storage.roster.load();
    // published library with the canonical chunk: CTOS alone would resolve it
    testing.registerLibrary(state.members);
    state.members = state.members.toLibraryReference();
    storage.roster = state.toCell();
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadSnake as int });
}

get fun `test init rejects a generation other than the pinned one`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    var config = storage.config.load();
    config.initialGeneration = 8;
    storage.config = config.toCell();
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadRoster as int });
}

get fun `test init rejects a bad threshold and bad member order`() {
    testing.setNow(T0);
    val roster = makeRoster(
        7,
        4,
        0,
        0,
        T0 * 1000 - HOUR_MS,
        T0 * 1000 + DAY_MS,
    );
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    var state = storage.roster.load();
    state.t = 4;
    storage.roster = state.toCell();
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadRoster as int });
}

get fun `test init applies the validity bounds`() {
    // valid_from too far in the future, validity too long, already expired,
    // and valid_until beyond now + MAX_ROSTER_VALIDITY
    val now = T0 * 1000;
    val bounds: array<[int, int]> = [
        [now + SCCP_MAX_CLOCK_SKEW_MS + 1, now + 2 * DAY_MS],
        [now - DAY_MS, now - DAY_MS + SCCP_MAX_ROSTER_VALIDITY_MS + 1],
        [now - 2 * DAY_MS, now],
        [now + 1000, now + SCCP_MAX_ROSTER_VALIDITY_MS + 1],
    ];
    var i = 0;
    while (i < bounds.size()) {
        testing.setNow(T0);
        val [from, until] = bounds.get(i);
        val roster = makeRoster(
            7,
            4,
            0,
            0,
            from,
            until,
        );
        val minter = deployUninitialized(
            canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster),
        );
        expect(sendInit(minter)).toHaveFailedTx<SccpInit>({
            to: minter.address,
            exitCode: SccpErrors.BadRosterValidity as int,
        });
        i += 1;
    }
    // the edges are accepted
    testing.setNow(T0);
    val edge = makeRoster(
        7,
        4,
        0,
        0,
        now + SCCP_MAX_CLOCK_SKEW_MS,
        now + SCCP_MAX_ROSTER_VALIDITY_MS,
    );
    val minter = deployUninitialized(
        canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, edge),
    );
    expect(sendInit(minter)).toHaveSuccessfulTx<SccpInit>({ to: minter.address });
}

get fun `test init rejects non-canonical initial data`() {
    testing.setNow(T0);
    val roster = defaultRoster(4, 0);
    var storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    storage.totalSupply = 5;
    val minter = deployUninitialized(storage);
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.BadConfig as int });

    var paused = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    paused.mintingPaused = true;
    val minter2 = deployUninitialized(paused);
    expect(
        sendInit(minter2),
    ).toHaveFailedTx<SccpInit>({ to: minter2.address, exitCode: SccpErrors.BadConfig as int });

    val zeroRevision = canonicalStorage(VEC_NETWORK_ID, 0, TEST_MAX_SUPPLY, roster);
    val minter3 = deployUninitialized(zeroRevision);
    expect(
        sendInit(minter3),
    ).toHaveFailedTx<SccpInit>({ to: minter3.address, exitCode: SccpErrors.BadConfig as int });
}

get fun `test init requires the ton mainnet global id`() {
    testing.setNow(T0);
    var config = testing.getConfig();
    config.setParamRaw(19, beginCell().storeInt(-3, 32).endCell());
    expect(testing.setConfig(config)).toBeTrue();
    val roster = defaultRoster(4, 0);
    val minter = deployUninitialized(
        canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster),
    );
    expect(
        sendInit(minter),
    ).toHaveFailedTx<SccpInit>({ to: minter.address, exitCode: SccpErrors.WrongChain as int });
}

get fun `test canonical storage matches the documented bit layout`() {
    val roster = defaultRoster(4, 0);
    val storage = canonicalStorage(VEC_NETWORK_ID, TEST_REVISION, TEST_MAX_SUPPLY, roster);
    val root = storage.toCell().beginParse();
    // initialized(1) total(4) pending(4) outbound(64) op(64) control(64) paused(1) deployed(64) prev-maybe(1)
    expect(root.remainingBitsCount()).toEqual(267);
    expect(root.remainingRefsCount()).toEqual(2);
}
