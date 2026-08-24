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
    func testTransferEncoderPreservesTairaAccountIdentities() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native encoder not linked"
        )
        let signingKey = try SigningKey.ed25519(
            privateKey: Data(repeating: 0x31, count: 32)
        )
        let authority = try AccountAddress
            .fromAccount(publicKey: signingKey.publicKey())
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let destinationSigningKey = try SigningKey.ed25519(
            privateKey: Data(repeating: 0x32, count: 32)
        )
        let destination = try AccountAddress
            .fromAccount(publicKey: destinationSigningKey.publicKey())
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let transfer = TransferRequest(
            networkId: TestNetworkIds.canonical,
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
        let inspection = try NoritoNativeBridge.shared.inspectDetachedTransactionScaffold(
            envelope.norito
        )
        XCTAssertEqual(
            try AccountAddress.parseEncoded(inspection.authority).canonicalHex(),
            try AccountAddress.parseEncoded(authority).canonicalHex()
        )
        guard case let .assetTransfer(asset) = inspection.executable else {
            return XCTFail("expected one asset-transfer instruction")
        }
        XCTAssertEqual(
            try AccountAddress.parseEncoded(asset.destinationAccountId).canonicalHex(),
            try AccountAddress.parseEncoded(destination).canonicalHex()
        )
    }

    func testAssetBuildersRejectNoncanonicalAndNegativeQuantitiesBeforeNativeDispatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 13, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

        for quantity in ["1.0", "01", "-1", " 1", "1e0"] {
            let transfer = TransferRequest(
                networkId: TestNetworkIds.canonical,
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
                networkId: TestNetworkIds.canonical,
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
                networkId: TestNetworkIds.canonical,
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
        let request = SetMetadataRequest(networkId: TestNetworkIds.canonical,
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
        let request = SetMetadataRequest(networkId: TestNetworkIds.canonical,
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
        let request = PersistCouncilRequest(networkId: TestNetworkIds.canonical,
                                            authority: authority,
                                            epoch: 1,
                                            members: ["bob"],
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
        let request = RemoveMetadataRequest(networkId: TestNetworkIds.canonical,
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
                    networkId: TestNetworkIds.canonical,
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
                    networkId: TestNetworkIds.canonical,
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
                    networkId: TestNetworkIds.canonical,
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
        let request = SetMetadataRequest(networkId: TestNetworkIds.canonical,
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
        let request = CastZkBallotRequest(networkId: TestNetworkIds.canonical,
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: .init(owner: owner),
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            guard case let TransactionInputError.invalidZkBallotPublicInputs(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("owner, amount, and duration_blocks"))
        }
    }

    func testCastPlainBallotRejectsNoncanonicalQuantityBeforeNativeDispatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let owner = try canonicalOwnerLiteral()
        let overflowing = String(repeating: "9", count: 155)

        for amount in ["+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing] {
            let request = CastPlainBallotRequest(
                networkId: TestNetworkIds.canonical,
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

    func testGovernanceTransactionEncodersRejectNoncanonicalSelectorsBeforeNativeDispatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let owner = try canonicalOwnerLiteral()
        let feePayment = FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)

        let plain = CastPlainBallotRequest(
            networkId: TestNetworkIds.canonical,
            authority: authority,
            referendumId: "invalid/referendum",
            owner: owner,
            amount: "1",
            durationBlocks: 1,
            direction: .aye,
            feePayment: feePayment,
            ttlMs: nil
        )
        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastPlainBallot(
                request: plain,
                signingKey: signingKey,
                creationTimeMs: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .invalidGovernanceSelector(
                    field: "referendum_id",
                    value: "invalid/referendum"
                )
            )
        }

        let zk = CastZkBallotRequest(
            networkId: TestNetworkIds.canonical,
            authority: authority,
            electionId: ".hidden",
            proofB64: "AAAA",
            feePayment: feePayment,
            ttlMs: nil
        )
        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(
                request: zk,
                signingKey: signingKey,
                creationTimeMs: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .invalidGovernanceSelector(field: "election_id", value: ".hidden")
            )
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
        let request = CastZkBallotRequest(networkId: TestNetworkIds.canonical,
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: .init(
                                            rootHint: "not-hex",
                                            owner: owner,
                                            amount: "1",
                                            durationBlocks: 1
                                          ),
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            guard case let TransactionInputError.invalidZkBallotPublicInputs(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("root_hint must be a 32-byte hex string"))
        }
    }

    func testCastZkBallotRejectsNoncanonicalQuantityBeforeNativeDispatch() throws {
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let overflowing = String(repeating: "9", count: 155)
        let invalidAmounts = [
            "+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing,
        ]

        for amount in invalidAmounts {
            let request = CastZkBallotRequest(
                networkId: TestNetworkIds.canonical,
                authority: authority,
                electionId: "election-1",
                proofB64: "AAAA",
                publicInputs: .init(owner: owner, amount: amount, durationBlocks: 1),
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
                guard case let TransactionInputError.invalidZkBallotPublicInputs(reason) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(
                    reason.contains("canonical non-negative Kotodama V1 Quantity"),
                    "\(amount): \(reason)"
                )
            }
        }
    }

    func testCastZkBallotPublicInputsEncodeOnlyCanonicalFieldsAndLosslessUInt64() throws {
        let owner = try canonicalOwnerLiteral()
        let data = try SwiftTransactionEncoder.normalizeZkBallotPublicInputs(
            .init(
                rootHint: "0x" + String(repeating: "Cc", count: 32),
                owner: owner,
                amount: "18446744073709551616.25",
                durationBlocks: UInt64.max,
                direction: .nay,
                nullifier: "blake2b32:" + String(repeating: "DD", count: 32)
            )
        )
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data) as? [String: Any]
        )
        XCTAssertEqual(
            Set(object.keys),
            Set(["root_hint", "owner", "amount", "duration_blocks", "direction", "nullifier"])
        )
        XCTAssertEqual(object["root_hint"] as? String, String(repeating: "cc", count: 32))
        XCTAssertEqual(object["owner"] as? String, owner)
        XCTAssertEqual(object["amount"] as? String, "18446744073709551616.25")
        XCTAssertEqual(object["direction"] as? String, "Nay")
        XCTAssertEqual(object["nullifier"] as? String, String(repeating: "dd", count: 32))
        XCTAssertTrue(
            String(decoding: data, as: UTF8.self)
                .contains("\"duration_blocks\":18446744073709551615")
        )
    }

    func testGovernanceSecretAliasGuardRejectsEveryAliasRecursively() throws {
        let aliases = [
            "private_key", "privateKey", "private_key_hex", "privateKeyHex",
            "private_key_bytes", "privateKeyBytes", "private_key_seed", "privateKeySeed",
            "private_key_multihash", "privateKeyMultihash", "private_key_algorithm",
            "privateKeyAlgorithm",
        ]
        for alias in aliases {
            let data = try JSONSerialization.data(withJSONObject: [
                "outer": [["safe": [alias: "secret"]]],
            ])
            XCTAssertThrowsError(
                try SwiftTransactionEncoder.rejectGovernancePrivateKeyAliases(inJSONData: data)
            ) { error in
                guard case let TransactionInputError.invalidZkBallotPublicInputs(reason) = error else {
                    return XCTFail("unexpected error for \(alias): \(error)")
                }
                XCTAssertTrue(reason.contains(alias))
                XCTAssertTrue(reason.contains("public_inputs.outer[0].safe"))
            }
        }
    }

    func testCastZkBallotAcceptsCanonicalHints() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge not available"
        )
        let owner = try canonicalOwnerLiteral(
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(
            from: signingKey,
            networkPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let request = CastZkBallotRequest(networkId: TestNetworkIds.canonical,
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: .init(
                                            rootHint: "0x" + String(repeating: "Cc", count: 32),
                                            owner: owner,
                                            amount: "18446744073709551616.25",
                                            durationBlocks: 1,
                                            direction: .aye,
                                            nullifier: "blake2b32:" + String(repeating: "DD", count: 32)
                                          ),
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        do {
            _ = try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                               signingKey: signingKey,
                                                               creationTimeMs: 1)
        } catch SwiftTransactionEncoderError.nativeBridgeError(.governance) {
            try failRequiredNativeTestCapability(
                "governance encoder unavailable in linked native bridge"
            )
        } catch SwiftTransactionEncoderError.nativeBridgeError(.authority) {
            try failRequiredNativeTestCapability(
                "authority encoder unavailable in linked native bridge"
            )
        }
    }

    func testCastZkBallotRejectsNoncanonicalOwner() throws {
        let owner = try noncanonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let request = CastZkBallotRequest(networkId: TestNetworkIds.canonical,
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: .init(
                                            owner: owner,
                                            amount: "1",
                                            durationBlocks: 1
                                          ),
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            guard case let TransactionInputError.invalidZkBallotPublicInputs(reason) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertTrue(reason.contains("owner must be a canonical I105 account id"))
        }
    }
}
