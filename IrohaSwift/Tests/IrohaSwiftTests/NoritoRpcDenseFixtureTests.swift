import CryptoKit
import XCTest
@testable import IrohaSwift

/// The Norito RPC fixture transactions in the dense layout
/// (`fixtures/norito_rpc/transaction_fixtures.dense.json`) through the SDK's
/// typed transaction decoder and builder.
///
/// Every transaction-level Norito layout rule here runs in Swift. The native
/// bridge validates account addresses, derives the fixture key from its seed and
/// transcodes each typed instruction frame to and from its bare dense payload
/// (`connect_norito_transcode_instruction_v1`); see `TransactionInstructionWire`.
final class NoritoRpcDenseFixtureTests: XCTestCase {
    private static let transactionPayloadType =
        "iroha_data_model::transaction::signed::model::TransactionPayload"
    private static let signedTransactionType =
        "iroha_data_model::transaction::signed::model::SignedTransaction"
    private static let networkPrefix: UInt16 = 753
    private static let signingSeedHex =
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032"

    // MARK: - Fixture round trip

    func testEveryDenseFixtureTransactionDecodesAndReencodesByteIdentically() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAccountAddressCodecAvailable,
            "account-address validation needs the NoritoBridge address codec"
        )
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isInstructionTranscoderAvailable,
            "typed dense instruction bodies need the NoritoBridge instruction transcoder"
        )
        let fixtures = try DenseTransactionFixtures.load()
        XCTAssertEqual(fixtures.cases.count, 27)
        var denseTotal = 0
        var compactTotal = 0

        for fixture in fixtures.cases {
            let name = fixture.name

            // 1. Decode: the typed decoder accepts the exact dense bytes and
            //    verifies the fixture signature over the dense payload hash.
            let inspected = try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                fixture.denseSigned,
                layout: .dense,
                context: name
            )
            XCTAssertEqual(inspected.signedTransaction, fixture.denseSigned, name)
            XCTAssertEqual(inspected.transactionPayload, fixture.densePayload, name)
            XCTAssertEqual(inspected.payload.layout, .dense, name)
            XCTAssertEqual(inspected.signature.count, 64, name)
            let payload = try ToriiCanonicalTransactionDraft.inspectTransactionPayload(
                fixture.densePayload,
                layout: .dense,
                context: name
            )
            XCTAssertEqual(payload.executable, inspected.payload.executable, name)

            // 2. Hash: the signature covers the hash of the dense payload bytes.
            let signingHash = IrohaHash.hash(fixture.densePayload)
            XCTAssertEqual(signingHash.hexLowercased(), fixture.signingHash, name)
            XCTAssertNotEqual(
                signingHash,
                IrohaHash.hash(fixture.compactPayload),
                "\(name): the dense and compact payloads are signed separately"
            )
            let signerKey = try Curve25519.Signing.PublicKey(
                rawRepresentation: inspected.signerPublicKey
            )
            XCTAssertTrue(signerKey.isValidSignature(inspected.signature, for: signingHash), name)

            // 3. Decoded values match the descriptor the fixture was built from.
            let descriptor = fixture.descriptor
            XCTAssertEqual(payload.networkId, try NetworkId(literal: descriptor.networkId), name)
            XCTAssertEqual(payload.creationTimeMs, descriptor.creationTimeMs, name)
            XCTAssertEqual(payload.timeToLiveMs, descriptor.timeToLiveMs, name)
            XCTAssertEqual(payload.nonce, descriptor.nonce, name)
            XCTAssertEqual(payload.metadata, descriptor.metadata, name)
            let authority = try AccountAddress.fromCanonicalCompactAccountPayload(
                payload.authority,
                layout: .dense
            )
            XCTAssertEqual(
                try authority.toI105(networkPrefix: Self.networkPrefix),
                descriptor.authority,
                name
            )
            XCTAssertEqual(
                payload.authority,
                try CanonicalNorito.encodeAccountId(descriptor.authority, layout: .dense),
                name
            )
            XCTAssertEqual(
                payload.feePayment,
                try descriptor.feePayment.norito(layout: .dense),
                "\(name): fee payment built from the descriptor"
            )
            XCTAssertEqual(
                payload.metadataWire,
                try CanonicalNorito.encodeTransactionMetadata(descriptor.metadata, layout: .dense),
                "\(name): metadata built from the descriptor"
            )

            // 4. The executable decodes to typed parts and re-encodes identically.
            let executable = try TransactionExecutableWire.decode(
                payload.executable,
                layout: .dense,
                context: "\(name).executable"
            )
            XCTAssertEqual(try executable.encode(layout: .dense), payload.executable, name)
            try assertExecutable(executable, matches: descriptor.executable, layout: .dense, name: name)

            // 5. Build: the SDK builder reproduces the dense payload and the dense
            //    signed transaction from the same inputs.
            let rebuiltPayload = try SingleInstructionSwiftNoritoEncoder.transactionPayload(
                networkId: try NetworkId(literal: descriptor.networkId),
                authority: descriptor.authority,
                creationTimeMs: descriptor.creationTimeMs,
                ttlMs: descriptor.timeToLiveMs,
                nonce: descriptor.nonce,
                executable: try descriptor.executable.wire(),
                feePayment: descriptor.feePayment,
                metadata: descriptor.metadata,
                layout: .dense
            )
            XCTAssertEqual(rebuiltPayload, fixture.densePayload, "\(name): rebuilt dense payload")
            XCTAssertEqual(
                SingleInstructionSwiftNoritoEncoder.signedTransaction(
                    transactionPayload: rebuiltPayload,
                    signature: inspected.signature,
                    layout: .dense
                ),
                fixture.denseSigned,
                "\(name): rebuilt dense signed transaction"
            )

            // 6. The current layout is unchanged: the compact twin decodes to the
            //    same values and the same builder reproduces it from the same
            //    descriptor, including every instruction frame.
            let compact = try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                fixture.compactSigned,
                layout: .compact,
                context: "\(name).compact"
            )
            XCTAssertEqual(compact.transactionPayload, fixture.compactPayload, name)
            XCTAssertEqual(compact.payload.layout, .compact, name)
            XCTAssertEqual(compact.payload.networkId, payload.networkId, name)
            XCTAssertEqual(compact.payload.creationTimeMs, payload.creationTimeMs, name)
            XCTAssertEqual(compact.payload.timeToLiveMs, payload.timeToLiveMs, name)
            XCTAssertEqual(compact.payload.nonce, payload.nonce, name)
            XCTAssertEqual(compact.payload.metadata, payload.metadata, name)
            XCTAssertEqual(compact.signerPublicKey, inspected.signerPublicKey, name)
            XCTAssertNotEqual(compact.signature, inspected.signature, name)
            XCTAssertEqual(
                try AccountAddress.fromCanonicalCompactAccountPayload(compact.payload.authority)
                    .canonicalBytes(),
                try authority.canonicalBytes(),
                name
            )
            XCTAssertEqual(compact.payload.feePayment, try descriptor.feePayment.compactNorito(), name)
            let compactExecutable = try TransactionExecutableWire.decode(
                compact.payload.executable,
                layout: .compact,
                context: "\(name).compact.executable"
            )
            try assertExecutable(
                compactExecutable,
                matches: descriptor.executable,
                layout: .compact,
                name: name
            )
            XCTAssertEqual(
                compactExecutable.instructionWireNames,
                executable.instructionWireNames,
                name
            )
            XCTAssertEqual(
                try SingleInstructionSwiftNoritoEncoder.transactionPayload(
                    networkId: try NetworkId(literal: descriptor.networkId),
                    authority: descriptor.authority,
                    creationTimeMs: descriptor.creationTimeMs,
                    ttlMs: descriptor.timeToLiveMs,
                    nonce: descriptor.nonce,
                    executable: try descriptor.executable.wire(),
                    feePayment: descriptor.feePayment,
                    metadata: descriptor.metadata,
                    layout: .compact
                ),
                fixture.compactPayload,
                "\(name): rebuilt compact payload"
            )
            XCTAssertEqual(
                SingleInstructionSwiftNoritoEncoder.signedTransaction(
                    transactionPayload: fixture.compactPayload,
                    signature: compact.signature,
                    layout: .compact
                ),
                fixture.compactSigned,
                "\(name): rebuilt compact signed transaction"
            )
            XCTAssertEqual(
                SingleInstructionSwiftNoritoEncoder.signedEnvelope(
                    transactionPayload: fixture.compactPayload,
                    signature: compact.signature
                ).signedTransaction,
                fixture.compactSigned,
                "\(name): the versioned envelope still carries the compact layout"
            )
            // Both layouts decode to the same typed instructions, and each
            // writes the other's bytes.
            XCTAssertEqual(executable, compactExecutable, name)
            XCTAssertEqual(try executable.encode(layout: .compact), compact.payload.executable, name)
            XCTAssertEqual(try compactExecutable.encode(layout: .dense), payload.executable, name)

            denseTotal += fixture.denseSigned.count
            compactTotal += fixture.compactSigned.count
            XCTAssertLessThan(fixture.denseSigned.count, fixture.compactSigned.count, name)
        }
        // Measured by the Rust data model on the same fixtures: about 47% smaller.
        XCTAssertLessThan(denseTotal * 100, compactTotal * 56)
    }

    func testSdkSignedDenseTransactionVerifiesOnlyInTheDenseLayout() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAccountAddressCodecAvailable,
            "account-address validation needs the NoritoBridge address codec"
        )
        let seed = try XCTUnwrap(Data(hexString: Self.signingSeedHex))
        let derived = NoritoNativeBridge.shared.keypairFromSeed(algorithm: .ed25519, seed: seed)
        try requireNativeTestCapability(
            derived != nil,
            "the fixture key derivation needs the NoritoBridge ed25519 seed helper"
        )
        let signingKey = try SigningKey.ed25519(privateKey: try XCTUnwrap(derived).privateKey)
        let fixtures = try DenseTransactionFixtures.load()

        for fixture in fixtures.cases {
            let name = fixture.name
            let reference = try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                fixture.denseSigned,
                layout: .dense,
                context: name
            )
            XCTAssertEqual(try signingKey.publicKey(), reference.signerPublicKey, name)

            // The SDK signs the hash of the dense payload bytes.
            let signature = try signingKey.sign(IrohaHash.hash(fixture.densePayload))
            let signed = SingleInstructionSwiftNoritoEncoder.signedTransaction(
                transactionPayload: fixture.densePayload,
                signature: signature,
                layout: .dense
            )
            let inspected = try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                signed,
                layout: .dense,
                context: "\(name).sdk-signed"
            )
            XCTAssertEqual(inspected.transactionPayload, fixture.densePayload, name)
            XCTAssertEqual(inspected.signerPublicKey, reference.signerPublicKey, name)
            // Ed25519 signing may be hedged, so only the signature bytes may
            // differ from the fixture; everything around them is identical.
            XCTAssertEqual(signed.count, fixture.denseSigned.count, name)
            XCTAssertEqual(signed.prefix(3), fixture.denseSigned.prefix(3), name)
            XCTAssertEqual(signed.dropFirst(3 + 64), fixture.denseSigned.dropFirst(3 + 64), name)

            // A signature over the compact payload does not authorize the dense
            // payload of the same transaction, and the reverse.
            let compactSignature = try signingKey.sign(IrohaHash.hash(fixture.compactPayload))
            XCTAssertThrowsError(
                try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                    SingleInstructionSwiftNoritoEncoder.signedTransaction(
                        transactionPayload: fixture.densePayload,
                        signature: compactSignature,
                        layout: .dense
                    ),
                    layout: .dense,
                    context: name
                ),
                name
            )
            XCTAssertThrowsError(
                try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                    SingleInstructionSwiftNoritoEncoder.signedTransaction(
                        transactionPayload: fixture.compactPayload,
                        signature: signature,
                        layout: .compact
                    ),
                    layout: .compact,
                    context: name
                ),
                name
            )
        }
    }

    // MARK: - Negatives

    func testDenseTransactionsAreRefusedInTheCurrentLayoutAndTheReverse() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAccountAddressCodecAvailable,
            "account-address validation needs the NoritoBridge address codec"
        )
        let fixtures = try DenseTransactionFixtures.load()
        for fixture in fixtures.cases {
            let name = fixture.name
            func inspectSigned(_ bytes: Data, _ layout: NoritoWireLayout) throws {
                _ = try ToriiCanonicalTransactionDraft.inspectSignedTransaction(
                    bytes,
                    layout: layout,
                    context: name
                )
            }
            func inspectPayload(_ bytes: Data, _ layout: NoritoWireLayout) throws {
                _ = try ToriiCanonicalTransactionDraft.inspectTransactionPayload(
                    bytes,
                    layout: layout,
                    context: name
                )
            }

            // A dense transaction read as the current layout, and the reverse.
            XCTAssertThrowsError(try inspectSigned(fixture.denseSigned, .compact), name)
            XCTAssertThrowsError(try inspectSigned(fixture.denseSigned, .fixed), name)
            XCTAssertThrowsError(try inspectSigned(fixture.compactSigned, .dense), name)
            XCTAssertThrowsError(try inspectPayload(fixture.densePayload, .compact), name)
            XCTAssertThrowsError(try inspectPayload(fixture.densePayload, .fixed), name)
            XCTAssertThrowsError(try inspectPayload(fixture.compactPayload, .dense), name)
            XCTAssertThrowsError(
                try TransactionExecutableWire.decode(
                    try ToriiCanonicalTransactionDraft.inspectTransactionPayload(
                        fixture.densePayload,
                        layout: .dense,
                        context: name
                    ).executable,
                    layout: .compact,
                    context: name
                ),
                name
            )

            // Truncated input and trailing bytes.
            XCTAssertThrowsError(try inspectSigned(Data(fixture.denseSigned.dropLast()), .dense), name)
            XCTAssertThrowsError(try inspectSigned(fixture.denseSigned + Data([0]), .dense), name)
            XCTAssertThrowsError(try inspectPayload(Data(fixture.densePayload.dropLast()), .dense), name)
            XCTAssertThrowsError(try inspectPayload(fixture.densePayload + Data([0]), .dense), name)
            XCTAssertThrowsError(try inspectSigned(Data(), .dense), name)
            XCTAssertThrowsError(try inspectPayload(Data(), .dense), name)

            // A flipped payload bit breaks the dense signature.
            var tampered = fixture.denseSigned
            tampered[tampered.count - 3] ^= 0x01
            XCTAssertThrowsError(try inspectSigned(tampered, .dense), name)

            // The versioned wire names the compact layout: version 1 never
            // carries a dense transaction.
            XCTAssertThrowsError(
                try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
                    Data([1]) + fixture.denseSigned,
                    context: name
                ),
                name
            )
            XCTAssertNoThrow(
                try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
                    Data([1]) + fixture.compactSigned,
                    context: name
                ),
                name
            )

            // Framed: the header minor is the only layout selector.
            let denseFrame = noritoEncode(
                typeName: Self.transactionPayloadType,
                payload: fixture.densePayload,
                layout: .dense
            )
            XCTAssertNil(noritoDecodeFrame(denseFrame), "\(name): current-layout decoders refuse minor 1")
            let declared = try XCTUnwrap(noritoDecodeDeclaredFrame(denseFrame), name)
            XCTAssertEqual(declared.layout, .dense, name)
            XCTAssertNoThrow(try inspectPayload(declared.payload, declared.layout), name)
            // The same payload under a minor-0 header declares the compact layout
            // and is then refused by the typed decoder, not misread.
            let mislabelled = try XCTUnwrap(
                noritoDecodeDeclaredFrame(
                    noritoEncode(
                        typeName: Self.transactionPayloadType,
                        payload: fixture.densePayload,
                        layout: .compact
                    )
                ),
                name
            )
            XCTAssertEqual(mislabelled.layout, .compact, name)
            XCTAssertThrowsError(try inspectPayload(mislabelled.payload, mislabelled.layout), name)
            for minor in [UInt8(2), 3, 0x7F, 0xFF] {
                var unknown = denseFrame
                unknown[5] = minor
                XCTAssertNil(noritoDecodeDeclaredFrame(unknown), "\(name): minor \(minor)")
            }
        }
    }

    // MARK: - Typed parts in the dense layout

    func testAccountControllerPayloadsRoundTripInTheDenseLayout() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAccountAddressCodecAvailable,
            "account-address validation needs the NoritoBridge address codec"
        )
        let url = Self.repositoryRoot.appendingPathComponent("fixtures/account/multisig_wire_v1.json")
        let root = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        )
        let positives = try XCTUnwrap(root["positive"] as? [[String: Any]])
        XCTAssertEqual(positives.count, 16)
        for item in positives {
            let name = try XCTUnwrap(item["name"] as? String)
            let literal = try XCTUnwrap(item["i105"] as? String)
            let compact = try XCTUnwrap(
                Data(hexString: XCTUnwrap(item["account_id_payload_hex"] as? String))
            )
            let address = try AccountAddress.parseEncoded(literal, expectedPrefix: Self.networkPrefix)

            // The current layouts are unchanged.
            XCTAssertEqual(try address.compactNoritoAccountControllerPayload(), compact, name)
            XCTAssertEqual(try address.noritoAccountControllerPayload(layout: .compact), compact, name)
            XCTAssertEqual(
                try address.noritoAccountControllerPayload(layout: .fixed),
                try address.noritoAccountControllerPayload(),
                name
            )

            let dense = try address.noritoAccountControllerPayload(layout: .dense)
            XCTAssertLessThan(dense.count, compact.count, name)
            let expectedTag: UInt8 = item["policy"] is [String: Any] ? 1 : 0
            XCTAssertEqual(dense.first, expectedTag, name)
            // The typed decoder reads the dense payload back to the same address
            // and requires it to be the exact dense encoding of that address.
            let decoded = try AccountAddress.fromCanonicalCompactAccountPayload(dense, layout: .dense)
            XCTAssertEqual(try decoded.canonicalBytes(), try address.canonicalBytes(), name)
            // Neither layout reads the other, and truncation or a suffix is
            // refused. The Swift decoder refuses each before any validation.
            XCTAssertFalse(AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(dense), name)
            XCTAssertFalse(
                AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(compact, layout: .dense),
                name
            )
            XCTAssertFalse(
                AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(
                    Data(dense.dropLast()),
                    layout: .dense
                ),
                name
            )
            XCTAssertFalse(
                AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(
                    dense + Data([0]),
                    layout: .dense
                ),
                name
            )
        }

        // Exact dense bytes, written out by hand from the layout rules.
        func item(_ name: String) throws -> [String: Any] {
            try XCTUnwrap(positives.first { $0["name"] as? String == name }, name)
        }
        let single = try item("ed25519")
        let singleCanonical = try XCTUnwrap(
            Data(hexString: XCTUnwrap(single["canonical_address_hex"] as? String))
        )
        // tag 0, field(34) = count 33, algorithm 0, the 32-byte key.
        let singleDense = Data([0x00, 0x22, 0x21, 0x00]) + singleCanonical.suffix(32)
        let singleLiteral = try XCTUnwrap(single["i105"] as? String)
        XCTAssertEqual(try CanonicalNorito.encodeAccountId(singleLiteral, layout: .dense), singleDense)
        XCTAssertEqual(
            try CanonicalNorito.encodeAccountId(singleLiteral, layout: .compact),
            try CanonicalNorito.encodeCompactAccountId(singleLiteral)
        )
        XCTAssertTrue(
            AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(singleDense, layout: .dense)
        )

        let multisig = try item("weighted-two")
        let policy = try XCTUnwrap(multisig["policy"] as? [String: Any])
        let members = try XCTUnwrap(policy["members"] as? [[String: Any]])
        XCTAssertEqual(members.count, 2)
        var expected = Data([
            0x01, 0x55, // tag 1 (multisig), policy field of 85 bytes
            0x01, 0x01, // version
            0x02, 0x02, 0x00, // threshold
            0x4F, 0x02, // members field of 79 bytes, two members
        ])
        for member in members {
            XCTAssertEqual(member["algorithm"] as? String, "ed25519")
            let key = try XCTUnwrap(Data(hexString: XCTUnwrap(member["public_key_hex"] as? String)))
            let weight = try XCTUnwrap(member["weight"] as? Int)
            // member field(38): key field(34) = count 33, algorithm 0, key; weight field(2).
            expected.append(contentsOf: [0x26, 0x22, 0x21, 0x00])
            expected.append(key)
            expected.append(contentsOf: [0x02, UInt8(weight), 0x00])
        }
        XCTAssertEqual(
            try CanonicalNorito.encodeAccountId(XCTUnwrap(multisig["i105"] as? String), layout: .dense),
            expected
        )
        XCTAssertTrue(
            AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(expected, layout: .dense)
        )
    }

    func testFeePaymentAndMetadataFollowTheNamedLayout() throws {
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let limits = [
            try FeeChargeLimit(kind: .nexus, assetDefinitionId: assetDefinitionId, maxAmount: "3.25"),
            try FeeChargeLimit(kind: .pipelineGas, assetDefinitionId: assetDefinitionId, maxAmount: "9"),
        ]
        var intents: [FeePaymentIntent] = [
            .authority(chargeLimits: [], gasLimit: nil),
            .authority(chargeLimits: limits, gasLimit: 500_000),
        ]
        if NoritoNativeBridge.shared.isAccountAddressCodecAvailable {
            // The sponsor account literal is validated by the native address codec.
            let sponsor = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
            intents.append(
                .sponsor(
                    programId: try FeeSponsorProgramId(sponsor: sponsor, name: "wallet_fx"),
                    programRevision: 7,
                    chargeLimits: limits,
                    gasLimit: 750_000
                )
            )
        }
        for intent in intents {
            let fixed = try intent.norito(layout: .fixed)
            let compact = try intent.norito(layout: .compact)
            let dense = try intent.norito(layout: .dense)
            XCTAssertEqual(fixed, try intent.canonicalNorito(), "\(intent)")
            XCTAssertEqual(compact, try intent.compactNorito(), "\(intent)")
            XCTAssertLessThan(dense.count, compact.count, "\(intent)")
            XCTAssertLessThan(compact.count, fixed.count, "\(intent)")

            XCTAssertNoThrow(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(compact),
                "\(intent)"
            )
            XCTAssertNoThrow(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    dense,
                    layout: .dense
                ),
                "\(intent)"
            )
            XCTAssertNoThrow(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    fixed,
                    layout: .fixed
                ),
                "\(intent)"
            )
            // Each layout refuses the other's bytes, truncation and a suffix.
            XCTAssertThrowsError(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(dense),
                "\(intent)"
            )
            XCTAssertThrowsError(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    compact,
                    layout: .dense
                ),
                "\(intent)"
            )
            XCTAssertThrowsError(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    Data(dense.dropLast()),
                    layout: .dense
                ),
                "\(intent)"
            )
            XCTAssertThrowsError(
                try TransactionFeePaymentValidation.requireCanonicalTransactionFeePayment(
                    dense + Data([0]),
                    layout: .dense
                ),
                "\(intent)"
            )
        }
        // Exact dense bytes of the smallest intent: tag 0, body field(4) holding
        // the empty charge-limit sequence and `None`.
        XCTAssertEqual(
            try FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil).norito(layout: .dense),
            Data([0x00, 0x04, 0x01, 0x00, 0x01, 0x00])
        )
        // One pipeline-gas limit of 1000 with a gas limit of 1000, as the
        // `typed_fee_payment_gas_limit` fixture carries it.
        let fixtureLimit = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            maxAmount: "1000"
        )
        XCTAssertEqual(
            try FeePaymentIntent.authority(chargeLimits: [fixtureLimit], gasLimit: 1000)
                .norito(layout: .dense)
                .hexLowercased(),
            "002e220120010110bef53c1ccd1749e180dfbad6519bfd660c0602000000e80304000000000a0108e803000000000000"
        )

        // Metadata.
        let metadata: [String: ToriiJSONValue] = ["checked": .bool(true)]
        XCTAssertEqual(
            try CanonicalNorito.encodeTransactionMetadata(metadata, layout: .compact),
            try CanonicalNorito.encodeCompactMetadata(metadata)
        )
        XCTAssertEqual(
            try CanonicalNorito.encodeTransactionMetadata(metadata, layout: .dense).hexLowercased(),
            "01100807636865636b656406050474727565"
        )
        XCTAssertEqual(try CanonicalNorito.encodeTransactionMetadata([:], layout: .dense), Data([0]))
        XCTAssertNoThrow(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(Data([0]), layout: .dense)
        )
        XCTAssertThrowsError(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(Data([0]))
        )
        XCTAssertNoThrow(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                Data(repeating: 0, count: 8)
            )
        )
        XCTAssertThrowsError(
            try TransactionFeePaymentValidation.requireEmptyTransactionMetadata(
                Data(repeating: 0, count: 8),
                layout: .dense
            )
        )
        XCTAssertThrowsError(
            try CanonicalNorito.encodeTransactionMetadata(["bad name": .null], layout: .dense)
        )
    }

    func testExecutableWireRoundTripsEveryVariantInBothLayoutClasses() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isInstructionTranscoderAvailable,
            "dense instruction pairs need the NoritoBridge instruction transcoder"
        )
        let frame = noritoEncode(
            typeName: "test.Instruction",
            payload: Data([0x01, 0x02, 0x03]),
            flags: NoritoHeader.compactLen
        )
        let invocation = try TransactionContractInvocation(
            contractAddress: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh",
            expectedCodeHash: try XCTUnwrap(
                Data(hexString: "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a9")
            ),
            entrypoint: "run",
            arguments: Data([1, 2, 3, 4])
        )
        let bareInvocation = try TransactionContractInvocation(
            contractAddress: invocation.contractAddress,
            expectedCodeHash: invocation.expectedCodeHash,
            entrypoint: "view"
        )
        for layout in NoritoWireLayout.allCases {
            let instruction = try TransactionInstructionWire(
                wireName: "iroha.test",
                payload: layout == .dense ? Data([0xAA, 0xBB]) : frame,
                layout: layout
            )
            XCTAssertEqual(instruction.isDense, layout == .dense)
            let executables: [TransactionExecutableWire] = [
                .instructions([instruction, instruction]),
                .contractCall(invocation),
                .contractCall(bareInvocation),
                .ivm(Data([0xDE, 0xAD, 0xBE, 0xEF])),
                .batch([.instruction(instruction), .contractCall(invocation), .instruction(instruction)]),
            ]
            for executable in executables {
                let encoded = try executable.encode(layout: layout)
                XCTAssertEqual(
                    try TransactionExecutableWire.decode(encoded, layout: layout, context: "executable"),
                    executable,
                    "\(layout) \(executable)"
                )
                XCTAssertThrowsError(
                    try TransactionExecutableWire.decode(
                        Data(encoded.dropLast()),
                        layout: layout,
                        context: "executable"
                    )
                )
                XCTAssertThrowsError(
                    try TransactionExecutableWire.decode(
                        encoded + Data([0]),
                        layout: layout,
                        context: "executable"
                    )
                )
                for other in NoritoWireLayout.allCases where other != layout {
                    XCTAssertThrowsError(
                        try TransactionExecutableWire.decode(encoded, layout: other, context: "executable"),
                        "\(layout) bytes read as \(other): \(executable)"
                    )
                }
            }
        }

        // Exact dense bytes.
        XCTAssertEqual(
            try TransactionExecutableWire.ivm(Data("metadata-test".utf8)).encode(layout: .dense),
            Data([0x02, 0x0F, 0x0E, 0x0D]) + Data("metadata-test".utf8)
        )
        // A wire ID the bridge does not register keeps an opaque dense payload.
        let densePair = try TransactionInstructionWire(
            wireName: "test.opaque",
            payload: Data([0x07]),
            layout: .dense
        )
        XCTAssertTrue(densePair.isDense)
        // field(12) = string(11) "test.opaque"; field(2) = count 1, the payload byte.
        XCTAssertEqual(
            try densePair.encode(layout: .dense),
            Data([0x0C, 0x0B]) + Data("test.opaque".utf8) + Data([0x02, 0x01, 0x07])
        )
        XCTAssertEqual(
            try TransactionInstructionWire.decode(
                try densePair.encode(layout: .dense),
                layout: .dense,
                context: "pair"
            ),
            densePair
        )
        XCTAssertThrowsError(try densePair.encode(layout: .compact))
        XCTAssertThrowsError(try densePair.encode(layout: .fixed))
        let encodedDensePair = try densePair.encode(layout: .dense)
        XCTAssertEqual(
            try TransactionBatchItemWire.instruction(densePair).encode(layout: .dense),
            Data([0x00, 0x10]) + encodedDensePair
        )
        XCTAssertEqual(
            TransactionBatchItemWire.encodeInstruction(
                instructionBoxPayload: Data([0x01]),
                layout: .compact
            ),
            Data([0x00, 0x00, 0x00, 0x00, 0x01, 0x01])
        )

        // Pair construction rules.
        XCTAssertThrowsError(try TransactionInstructionWire(wireName: "", payload: frame, layout: .compact))
        XCTAssertThrowsError(
            try TransactionInstructionWire(wireName: "has space", payload: frame, layout: .compact)
        )
        XCTAssertThrowsError(
            try TransactionInstructionWire(
                wireName: String(repeating: "a", count: 193),
                payload: frame,
                layout: .compact
            )
        )
        XCTAssertNoThrow(
            try TransactionInstructionWire(
                wireName: "iroha.instruction.v1::a_b/c-d@1:" + String(repeating: "a", count: 160),
                payload: frame,
                layout: .compact
            )
        )
        XCTAssertThrowsError(
            try TransactionInstructionWire(wireName: "iroha.test", payload: Data([1, 2]), layout: .compact),
            "a current-layout pair carries one nested Norito frame"
        )
        // A registered wire ID admits only its instruction's canonical dense
        // body; an unregistered one stays opaque, even when empty.
        XCTAssertThrowsError(
            try TransactionInstructionWire(wireName: "iroha.log", payload: Data([0x07]), layout: .dense)
        )
        XCTAssertThrowsError(
            try TransactionInstructionWire(wireName: "iroha.log", payload: Data(), layout: .dense)
        )
        XCTAssertTrue(
            try TransactionInstructionWire(wireName: "iroha.test", payload: Data(), layout: .dense).isDense
        )
        XCTAssertTrue(TransactionInstructionWire.isWellFormedWireName("A.z_0:-@/"))
        XCTAssertFalse(TransactionInstructionWire.isWellFormedWireName("é"))

        // Executable variants this decoder refuses.
        XCTAssertThrowsError(
            try TransactionExecutableWire.decode(Data([0x03, 0x00]), layout: .dense, context: "proved")
        ) { error in
            XCTAssertTrue("\(error)".contains("IvmProved"), "\(error)")
        }
        XCTAssertThrowsError(
            try TransactionExecutableWire.decode(Data([0x05, 0x00]), layout: .dense, context: "unknown")
        )
        // An overlong count is not the canonical encoding.
        XCTAssertThrowsError(
            try TransactionExecutableWire.decode(
                Data([0x00, 0x02, 0x80, 0x00]),
                layout: .dense,
                context: "overlong"
            )
        )
        XCTAssertTrue(TransactionExecutableWire.instructions([]).isEmpty)
        XCTAssertTrue(TransactionExecutableWire.ivm(Data()).isEmpty)
        XCTAssertFalse(TransactionExecutableWire.contractCall(invocation).isEmpty)
        XCTAssertTrue(TransactionExecutableWire.contractCall(invocation).requiresGasLimit)
        XCTAssertTrue(TransactionExecutableWire.ivm(Data([1])).requiresGasLimit)
        XCTAssertFalse(TransactionExecutableWire.instructions([densePair]).requiresGasLimit)
        XCTAssertFalse(TransactionExecutableWire.batch([.instruction(densePair)]).requiresGasLimit)
        XCTAssertTrue(
            TransactionExecutableWire.batch([.instruction(densePair), .contractCall(invocation)])
                .requiresGasLimit
        )
        XCTAssertEqual(
            TransactionExecutableWire.batch([.instruction(densePair), .contractCall(invocation)])
                .instructionWireNames,
            ["test.opaque"]
        )
    }

    func testLayoutAwarePayloadBuilderKeepsTheBuilderInputRules() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAccountAddressCodecAvailable,
            "account-address validation needs the NoritoBridge address codec"
        )
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isInstructionTranscoderAvailable,
            "dense instruction pairs need the NoritoBridge instruction transcoder"
        )
        let fixtures = try DenseTransactionFixtures.load()
        let descriptor = try XCTUnwrap(fixtures.cases.first { $0.name == "transfer_asset" }).descriptor
        let networkId = try NetworkId(literal: descriptor.networkId)
        let pair = try TransactionInstructionWire(
            wireName: "test.opaque",
            payload: Data([0x07]),
            layout: .dense
        )
        func build(
            executable: TransactionExecutableWire,
            ttlMs: UInt64? = 1_000,
            nonce: UInt32? = nil,
            gasLimit: UInt64? = nil,
            layout: NoritoWireLayout = .dense
        ) throws -> Data {
            try SingleInstructionSwiftNoritoEncoder.transactionPayload(
                networkId: networkId,
                authority: descriptor.authority,
                creationTimeMs: 1,
                ttlMs: ttlMs,
                nonce: nonce,
                executable: executable,
                feePayment: .authority(chargeLimits: [], gasLimit: gasLimit),
                metadata: [:],
                layout: layout
            )
        }
        func assertInputError(
            _ expected: ExecutableBatchInputError,
            _ body: @autoclosure () throws -> Data,
            line: UInt = #line
        ) {
            XCTAssertThrowsError(try body(), line: line) { error in
                XCTAssertEqual(error as? ExecutableBatchInputError, expected, line: line)
            }
        }

        let payload = try build(executable: .instructions([pair]))
        let inspected = try ToriiCanonicalTransactionDraft.inspectTransactionPayload(
            payload,
            layout: .dense,
            context: "built"
        )
        XCTAssertEqual(inspected.creationTimeMs, 1)
        XCTAssertEqual(inspected.timeToLiveMs, 1_000)
        XCTAssertNil(inspected.nonce)
        XCTAssertEqual(inspected.networkId, networkId)

        assertInputError(.emptyBatch, try build(executable: .instructions([])))
        assertInputError(.zeroTimeToLive, try build(executable: .instructions([pair]), ttlMs: 0))
        assertInputError(.zeroNonce, try build(executable: .instructions([pair]), nonce: 0))
        assertInputError(.missingGasLimit, try build(executable: .ivm(Data([1]))))
        XCTAssertNoThrow(try build(executable: .ivm(Data([1])), gasLimit: 10))
        // Privacy instructions are admitted only through their instruction
        // frames: this opaque path refuses every wire ID of that namespace.
        for privacyWireId in [
            PrivacyExact12FixtureCodecV1.submitProofWireId,
            PrivacyExact12FixtureCodecV1.submitX509WireId,
        ] {
            XCTAssertTrue(PrivacyExact12FixtureCodecV1.isPrivacyInstructionWire(privacyWireId))
            // The namespace is refused before any body is read, so any frame serves.
            let privacyPair = try TransactionInstructionWire(
                wireName: privacyWireId,
                payload: noritoEncode(
                    typeName: "test.Instruction",
                    payload: Data([0x07]),
                    flags: NoritoHeader.compactLen
                ),
                layout: .compact
            )
            for executable in [
                TransactionExecutableWire.instructions([privacyPair]),
                .batch([.instruction(pair), .instruction(privacyPair)]),
            ] {
                XCTAssertThrowsError(try build(executable: executable), privacyWireId) { error in
                    guard case SwiftTransactionEncoderError.invalidInput = error else {
                        return XCTFail("unexpected error for \(privacyWireId): \(error)")
                    }
                }
            }
        }
        // An instruction pair of one layout class is not written into the other.
        XCTAssertThrowsError(try build(executable: .instructions([pair]), layout: .compact))
    }

    func testDraftDecodersNameTheirLayout() throws {
        // `decodeString` and the Ed25519 authority decoder default to the
        // compact layout and follow an explicit dense layout.
        let text = "dense"
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.decodeString(
                NoritoLayoutCodec.encodeString(text, layout: .compact),
                field: "text"
            ),
            text
        )
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.decodeString(
                NoritoLayoutCodec.encodeString(text, layout: .dense),
                field: "text",
                layout: .dense
            ),
            text
        )
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.decodeString(
                NoritoLayoutCodec.encodeString(text, layout: .fixed),
                field: "text",
                layout: .fixed
            ),
            text
        )
        XCTAssertThrowsError(
            try ToriiCanonicalTransactionDraft.decodeString(
                NoritoLayoutCodec.encodeString(text, layout: .fixed),
                field: "text",
                layout: .dense
            )
        )

        let key = Curve25519.Signing.PrivateKey().publicKey.rawRepresentation
        func controller(_ layout: NoritoWireLayout) -> Data {
            NoritoLayoutCodec.encodeEnum(
                tag: 0,
                fields: [NoritoLayoutCodec.encodeConstVec(Data([0]) + key, layout: layout)],
                layout: layout
            )
        }
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.decodeEd25519Authority(
                controller(.compact),
                context: "authority"
            ),
            key
        )
        XCTAssertEqual(
            try ToriiCanonicalTransactionDraft.decodeEd25519Authority(
                controller(.dense),
                context: "authority",
                layout: .dense
            ),
            key
        )
        XCTAssertEqual(controller(.dense), Data([0x00, 0x22, 0x21, 0x00]) + key)
        XCTAssertThrowsError(
            try ToriiCanonicalTransactionDraft.decodeEd25519Authority(
                controller(.dense),
                context: "authority"
            )
        )
        XCTAssertThrowsError(
            try ToriiCanonicalTransactionDraft.decodeEd25519Authority(
                controller(.compact),
                context: "authority",
                layout: .dense
            )
        )
    }

    // MARK: - Helpers

    private func assertExecutable(
        _ executable: TransactionExecutableWire,
        matches descriptor: DenseTransactionFixtures.Executable,
        layout: NoritoWireLayout,
        name: String
    ) throws {
        func assertInstruction(
            _ instruction: TransactionInstructionWire,
            _ expected: DenseTransactionFixtures.Instruction
        ) throws {
            // Every layout decodes the instruction to the descriptor's typed frame.
            XCTAssertEqual(instruction.wireName, expected.wireName, name)
            XCTAssertFalse(instruction.isDense, name)
            XCTAssertEqual(instruction.payload, expected.framedPayload, name)
            if layout == .dense {
                // The dense pair carries the bare payload: no nested header.
                let bare = try instruction.envelopePayload(layout: .dense)
                XCTAssertNil(noritoDecodeDeclaredFrame(bare), name)
                XCTAssertLessThan(bare.count, expected.framedPayload.count, name)
            }
        }
        switch (executable, descriptor) {
        case let (.instructions(instructions), .instructions(expected)):
            XCTAssertEqual(instructions.count, expected.count, name)
            for (instruction, expectedInstruction) in zip(instructions, expected) {
                try assertInstruction(instruction, expectedInstruction)
            }
        case let (.ivm(bytecode), .ivm(expected)):
            XCTAssertEqual(bytecode, expected, name)
        case let (.contractCall(invocation), .contractCall(expected)):
            XCTAssertEqual(invocation, try expected.invocation(), name)
        case let (.batch(items), .batch(expected)):
            XCTAssertEqual(items.count, expected.count, name)
            for (item, expectedItem) in zip(items, expected) {
                switch (item, expectedItem) {
                case let (.instruction(instruction), .instruction(expectedInstruction)):
                    try assertInstruction(instruction, expectedInstruction)
                case let (.contractCall(invocation), .contractCall(expectedCall)):
                    XCTAssertEqual(invocation, try expectedCall.invocation(), name)
                default:
                    XCTFail("\(name): batch item kind differs from the descriptor")
                }
            }
        default:
            XCTFail("\(name): executable variant differs from the descriptor")
        }
    }

    fileprivate static var repositoryRoot: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // NoritoRpcDenseFixtureTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
    }

    fileprivate static func bareFixtureFrame(_ data: Data, typeName: String, name: String) throws -> Data {
        guard let frame = noritoDecodeFrame(data),
              frame.layout == .compact,
              frame.header.schema == noritoSchemaHash(forTypeName: typeName),
              frame.paddingLength == 0 else {
            throw DenseTransactionFixtures.LoadError.invalidCompactFrame(name)
        }
        return frame.payload
    }
}

/// The dense manifest joined with the current-layout manifest and the payload
/// descriptor both were generated from.
private struct DenseTransactionFixtures {
    enum LoadError: Error {
        case layout(String)
        case nameSetMismatch
        case invalidBase64(String)
        case invalidCompactFrame(String)
        case invalidDescriptor(String)
    }

    struct Case {
        let name: String
        let densePayload: Data
        let denseSigned: Data
        let signingHash: String
        /// Bare compact payload and signed transaction (their frames removed).
        let compactPayload: Data
        let compactSigned: Data
        let descriptor: Descriptor
    }

    struct Instruction: Equatable {
        let wireName: String
        /// The instruction as the descriptor carries it: one current-layout frame.
        let framedPayload: Data
    }

    struct ContractCall: Equatable {
        let contractAddress: String
        let expectedCodeHash: Data
        let entrypoint: String
        let arguments: Data?

        func invocation() throws -> TransactionContractInvocation {
            try TransactionContractInvocation(
                contractAddress: contractAddress,
                expectedCodeHash: expectedCodeHash,
                entrypoint: entrypoint,
                arguments: arguments
            )
        }
    }

    enum BatchItem: Equatable {
        case instruction(Instruction)
        case contractCall(ContractCall)
    }

    enum Executable: Equatable {
        case instructions([Instruction])
        case contractCall(ContractCall)
        case ivm(Data)
        case batch([BatchItem])

        /// The typed executable, entirely from the descriptor: each instruction
        /// is its current-layout frame, which encodes in either layout.
        func wire() throws -> TransactionExecutableWire {
            func pair(_ instruction: Instruction) throws -> TransactionInstructionWire {
                try TransactionInstructionWire(
                    wireName: instruction.wireName,
                    payload: instruction.framedPayload,
                    layout: .compact
                )
            }
            let wire: TransactionExecutableWire
            switch self {
            case let .instructions(instructions):
                wire = .instructions(try instructions.map(pair))
            case let .contractCall(call):
                wire = .contractCall(try call.invocation())
            case let .ivm(bytecode):
                wire = .ivm(bytecode)
            case let .batch(items):
                wire = .batch(
                    try items.map { item -> TransactionBatchItemWire in
                        switch item {
                        case let .instruction(instruction):
                            return .instruction(try pair(instruction))
                        case let .contractCall(call):
                            return .contractCall(try call.invocation())
                        }
                    }
                )
            }
            return wire
        }
    }

    struct Descriptor {
        let authority: String
        let networkId: String
        let creationTimeMs: UInt64
        let timeToLiveMs: UInt64
        let nonce: UInt32?
        let executable: Executable
        let feePayment: FeePaymentIntent
        let metadata: [String: ToriiJSONValue]
    }

    let cases: [Case]

    static func load() throws -> DenseTransactionFixtures {
        let directory = NoritoRpcDenseFixtureTests.repositoryRoot
            .appendingPathComponent("fixtures/norito_rpc")
        let decoder = JSONDecoder()
        func document<T: Decodable>(_ type: T.Type, _ file: String) throws -> T {
            let url = directory.appendingPathComponent(file)
            return try StrictFixtureJSON.decode(
                type,
                from: Data(contentsOf: url),
                using: decoder,
                context: url.path
            )
        }
        let dense = try document(DenseManifest.self, "transaction_fixtures.dense.json")
        guard dense.layout == "dense" else { throw LoadError.layout(dense.layout) }
        let manifest = try document(CompactManifest.self, "transaction_fixtures.manifest.json")
        let descriptors = try document([DescriptorEntry].self, "transaction_payloads.json")
        let compactByName = Dictionary(
            manifest.fixtures.map { ($0.name, $0) },
            uniquingKeysWith: { first, _ in first }
        )
        let descriptorByName = Dictionary(
            descriptors.map { ($0.name, $0) },
            uniquingKeysWith: { first, _ in first }
        )
        guard Set(dense.fixtures.map(\.name)).count == dense.fixtures.count,
              Set(dense.fixtures.map(\.name)) == Set(compactByName.keys),
              Set(dense.fixtures.map(\.name)) == Set(descriptorByName.keys),
              manifest.fixtures.count == dense.fixtures.count,
              descriptors.count == dense.fixtures.count else {
            throw LoadError.nameSetMismatch
        }
        func base64(_ value: String, _ context: String) throws -> Data {
            guard let data = Data(base64Encoded: value), !data.isEmpty,
                  data.base64EncodedString() == value else {
                throw LoadError.invalidBase64(context)
            }
            return data
        }
        let cases = try dense.fixtures.map { entry -> Case in
            guard let compact = compactByName[entry.name],
                  let descriptor = descriptorByName[entry.name] else {
                throw LoadError.nameSetMismatch
            }
            return Case(
                name: entry.name,
                densePayload: try base64(entry.payloadBase64, "\(entry.name).dense.payload"),
                denseSigned: try base64(entry.signedBase64, "\(entry.name).dense.signed"),
                signingHash: entry.signingHash,
                compactPayload: try NoritoRpcDenseFixtureTests.bareFixtureFrame(
                    try base64(compact.payloadBase64, "\(entry.name).payload"),
                    typeName: "iroha_data_model::transaction::signed::model::TransactionPayload",
                    name: entry.name
                ),
                compactSigned: try NoritoRpcDenseFixtureTests.bareFixtureFrame(
                    try base64(compact.signedBase64, "\(entry.name).signed"),
                    typeName: "iroha_data_model::transaction::signed::model::SignedTransaction",
                    name: entry.name
                ),
                descriptor: try descriptor.payload.descriptor(base64: base64)
            )
        }
        return DenseTransactionFixtures(cases: cases)
    }

    // MARK: JSON shapes

    private struct DenseManifest: Decodable {
        struct Entry: Decodable {
            let name: String
            let payloadBase64: String
            let signingHash: String
            let signedBase64: String

            private enum CodingKeys: String, CodingKey {
                case name
                case payloadBase64 = "payload_base64"
                case signingHash = "signing_hash"
                case signedBase64 = "signed_base64"
            }

            init(from decoder: Decoder) throws {
                try requireExactFixtureKeys(
                    decoder,
                    ["name", "payload_base64", "signing_hash", "signed_base64"],
                    context: "dense transaction fixture"
                )
                let container = try decoder.container(keyedBy: CodingKeys.self)
                name = try container.decode(String.self, forKey: .name)
                payloadBase64 = try container.decode(String.self, forKey: .payloadBase64)
                signingHash = try container.decode(String.self, forKey: .signingHash)
                signedBase64 = try container.decode(String.self, forKey: .signedBase64)
            }
        }

        let layout: String
        let fixtures: [Entry]

        private enum CodingKeys: String, CodingKey {
            case layout, fixtures
        }

        init(from decoder: Decoder) throws {
            try requireExactFixtureKeys(decoder, ["layout", "fixtures"], context: "dense manifest")
            let container = try decoder.container(keyedBy: CodingKeys.self)
            layout = try container.decode(String.self, forKey: .layout)
            fixtures = try container.decode([Entry].self, forKey: .fixtures)
        }
    }

    private struct CompactManifest: Decodable {
        struct Entry: Decodable {
            let name: String
            let payloadBase64: String
            let signedBase64: String

            private enum CodingKeys: String, CodingKey {
                case name
                case payloadBase64 = "payload_base64"
                case signedBase64 = "signed_base64"
            }
        }

        let fixtures: [Entry]
    }

    private struct DescriptorEntry: Decodable {
        let name: String
        let payload: DescriptorPayload
    }

    private struct DescriptorPayload: Decodable {
        let authority: String
        let networkId: String
        let creationTimeMs: UInt64
        let timeToLiveMs: UInt64
        let nonce: UInt32?
        let executable: ExecutableJSON
        let feePayment: FeePaymentIntent
        let metadata: [String: ToriiJSONValue]

        private enum CodingKeys: String, CodingKey {
            case authority
            case networkId = "network_id"
            case creationTimeMs = "creation_time_ms"
            case timeToLiveMs = "time_to_live_ms"
            case nonce
            case executable
            case feePayment = "fee_payment"
            case metadata
        }

        func descriptor(base64: (String, String) throws -> Data) throws -> Descriptor {
            Descriptor(
                authority: authority,
                networkId: networkId,
                creationTimeMs: creationTimeMs,
                timeToLiveMs: timeToLiveMs,
                nonce: nonce,
                executable: try executable.executable(base64: base64),
                feePayment: feePayment,
                metadata: metadata
            )
        }
    }

    private struct InstructionJSON: Decodable {
        let payloadBase64: String
        let wireName: String

        private enum CodingKeys: String, CodingKey {
            case payloadBase64 = "payload_base64"
            case wireName = "wire_name"
        }

        func instruction(base64: (String, String) throws -> Data) throws -> Instruction {
            Instruction(
                wireName: wireName,
                framedPayload: try base64(payloadBase64, "instruction \(wireName)")
            )
        }
    }

    private struct ContractCallJSON: Decodable {
        let contractAddress: String
        let expectedCodeHash: String
        let entrypoint: String
        let arguments: [UInt8]?

        private enum CodingKeys: String, CodingKey {
            case contractAddress = "contract_address"
            case expectedCodeHash = "expected_code_hash"
            case entrypoint
            case arguments
        }

        func contractCall() throws -> ContractCall {
            // `hash:<64 uppercase hex>#<CRC16>`.
            let literal = Array(expectedCodeHash.utf8)
            guard literal.count == 74,
                  expectedCodeHash.hasPrefix("hash:"),
                  let hash = Data(hexString: String(decoding: literal[5..<69], as: UTF8.self)),
                  hash.count == 32 else {
                throw LoadError.invalidDescriptor("expected_code_hash")
            }
            return ContractCall(
                contractAddress: contractAddress,
                expectedCodeHash: hash,
                entrypoint: entrypoint,
                arguments: arguments.map { Data($0) }
            )
        }
    }

    private struct BatchItemJSON: Decodable {
        let instruction: InstructionJSON?
        let contractCall: ContractCallJSON?

        private enum CodingKeys: String, CodingKey {
            case instruction = "Instruction"
            case contractCall = "ContractCall"
        }
    }

    private struct ExecutableJSON: Decodable {
        let instructions: [InstructionJSON]?
        let contractCall: ContractCallJSON?
        let ivm: String?
        let batch: [BatchItemJSON]?

        private enum CodingKeys: String, CodingKey {
            case instructions = "Instructions"
            case contractCall = "ContractCall"
            case ivm = "Ivm"
            case batch = "Batch"
        }

        func executable(base64: (String, String) throws -> Data) throws -> Executable {
            switch (instructions, contractCall, ivm, batch) {
            case let (.some(instructions), nil, nil, nil):
                return .instructions(try instructions.map { try $0.instruction(base64: base64) })
            case let (nil, .some(call), nil, nil):
                return .contractCall(try call.contractCall())
            case let (nil, nil, .some(bytecode), nil):
                return .ivm(try base64(bytecode, "Ivm"))
            case let (nil, nil, nil, .some(items)):
                return .batch(
                    try items.map { item -> BatchItem in
                        switch (item.instruction, item.contractCall) {
                        case let (.some(instruction), nil):
                            return .instruction(try instruction.instruction(base64: base64))
                        case let (nil, .some(call)):
                            return .contractCall(try call.contractCall())
                        default:
                            throw LoadError.invalidDescriptor("batch item")
                        }
                    }
                )
            default:
                throw LoadError.invalidDescriptor("executable")
            }
        }
    }
}
