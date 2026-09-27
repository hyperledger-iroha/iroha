// Parliament controls (§5.1.6, §5.3.4 "apply control"): pause and resume,
// stale, equal and gapped nonces, historical mode, foreign leaves (network,
// destination, revision, Taira network, transfer leaf), and burns, rotations
// and voids staying open while minting is paused.

import "@acton/emulation/config"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@acton/types/message"
import "@contracts/sccp-addresses"
import "@contracts/sccp-cells"
import "@contracts/sccp-constants"
import "@contracts/sccp-errors"
import "@contracts/sccp-messages"
import "@contracts/sccp-verify"
import "@wrappers/SccpTairaXorMinter.gen"
import "sccp-test-helpers"
import "sccp-test-vectors.gen"

get fun `test pause blocks finalization and resume re-enables it`() {
    val env = setupEnv();
    val alice = testing.treasury("alice").address;
    val res = applyControlOk(env, 1, true);
    val event = res
        .findExternalOutMessage<SccpControlAppliedEvent>({ from: env.minter.address })!
        .loadBody();
    expect(event.controlNonce).toEqual(1);
    expect(event.paused).toBeTrue();
    expect(res).toHaveSuccessfulTx<ReturnExcessesBack>({ to: env.deployer });
    expect(env.minter.getSccpState().opCount).toEqual(1);
    val d = deliverDefault(env, 0, 10, alice);
    expect(sendFinalize(env, env.deployer, d, finalizeValue(env, 0))).toHaveFailedTx<SccpFinalize>({
        to: env.minter.address,
        exitCode: SccpErrors.MintingIsPaused as int,
    });
    applyControlOk(env, 2, false);
    expect(env.minter.getSccpState().opCount).toEqual(2);
    expect(
        sendFinalize(env, env.deployer, d, finalizeValue(env, 0)),
    ).toHaveSuccessfulTx<SccpConsumed>({
        to: env.minter.address,
    });
    expect(jettonBalanceOf(env, alice)).toEqual(10);
}

get fun `test stale and equal control nonces are rejected and gaps are accepted`() {
    val env = setupEnv();
    applyControlOk(env, 2, true);
    expect(sendApplyControl(env, deliverControl(env, 2, false))).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.StaleControl as int,
    });
    expect(sendApplyControl(env, deliverControl(env, 1, false))).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.StaleControl as int,
    });
    applyControlOk(env, 7, false);
    // pause, resume and pause again as three distinct controls
    applyControlOk(env, 8, true);
    expect(env.minter.getSccpState().opCount).toEqual(3);
    // a zero nonce can never apply
    expect(sendApplyControl(env, deliverControl(env, 0, false))).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.StaleControl as int,
    });
}

get fun `test apply control historical`() {
    val env = setupEnv();
    val leaf = sccpControlLeaf(
        env.networkId,
        env.minter.address.accountId(),
        TEST_REVISION,
        3,
        true,
    );
    val blockLeaves = leavesWith(leaf, 1, 2);
    val blockRoot = treeRoot(blockLeaves);
    val history = historyLeaves(4, 2, 44, blockRoot, 2);
    val statement: TestStatement = {
        height: 90,
        epoch: 2,
        timestampMs: T0 * 1000,
        blockHash: 0x77,
        sccpRoot: 0,
        messageCount: 0,
        historyRoot: treeRoot(history),
        historySize: 4,
        rosterDigest: env.digest(),
        nextRosterDigest: 0,
    };
    val blockPath = treePath(blockLeaves, 1);
    val historyPath = treePath(history, 2);
    val res = env
        .minter
        .sendSccpApplyControlHistorical(
            env.deployer,
            1,
            env.deployer,
            statement.attestation(),
            signByThreshold(env.roster, statement.digest(env.networkId)),
            {
                height: 44,
                sccpRoot: blockRoot,
                messageCount: 2,
                leafIndex: 2,
                pathLen: historyPath.size(),
                path: sccpBuildHashPath(historyPath),
            },
            {
                controlNonce: 3,
                paused: true,
                leafIndex: 1,
                pathLen: blockPath.size(),
                path: sccpBuildHashPath(blockPath),
            },
            { value: grams("1"), bounce: true },
        );
    expect(res).toHaveSuccessfulTx<SccpApplyControlHistorical>({ to: env.minter.address });
    expect(env.minter.getSccpState().mintingPaused).toBeTrue();
    expect(env.minter.getSccpState().controlNonce).toEqual(3);
}

get fun `test foreign control leaves are rejected`() {
    val env = setupEnv();
    val own = env.minter.address.accountId();
    val leaves: array<int> = [
        // another target network (ethereum-mainnet)
        controlLeafFor(
            env.networkId,
            0x41,
            1,
            own,
            TEST_REVISION,
            1,
            true,
        ),
        // another destination word
        controlLeafFor(
            env.networkId,
            SCCP_TAG_TON,
            SCCP_TON_IDENTITY_WORD,
            own ^ 1,
            TEST_REVISION,
            1,
            true,
        ),
        // another revision
        controlLeafFor(
            env.networkId,
            SCCP_TAG_TON,
            SCCP_TON_IDENTITY_WORD,
            own,
            TEST_REVISION + 1,
            1,
            true,
        ),
        // another Taira network
        controlLeafFor(
            env.networkId ^ 1,
            SCCP_TAG_TON,
            SCCP_TON_IDENTITY_WORD,
            own,
            TEST_REVISION,
            1,
            true,
        ),
        // the same control with the opposite pause value
        controlLeafFor(
            env.networkId,
            SCCP_TAG_TON,
            SCCP_TON_IDENTITY_WORD,
            own,
            TEST_REVISION,
            1,
            false,
        ),
    ];
    var i = 0;
    while (i < leaves.size()) {
        val c = deliverControlLeaf(env, leaves.get(i), 1, true, 3);
        expect(sendApplyControl(env, c)).toHaveFailedTx<SccpApplyControl>({
            to: env.minter.address,
            exitCode: SccpErrors.BadProof as int,
        });
        i += 1;
    }
    // sanity: the genuine leaf applies
    applyControlOk(env, 1, true);
}

get fun `test a transfer leaf offered as a control is rejected`() {
    val env = setupEnv();
    val alice = testing.treasury("alice").address;
    val payload = inboundPayloadCell(0, TEST_REVISION, T0 * 1000 + DAY_MS, 1, alice.accountId());
    val transferLeaf = inboundLeaf(env.networkId, payload, env.minter.address);
    var nonce = 0;
    while (nonce < 3) {
        val c = deliverControlLeaf(env, transferLeaf, nonce, nonce == 1, 2);
        expect(sendApplyControl(env, c)).toHaveFailedTx<SccpApplyControl>({
            to: env.minter.address,
            exitCode: SccpErrors.BadProof as int,
        });
        nonce += 1;
    }
    // and a control leaf never finalizes as a transfer
    val controlLeaf = sccpControlLeaf(
        env.networkId,
        env.minter.address.accountId(),
        TEST_REVISION,
        1,
        true,
    );
    val leaves = leavesWith(controlLeaf, 0, 2);
    val statement = blockStatement(9, leaves, env.digest());
    val path = treePath(leaves, 0);
    val d: Delivery = {
        payload,
        statement,
        signatures: signByThreshold(env.roster, statement.digest(env.networkId)),
        proof: {
            leafIndex: 0,
            pathLen: path.size(),
            payload,
            path: sccpBuildHashPath(path),
        },
        messageId: 0,
    };
    expect(sendFinalize(env, env.deployer, d, grams("5"))).toHaveFailedTx<SccpFinalize>({
        to: env.minter.address,
        exitCode: SccpErrors.BadProof as int,
    });
}

get fun `test burns, rotations and voids stay open while paused`() {
    val env = setupEnv();
    val alice = testing.treasury("alice").address;
    finalizeOk(env, 0, 1000, alice);
    val expiring = deliver(
        env,
        env.roster,
        1,
        5,
        alice,
        (T0 + 50) * 1000,
        0,
        1,
    );
    applyControlOk(env, 1, true);
    // burn
    val burn = sendBurn(env, alice, 400, snakeOfLength(32), burnValue(env, alice));
    expect(burn).toHaveSuccessfulTx<BurnNotificationForMinter>({ to: env.minter.address });
    expect(jettonBalanceOf(env, alice)).toEqual(600);
    // rotation
    val next = nextRoster(env.roster, 4, 4, T0 * 1000 - 5000);
    rotateTo(env, next, 300);
    // void (signed by the previous roster, still in grace)
    testing.setNow(T0 + 51);
    expect(
        sendVoidExpired(env, env.deployer, 1, expiring, env.minter.voidRequiredValue(1)),
    ).toHaveSuccessfulTx<SccpConsumed>({
        to: env.minter.address,
    });
    // controls themselves are not blocked by the pause (resume under the new roster)
    val env8: Env = {
        minter: env.minter,
        deployer: env.deployer,
        networkId: env.networkId,
        roster: next,
    };
    applyControlOk(env8, 2, false);
    // opCount: finalize, pause, burn, void, resume
    expect(env.minter.getSccpState().opCount).toEqual(5);
}

get fun `test a frozen destination cannot apply controls`() {
    val env = setupEnv();
    val c = deliverControl(env, 1, true);
    testing.setNow(env.roster.validUntilMs / 1000 + 1);
    expect(sendApplyControl(env, c)).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.RosterNotAccepted as int,
    });
}

get fun `test apply control needs signatures and the mainnet global id`() {
    val env = setupEnv();
    var c = deliverControl(env, 1, true);
    c.signatures = signBySlots(env.roster, c.statement.digest(env.networkId), [3]);
    expect(sendApplyControl(env, c)).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.TooFewSignatures as int,
    });
    var config = testing.getConfig();
    config.setParamRaw(19, beginCell().storeInt(-3, 32).endCell());
    expect(testing.setConfig(config)).toBeTrue();
    expect(sendApplyControl(env, deliverControl(env, 1, true))).toHaveFailedTx<SccpApplyControl>({
        to: env.minter.address,
        exitCode: SccpErrors.WrongChain as int,
    });
}
