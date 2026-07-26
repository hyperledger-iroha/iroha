import Foundation
import XCTest
@testable import IrohaSwift

private func canonicalOwnerLiteral(
    networkPrefix: UInt16 = AccountId.defaultNetworkPrefix
) throws -> String {
    let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
    let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
    let i105 = try address.toI105(networkPrefix: networkPrefix)
    return i105
}

private func noncanonicalOwnerLiteral() throws -> String {
    let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
    let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
    let canonicalHex = try address.canonicalHex()
    return canonicalHex
}

@available(macOS 10.15, iOS 13.0, *)
private func canonicalAuthorityLiteral(
    from signingKey: SigningKey,
    networkPrefix: UInt16 = AccountId.defaultNetworkPrefix
) throws -> String {
    let publicKey = try signingKey.publicKey()
    let address = try AccountAddress.fromAccount(publicKey: publicKey)
    let i105 = try address.toI105(networkPrefix: networkPrefix)
    return i105
}

final class TransactionEncoderValidationTests: XCTestCase {
    func testTransferEncoderPreservesTairaAccountLiterals() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native encoder not linked")
        }
        let signingKey = try SigningKey.ed25519(
            privateKey: Data(repeating: 0x31, count: 32)
        )
        let authority = try AccountAddress
            .fromAccount(publicKey: signingKey.publicKey())
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let destination = try AccountAddress
            .fromAccount(publicKey: Data(repeating: 0x32, count: 32))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let transfer = TransferRequest(
            chainId: "taira",
            authority: authority,
            assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            quantity: "1",
            destination: destination,
            description: nil,
            feePayment: .authority(chargeLimits: [], gasLimit: nil)
        )

        let envelope = try SwiftTransactionEncoder.encodeTransfer(
            transfer: transfer,
            signingKey: signingKey,
            creationTimeMs: 1
        )
        let decoded = try XCTUnwrap(
            NoritoNativeBridge.shared.decodeSignedTransaction(envelope.norito)
        )
        XCTAssertTrue(decoded.contains(authority), decoded)
        XCTAssertTrue(decoded.contains(destination), decoded)
    }

    func testAssetBuildersRejectNoncanonicalAndNegativeQuantitiesBeforeNativeDispatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 13, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

        for quantity in ["1.0", "01", "-1", " 1", "1e0"] {
            let transfer = TransferRequest(
                chainId: "chain",
                authority: authority,
                assetDefinitionId: assetDefinitionId,
                quantity: quantity,
                destination: authority,
                description: nil,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            )
            XCTAssertThrowsError(
                try SwiftTransactionEncoder.encodeTransfer(
                    transfer: transfer,
                    signingKey: signingKey,
                    creationTimeMs: 1
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }

            let mint = MintRequest(
                chainId: "chain",
                authority: authority,
                assetDefinitionId: assetDefinitionId,
                quantity: quantity,
                destination: authority,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            )
            XCTAssertThrowsError(
                try SwiftTransactionEncoder.encodeMint(
                    request: mint,
                    signingKey: signingKey,
                    creationTimeMs: 1
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }

            let burn = BurnRequest(
                chainId: "chain",
                authority: authority,
                assetDefinitionId: assetDefinitionId,
                quantity: quantity,
                destination: authority,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            )
            XCTAssertThrowsError(
                try SwiftTransactionEncoder.encodeBurn(
                    request: burn,
                    signingKey: signingKey,
                    creationTimeMs: 1
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
        }
    }

    func testSetMetadataRejectsMalformedAuthority() throws {
        let targetAccount = try canonicalOwnerLiteral()
        let value = try NoritoJSON(["profile": "demo"])
        let request = SetMetadataRequest(chainId: "chain",
                                         authority: "alice",
                                         target: .account(targetAccount),
                                         key: "profile",
                                         value: value,
                                         feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                         ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 1, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                          signingKey: signingKey,
                                                          creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice"))
        }
    }

    func testSetMetadataRejectsEncodedAuthorityWithDomainSuffix() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 9, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        let i105 = try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        let authority = "\(i105)@banka.dataspace"
        let targetAccount = try canonicalOwnerLiteral()
        let value = try NoritoJSON(["profile": "demo"])
        let request = SetMetadataRequest(chainId: "chain",
                                         authority: authority,
                                         target: .account(targetAccount),
                                         key: "profile",
                                         value: value,
                                         feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                         ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 1, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                          signingKey: signingKey,
                                                          creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: authority))
        }
    }

    func testPersistCouncilRejectsInvalidMemberAccount() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 2, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let request = PersistCouncilRequest(chainId: "chain",
                                            authority: authority,
                                            epoch: 1,
                                            members: ["bob"],
                                            candidatesCount: 1,
                                            derivedBy: .vrf,
                                            feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                            ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                             signingKey: signingKey,
                                                             creationTimeMs: 10)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "members[0]", value: "bob"))
        }
    }

    func testRemoveMetadataRejectsMalformedAssetTarget() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 3, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let malformed = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(try canonicalOwnerLiteral())"
        let request = RemoveMetadataRequest(chainId: "chain",
                                            authority: authority,
                                            target: .asset(malformed),
                                            key: "profile",
                                            feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                            ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeRemoveMetadata(request: request,
                                                             signingKey: signingKey,
                                                             creationTimeMs: 5)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId(malformed))
        }
    }


    func testNativeAssetEntryPointsRejectNoncanonicalQuantitiesBeforeDispatch() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 15, count: 32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

        for quantity in ["-1", "01", "1.0", "1.2300", " 1", "1e0"] {
            XCTAssertThrowsError(
                try NoritoNativeBridge.shared.encodeTransfer(
                    chainId: "chain",
                    authority: authority,
                    creationTimeMs: 1,
                    ttlMs: nil,
                    assetDefinitionId: assetDefinitionId,
                    quantity: quantity,
                    destination: authority,
                    feePaymentJSON: try FeePaymentIntent.authority(
                        chargeLimits: [],
                        gasLimit: nil
                    ).canonicalJSONData(),
                    privateKey: keypair.privateKeyBytes
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
            XCTAssertThrowsError(
                try NoritoNativeBridge.shared.encodeMint(
                    chainId: "chain",
                    authority: authority,
                    creationTimeMs: 1,
                    ttlMs: nil,
                    assetDefinitionId: assetDefinitionId,
                    quantity: quantity,
                    destination: authority,
                    feePaymentJSON: try FeePaymentIntent.authority(
                        chargeLimits: [],
                        gasLimit: nil
                    ).canonicalJSONData(),
                    privateKey: keypair.privateKeyBytes
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
            XCTAssertThrowsError(
                try NoritoNativeBridge.shared.encodeBurn(
                    chainId: "chain",
                    authority: authority,
                    creationTimeMs: 1,
                    ttlMs: nil,
                    assetDefinitionId: assetDefinitionId,
                    quantity: quantity,
                    destination: authority,
                    feePaymentJSON: try FeePaymentIntent.authority(
                        chargeLimits: [],
                        gasLimit: nil
                    ).canonicalJSONData(),
                    privateKey: keypair.privateKeyBytes
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
            XCTAssertThrowsError(
                try NoritoNativeBridge.shared.encodeTransferInstructionBox(
                    authority: authority,
                    assetDefinitionId: assetDefinitionId,
                    quantity: quantity,
                    destination: authority
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
        }
    }

    func testSetMetadataRejectsMalformedRwaTarget() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 7, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let value = try NoritoJSON(["serial": "vault-1"])
        let request = SetMetadataRequest(chainId: "chain",
                                         authority: authority,
                                         target: .rwa("lot-001"),
                                         key: "serial",
                                         value: value,
                                         feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                         ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                          signingKey: signingKey,
                                                          creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedRwaId(field: "target", value: "lot-001"))
        }
    }

    func testMetadataTargetAcceptsCanonicalAssetId() throws {
        let target = try TransactionInputValidator.sanitizeMetadataTarget(
            .asset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        )
        guard case let .asset(assetId) = target else {
            return XCTFail("expected asset target")
        }
        XCTAssertEqual(assetId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    func testCastZkBallotRejectsIncompleteLockHints() throws {
        let owner = try canonicalOwnerLiteral(
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(
            from: signingKey,
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let publicInputs = try NoritoJSON(["owner": owner])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("lock hints must include owner, amount, duration_blocks"))
        }
    }

    func testCastPlainBallotRejectsNoncanonicalQuantityBeforeNativeDispatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let owner = try canonicalOwnerLiteral()
        let overflowing = String(repeating: "9", count: 155)

        for amount in ["+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing] {
            let request = CastPlainBallotRequest(
                chainId: "chain",
                authority: authority,
                referendumId: "referendum-1",
                owner: owner,
                amount: amount,
                durationBlocks: 1,
                direction: .aye,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
                ttlMs: nil
            )

            XCTAssertThrowsError(
                try SwiftTransactionEncoder.encodeCastPlainBallot(
                    request: request,
                    signingKey: signingKey,
                    creationTimeMs: 1
                )
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error, "\(amount): \(error)")
            }
        }
    }

    func testCastZkBallotRejectsInvalidRootHintHex() throws {
        let owner = try canonicalOwnerLiteral(
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(
            from: signingKey,
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
            "root_hint": "not-hex",
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("root_hint must be 32-byte hex"))
        }
    }

    func testCastZkBallotRejectsNoncanonicalQuantityBeforeNativeDispatch() throws {
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let overflowing = String(repeating: "9", count: 155)
        let invalidAmounts: [Any] = [
            1, "+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing,
        ]

        for amount in invalidAmounts {
            let publicInputs = try NoritoJSON.fromJSONObject([
                "owner": owner,
                "amount": amount,
                "duration_blocks": 1,
            ])
            let request = CastZkBallotRequest(
                chainId: "chain",
                authority: authority,
                electionId: "election-1",
                proofB64: "AAAA",
                publicInputs: publicInputs,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
                ttlMs: nil
            )

            XCTAssertThrowsError(
                try SwiftTransactionEncoder.encodeCastZkBallot(
                    request: request,
                    signingKey: signingKey,
                    creationTimeMs: 1
                )
            ) { error in
                XCTAssertEqual(
                    error as? TransactionInputError,
                    .invalidZkBallotPublicInputs(
                        "amount must be a canonical non-negative Kotodama V1 Quantity string"
                    )
                )
            }
        }
    }

    func testCastZkBallotRejectsDeprecatedAliases() throws {
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "18446744073709551616.25",
            "duration_blocks": 1,
            "root_hint_hex": "0x" + String(repeating: "Cc", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        )
    }

    func testCastZkBallotAcceptsCanonicalHints() throws {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge not available")
        }
        let owner = try canonicalOwnerLiteral(
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(
            from: signingKey,
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "18446744073709551616.25",
            "duration_blocks": 1,
            "root_hint": "0x" + String(repeating: "Cc", count: 32),
            "nullifier": "blake2b32:" + String(repeating: "DD", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        do {
            _ = try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                               signingKey: signingKey,
                                                               creationTimeMs: 1)
        } catch SwiftTransactionEncoderError.nativeBridgeError(.governance) {
            throw XCTSkip("governance encoder unavailable in linked native bridge")
        } catch SwiftTransactionEncoderError.nativeBridgeError(.authority) {
            throw XCTSkip("authority encoder unavailable in linked native bridge")
        }
    }

    func testCastZkBallotRejectsNoncanonicalOwner() throws {
        let owner = try noncanonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("owner must be a canonical I105 account id"))
        }
    }
}
