import CryptoKit
import XCTest
@testable import IrohaSwift

final class ConfidentialNoteTests: XCTestCase {
    func testDerivesCanonicalNativeConfidentialV3Values() throws {
        XCTAssertEqual(
            ConfidentialNoteNativeDerivation.loadedContractRevisionV3(),
            ConfidentialNoteNativeDerivation.contractRevisionV3
        )
        let spendKey = Data(repeating: 0x11, count: 32)
        let rho = Data(repeating: 0x22, count: 32)
        let ownerTag = try ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        let opening = try ConfidentialNoteOpening(
            rho: rho,
            spendKey: spendKey,
            ownerTag: ownerTag,
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "7"
        )

        for digest in [
            ownerTag,
            try ConfidentialNoteCommitment.deriveFromOpening(opening),
            try ConfidentialNoteNullifier.deriveFromOpening(opening),
            try ConfidentialNoteTags.deriveAssetTag("rose#wonderland"),
            try ConfidentialNoteTags.deriveNetworkTag(TestNetworkIds.canonical),
        ] {
            XCTAssertEqual(digest.count, 32)
            XCTAssertNotEqual(digest, Data(repeating: 0, count: 32))
        }
        XCTAssertNotEqual(
            try ConfidentialNoteTags.deriveNetworkTag(TestNetworkIds.canonical),
            try ConfidentialNoteTags.deriveNetworkTag(TestNetworkIds.other)
        )

        let diversifier = try ConfidentialOwnerTag.deriveDiversifier(Data("recipient".utf8))
        XCTAssertEqual(diversifier.count, 32)
        XCTAssertNotEqual(diversifier, Data(repeating: 0, count: 32))
        let diversifiedOwner = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(
            spendKey,
            diversifier: diversifier
        )
        XCTAssertEqual(diversifiedOwner.count, 32)
        XCTAssertNotEqual(diversifiedOwner, Data(repeating: 0, count: 32))
    }

    func testOpeningCopiesInputsAndAccessorsCannotMutateState() throws {
        var rho = Data(repeating: 0x22, count: 32)
        var spendKey = Data(repeating: 0x11, count: 32)
        var ownerTag = try ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        let opening = try ConfidentialNoteOpening(
            rho: rho,
            spendKey: spendKey,
            ownerTag: ownerTag,
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "1"
        )

        rho[0] = 0x55
        spendKey[0] = 0x66
        ownerTag[0] = 0x77
        var exposedRho = opening.rho
        var exposedSpendKey = opening.spendKey
        var exposedOwnerTag = opening.ownerTag
        exposedRho[0] = 0x44
        exposedSpendKey[0] = 0x33
        exposedOwnerTag[0] = 0x22

        XCTAssertEqual(opening.rho, Data(repeating: 0x22, count: 32))
        XCTAssertEqual(opening.spendKey, Data(repeating: 0x11, count: 32))
        XCTAssertEqual(
            opening.ownerTag,
            try ConfidentialOwnerTag.deriveFromSpendKey(Data(repeating: 0x11, count: 32))
        )
    }

    func testDerivationsAreDomainSeparated() throws {
        let base = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x22, count: 32),
            spendKey: Data(repeating: 0x11, count: 32),
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "7"
        )
        let differentRho = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x23, count: 32),
            spendKey: Data(repeating: 0x11, count: 32),
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "7"
        )
        let differentChain = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x22, count: 32),
            spendKey: Data(repeating: 0x11, count: 32),
            asset: "rose#wonderland",
            networkId: TestNetworkIds.other,
            amount: "7"
        )
        let differentAsset = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x22, count: 32),
            spendKey: Data(repeating: 0x11, count: 32),
            asset: "iris#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "7"
        )
        let differentAmount = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x22, count: 32),
            spendKey: Data(repeating: 0x11, count: 32),
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "8"
        )

        let baseCommitment = try ConfidentialNoteCommitment.deriveFromOpening(base)
        XCTAssertNotEqual(baseCommitment, try ConfidentialNoteCommitment.deriveFromOpening(differentRho))
        XCTAssertEqual(baseCommitment, try ConfidentialNoteCommitment.deriveFromOpening(differentChain))
        XCTAssertNotEqual(baseCommitment, try ConfidentialNoteCommitment.deriveFromOpening(differentAsset))
        XCTAssertNotEqual(baseCommitment, try ConfidentialNoteCommitment.deriveFromOpening(differentAmount))

        let baseNullifier = try ConfidentialNoteNullifier.deriveFromOpening(base)
        XCTAssertNotEqual(baseNullifier, try ConfidentialNoteNullifier.deriveFromOpening(differentRho))
        XCTAssertNotEqual(baseNullifier, try ConfidentialNoteNullifier.deriveFromOpening(differentChain))
        XCTAssertNotEqual(baseNullifier, try ConfidentialNoteNullifier.deriveFromOpening(differentAsset))
        XCTAssertEqual(baseNullifier, try ConfidentialNoteNullifier.deriveFromOpening(differentAmount))
    }

    func testEncryptsKotlinPlaintextContractVector() throws {
        let spendKey = Data(repeating: 0x11, count: 32)
        let opening = try ConfidentialNoteOpening.fromSpendKey(
            rho: Data(repeating: 0x22, count: 32),
            spendKey: spendKey,
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "7"
        )
        let recipientPrivateKey = Data(repeating: 0x55, count: 32)
        let recipientPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey)
        let payload = try ConfidentialNoteEncryption.encryptNote(
            opening: opening,
            recipientPublicKey: recipientPublicKey,
            ephemeralPrivateKey: Data(repeating: 0x66, count: 32),
            nonce: Data(repeating: 0x77, count: 24)
        )

        XCTAssertEqual(
            recipientPublicKey,
            try hex("38ab664bd86f77d7e66bdd9ae0792913a94fd8b33a1260027e4b46c1f4884c67")
        )
        XCTAssertEqual(
            payload.ephemeralPublicKey,
            try hex("219e4d800da968d2a5fcb009c784f4746c7138edb9ee4844b739e830b05cf424")
        )
        XCTAssertEqual(payload.nonce, Data(repeating: 0x77, count: 24))
        XCTAssertFalse(payload.ciphertext.isEmpty)

        let decrypted = try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        )
        assertOpeningEquals(opening, decrypted)
        XCTAssertEqual(
            try ConfidentialNoteCommitment.deriveFromOpening(decrypted),
            try ConfidentialNoteCommitment.deriveFromOpening(opening)
        )
        XCTAssertEqual(
            try ConfidentialNoteNullifier.deriveFromOpening(decrypted),
            try ConfidentialNoteNullifier.deriveFromOpening(opening)
        )

        var tamperedCiphertext = payload.ciphertext
        tamperedCiphertext[tamperedCiphertext.index(before: tamperedCiphertext.endIndex)] ^= 0x01
        let tamperedPayload = try ConfidentialEncryptedPayload(
            ephemeralPublicKey: payload.ephemeralPublicKey,
            nonce: payload.nonce,
            ciphertext: tamperedCiphertext
        )
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: tamperedPayload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        ))
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: Data(repeating: 0x56, count: 32),
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        ))
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.other
        ))
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: Data(repeating: 0, count: 32),
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        ))
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: Data(repeating: 0x12, count: 32),
            expectedNetworkId: TestNetworkIds.canonical
        ))
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
            encryptedPayload: payload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedOwnerTag: try ConfidentialOwnerTag.deriveFromSpendKey(Data(repeating: 0x12, count: 32)),
            expectedNetworkId: TestNetworkIds.canonical
        ))

        let diversifier = try ConfidentialOwnerTag.deriveDiversifier(Data("invoice-1".utf8))
        let diversifiedOpening = try ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
            rho: Data(repeating: 0x24, count: 32),
            spendKey: spendKey,
            diversifier: diversifier,
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "11"
        )
        let diversifiedPayload = try ConfidentialNoteEncryption.encryptNote(
            opening: diversifiedOpening,
            recipientPublicKey: recipientPublicKey,
            ephemeralPrivateKey: Data(repeating: 0x68, count: 32),
            nonce: Data(repeating: 0x79, count: 24)
        )
        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: diversifiedPayload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        ))
        assertOpeningEquals(
            diversifiedOpening,
            try ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                encryptedPayload: diversifiedPayload,
                recipientPrivateKey: recipientPrivateKey,
                spendKey: spendKey,
                expectedOwnerTag: diversifiedOpening.ownerTag,
                expectedNetworkId: TestNetworkIds.canonical
            )
        )
    }

    func testDecryptRejectsNonCanonicalPlaintextLengthVarints() throws {
        let spendKey = Data(repeating: 0x11, count: 32)
        let ownerTag = try ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        let recipientPrivateKey = Data(repeating: 0x55, count: 32)
        let recipientPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey)
        let ephemeralPrivateKey = Data(repeating: 0x66, count: 32)
        let nonce = Data(repeating: 0x77, count: 24)
        let ephemeralPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(ephemeralPrivateKey)
        let key = try ConfidentialNoteEncryption.derivePayloadKey(
            localPrivateKey: ephemeralPrivateKey,
            peerPublicKey: recipientPublicKey,
            ephemeralPublicKey: ephemeralPublicKey,
            recipientPublicKey: recipientPublicKey
        )

        var plaintext = Data()
        plaintext.append(ConfidentialNoteEncryption.plaintextVersion)
        plaintext.append(Data(repeating: 0x22, count: 32))
        plaintext.append(ownerTag)
        plaintext.append(contentsOf: [0x8f, 0x00])
        plaintext.append(Data("rose#wonderland".utf8))
        plaintext.append(TestNetworkIds.canonical.bytes)
        appendCanonicalVarint(1, to: &plaintext)
        plaintext.append(Data("7".utf8))

        let ciphertext = try ConfidentialNoteEncryption.xChaCha20Poly1305Seal(
            key: key,
            nonce: nonce,
            aad: ConfidentialNoteEncryption.payloadAad(
                ephemeralPublicKey: ephemeralPublicKey,
                recipientPublicKey: recipientPublicKey
            ),
            plaintext: plaintext
        )
        let payload = try ConfidentialEncryptedPayload(
            ephemeralPublicKey: ephemeralPublicKey,
            nonce: nonce,
            ciphertext: ciphertext
        )

        XCTAssertThrowsError(try ConfidentialNoteDecryption.decryptNote(
            encryptedPayload: payload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedNetworkId: TestNetworkIds.canonical
        ))
    }

    func testDecryptRejectsAuthenticatedMalformedPlaintextShapes() throws {
        let spendKey = Data(repeating: 0x11, count: 32)
        let ownerTag = try ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        let recipientPrivateKey = Data(repeating: 0x55, count: 32)
        let recipientPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey)
        let ephemeralPrivateKey = Data(repeating: 0x66, count: 32)
        let nonce = Data(repeating: 0x77, count: 24)
        let ephemeralPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(ephemeralPrivateKey)
        let key = try ConfidentialNoteEncryption.derivePayloadKey(
            localPrivateKey: ephemeralPrivateKey,
            peerPublicKey: recipientPublicKey,
            ephemeralPublicKey: ephemeralPublicKey,
            recipientPublicKey: recipientPublicKey
        )

        func payload(for plaintext: Data) throws -> ConfidentialEncryptedPayload {
            let ciphertext = try ConfidentialNoteEncryption.xChaCha20Poly1305Seal(
                key: key,
                nonce: nonce,
                aad: ConfidentialNoteEncryption.payloadAad(
                    ephemeralPublicKey: ephemeralPublicKey,
                    recipientPublicKey: recipientPublicKey
                ),
                plaintext: plaintext
            )
            return try ConfidentialEncryptedPayload(
                ephemeralPublicKey: ephemeralPublicKey,
                nonce: nonce,
                ciphertext: ciphertext
            )
        }

        let valid = plaintext(
            ownerTag: ownerTag,
            asset: Data("rose#wonderland".utf8),
            networkId: TestNetworkIds.canonical.bytes,
            amount: Data("7".utf8)
        )
        var invalidOwnerTag = valid
        invalidOwnerTag.replaceSubrange(33..<65, with: Data(repeating: 0xff, count: 32))
        var trailingPlaintext = valid
        trailingPlaintext.append(0)
        var truncatedPlaintext = valid
        truncatedPlaintext.removeLast()
        var overflowingVarint = Data()
        overflowingVarint.append(ConfidentialNoteEncryption.plaintextVersion)
        overflowingVarint.append(Data(repeating: 0x22, count: 32))
        overflowingVarint.append(ownerTag)
        overflowingVarint.append(Data(repeating: 0x80, count: 10))
        overflowingVarint.append(0x02)

        let malformedPlaintexts: [(field: String, plaintext: Data)] = [
            ("plaintext.version", Data()),
            ("plaintext.version", Data([ConfidentialNoteEncryption.plaintextVersion &+ 1])),
            (
                "plaintext",
                Data([ConfidentialNoteEncryption.plaintextVersion])
                    + Data(repeating: 0x22, count: 10)
            ),
            ("ownerTag", invalidOwnerTag),
            (
                "asset",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data(),
                    networkId: TestNetworkIds.canonical.bytes,
                    amount: Data("1".utf8)
                )
            ),
            (
                "asset",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data([0xff]),
                    networkId: TestNetworkIds.canonical.bytes,
                    amount: Data("1".utf8)
                )
            ),
            (
                "asset",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data(" rose#wonderland".utf8),
                    networkId: TestNetworkIds.canonical.bytes,
                    amount: Data("1".utf8)
                )
            ),
            (
                "networkId",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data("rose#wonderland".utf8),
                    networkId: Data(),
                    amount: Data("1".utf8)
                )
            ),
            (
                "amount",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data("rose#wonderland".utf8),
                    networkId: TestNetworkIds.canonical.bytes,
                    amount: Data("07".utf8)
                )
            ),
            (
                "amount",
                plaintext(
                    ownerTag: ownerTag,
                    asset: Data("rose#wonderland".utf8),
                    networkId: TestNetworkIds.canonical.bytes,
                    amount: Data("340282366920938463463374607431768211456".utf8)
                )
            ),
            ("plaintext.trailing", trailingPlaintext),
            ("amount", truncatedPlaintext),
            ("varint", overflowingVarint)
        ]

        for malformed in malformedPlaintexts {
            let encryptedPayload = try payload(for: malformed.plaintext)
            XCTAssertInvalidField(malformed.field) {
                try ConfidentialNoteDecryption.decryptNote(
                    encryptedPayload: encryptedPayload,
                    recipientPrivateKey: recipientPrivateKey,
                    spendKey: spendKey,
                    expectedNetworkId: TestNetworkIds.canonical
                )
            }
        }
    }

    func testRejectsMalformedInputs() throws {
        let spendKey = Data(repeating: 0x11, count: 32)
        let rho = Data(repeating: 0x22, count: 32)
        let ownerTag = try ConfidentialOwnerTag.deriveFromSpendKey(spendKey)
        let opening = try ConfidentialNoteOpening(
            rho: rho,
            spendKey: spendKey,
            ownerTag: ownerTag,
            asset: "rose#wonderland",
            networkId: TestNetworkIds.canonical,
            amount: "1"
        )
        let recipientPrivateKey = Data(repeating: 0x55, count: 32)
        let recipientPublicKey = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey)
        let payload = try ConfidentialNoteEncryption.encryptNote(
            opening: opening,
            recipientPublicKey: recipientPublicKey,
            ephemeralPrivateKey: Data(repeating: 0x66, count: 32),
            nonce: Data(repeating: 0x77, count: 24)
        )

        XCTAssertInvalidField("rho") {
            try ConfidentialNoteOpening(
                rho: Data(repeating: 0, count: 31),
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("rho") {
            try ConfidentialNoteOpening(
                rho: Data(repeating: 0, count: 32),
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("spendKey") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: Data(),
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("spendKey") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: Data(repeating: 0, count: 32),
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("ownerTag") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: Data(repeating: 0, count: 32),
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("ownerTag") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: Data(repeating: 0xff, count: 32),
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("asset") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: " rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "1"
            )
        }
        XCTAssertInvalidField("amount") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "01"
            )
        }
        XCTAssertInvalidField("amount") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "0"
            )
        }
        XCTAssertInvalidField("amount") {
            try ConfidentialNoteOpening(
                rho: rho,
                spendKey: spendKey,
                ownerTag: ownerTag,
                asset: "rose#wonderland",
                networkId: TestNetworkIds.canonical,
                amount: "340282366920938463463374607431768211456"
            )
        }
        XCTAssertInvalidField("privateKey") {
            try ConfidentialNoteEncryption.publicKeyFromPrivateKey(Data(repeating: 0, count: 32))
        }
        XCTAssertInvalidField("recipientPublicKey") {
            try ConfidentialNoteEncryption.encryptNote(
                opening: opening,
                recipientPublicKey: Data(repeating: 0x55, count: 31),
                ephemeralPrivateKey: Data(repeating: 0x66, count: 32),
                nonce: Data(repeating: 0x77, count: 24)
            )
        }
        XCTAssertInvalidField("peerPublicKey") {
            try ConfidentialNoteEncryption.encryptNote(
                opening: opening,
                recipientPublicKey: Data([1]) + Data(repeating: 0, count: 31),
                ephemeralPrivateKey: Data(repeating: 0x66, count: 32),
                nonce: Data(repeating: 0x77, count: 24)
            )
        }
        XCTAssertInvalidField("ephemeralPrivateKey") {
            try ConfidentialNoteEncryption.encryptNote(
                opening: opening,
                recipientPublicKey: recipientPublicKey,
                ephemeralPrivateKey: Data(repeating: 0, count: 32),
                nonce: Data(repeating: 0x77, count: 24)
            )
        }
        XCTAssertInvalidField("nonce") {
            try ConfidentialNoteEncryption.encryptNote(
                opening: opening,
                recipientPublicKey: recipientPublicKey,
                ephemeralPrivateKey: Data(repeating: 0x66, count: 32),
                nonce: Data(repeating: 0x77, count: 23)
            )
        }
        XCTAssertInvalidField("recipientPrivateKey") {
            try ConfidentialNoteDecryption.decryptNote(
                encryptedPayload: payload,
                recipientPrivateKey: Data(repeating: 0, count: 32),
                spendKey: spendKey,
                expectedNetworkId: TestNetworkIds.canonical
            )
        }
        XCTAssertInvalidField("expectedOwnerTag") {
            try ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
                encryptedPayload: payload,
                recipientPrivateKey: recipientPrivateKey,
                spendKey: spendKey,
                expectedOwnerTag: Data(repeating: 0x44, count: 31),
                expectedNetworkId: TestNetworkIds.canonical
            )
        }
    }

    private func assertOpeningEquals(
        _ expected: ConfidentialNoteOpening,
        _ actual: ConfidentialNoteOpening,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertEqual(actual.rho, expected.rho, file: file, line: line)
        XCTAssertEqual(actual.spendKey, expected.spendKey, file: file, line: line)
        XCTAssertEqual(actual.ownerTag, expected.ownerTag, file: file, line: line)
        XCTAssertEqual(actual.asset, expected.asset, file: file, line: line)
        XCTAssertEqual(actual.networkId, expected.networkId, file: file, line: line)
        XCTAssertEqual(actual.amount, expected.amount, file: file, line: line)
    }

    private func hex(_ value: String) throws -> Data {
        try XCTUnwrap(Data(hexString: value))
    }

    private func plaintext(
        rho: Data = Data(repeating: 0x22, count: 32),
        ownerTag: Data,
        asset: Data,
        networkId: Data,
        amount: Data
    ) -> Data {
        var out = Data()
        out.append(ConfidentialNoteEncryption.plaintextVersion)
        out.append(rho)
        out.append(ownerTag)
        appendCanonicalVarint(UInt64(asset.count), to: &out)
        out.append(asset)
        out.append(networkId)
        appendCanonicalVarint(UInt64(amount.count), to: &out)
        out.append(amount)
        return out
    }

    private func appendCanonicalVarint(_ value: UInt64, to data: inout Data) {
        var remaining = value
        while remaining >= 0x80 {
            data.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        data.append(UInt8(remaining))
    }

    private func XCTAssertInvalidField<T>(
        _ field: String,
        file: StaticString = #filePath,
        line: UInt = #line,
        _ expression: () throws -> T
    ) {
        XCTAssertThrowsError(try expression(), file: file, line: line) { error in
            XCTAssertEqual(
                error as? ConfidentialNoteError,
                ConfidentialNoteError.invalidField(field),
                file: file,
                line: line
            )
        }
    }
}
