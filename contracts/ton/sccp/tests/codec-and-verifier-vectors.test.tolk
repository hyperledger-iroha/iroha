//! Cross-language SCCP codec, I105 admission, and BLS12-381 signal vectors.

import "@acton/testing/expect"
import "@contracts/constants"
import "@contracts/proof-verifier"
import "@contracts/sccp-codec"

fun canonicalRecipientFromSlice(value: slice): cell {
    var input = value;
    if (input.remainingBitsCount() <= SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8) {
        return beginCell().storeSlice(input).endCell();
    }
    val head = input.loadBits(SCCP_TAIRA_RECIPIENT_HEAD_BYTES * 8);
    val tail = beginCell().storeSlice(input).endCell();
    return beginCell().storeSlice(head).storeRef(tail).endCell();
}

fun validTairaRecipient(): cell {
    return canonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
}

fun vectorG1(
    a: int,
    b: int,
    c: int,
    d: int,
    e: int,
    f: int,
    g: int,
    h: int,
): cell {
    return beginCell()
        .storeUint(a, 48)
        .storeUint(b, 48)
        .storeUint(c, 48)
        .storeUint(d, 48)
        .storeUint(e, 48)
        .storeUint(f, 48)
        .storeUint(g, 48)
        .storeUint(h, 48)
        .endCell();
}

fun vectorG2(
    a: int,
    b: int,
    c: int,
    d: int,
    e: int,
    f: int,
    g: int,
    h: int,
): cell {
    return beginCell()
        .storeUint(a, 96)
        .storeUint(b, 96)
        .storeUint(c, 96)
        .storeUint(d, 96)
        .storeUint(e, 96)
        .storeUint(f, 96)
        .storeUint(g, 96)
        .storeUint(h, 96)
        .endCell();
}

fun vectorVerifyingKey(): cell {
    val alpha = vectorG1(
        183011293753374,
        153766608626527,
        266145638431761,
        75466329404921,
        230334637350991,
        89164996524245,
        97502668511874,
        101038412596841,
    );
    val beta = vectorG2(
        46408002589580840726232933963,
        47345780740936842582462518758,
        56525246792141261468662636595,
        33301243340976382265661917167,
        6429032730315346955197275799,
        1992263385287759932813953247,
        65732417395468142527703105096,
        30733837601032853270691364217,
    );
    val gamma = vectorG2(
        45765300665918301128790692768,
        42137484379184671317244818970,
        56280794141317933024253112594,
        6059577009407902906031811454,
        709198854518927808499549479,
        14165060894806320894179293954,
        55805278558791767872231965478,
        52000413785700509085167893944,
    );
    val delta = vectorG2(
        49789121803873806817509672708,
        18726252031417989091448011696,
        74063063928443088317504713506,
        19263010647859157794393177,
        1686241465902126740349150760,
        15941168503749452830055877516,
        5211477145183535490501541634,
        41311053173765659121573254739,
    );
    val ic = vectorG1(
        178265951040853,
        56473986965074,
        106075490475640,
        243422637637155,
        231625369907622,
        96827964420860,
        167399507869996,
        45547050341048,
    );
    var points: map<uint8, cell> = [];
    var index = 0;
    while (index < 12) {
        points.set(index as uint8, ic);
        index += 1;
    }
    val tail: Groth16Bls12381VerificationKeyTail = { deltaG2: delta, ic: points };
    val key: Groth16Bls12381VerificationKey = {
        version: 1,
        publicSignalCount: 11,
        alphaG1: alpha,
        betaG2: beta,
        gammaG2: gamma,
        tail: tail.toCell(),
    };
    return key.toCell();
}

get fun `test canonical Taira I105 recipient passes complete source admission`() {
    expect(isCanonicalTairaRecipient(validTairaRecipient())).toEqual(true);
}

get fun `test I105 checksum network and nonminimal mutations fail closed`() {
    val badChecksum = canonicalRecipientFromSlice(
        "testuﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YD".literalSlice(),
    );
    val wrongNetwork = canonicalRecipientFromSlice(
        "sorauﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
    val nonminimal = canonicalRecipientFromSlice(
        "test1uﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC".literalSlice(),
    );
    expect(isCanonicalTairaRecipient(badChecksum)).toEqual(false);
    expect(isCanonicalTairaRecipient(wrongNetwork)).toEqual(false);
    expect(isCanonicalTairaRecipient(nonminimal)).toEqual(false);
}

get fun `test I105 wrong curve and small-order Ed25519 controller fail closed`() {
    // Both literals have valid I105 checksums; only their decoded controller material is invalid.
    val wrongCurve = canonicalRecipientFromSlice(
        "testuﾛ5ﾘwｹｼﾏLqﾎｳｽMﾌwmﾓVｿkZbW4ﾈ7jVcMFﾙcP9ｿrﾙﾏﾍｺLDQ5VR".literalSlice(),
    );
    val smallOrder = canonicalRecipientFromSlice(
        "testuﾛ1NcﾐMkRAﾄMCﾘﾜRnヰSRﾐﾋhﾐdJﾏTｴ7ｼvｾSﾒﾖmﾉ9vﾒﾘDH7VEV".literalSlice(),
    );
    expect(isCanonicalTairaRecipient(wrongCurve)).toEqual(false);
    expect(isCanonicalTairaRecipient(smallOrder)).toEqual(false);
}

get fun `test recipient snake rejects overlength and ambiguous split`() {
    val tooLongTail = beginCell().storeUint(0, 256).storeUint(0, 256).storeUint(0, 40).endCell();
    val tooLong = beginCell()
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 128)
        .storeRef(tooLongTail)
        .endCell();
    val ambiguous = beginCell()
        .storeUint(0, 256)
        .storeUint(0, 256)
        .storeUint(0, 120)
        .storeRef(beginCell().storeUint(0, 15 * 8).endCell())
        .endCell();
    expect(isCanonicalTairaRecipient(tooLong)).toEqual(false);
    expect(isCanonicalTairaRecipient(ambiguous)).toEqual(false);
}

get fun `test codec seven is signed i32 big endian plus account hash`() {
    val accountHash = 0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20;
    val encoded = canonicalTonAddress36(address.fromWorkchainAndHash(0, accountHash));
    val expected = beginCell().storeUint(0, 32).storeUint(accountHash, 256).endCell();
    expect(encoded.beginParse().bitsEqual(expected.beginParse())).toEqual(true);
}

get fun `test canonical four-cell payload and hashes match Rust vectors`() {
    val accountHash = 0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20;
    val payload = buildTonToTairaPayload(
        address.fromWorkchainAndHash(0, accountHash),
        validTairaRecipient(),
        9,
        7,
        123456789,
    );
    expect(ordinaryByteLength(payload.header)).toEqual(50);
    expect(ordinaryByteLength(payload.chunk1)).toEqual(100);
    expect(ordinaryByteLength(payload.chunk2)).toEqual(58);
    expect(ordinaryByteLength(payload.chunk3)).toEqual(0);
    expect(canonicalPayloadByteLength(payload)).toEqual(208);
    expect(sccpPayloadHash(payload)).toEqual(
        0xa5b2070d585a783ee4450eb9694cb864fe8cb6a57622c6ec29f15ea168b8afff,
    );
    val lane = beginCell().storeSlice("lane".literalSlice()).endCell();
    expect(sccpLaneHash(lane)).toEqual(
        0x7f3f58178cd1d97f60aa761563b30da931bf8a5284d488b6204198f2e0e9df2f,
    );
    val messageId = sccpMessageId(lane, payload);
    expect(messageId).toEqual(
        0x515d4b69a9303452d5d580c4bca71978a60fd3cb4dc906dff5c414d64117c24d,
    );
    expect(sccpSourceEventDigest(sccpLaneHash(lane), messageId, sccpPayloadHash(payload))).toEqual(
        0xdeb773d913725a4b7557329552b29501ae2de915321d02d59488036b513703ed,
    );
}

get fun `test all eleven BLS12-381 SHA256 signal words match Rust vectors`() {
    expect(signalMessageId(1)).toEqual(
        0x38233bedf9a99fc596a893ac8bf0a54259ec746eaac94496ac6aab46bddf9865,
    );
    expect(signalPayloadHash(2)).toEqual(
        0x50e47a35a9dd26fb214618cb6bc80c9a43ff8d6f2140598cbd64335690a7df4b,
    );
    expect(signalTargetDomain(3)).toEqual(
        0x138c9b3073bb270f5c9276cf11bd8347e651baa6d4f8bafbe05888a4b18cc65f,
    );
    expect(signalCommitmentRoot(4)).toEqual(
        0x48479cbdf01e0f652b7ee77b4d52f2532b8219127ae9164aa0b8ca466fbeea33,
    );
    expect(signalFinalityHeight(5)).toEqual(
        0x4337fd2cc86f4069ddc7d9299295e14ad47df28d07c6e30e51e0ca60a045fbea,
    );
    expect(signalFinalityBlockHash(6)).toEqual(
        0x55f0ebbd28b5a253a0a159c3b02de5231e2b145f49428db98238d5273f0a9537,
    );
    expect(signalSourceDomain(7)).toEqual(
        0x11de378b213429cb66e55361bf87c82db99ba97e9f2a1b7d61b365fe59e472fb,
    );
    expect(signalStatementHash(8)).toEqual(
        0x632e88b4a9f1aac23582295df957ba41e99a0cabbb8c9d7639cec9e0575109b5,
    );
    expect(signalDestinationBindingHash(9)).toEqual(
        0x4ca329795ff859e41e41d4e0ad98fe63a05038f40ddd03f08664ade7483fcc95,
    );
    expect(signalRouteConfigurationHash(10)).toEqual(
        0x57d6ec7115c11cc182f98740ce557084044efb19c13e15b58db6617b5a600aa0,
    );
    expect(signalSoraFinalityAnchorHash(11)).toEqual(
        0x4f59d560e129108ff1283a7682257ba55cdc96082c9b7a2a3955b9c0300c4abb,
    );
}

get fun `test canonical BLS12-381 key SHA256 matches Rust byte commitment`() {
    expect(canonicalBls12381VerifyingKeyHash(vectorVerifyingKey())).toEqual(
        0x7877d1290baec55477c6797800c6f7f8798bf088efb98ad7c6702a7699c3db74,
    );
}
