import Foundation
import XCTest
@testable import IrohaSwift

final class NexusAppClientTests: XCTestCase {
    private static let accountChainDiscriminant: UInt16 = 753
    private static let assetDefinitionID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
    private static let publicKey = Data(hexString: "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737")!
    private static let walletSignature = Data(hexString: "d39065822f28108f70f8089f64357cc33a0072e45aa65f6b3e2696b93a3d9779d376ddf19c8e7dabce79a484275b681dea5213df060848d8fe098edeebcc3c07")!
    private static let accountID = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
    private static let destinationAccountID = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L"

    func testTransferWithWalletBuildsSignsSubmitsAndWaits() async throws {
        let connect = FakeConnect()
        let torii = FakeToriiSubmitter()
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   appId: "sample-app",
                                   signingPublicKey: Self.publicKey),
            connectTransport: connect,
            toriiSubmitter: torii
        )

        let session = try await client.startConnect(
            options: NexusConnectOptions(walletURIBase: URL(string: "sora://wallet/connect")!)
        )
        let approved = try await client.awaitApproval(session: session)
        let approvedSession = try XCTUnwrap(approved.session)
        let receipt = try await client.transferWithWallet(
            session: approvedSession,
            input: sampleInput(),
            options: NexusFinalizeOptions(waitForFinalStatus: true)
        )

        XCTAssertEqual(approved.accountID, Self.accountID)
        XCTAssertEqual(receipt.finalStatus, "Applied")
        XCTAssertEqual(receipt.transactionHashHex, torii.submittedHash)
        XCTAssertEqual(receipt.transactionHashHex, receipt.signedTransaction.hashHex)
        XCTAssertEqual(receipt.signedTransaction.payload, connect.lastSignable?.payloadBytes)
        XCTAssertEqual(receipt.signedTransaction.signedTransaction.isEmpty, false)
    }

    func testBuildTransferDraftFailsClosedWithoutSigningPublicKey() throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   authority: Self.accountID)
        )

        let error = try expectNexusError {
            _ = try client.buildTransferDraft(input: sampleInput())
        }

        XCTAssertEqual(error.code, "missing_signing_public_key")
    }

    func testBuildTransferDraftUsesSharedFixturePayload() throws {
        let fixture = try Self.sharedNexusFixture()
        let client = NexusAppClient(
            config: NexusAppConfig(
                chainId: fixture.chainID,
                accountChainDiscriminant: fixture.accountChainDiscriminant,
                authority: fixture.authority,
                signingPublicKey: fixture.signingPublicKey
            )
        )

        let draft = try client.buildTransferDraft(input: fixture.input)

        XCTAssertEqual(draft.signable.payloadHashHex, fixture.payloadHashHex)
        XCTAssertEqual(draft.signable.payloadBytes, fixture.payloadBytes)
    }

    func testNexusFacadeRejectsWrongChainTransferAndApprovalAccounts() async throws {
        let fixture = try Self.sharedNexusFixture()
        let destination = try AccountAddress.parseEncoded(
            fixture.input.destinationAccountID,
            expectedPrefix: fixture.accountChainDiscriminant
        )
        let wrongChainDestination = try destination.toI105(
            chainDiscriminant: fixture.accountChainDiscriminant + 1
        )
        let client = NexusAppClient(
            config: NexusAppConfig(
                chainId: fixture.chainID,
                accountChainDiscriminant: fixture.accountChainDiscriminant,
                authority: fixture.authority,
                signingPublicKey: fixture.signingPublicKey
            )
        )
        let wrongChainInput = NexusTransferInput(
            sourceAssetID: fixture.input.sourceAssetID,
            quantity: fixture.input.quantity,
            destinationAccountID: wrongChainDestination,
            feePayment: fixture.input.feePayment,
            authority: fixture.input.authority,
            signingPublicKey: fixture.input.signingPublicKey,
            creationTimeMs: fixture.input.creationTimeMs,
            ttlMs: fixture.input.ttlMs,
            nonce: fixture.input.nonce,
            metadata: fixture.input.metadata
        )

        let transferError = try expectNexusError {
            _ = try client.buildTransferDraft(input: wrongChainInput)
        }
        XCTAssertEqual(transferError.code, "invalid_account_id")

        let invalidScopeInput = NexusTransferInput(
            sourceAssetID: "\(fixture.input.sourceAssetID)#dataspace:01",
            quantity: fixture.input.quantity,
            destinationAccountID: fixture.input.destinationAccountID,
            feePayment: fixture.input.feePayment
        )
        let scopeError = try expectNexusError {
            _ = try client.buildTransferDraft(input: invalidScopeInput)
        }
        XCTAssertEqual(scopeError.code, "invalid_account_id")

        let authority = try AccountAddress.parseEncoded(
            fixture.authority,
            expectedPrefix: fixture.accountChainDiscriminant
        )
        let wrongChainAuthority = try authority.toI105(
            chainDiscriminant: fixture.accountChainDiscriminant + 1
        )
        let approvalClient = NexusAppClient(
            config: NexusAppConfig(
                chainId: fixture.chainID,
                accountChainDiscriminant: fixture.accountChainDiscriminant
            ),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(
                    accountID: wrongChainAuthority,
                    signingPublicKey: fixture.signingPublicKey
                )
            )
        )
        let approvalError = await expectNexusErrorAsync {
            _ = try await approvalClient.awaitApproval(
                session: NexusConnectSession(
                    sessionID: "wrong-chain",
                    walletLaunchURI: URL(string: "sora://wallet/connect")!
                )
            )
        }
        XCTAssertEqual(approvalError.code, "invalid_account_id")
    }

    func testBuildTransferDraftRejectsInvalidQuantityBeforeCustomCodec() throws {
        let client = NexusAppClient(
            config: NexusAppConfig(
                chainId: "test-chain",
                accountChainDiscriminant: Self.accountChainDiscriminant,
                authority: Self.accountID,
                signingPublicKey: Self.publicKey
            ),
            transactionCodec: PermissiveNexusCodec()
        )

        for quantity in ["-1", "+1", "01", "1.0", " 1", "1e0"] {
            let input = NexusTransferInput(
                sourceAssetID: "\(Self.assetDefinitionID)#\(Self.accountID)",
                quantity: quantity,
                destinationAccountID: Self.destinationAccountID,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            )
            XCTAssertThrowsError(try client.buildTransferDraft(input: input), quantity) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
        }
    }

    func testFinalizeAndSubmitUsesSharedFixtureSignedTransactionHash() async throws {
        let fixture = try Self.sharedNexusFixture()
        let torii = FakeToriiSubmitter()
        let client = NexusAppClient(
            config: NexusAppConfig(
                chainId: fixture.chainID,
                accountChainDiscriminant: fixture.accountChainDiscriminant,
                authority: fixture.authority,
                signingPublicKey: fixture.signingPublicKey
            ),
            toriiSubmitter: torii
        )
        let draft = try client.buildTransferDraft(input: fixture.input)

        let receipt = try await client.finalizeAndSubmit(
            signable: draft.signable,
            signature: NexusWalletSignature(signature: fixture.walletSignature),
            options: NexusFinalizeOptions(waitForFinalStatus: false)
        )

        XCTAssertEqual(receipt.transactionHashHex, fixture.signedTransactionHashHex)
        XCTAssertEqual(receipt.signedTransaction.hashHex, fixture.signedTransactionHashHex)
        XCTAssertEqual(torii.submittedHash, fixture.signedTransactionHashHex)
    }

    func testFinalizeAndSubmitAcceptsExactZeroSignatureAlgorithmAlias() async throws {
        let fixture = try Self.sharedNexusFixture()
        let torii = FakeToriiSubmitter()
        let client = NexusAppClient(
            config: NexusAppConfig(
                chainId: fixture.chainID,
                accountChainDiscriminant: fixture.accountChainDiscriminant,
                authority: fixture.authority,
                signingPublicKey: fixture.signingPublicKey
            ),
            toriiSubmitter: torii
        )
        let draft = try client.buildTransferDraft(input: fixture.input)
        let signable = NexusSignableTransaction(payloadBytes: draft.signable.payloadBytes,
                                                payloadHashHex: draft.signable.payloadHashHex,
                                                authority: draft.signable.authority,
                                                signingPublicKey: draft.signable.signingPublicKey,
                                                signatureAlgorithm: "0")

        let receipt = try await client.finalizeAndSubmit(
            signable: signable,
            signature: NexusWalletSignature(signature: fixture.walletSignature, algorithm: "0"),
            options: NexusFinalizeOptions(waitForFinalStatus: false)
        )

        XCTAssertEqual(receipt.transactionHashHex, fixture.signedTransactionHashHex)
        XCTAssertEqual(torii.submittedHash, fixture.signedTransactionHashHex)
    }

    func testFinalizeAndSubmitRejectsUnsupportedSignatureAlgorithm() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        for algorithm in [
            "",
            " ",
            "\t",
            "\n",
            "\u{00A0}",
            "ed25519 ",
            " ed25519",
            "\ted25519",
            "ed25519\n",
            "ed25519\u{00A0}",
            "0 ",
            " 0",
            "\t0",
            "00",
            "\u{FF10}",
            "secp256k1",
            "ed\t25519",
            "ed\u{0000}25519",
            "ed\u{001F}25519",
            "ed\u{007F}25519",
            "ed\u{200B}25519",
            "\u{0435}d25519",
            "ed\u{FF0D}25519",
            "ED25519",
            "Ed25519",
            " ED25519 ",
        ] {
            let error = await expectNexusErrorAsync {
                _ = try await client.finalizeAndSubmit(
                    signable: draft.signable,
                    signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 64),
                                                     algorithm: algorithm)
                )
            }

            XCTAssertEqual(error.code, "unsupported_signature_algorithm", String(reflecting: algorithm))
        }

        for algorithm in [
            "",
            " ",
            "ed25519 ",
            " ed25519",
            "0 ",
            " 0",
            "00",
            "ED25519",
            "ed\u{0000}25519",
            "ed\u{200B}25519",
            "\u{0435}d25519",
        ] {
            let signable = NexusSignableTransaction(payloadBytes: draft.signable.payloadBytes,
                                                    payloadHashHex: draft.signable.payloadHashHex,
                                                    authority: draft.signable.authority,
                                                    signingPublicKey: draft.signable.signingPublicKey,
                                                    signatureAlgorithm: algorithm)
            let signableError = await expectNexusErrorAsync {
                _ = try await client.finalizeAndSubmit(
                    signable: signable,
                    signature: NexusWalletSignature(signature: Self.walletSignature)
                )
            }
            XCTAssertEqual(signableError.code, "unsupported_signature_algorithm", String(reflecting: algorithm))
        }
    }

    func testRequestSignatureRejectsUnsupportedSignatureAlgorithmsAtTransportBoundary() async throws {
        let session = NexusConnectSession(sessionID: "session-1",
                                          walletLaunchURI: URL(string: "sora://wallet/connect")!,
                                          approvedAccount: Self.accountID,
                                          signingPublicKey: Self.publicKey)
        let signable = NexusSignableTransaction(payloadBytes: Data([0x01, 0x02, 0x03]),
                                                payloadHashHex: String(repeating: "0", count: 64),
                                                authority: Self.accountID,
                                                signingPublicKey: Self.publicKey)

        for algorithm in ["", "ed25519 ", " 0", "ED25519", "ed\u{200B}25519"] {
            let connect = SignatureConnect(signature: Self.walletSignature)
            let client = NexusAppClient(config: NexusAppConfig(
                chainId: "test-chain",
                accountChainDiscriminant: Self.accountChainDiscriminant
            ),
                                        connectTransport: connect)
            let badSignable = NexusSignableTransaction(payloadBytes: signable.payloadBytes,
                                                       payloadHashHex: signable.payloadHashHex,
                                                       authority: signable.authority,
                                                       signingPublicKey: signable.signingPublicKey,
                                                       signatureAlgorithm: algorithm)

            let error = await expectNexusErrorAsync {
                _ = try await client.requestSignature(session: session, signable: badSignable)
            }

            XCTAssertEqual(error.code, "unsupported_signature_algorithm", String(reflecting: algorithm))
            XCTAssertNil(connect.lastSignable)
        }

        for algorithm in ["ed25519 ", " 0", "\u{FF10}", "ed\u{0000}25519", "\u{0435}d25519"] {
            let connect = SignatureConnect(
                signature: Self.walletSignature,
                algorithm: algorithm
            )
            let client = NexusAppClient(config: NexusAppConfig(
                chainId: "test-chain",
                accountChainDiscriminant: Self.accountChainDiscriminant
            ),
                                        connectTransport: connect)

            let error = await expectNexusErrorAsync {
                _ = try await client.requestSignature(session: session, signable: signable)
            }

            XCTAssertEqual(error.code, "unsupported_signature_algorithm", String(reflecting: algorithm))
            XCTAssertNotNil(connect.lastSignable)
        }
    }

    func testAwaitApprovalRejectsMissingAccountAndSigningKey() async throws {
        let missingAccount = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: "")
            )
        )
        let accountError = await expectNexusErrorAsync {
            _ = try await missingAccount.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(accountError.code, "approval_missing_account")

        let missingKey = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: Self.accountID)
            )
        )
        let keyError = await expectNexusErrorAsync {
            _ = try await missingKey.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(keyError.code, "missing_signing_public_key")

        let invalidKey = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: Self.accountID,
                                               signingPublicKey: Data(repeating: 0x01, count: 31))
            )
        )
        let invalidKeyError = await expectNexusErrorAsync {
            _ = try await invalidKey.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(invalidKeyError.code, "invalid_signing_public_key")
    }

    func testAwaitApprovalRejectsFixtureKeyAndSessionSubstitution() async throws {
        let fixture = try Self.loadNexusFixture()
        let connectFixture = try XCTUnwrap(fixture["connect"] as? [String: Any])
        let transferFixture = try XCTUnwrap(fixture["transfer_input"] as? [String: Any])
        let discriminantNumber = try XCTUnwrap(
            transferFixture["account_chain_discriminant"] as? NSNumber
        )
        let chainDiscriminant = UInt16(discriminantNumber.uint64Value)
        let callerSessionID = try XCTUnwrap(connectFixture["sid"] as? String)
        let callerWalletLaunchURIText = try XCTUnwrap(
            connectFixture["wallet_launch_uri"] as? String
        )
        let callerWalletLaunchURI = try XCTUnwrap(URL(string: callerWalletLaunchURIText))
        let callerSession = NexusConnectSession(
            sessionID: callerSessionID,
            walletLaunchURI: callerWalletLaunchURI
        )

        let keyCase = try Self.errorCase(
            fixture,
            named: "approval signing key mismatch"
        )
        let keyApproval = try XCTUnwrap(keyCase["approval_frame"] as? [String: Any])
        let keyClient = NexusAppClient(
            config: NexusAppConfig(
                chainId: try XCTUnwrap(connectFixture["chain_id"] as? String),
                accountChainDiscriminant: chainDiscriminant
            ),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(
                    accountID: try XCTUnwrap(keyApproval["account_id"] as? String),
                    signingPublicKey: try Self.hexData(
                        try XCTUnwrap(keyApproval["signing_public_key_hex"] as? String)
                    )
                )
            )
        )
        let keyError = await expectNexusErrorAsync {
            _ = try await keyClient.awaitApproval(session: callerSession)
        }
        XCTAssertEqual(keyError.code, try XCTUnwrap(keyCase["expected_code"] as? String))

        let sessionCase = try Self.errorCase(
            fixture,
            named: "approval session substitution"
        )
        let sessionApproval = try XCTUnwrap(
            sessionCase["approval_frame"] as? [String: Any]
        )
        let substituted = try XCTUnwrap(sessionApproval["session"] as? [String: Any])
        let sessionClient = NexusAppClient(
            config: NexusAppConfig(
                chainId: try XCTUnwrap(connectFixture["chain_id"] as? String),
                accountChainDiscriminant: chainDiscriminant
            ),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(
                    accountID: try XCTUnwrap(sessionApproval["account_id"] as? String),
                    signingPublicKey: try Self.hexData(
                        try XCTUnwrap(sessionApproval["signing_public_key_hex"] as? String)
                    ),
                    session: NexusConnectSession(
                        sessionID: try XCTUnwrap(substituted["sid"] as? String),
                        walletLaunchURI: try XCTUnwrap(
                            URL(
                                string: try XCTUnwrap(
                                    substituted["wallet_launch_uri"] as? String
                                )
                            )
                        )
                    )
                )
            )
        )
        let sessionError = await expectNexusErrorAsync {
            _ = try await sessionClient.awaitApproval(session: callerSession)
        }
        XCTAssertEqual(
            sessionError.code,
            try XCTUnwrap(sessionCase["expected_code"] as? String)
        )
        XCTAssertEqual(callerSession.sessionID, callerSessionID)
        XCTAssertEqual(callerSession.walletLaunchURI.absoluteString, callerWalletLaunchURIText)
    }

    func testTransferWithWalletRejectsAuthorityMismatchBeforeSigning() async throws {
        let connect = FakeConnect()
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   signingPublicKey: Self.publicKey),
            connectTransport: connect,
            toriiSubmitter: FakeToriiSubmitter()
        )
        let session = NexusConnectSession(sessionID: "session-1",
                                          walletLaunchURI: URL(string: "sora://wallet/connect")!,
                                          approvedAccount: Self.accountID,
                                          signingPublicKey: Self.publicKey)

        let error = await expectNexusErrorAsync {
            _ = try await client.transferWithWallet(
                session: session,
                input: NexusTransferInput(sourceAssetID: "\(Self.assetDefinitionID)#\(Self.destinationAccountID)",
                                          quantity: "12.34",
                                          destinationAccountID: Self.destinationAccountID,
                                          feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                          authority: Self.destinationAccountID,
                                          signingPublicKey: Self.publicKey)
            )
        }

        XCTAssertEqual(error.code, "approval_account_mismatch")
        XCTAssertNil(connect.lastSignable)
    }

    func testFinalizeAndSubmitRejectsInvalidSignatureLength() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let error = await expectNexusErrorAsync {
            _ = try await client.finalizeAndSubmit(
                signable: draft.signable,
                signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 63))
            )
        }

        XCTAssertEqual(error.code, "invalid_signature")
    }

    func testFinalizeAndSubmitRejectsInvalidSignatureBytes() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let error = await expectNexusErrorAsync {
            _ = try await client.finalizeAndSubmit(
                signable: draft.signable,
                signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 64))
            )
        }

        XCTAssertEqual(error.code, "invalid_signature")
    }

    func testFinalizeAndSubmitRejectsHashMismatchAndMapsSubmitStatusFailures() async throws {
        let draftClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant,
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try draftClient.buildTransferDraft(input: sampleInput())
        let signature = NexusWalletSignature(signature: Self.walletSignature)

        let mismatchClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            toriiSubmitter: FakeToriiSubmitter(responseHash: String(repeating: "f", count: 64))
        )
        let mismatchError = await expectNexusErrorAsync {
            _ = try await mismatchClient.finalizeAndSubmit(signable: draft.signable,
                                                           signature: signature)
        }
        XCTAssertEqual(mismatchError.code, "transaction_hash_mismatch")

        let submitFailureClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            toriiSubmitter: FakeToriiSubmitter(submitError: FakeToriiError.down)
        )
        let submitError = await expectNexusErrorAsync {
            _ = try await submitFailureClient.finalizeAndSubmit(signable: draft.signable,
                                                                signature: signature)
        }
        XCTAssertEqual(submitError.code, "submit_failed")

        let statusFailureClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            toriiSubmitter: FakeToriiSubmitter(statusError: FakeToriiError.timeout)
        )
        let statusError = await expectNexusErrorAsync {
            _ = try await statusFailureClient.finalizeAndSubmit(signable: draft.signable,
                                                                signature: signature)
        }
        XCTAssertEqual(statusError.code, "status_wait_failed")

        let committedStatusClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   accountChainDiscriminant: Self.accountChainDiscriminant),
            toriiSubmitter: FakeToriiSubmitter(status: "Committed")
        )
        let committedStatusError = await expectNexusErrorAsync {
            _ = try await committedStatusClient.finalizeAndSubmit(signable: draft.signable,
                                                                  signature: signature)
        }
        XCTAssertEqual(committedStatusError.code, "status_wait_non_applied")
    }

    func testSwiftTransferCodecRejectsNoncanonicalQuantitiesBeforeEncoding() throws {
        let codec = SwiftNexusTransactionCodec()
        let config = NexusAppConfig(chainId: "test-chain",
                                    accountChainDiscriminant: Self.accountChainDiscriminant)
        for quantity in ["-1", "01", "1.0", "1.2300", " 1", "1e0"] {
            let input = NexusTransferInput(
                sourceAssetID: "\(Self.assetDefinitionID)#\(Self.accountID)",
                quantity: quantity,
                destinationAccountID: Self.destinationAccountID,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
            )
            XCTAssertThrowsError(
                try codec.buildTransferPayload(
                    input: input,
                    config: config,
                    authority: Self.accountID
                ),
                "accepted quantity \(quantity)"
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
            XCTAssertThrowsError(
                try codec.buildTransferInstructionBox(
                    input: input,
                    accountChainDiscriminant: Self.accountChainDiscriminant
                ),
                "accepted instruction quantity \(quantity)"
            ) { error in
                XCTAssertTrue(error is KotodamaNumericV1Error)
            }
        }
    }

    private struct SharedNexusFixture {
        let chainID: String
        let accountChainDiscriminant: UInt16
        let authority: String
        let signingPublicKey: Data
        let input: NexusTransferInput
        let payloadBytes: Data
        let payloadHashHex: String
        let walletSignature: Data
        let signedTransactionHashHex: String
    }

    private static func sharedNexusFixture() throws -> SharedNexusFixture {
        let fixture = try loadNexusFixture()
        let connect = try XCTUnwrap(fixture["connect"] as? [String: Any])
        let approval = try XCTUnwrap(connect["approval_frame"] as? [String: Any])
        let transfer = try XCTUnwrap(fixture["transfer_input"] as? [String: Any])
        let expected = try XCTUnwrap(fixture["expected"] as? [String: Any])
        let discriminantNumber = try XCTUnwrap(
            transfer["account_chain_discriminant"] as? NSNumber
        )
        guard discriminantNumber.uint64Value <= UInt16.max else {
            throw NexusAppError(code: "invalid_fixture",
                                message: "Fixture account chain discriminant must fit in UInt16.")
        }
        let discriminant = UInt16(discriminantNumber.uint64Value)
        let authority = try XCTUnwrap(transfer["authority"] as? String)
        let destination = try XCTUnwrap(transfer["destination_account_id"] as? String)
        let sourceAssetID = try XCTUnwrap(transfer["source_asset_id"] as? String)
        let sourceParts = sourceAssetID.split(separator: "#", omittingEmptySubsequences: false)
        guard (2...3).contains(sourceParts.count) else {
            throw NexusAppError(code: "invalid_fixture",
                                message: "Fixture source asset must contain one owner account.")
        }
        for account in [authority, destination, String(sourceParts[1])] {
            let address = try AccountAddress.parseEncoded(account,
                                                          expectedPrefix: discriminant)
            guard try address.toI105(chainDiscriminant: discriminant) == account else {
                throw NexusAppError(code: "invalid_fixture",
                                    message: "Fixture account is not canonical for its declared chain.")
            }
        }
        let metadata = try XCTUnwrap(transfer["metadata"] as? [String: String])
        let input = NexusTransferInput(
            sourceAssetID: sourceAssetID,
            quantity: try XCTUnwrap(transfer["quantity"] as? String),
            destinationAccountID: destination,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            creationTimeMs: try XCTUnwrap(transfer["creation_time_ms"] as? NSNumber).uint64Value,
            ttlMs: try XCTUnwrap(transfer["ttl_ms"] as? NSNumber).uint64Value,
            nonce: UInt32(try XCTUnwrap(transfer["nonce"] as? NSNumber).uint64Value),
            metadata: metadata
        )
        return SharedNexusFixture(
            chainID: try XCTUnwrap(connect["chain_id"] as? String),
            accountChainDiscriminant: discriminant,
            authority: authority,
            signingPublicKey: try hexData(
                try XCTUnwrap(approval["signing_public_key_hex"] as? String)
            ),
            input: input,
            payloadBytes: try hexData(
                try XCTUnwrap(expected["payload_bytes_hex"] as? String)
            ),
            payloadHashHex: try XCTUnwrap(expected["payload_hash_hex"] as? String),
            walletSignature: try hexData(
                try XCTUnwrap(expected["wallet_signature_hex"] as? String)
            ),
            signedTransactionHashHex: try XCTUnwrap(
                expected["signed_transaction_hash_hex"] as? String
            )
        )
    }

    private static func sampleInput() -> NexusTransferInput {
        NexusTransferInput(
            sourceAssetID: "\(assetDefinitionID)#\(accountID)",
            quantity: "12.34",
            destinationAccountID: destinationAccountID,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            creationTimeMs: 1_700_000_000_000,
            ttlMs: 30_000,
            nonce: 7,
            metadata: ["purpose": "nexus-app-fixture"]
        )
    }

    private func sampleInput() -> NexusTransferInput {
        Self.sampleInput()
    }

    private static func loadNexusFixture() throws -> [String: Any] {
        var url = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        while true {
            let candidate = url.appendingPathComponent("fixtures/sdk/nexus_connect_transfer_v1.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                let json = try JSONSerialization.jsonObject(with: data)
                guard let object = json as? [String: Any] else {
                    throw NexusAppError(code: "invalid_fixture", message: "fixture root must be an object")
                }
                return object
            }
            let parent = url.deletingLastPathComponent()
            if parent.path == url.path { break }
            url = parent
        }
        throw NexusAppError(code: "missing_fixture", message: "Nexus fixture was not found")
    }

    private static func errorCase(
        _ fixture: [String: Any],
        named name: String
    ) throws -> [String: Any] {
        let cases = try XCTUnwrap(fixture["error_cases"] as? [[String: Any]])
        return try XCTUnwrap(cases.first { ($0["name"] as? String) == name })
    }

    private static func hexData(_ hex: String) throws -> Data {
        guard hex.count % 2 == 0 else {
            throw NexusAppError(code: "invalid_hex", message: "hex length must be even")
        }
        var data = Data()
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else {
                throw NexusAppError(code: "invalid_hex", message: "invalid hex byte")
            }
            data.append(byte)
            index = next
        }
        return data
    }

    private func expectNexusError(_ block: () throws -> Void) throws -> NexusAppError {
        do {
            try block()
        } catch let error as NexusAppError {
            return error
        }
        XCTFail("Expected NexusAppError")
        return NexusAppError(code: "missing_test_error", message: "Expected NexusAppError")
    }

    private func expectNexusErrorAsync(_ block: () async throws -> Void) async -> NexusAppError {
        do {
            try await block()
        } catch let error as NexusAppError {
            return error
        } catch {
            XCTFail("Expected NexusAppError, got \(error)")
            return NexusAppError(code: "wrong_test_error", message: "Expected NexusAppError")
        }
        XCTFail("Expected NexusAppError")
        return NexusAppError(code: "missing_test_error", message: "Expected NexusAppError")
    }

    private final class FakeConnect: NexusConnectTransport {
        private(set) var lastSignable: NexusSignableTransaction?

        func startConnect(options: NexusConnectOptions,
                          config: NexusAppConfig) async throws -> NexusConnectSession {
            let base = options.walletURIBase ?? URL(string: "sora://wallet/connect")!
            let sessionID = options.sessionID ?? "session-1"
            return NexusConnectSession(sessionID: sessionID,
                                       walletLaunchURI: URL(string: "\(base.absoluteString)?session=\(sessionID)")!,
                                       appId: config.appId,
                                       relayURL: config.relayURL,
                                       node: options.node ?? config.node)
        }

        func awaitApproval(session: NexusConnectSession,
                           config: NexusAppConfig) async throws -> NexusApprovedAccount {
            NexusApprovedAccount(accountID: NexusAppClientTests.accountID,
                                 signingPublicKey: NexusAppClientTests.publicKey)
        }

        func requestSignature(session: NexusConnectSession,
                              signable: NexusSignableTransaction,
                              config: NexusAppConfig) async throws -> NexusWalletSignature {
            lastSignable = signable
            XCTAssertEqual(signable.signatureAlgorithm, NexusSignatureAlgorithmEd25519)
            return NexusWalletSignature(signature: NexusAppClientTests.walletSignature)
        }
    }

    private final class SignatureConnect: NexusConnectTransport {
        private(set) var lastSignable: NexusSignableTransaction?
        private let signature: Data
        private let algorithm: String

        init(signature: Data, algorithm: String = NexusSignatureAlgorithmEd25519) {
            self.signature = signature
            self.algorithm = algorithm
        }

        func startConnect(options: NexusConnectOptions,
                          config: NexusAppConfig) async throws -> NexusConnectSession {
            NexusConnectSession(sessionID: "session-1",
                                walletLaunchURI: URL(string: "sora://wallet/connect")!)
        }

        func awaitApproval(session: NexusConnectSession,
                           config: NexusAppConfig) async throws -> NexusApprovedAccount {
            NexusApprovedAccount(accountID: NexusAppClientTests.accountID,
                                 signingPublicKey: NexusAppClientTests.publicKey)
        }

        func requestSignature(session: NexusConnectSession,
                              signable: NexusSignableTransaction,
                              config: NexusAppConfig) async throws -> NexusWalletSignature {
            lastSignable = signable
            return NexusWalletSignature(signature: signature, algorithm: algorithm)
        }
    }

    private final class ApprovalConnect: NexusConnectTransport {
        let approval: NexusApprovedAccount

        init(approval: NexusApprovedAccount) {
            self.approval = approval
        }

        func startConnect(options: NexusConnectOptions,
                          config: NexusAppConfig) async throws -> NexusConnectSession {
            NexusConnectSession(sessionID: "session-1",
                                walletLaunchURI: URL(string: "sora://wallet/connect")!)
        }

        func awaitApproval(session: NexusConnectSession,
                           config: NexusAppConfig) async throws -> NexusApprovedAccount {
            approval
        }

        func requestSignature(session: NexusConnectSession,
                              signable: NexusSignableTransaction,
                              config: NexusAppConfig) async throws -> NexusWalletSignature {
            throw NexusAppError(code: "unexpected_signature_request",
                                message: "signature request should not be called")
        }
    }

    private enum FakeToriiError: Error {
        case down
        case timeout
    }

    private final class FakeToriiSubmitter: NexusToriiSubmitting {
        private(set) var submittedHash: String?
        private let responseHash: String?
        private let submitError: Error?
        private let statusError: Error?
        private let status: String

        init(responseHash: String? = nil,
             submitError: Error? = nil,
             statusError: Error? = nil,
             status: String = "Applied") {
            self.responseHash = responseHash
            self.submitError = submitError
            self.statusError = statusError
            self.status = status
        }

        func submitNexusTransaction(_ envelope: SignedTransactionEnvelope) async throws -> ToriiSubmitTransactionResponse? {
            if let submitError {
                throw submitError
            }
            submittedHash = envelope.hashHex
            return ToriiSubmitTransactionResponse(
                payload: ToriiSubmitTransactionResponse.Payload(
                    txHash: responseHash ?? envelope.hashHex,
                    submittedAtMs: 1_700_000_000_500,
                    submittedAtHeight: 7,
                    signer: NexusAppClientTests.accountID
                ),
                signature: "receipt-signature"
            )
        }

        func waitForNexusTransactionStatus(hashHex: String,
                                           options: PipelineStatusPollOptions) async throws -> String {
            if let statusError {
                throw statusError
            }
            XCTAssertEqual(hashHex, submittedHash)
            return status
        }
    }
}

private struct PermissiveNexusCodec: NexusTransactionCodec {
    func buildTransferPayload(
        input _: NexusTransferInput,
        config _: NexusAppConfig,
        authority _: String
    ) throws -> Data {
        Data([1])
    }

    func finalizeSignedTransaction(
        signable: NexusSignableTransaction,
        signature: NexusWalletSignature
    ) throws -> SignedTransactionEnvelope {
        try SwiftNexusTransactionCodec().finalizeSignedTransaction(
            signable: signable,
            signature: signature
        )
    }
}
