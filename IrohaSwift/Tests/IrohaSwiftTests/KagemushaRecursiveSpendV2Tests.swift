import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendTests: XCTestCase {
    func testSingleInputSplitConservesFractionalAtomicValue() throws {
        let input = try branch(seed: 0x10, amount: "625", path: path(bit: nil))
        let recipient = try note(seed: 0x30, amount: "210")
        let change = try note(seed: 0x40, amount: "415")
        let split = try KagemushaRecursiveSpendSplitIntent(
            chainID: input.inputNote.chainID,
            assetDefinitionID: input.inputNote.assetDefinitionID,
            inputs: [input],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
            recipientOutput: recipient,
            changeOutput: change,
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )

        XCTAssertEqual(split.inputs, [input])
        XCTAssertEqual(split.transferAmount.displayDecimal, "2.1")
        XCTAssertEqual(split.changeOutput?.amount.atomicUnits, "415")
    }

    func testSingleInputSplitRejectsMissingMismatchedChangeAndScale() throws {
        let input = try branch(seed: 0x10, amount: "625", path: path(bit: nil))
        let recipient = try note(seed: 0x30, amount: "210")

        func assertRejected(
            change: KagemushaSpendableNoteDescriptor?,
            assetScale: UInt32 = 2,
            field: String
        ) {
            XCTAssertThrowsError(try KagemushaRecursiveSpendSplitIntent(
                chainID: input.inputNote.chainID,
                assetDefinitionID: input.inputNote.assetDefinitionID,
                inputs: [input],
                topUpAnchorRefs: [try topUpAnchorRef()],
                assetScale: assetScale,
                lineageMode: .semantic,
                outputArtifactGeneration: "generation-v2-test",
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
                changeOutput: change,
                recipientRequestDigest: fixed32(0x51),
                operationID: fixed32(0x52)
            )) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField(field)
                )
            }
        }

        assertRejected(change: nil, field: "changeOutput")
        assertRejected(
            change: try note(seed: 0x40, amount: "414"),
            field: "changeOutput.amount"
        )
        assertRejected(
            change: try note(seed: 0x40, amount: "415"),
            assetScale: 9,
            field: "split.context"
        )
    }

    func testCanonicalTwoInputSiblingJoinConservesValue() throws {
        let left = try branch(seed: 0x10, amount: "210", path: path(bit: 0))
        let right = try branch(seed: 0x20, amount: "415", path: path(bit: 1))
        let output = try note(seed: 0x40, amount: "625")
        let split = try KagemushaRecursiveSpendSplitIntent(
            chainID: left.inputNote.chainID,
            assetDefinitionID: left.inputNote.assetDefinitionID,
            inputs: [left, right],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: "625", scale: 2),
            recipientOutput: output,
            changeOutput: nil,
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )

        XCTAssertEqual(split.inputs.count, 2)
        XCTAssertNil(split.changeOutput)
        XCTAssertFalse(left.branchClaims[0].path.conflicts(with: right.branchClaims[0].path))
        XCTAssertEqual(
            left.branchClaims[0].transitionTags[0],
            right.branchClaims[0].transitionTags[0]
        )
    }

    func testBranchClaimsAllowIndependentTopUpLineages() throws {
        let first = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA0)
        )
        let second = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA1)
        )

        XCTAssertNoThrow(try KagemushaRecursiveSpend.validateBranchClaims([first, second]))
    }

    func testBranchClaimsRejectWrongTagShapeAndNonCanonicalClaimOrder() throws {
        let depthOne = try path(bit: 0)
        for tags in [
            [Data](),
            [Data(repeating: 0x41, count: KagemushaRecursiveSpend.transitionTagBytes - 1)],
            [Data(repeating: 0, count: KagemushaRecursiveSpend.transitionTagBytes)],
        ] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendBranchClaim(
                path: depthOne,
                transitionTags: tags
            )) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField("branchClaim.transitionTags")
                )
            }
        }

        let first = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA0)
        )
        let second = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA1)
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpend.validateBranchClaims([second, first])
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("branchClaims.order")
            )
        }
    }

    func testTransitionTagMatchesRustSHA256_192Golden() throws {
        let binding = Data(repeating: 0x42, count: 32)
        let expected = try XCTUnwrap(
            Data(hexString: "e374b01fb0b930690428174bbe03fd67dedf1063197e9a36")
        )

        XCTAssertEqual(
            try KagemushaRecursiveSpend.transitionTag(for: binding),
            expected
        )
    }

    func testBranchClaimWireUsesOneContiguousExactDepthByteVector() throws {
        var pathBits = Data(repeating: 0, count: 8)
        pathBits[0] = 0x40
        let branchPath = try KagemushaRecursiveSpendBranchPath(
            lineageRoot: fixed32(0xA0),
            depth: 2,
            pathBits: pathBits
        )
        let firstTag = Data(repeating: 0x11, count: KagemushaRecursiveSpend.transitionTagBytes)
        let secondTag = Data(repeating: 0x22, count: KagemushaRecursiveSpend.transitionTagBytes)
        let claim = try KagemushaRecursiveSpendBranchClaim(
            path: branchPath,
            transitionTags: [firstTag, secondTag]
        )

        let encoded = try KagemushaRecursiveSpendCodecs.encodeBranchClaim(claim)
        XCTAssertEqual(
            try KagemushaRecursiveSpendCodecs.decodeBranchClaim(encoded),
            claim
        )

        var claimReader = OfflineNoritoReader(data: encoded)
        let encodedPath = try claimReader.readCompactField()
        let encodedTags = try claimReader.readCompactField()
        XCTAssertEqual(claimReader.remaining(), 0)
        var flattened = firstTag
        flattened.append(secondTag)
        var tagsReader = OfflineNoritoReader(data: encodedTags)
        XCTAssertEqual(try tagsReader.readUInt64LE(), UInt64(flattened.count))
        XCTAssertEqual(try tagsReader.readBytes(flattened.count), flattened)
        XCTAssertEqual(tagsReader.remaining(), 0)

        let retiredNestedTags = sequence([
            constVector(firstTag),
            constVector(secondTag),
        ])
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodeBranchClaim(
            fields([encodedPath, retiredNestedTags])
        ))
    }

    func testPeerPaymentWireCarriesOnlyBundleAndDerivesReplayIdentity() throws {
        let operationID = fixed32(0x52)
        let requestDigest = fixed32(0x51)
        let bundle = try syntheticPeerSplitBundle(
            branch: .recipient,
            operationID: operationID,
            requestDigest: requestDigest
        )

        let payment = try KagemushaRecursiveSpendPeerPayment.create(
            recipientBundle: bundle
        )
        XCTAssertEqual(payment.operationID, operationID)
        XCTAssertEqual(payment.recipientRequestDigest, requestDigest)
        XCTAssertEqual(payment.recipientBundle, bundle)

        let paymentFrame = try XCTUnwrap(noritoDecodeFrame(payment.archive))
        let bundleFrame = try XCTUnwrap(noritoDecodeFrame(bundle.archive))
        var paymentReader = OfflineNoritoReader(data: paymentFrame.payload)
        XCTAssertEqual(try paymentReader.readCompactField(), bundleFrame.payload)
        XCTAssertEqual(paymentReader.remaining(), 0)

        let retiredDuplicatedIdentity = noritoEncode(
            typeName: KagemushaRecursiveSpend.peerPaymentWireName,
            payload: fields([operationID, requestDigest, bundleFrame.payload]),
            flags: NoritoHeader.compactLen
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodePeerPayment(
            retiredDuplicatedIdentity
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidArchive("peerPayment.trailing")
            )
        }
    }

    func testPeerPaymentRejectsNonRecipientOrZeroIdentityTransition() throws {
        XCTAssertThrowsError(try KagemushaRecursiveSpendPeerPayment.create(
            recipientBundle: syntheticPeerSplitBundle(
                branch: .change,
                operationID: fixed32(0x52),
                requestDigest: fixed32(0x51)
            )
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidArchive("peerPayment.branch")
            )
        }
        XCTAssertThrowsError(try KagemushaRecursiveSpendPeerPayment.create(
            recipientBundle: syntheticPeerSplitBundle(
                branch: .recipient,
                operationID: Data(repeating: 0, count: 32),
                requestDigest: fixed32(0x51)
            )
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("peerPayment.operationID")
            )
        }
    }

    func testSplitRejectsNonCanonicalOrMismatchedTopUpAnchorReferences() throws {
        let left = try branch(
            seed: 0x10,
            amount: "210",
            path: path(bit: nil, lineageRoot: fixed32(0xA0))
        )
        let right = try branch(
            seed: 0x20,
            amount: "415",
            path: path(bit: nil, lineageRoot: fixed32(0xA1))
        )
        let first = try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: fixed32(0x70),
            anchorDigest: fixed32(0xA0)
        )
        let second = try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: fixed32(0x71),
            anchorDigest: fixed32(0xA1)
        )

        func split(_ refs: [KagemushaRecursiveSpendTopUpAnchorRef]) throws {
            _ = try KagemushaRecursiveSpendSplitIntent(
                chainID: left.inputNote.chainID,
                assetDefinitionID: left.inputNote.assetDefinitionID,
                inputs: [left, right],
                topUpAnchorRefs: refs,
                assetScale: 2,
                lineageMode: .semantic,
                outputArtifactGeneration: "generation-v2-test",
                transferAmount: KagemushaScaledAmount(atomicUnits: "625", scale: 2),
                recipientOutput: try note(seed: 0x40, amount: "625"),
                changeOutput: nil,
                recipientRequestDigest: fixed32(0x51),
                operationID: fixed32(0x52)
            )
        }

        XCTAssertNoThrow(try split([first, second]))
        XCTAssertThrowsError(try split([second, first])) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("split.topUpAnchorRefs.order")
            )
        }
        let mismatched = try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: first.topUpOperationID,
            anchorDigest: fixed32(0xA2)
        )
        XCTAssertThrowsError(try split([mismatched, second])) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("split.topUpAnchorRefs.identity")
            )
        }
        let duplicateLineage = try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: second.topUpOperationID,
            anchorDigest: first.anchorDigest
        )
        XCTAssertThrowsError(try split([first, duplicateLineage])) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("split.topUpAnchorRefs.identity")
            )
        }
        XCTAssertThrowsError(try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: Data(repeating: 0, count: 32),
            anchorDigest: fixed32(0xA0)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("topUpAnchorRef.topUpOperationID")
            )
        }
    }

    func testTwoInputJoinRejectsNonCanonicalOrderAndAllowsProofBoundChange() throws {
        let left = try branch(seed: 0x10, amount: "210", path: path(bit: 0))
        let right = try branch(seed: 0x20, amount: "415", path: path(bit: 1))
        let fullOutput = try note(seed: 0x40, amount: "625")

        XCTAssertThrowsError(try KagemushaRecursiveSpendSplitIntent(
            chainID: left.inputNote.chainID,
            assetDefinitionID: left.inputNote.assetDefinitionID,
            inputs: [right, left],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: "625", scale: 2),
            recipientOutput: fullOutput,
            changeOutput: nil,
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("split.inputs.order")
            )
        }
        let partial = try KagemushaRecursiveSpendSplitIntent(
            chainID: left.inputNote.chainID,
            assetDefinitionID: left.inputNote.assetDefinitionID,
            inputs: [left, right],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: "600", scale: 2),
            recipientOutput: try note(seed: 0x40, amount: "600"),
            changeOutput: try note(seed: 0x50, amount: "25"),
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )
        XCTAssertEqual(partial.changeOutput?.amount.atomicUnits, "25")
    }

    func testSplitRejectsU128OverflowAcrossInputs() throws {
        let maximum = KagemushaScaledAmount.maximumAtomicUnits
        let left = try branch(seed: 0x10, amount: maximum, path: path(bit: 0))
        let right = try branch(seed: 0x20, amount: "1", path: path(bit: 1))
        XCTAssertThrowsError(try KagemushaRecursiveSpendSplitIntent(
            chainID: left.inputNote.chainID,
            assetDefinitionID: left.inputNote.assetDefinitionID,
            inputs: [left, right],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: maximum, scale: 2),
            recipientOutput: try note(seed: 0x40, amount: maximum),
            changeOutput: nil,
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )) { error in
            XCTAssertEqual(error as? KagemushaScaledAmountError, .atomicUnitsOverflow)
        }
    }

    func testTwoInputJoinRejectsMixedAlternativeSplitClaims() throws {
        let left = try branch(
            seed: 0x10,
            amount: "210",
            path: path(bit: 0),
            transitionBinding: fixed32(0xB0)
        )
        let right = try branch(
            seed: 0x20,
            amount: "415",
            path: path(bit: 1),
            transitionBinding: fixed32(0xB1)
        )

        XCTAssertThrowsError(try KagemushaRecursiveSpendSplitIntent(
            chainID: left.inputNote.chainID,
            assetDefinitionID: left.inputNote.assetDefinitionID,
            inputs: [left, right],
            topUpAnchorRefs: [try topUpAnchorRef()],
            assetScale: 2,
            lineageMode: .semantic,
            outputArtifactGeneration: "generation-v2-test",
            transferAmount: KagemushaScaledAmount(atomicUnits: "625", scale: 2),
            recipientOutput: try note(seed: 0x40, amount: "625"),
            changeOutput: nil,
            recipientRequestDigest: fixed32(0x51),
            operationID: fixed32(0x52)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("branchClaims.transitionChoice")
            )
        }
    }

    func testSemanticLineageDAGRoundTripsCrossTopUpJoin() throws {
        let left = try lineageNode(seed: 0x10, parents: [], step: 1, archiveByte: 0xA1)
        let right = try lineageNode(seed: 0x20, parents: [], step: 1, archiveByte: 0xB2)
        let joined = try lineageNode(
            seed: 0x30,
            parents: [left.resultBundleDigest, right.resultBundleDigest],
            step: 2,
            archiveByte: 0xC3
        )
        let witness = try KagemushaRecursiveSpendLineageWitness(
            nodes: [left, right, joined],
            finalBundleDigest: joined.resultBundleDigest
        )

        let archive = try witness.noritoEncoded()
        let rustGolden = try XCTUnwrap(Data(hexString: Self.semanticDAGRustGoldenHex))
        XCTAssertEqual(archive, rustGolden, "Rust/Swift semantic DAG wire drift")
        XCTAssertEqual(
            try KagemushaRecursiveSpendCodecs.decodeLineageWitness(archive),
            witness
        )
        XCTAssertEqual(witness.nodes.count, 3)
        XCTAssertEqual(witness.nodes.last?.parentBundleDigests.count, 2)
    }

    func testSemanticLineageDAGRejectsAmbiguousOrDisconnectedHistory() throws {
        let left = try lineageNode(seed: 0x10, parents: [], step: 1)
        let right = try lineageNode(seed: 0x20, parents: [], step: 1)
        let joined = try lineageNode(
            seed: 0x30,
            parents: [left.resultBundleDigest, right.resultBundleDigest],
            step: 2
        )

        func assertWitnessRejected(
            _ nodes: [KagemushaRecursiveSpendLineageNode],
            final: Data = Data(repeating: 0x30, count: 32),
            field: String
        ) {
            XCTAssertThrowsError(try KagemushaRecursiveSpendLineageWitness(
                nodes: nodes,
                finalBundleDigest: final
            )) { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendError, .invalidField(field))
            }
        }

        assertWitnessRejected(
            [right, left, joined],
            field: "lineageWitness.nodes.order"
        )
        assertWitnessRejected(
            [left, right, try lineageNode(seed: 0x30, parents: [fixed32(0x7F)], step: 2)],
            field: "lineageWitness.nodes.parentBundleDigests.missing"
        )
        assertWitnessRejected(
            [left, right, try lineageNode(
                seed: 0x30,
                parents: [left.resultBundleDigest, right.resultBundleDigest],
                step: 3
            )],
            field: "lineageWitness.nodes.proofStepCount"
        )
        assertWitnessRejected(
            [left, right, try lineageNode(
                seed: 0x30,
                parents: [left.resultBundleDigest, right.resultBundleDigest],
                step: 2,
                verificationHeight: 99
            )],
            field: "lineageWitness.nodes.verifiedAtBlockHeight"
        )
        assertWitnessRejected(
            [left, right, try lineageNode(
                seed: 0x30,
                parents: [left.resultBundleDigest],
                step: 2
            )],
            field: "lineageWitness.nodes.sink"
        )
        assertWitnessRejected(
            [left, right, try lineageNode(
                seed: 0x20,
                parents: [left.resultBundleDigest],
                step: 2
            )],
            final: right.resultBundleDigest,
            field: "lineageWitness.nodes.resultBundleDigest.duplicate"
        )
        XCTAssertThrowsError(try lineageNode(
            seed: 0x30,
            parents: [right.resultBundleDigest, left.resultBundleDigest],
            step: 2
        )) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .invalidField("lineageNode"))
        }
    }

    func testABI18InventoryRequiresExplicitFailClosedCapabilities() {
        let expectedProofSymbols = [
            "connect_norito_kagemusha_recursive_spend_init_v2",
            "connect_norito_kagemusha_recursive_spend_append_v2",
            "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
            "connect_norito_kagemusha_recursive_spend_verify_v2",
            "connect_norito_kagemusha_recursive_spend_redeem_v2",
        ]
        let expectedProtocolSymbols = [
            "connect_norito_kagemusha_recursive_spend_capabilities_v1",
            "connect_norito_kagemusha_topup_finality_verify_v2",
            "connect_norito_kagemusha_recursive_spend_topup_v2",
            "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
            "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
            "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
            "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
            "connect_norito_kagemusha_receiver_key_reference_v2",
            "connect_norito_kagemusha_recipient_output_derive_v2",
            "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
            "connect_norito_kagemusha_recipient_payment_request_create_v2",
            "connect_norito_kagemusha_recipient_payment_request_verify_v2",
            "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
            "connect_norito_kagemusha_request_authorization_create_v2",
            "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
            "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
            "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
            "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
            "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
            "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
            "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
            "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
            "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
            "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
        ]

        XCTAssertFalse(KagemushaRecursiveSpend.isProofBackendAvailable)
        XCTAssertEqual(KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion, 18)
        XCTAssertEqual(
            KagemushaRecursiveSpendError.nativeBridgeUnavailable.errorDescription,
            "The ABI-18 Kagemusha recursive spend V2 bridge is unavailable."
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendError.finalityTrustUnavailable.errorDescription,
            "Kagemusha top-up finality is unavailable until the authenticated release trust root is wired and recursive init consumes its result."
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.artifactManifestSchema,
            "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        )
        XCTAssertEqual(KagemushaRecursiveSpend.mode, "recursive_spend_v1")
        XCTAssertEqual(KagemushaOfflineSpendMode.recursiveSpendV1.rawValue, "recursive_spend_v1")
        XCTAssertTrue(KagemushaRecursiveSpend.isSpendAgainMode("recursive_spend_v1"))
        XCTAssertFalse(KagemushaRecursiveSpend.isSpendAgainMode("recursive_spend_v2"))
        XCTAssertFalse(KagemushaRecursiveSpend.isSpendAgainMode("recursive_compact_v1"))
        XCTAssertNil(KagemushaOfflineSpendMode(rawValue: "recursive_spend_v2"))
        XCTAssertNil(KagemushaOfflineSpendMode(rawValue: "recursive_compact_v1"))
        XCTAssertFalse(KagemushaRecursiveSpend.isProductionAvailable)
        XCTAssertNil(KagemushaRecursiveSpend.preferredProductionMode)
        XCTAssertEqual(
            KagemushaRecursiveSpend.pastaCycleBackend,
            "halo2/ipa-pasta-cycle-v1"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.pastaCycleTranscript,
            "kagemusha-pasta-cycle-poseidon-v1"
        )
        XCTAssertEqual(KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion, 1)
        XCTAssertEqual(KagemushaRecursiveSpend.stateBoundaryVersion, 1)
        XCTAssertEqual(KagemushaRecursiveSpend.releaseMaximumProofBytes, 4_096)
        XCTAssertEqual(
            KagemushaRecursiveSpend.artifactMaximumFileBytes,
            256 * 1_024 * 1_024
        )
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerArchiveBytes, 9_211)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerTextEnvelopeBytes, 12 * 1_024)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumInputNullifiers, 2)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumBranchClaims, 2)
        XCTAssertEqual(KagemushaRecursiveSpend.transitionTagBytes, 24)
        XCTAssertEqual(KagemushaRecursiveSpend.semanticLineageMaximumNodes, 64)
        XCTAssertEqual(
            KagemushaRecursiveSpend.semanticLineageMaximumNodeArchiveBytes,
            64 * 1_024
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.semanticLineageMaximumTotalArchiveBytes,
            2 * 1_024 * 1_024
        )
        XCTAssertEqual(KagemushaRecursiveSpend.requiredProofSymbols, expectedProofSymbols)
        XCTAssertEqual(KagemushaRecursiveSpend.requiredProtocolSymbols, expectedProtocolSymbols)
        XCTAssertEqual(
            KagemushaRecursiveSpend.requiredNativeSymbols,
            expectedProofSymbols + expectedProtocolSymbols
        )
        XCTAssertNil(
            KagemushaRecursiveSpend.preferredProductionMode(
                proofBackendAvailable: false,
                nativeStubAvailable: false
            )
        )
        XCTAssertNil(
            KagemushaRecursiveSpend.preferredProductionMode(
                proofBackendAvailable: true,
                nativeStubAvailable: false
            )
        )
        XCTAssertNil(
            KagemushaRecursiveSpend.preferredProductionMode(
                proofBackendAvailable: false,
                nativeStubAvailable: true
            )
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.preferredProductionMode(
                proofBackendAvailable: true,
                nativeStubAvailable: true
            ),
            .recursiveSpendV1
        )
        XCTAssertEqual(
            NativeBridgeError.fromStatus(-314),
            .kagemushaRecursiveSpendV2Unavailable
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.splitIntentWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendSplitIntent"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.branchWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendBranch"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.splitResultWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendSplitResult"
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpend.ensureProofBackendAvailable()) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendError, .proofBackendUnavailable)
        }
    }

    func testNativeCapabilitiesRequireExactABI18ContractAndGateSet() throws {
        let capabilities = try KagemushaRecursiveSpendNativeCapabilities(
            bridgeABIVersion: 18,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchema,
            mode: KagemushaRecursiveSpend.mode,
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackend,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscript,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion,
            stateBoundaryVersion: KagemushaRecursiveSpend.stateBoundaryVersion,
            transitionCircuitID: KagemushaRecursiveSpend.transitionEqCircuitID,
            stateCircuitID: KagemushaRecursiveSpend.stateEpCircuitID,
            maxProofBytes: UInt32(KagemushaRecursiveSpend.releaseMaximumProofBytes),
            proofBackendAvailable: false,
            missingGates: KagemushaRecursiveSpend.unavailableProofBackendGates
        )
        XCTAssertFalse(capabilities.proofBackendAvailable)
        XCTAssertEqual(capabilities.mode, "recursive_spend_v1")
        XCTAssertEqual(
            capabilities.missingGates,
            KagemushaRecursiveSpend.unavailableProofBackendGates
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilities(
            bridgeABIVersion: 18,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchema,
            mode: KagemushaRecursiveSpend.mode,
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackend,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscript,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion,
            stateBoundaryVersion: KagemushaRecursiveSpend.stateBoundaryVersion,
            transitionCircuitID: KagemushaRecursiveSpend.transitionEqCircuitID,
            stateCircuitID: KagemushaRecursiveSpend.stateEpCircuitID,
            maxProofBytes: UInt32(KagemushaRecursiveSpend.releaseMaximumProofBytes),
            proofBackendAvailable: false,
            missingGates: []
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilities(
            bridgeABIVersion: 18,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchema,
            mode: "unsupported_mode",
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackend,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscript,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion,
            stateBoundaryVersion: KagemushaRecursiveSpend.stateBoundaryVersion,
            transitionCircuitID: KagemushaRecursiveSpend.transitionEqCircuitID,
            stateCircuitID: KagemushaRecursiveSpend.stateEpCircuitID,
            maxProofBytes: UInt32(KagemushaRecursiveSpend.releaseMaximumProofBytes),
            proofBackendAvailable: true,
            missingGates: []
        ))
        for rejectedMode in ["unknown_recursive_mode", "recursive_spend_v2"] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilities(
                bridgeABIVersion: 18,
                artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchema,
                mode: rejectedMode,
                proofBackend: KagemushaRecursiveSpend.pastaCycleBackend,
                transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscript,
                proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion,
                stateBoundaryVersion: KagemushaRecursiveSpend.stateBoundaryVersion,
                transitionCircuitID: KagemushaRecursiveSpend.transitionEqCircuitID,
                stateCircuitID: KagemushaRecursiveSpend.stateEpCircuitID,
                maxProofBytes: UInt32(KagemushaRecursiveSpend.releaseMaximumProofBytes),
                proofBackendAvailable: false,
                missingGates: KagemushaRecursiveSpend.unavailableProofBackendGates
            ), rejectedMode)
        }
    }

    func testTopUpFinalityOpaqueTypesPinExactNoritoSchemasAndCopyBytes() throws {
        let proofArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.topUpFinalityProofWireName
        )
        let rosterArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName
        )
        let proof = try KagemushaTopUpFinalityProofArchive(
            noritoArchive: proofArchive
        )
        let roster = try KagemushaTopUpFinalityRosterArtifactArchive(
            noritoArchive: rosterArchive
        )
        let manifestArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName
        )
        let manifest = try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: manifestArchive,
            expectedSHA256: Data(SHA256.hash(data: manifestArchive))
        )
        XCTAssertEqual(proof.noritoArchive, proofArchive)
        XCTAssertEqual(roster.noritoArchive, rosterArchive)
        XCTAssertEqual(manifest.noritoArchive, manifestArchive)

        XCTAssertThrowsError(try KagemushaTopUpFinalityProofArchive(
            noritoArchive: rosterArchive
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidArchive("topUpFinalityProof")
            )
        }
        XCTAssertThrowsError(try KagemushaTopUpFinalityRosterArtifactArchive(
            noritoArchive: proofArchive
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidArchive("topUpFinalityRosterArtifact")
            )
        }
        XCTAssertThrowsError(try KagemushaTopUpFinalityProofArchive(
            noritoArchive: noritoEncode(
                typeName: KagemushaRecursiveSpend.topUpFinalityProofWireName,
                payload: Data(
                    repeating: 0xa5,
                    count: KagemushaRecursiveSpend
                        .topUpFinalityProofMaximumArchiveBytes
                ),
                flags: NoritoHeader.compactLen
            )
        ))
        XCTAssertThrowsError(try KagemushaTopUpFinalityRosterArtifactArchive(
            noritoArchive: noritoEncode(
                typeName: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
                payload: Data(
                    repeating: 0xa6,
                    count: KagemushaRecursiveSpend
                        .topUpFinalityRosterMaximumArchiveBytes
                ),
                flags: NoritoHeader.compactLen
            )
        ))
    }

    func testV3ArtifactManifestArchivePinsSchemaAndDigest() throws {
        let manifestArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName
        )
        let nonManifestArchive = framedArchive(
            typeName: "iroha_core::zk::kagemusha_v2::KagemushaRecursiveSpendPastaCycleArtifactsV3"
        )
        let manifestSHA256 = Data(SHA256.hash(data: manifestArchive))
        let manifest = try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: manifestArchive,
            expectedSHA256: manifestSHA256
        )
        XCTAssertEqual(manifest.noritoArchive, manifestArchive)
        XCTAssertEqual(manifest.sha256, manifestSHA256)
        XCTAssertEqual(
            KagemushaRecursiveSpend.artifactManifestWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendArtifactManifestV3"
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: nonManifestArchive,
            expectedSHA256: Data(SHA256.hash(data: nonManifestArchive))
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: manifestArchive,
            expectedSHA256: Data(repeating: 0xA5, count: 32)
        ))
    }

    func testV3ArtifactInstallSessionValidatesLocallyAndCannotReopenAfterCancel() throws {
        let manifestArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName
        )
        let manifest = try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: manifestArchive,
            expectedSHA256: Data(SHA256.hash(data: manifestArchive))
        )
        let session = KagemushaRecursiveSpendArtifactInstallSessionV3(manifest: manifest)
        XCTAssertEqual(session.manifest, manifest)

        XCTAssertThrowsError(try session.beginPastaCycleV3ArtifactIngest(
            expectedArtifactSHA256: Data(repeating: 0, count: 32)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifact.sha256")
            )
        }

        // An empty pending session cancels without resolving native symbols.
        try session.cancel()
        XCTAssertThrowsError(try session.beginPastaCycleV3ArtifactIngest(
            expectedArtifactSHA256: fixed32(0xA5)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.state")
            )
        }
    }

    func testTopUpAnchorRejectsCrossContextAndNonCanonicalBindings() throws {
        func assertRejected(
            _ label: String,
            field: String,
            _ build: () throws -> KagemushaRecursiveSpendTopUpAnchor
        ) {
            XCTAssertThrowsError(try build(), label) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveSpendError,
                    .invalidField(field),
                    label
                )
            }
        }

        XCTAssertEqual(try topUpAnchor().version, 2)
        assertRejected("V1 anchor", field: "topUpAnchor") {
            try topUpAnchor(version: 1)
        }
        assertRejected("amount mismatch", field: "topUpAnchor") {
            try topUpAnchor(amount: "2")
        }
        assertRejected("chain mismatch", field: "topUpAnchor") {
            try topUpAnchor(chainID: "another-chain")
        }
        let otherPayer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC1))
            .toI105(networkPrefix: 0x02F1)
        assertRejected("asset owner mismatch", field: "topUpAnchor") {
            try topUpAnchor(assetID: "\(assetDefinitionID())#\(otherPayer)")
        }
        var otherDefinitionBytes = Data((0..<16).map { UInt8($0 + 17) })
        otherDefinitionBytes[6] = (otherDefinitionBytes[6] & 0x0f) | 0x40
        otherDefinitionBytes[8] = (otherDefinitionBytes[8] & 0x3f) | 0x80
        let otherDefinition = try XCTUnwrap(
            AssetDefinitionAddress.encode(uuidBytes: otherDefinitionBytes)
        )
        let payer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC0))
            .toI105(networkPrefix: 0x02F1)
        assertRejected("asset definition mismatch", field: "topUpAnchor") {
            try topUpAnchor(assetID: "\(otherDefinition)#\(payer)")
        }
        assertRejected("unchanged root", field: "topUpAnchor") {
            try topUpAnchor(initialRoot: fixed32(0xD3))
        }
        assertRejected("note/nullifier collision", field: "topUpAnchor") {
            try topUpAnchor(nullifiers: [fixed32(0xD0)])
        }
        assertRejected("too many input nullifiers", field: "topUpAnchor") {
            try topUpAnchor(nullifiers: [fixed32(0xD4), fixed32(0xD5), fixed32(0xD6)])
        }
        assertRejected("non-canonical nullifier order", field: "topUpAnchorNullifiers.order") {
            try topUpAnchor(nullifiers: [fixed32(0xD5), fixed32(0xD4)])
        }
        assertRejected("zero operation id", field: "topUpOperationID") {
            try topUpAnchor(operationID: Data(repeating: 0, count: 32))
        }
        assertRejected("blank artifact generation", field: "artifactGeneration") {
            try topUpAnchor(artifactGeneration: " ")
        }
    }

    func testFlatInitRequestRoundTripsAndRejectsAnchorEvidenceMismatch() throws {
        let currentNote = try note(seed: 0xD0, amount: "1")
        let transfer = try transferFixture(currentNote: currentNote)
        let anchor = try canonicalTopUpAnchor(
            currentNote: currentNote,
            transfer: transfer
        )
        let request = try KagemushaRecursiveSpendInitRequest(
            topUpAnchor: anchor,
            recordBundle: transfer.recordBundle,
            pallasOpenEnvelopesArchive: transfer.pallasOpenEnvelopes,
            lineageMode: .semantic
        )
        let encoded = try request.noritoEncoded()
        XCTAssertEqual(try KagemushaRecursiveSpendInitRequest.decode(encoded), request)
        XCTAssertThrowsError(try KagemushaRecursiveSpend.initSpend(request: request)) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .proofBackendUnavailable
            )
        }

        let mismatchedAnchor = try canonicalTopUpAnchor(
            currentNote: currentNote,
            transfer: transfer,
            finalizedRoot: fixed32(0xE9)
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendInitRequest(
            topUpAnchor: mismatchedAnchor,
            recordBundle: transfer.recordBundle,
            pallasOpenEnvelopesArchive: transfer.pallasOpenEnvelopes,
            lineageMode: .semantic
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("topUpAnchor.transferEvidence")
            )
        }

        let futureVerifier = try transferFixture(
            currentNote: currentNote,
            activationHeight: 2
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendInitRequest(
            topUpAnchor: anchor,
            recordBundle: futureVerifier.recordBundle,
            pallasOpenEnvelopesArchive: futureVerifier.pallasOpenEnvelopes,
            lineageMode: .semantic
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendRequestCodecError,
                .invalidArchive("initRequest.recordBundle.verifierRecords[0].record")
            )
        }
    }

    func testRecipientPublicKeyUsesCanonicalDynamicVector() throws {
        let publicKeyBytes = fixed32(0xC2)
        let publicKey = try KagemushaPublicKey(payload: publicKeyBytes)
        let recipient = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC3))
            .toI105(networkPrefix: 0x02F1)
        let output = try note(seed: 0xE0, amount: "1")
        let payload = try KagemushaRecipientPaymentRequestSigningPayload(
            chainID: output.chainID,
            assetDefinitionID: output.assetDefinitionID,
            amount: output.amount,
            recipient: recipient,
            recipientKeyReference: fixed32(0xE2),
            receiverDeviceID: "fixture-device",
            receiverPublicKey: publicKey,
            requestID: fixed32(0xE3),
            issuedAtMilliseconds: 1,
            expiresAtMilliseconds: 2,
            recipientOutput: output,
            recipientOutputProverMaterial: Data([0xE4])
        )

        let archive = try KagemushaRecursiveSpendCodecs
            .encodeRecipientRequestPayload(payload)
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        var payloadReader = OfflineNoritoReader(data: frame.payload)
        for _ in 0..<6 {
            _ = try payloadReader.readCompactField()
        }
        let encodedPublicKey = try payloadReader.readCompactField()
        var publicKeyReader = OfflineNoritoReader(data: encodedPublicKey)
        XCTAssertEqual(try publicKeyReader.readUInt64LE(), 33)
        var decoded = Data()
        for _ in 0..<33 {
            let byte = try publicKeyReader.readCompactField()
            XCTAssertEqual(byte.count, 1)
            decoded.append(byte[byte.startIndex])
        }
        XCTAssertEqual(publicKeyReader.remaining(), 0)
        XCTAssertEqual(decoded.first, 0)
        XCTAssertEqual(Data(decoded.dropFirst()), publicKeyBytes)
    }

    #if canImport(Darwin)
    func testStaticProcessHandleReplacesDynamicHandleForV2Resolution() throws {
        let dynamicHandle = try XCTUnwrap(UnsafeMutableRawPointer(bitPattern: 0x01))
        let processHandle = try XCTUnwrap(UnsafeMutableRawPointer(bitPattern: 0x02))

        XCTAssertEqual(
            NoritoNativeBridge.bridgeHandleForStaticFallback(
                currentHandle: dynamicHandle,
                processHandle: processHandle
            ),
            processHandle
        )
        XCTAssertEqual(
            NoritoNativeBridge.bridgeHandleForStaticFallback(
                currentHandle: dynamicHandle,
                processHandle: nil
            ),
            dynamicHandle
        )
        XCTAssertEqual(
            NoritoNativeBridge.bridgeHandleForStaticFallback(
                currentHandle: nil,
                processHandle: processHandle
            ),
            processHandle
        )
        XCTAssertNil(
            NoritoNativeBridge.bridgeHandleForStaticFallback(
                currentHandle: nil,
                processHandle: nil
            )
        )
    }
    #endif

    private struct TransferFixture {
        let recordBundle: Data
        let pallasOpenEnvelopes: Data
        let initialRoot: Data
        let finalizedRoot: Data
        let inputNullifiers: [Data]
        let verifierKeyID: String
        let verifierKeyCommitment: Data
    }

    private func transferFixture(
        currentNote: KagemushaSpendableNoteDescriptor,
        activationHeight: UInt64? = 1
    ) throws -> TransferFixture {
        let backend = "halo2/ipa"
        let verifierName = "fixture-transfer"
        let verifierKeyID = "\(backend):\(verifierName)"
        let verifierKeyBytes = fixed32(0xE0)
        let verifierKeyCommitment = verifierCommitment(
            backend: backend,
            bytes: verifierKeyBytes
        )
        let initialRoot = fixed32(0xD2)
        let finalizedRoot = fixed32(0xD3)
        let inputNullifiers = [fixed32(0xD4)]

        let proofBox = fields([
            OfflineCompactNorito.encodeString(backend),
            byteVector(Data([0xA5])),
        ])
        let verifierKeyIDPayload = fields([
            OfflineCompactNorito.encodeString(backend),
            OfflineCompactNorito.encodeString(verifierName),
        ])
        let attachment = fields([
            OfflineCompactNorito.encodeString(backend),
            proofBox,
            verifierKeyIDPayload,
            option(verifierKeyCommitment),
            option(nil),
            option(nil),
        ])
        let verifierKey = fields([
            OfflineCompactNorito.encodeString(backend),
            byteVector(verifierKeyBytes),
        ])
        let step = fields([
            initialRoot,
            fixed32Sequence(inputNullifiers),
            fixed32Sequence([currentNote.noteCommitment]),
            finalizedRoot,
            attachment,
            verifierKey,
        ])
        let bundle = fields([
            fields([OfflineCompactNorito.encodeString(currentNote.chainID)]),
            constVector(try XCTUnwrap(AssetDefinitionAddress.decode(
                currentNote.assetDefinitionID
            ))),
            sequence([step]),
        ])

        let verifierRecord = fields([
            uint32(1),
            OfflineCompactNorito.encodeString(verifierName),
            option(nil),
            OfflineCompactNorito.encodeString("offline_kagemusha"),
            uint32(VerifyingKeyBackendTag.halo2IpaPasta.rawValue),
            OfflineCompactNorito.encodeString("pallas"),
            fixed32(0x92),
            verifierKeyCommitment,
            uint32(UInt32(verifierKeyBytes.count)),
            uint32(4_096),
            option(nil),
            option(nil),
            option(nil),
            optionalUInt64(activationHeight),
            optionalUInt64(nil),
            option(verifierKey),
            uint32(1),
        ])
        let verifierEntry = fields([verifierKeyIDPayload, verifierRecord])
        let recordBundle = noritoEncode(
            typeName: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            payload: fields([bundle, sequence([verifierEntry])]),
            flags: NoritoHeader.compactLen
        )

        let pallasOpenEnvelopes = pallasOpenEnvelopesArchive(
            verifierKeyCommitment: verifierKeyCommitment
        )
        return TransferFixture(
            recordBundle: recordBundle,
            pallasOpenEnvelopes: pallasOpenEnvelopes,
            initialRoot: initialRoot,
            finalizedRoot: finalizedRoot,
            inputNullifiers: inputNullifiers,
            verifierKeyID: verifierKeyID,
            verifierKeyCommitment: verifierKeyCommitment
        )
    }

    private func canonicalTopUpAnchor(
        currentNote: KagemushaSpendableNoteDescriptor,
        transfer: TransferFixture,
        finalizedRoot: Data? = nil
    ) throws -> KagemushaRecursiveSpendTopUpAnchor {
        let payer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC0))
            .toI105(networkPrefix: 0x02F1)
        let draft = try KagemushaRecursiveSpendTopUpAnchor(
            version: 2,
            chainID: currentNote.chainID,
            payer: payer,
            assetID: "\(currentNote.assetDefinitionID)#\(payer)",
            assetScale: currentNote.amount.scale,
            amount: currentNote.amount,
            initialRoot: transfer.initialRoot,
            finalizedRoot: finalizedRoot ?? transfer.finalizedRoot,
            topUpAnchorNullifiers: transfer.inputNullifiers,
            currentNote: currentNote,
            topUpOperationID: fixed32(0xD5),
            transferVerifierID: transfer.verifierKeyID,
            transferVerifierCommitment: transfer.verifierKeyCommitment,
            artifactGeneration: "generation-v2-test",
            finalizedHeight: 1,
            finalizedTransactionHash: fixed32(0xD7),
            anchorDigest: fixed32(0xD8),
            archive: Data([1])
        )
        return try KagemushaRecursiveSpendTopUpAnchor.decode(
            KagemushaRecursiveSpendCodecs.encodeTopUpAnchor(draft)
        )
    }

    private func pallasOpenEnvelopesArchive(
        verifierKeyCommitment: Data
    ) -> Data {
        let n: UInt32 = 4
        let params = fields([
            uint16(1),
            uint16(1),
            uint32(n),
            fixed32Sequence((0..<Int(n)).map { fixed32(0x10 &+ UInt8($0)) }),
            fixed32Sequence((0..<Int(n)).map { fixed32(0x20 &+ UInt8($0)) }),
            fixed32(0x30),
        ])
        let publicOpening = fields([
            uint16(1),
            uint16(1),
            uint32(n),
            fixed32(0x31),
            fixed32(0x32),
            fixed32(0x33),
        ])
        let proof = fields([
            uint16(1),
            fixed32Sequence([fixed32(0x40), fixed32(0x41)]),
            fixed32Sequence([fixed32(0x50), fixed32(0x51)]),
            fixed32(0x60),
            fixed32(0x61),
        ])
        let envelope = fields([
            params,
            publicOpening,
            proof,
            OfflineCompactNorito.encodeString("pallas-open"),
            option(verifierKeyCommitment),
            option(fixed32(0x71)),
            option(fixed32(0x72)),
        ])
        var archive = noritoEncode(
            typeName: "test.PallasOpenEnvelopeVector",
            payload: sequence([envelope]),
            flags: NoritoHeader.compactLen
        )
        let schema = Data([
            0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
            0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca,
        ])
        archive.replaceSubrange(6..<22, with: schema)
        return archive
    }

    private func verifierCommitment(backend: String, bytes: Data) -> Data {
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.utf8.count), to: &preimage)
        preimage.append(Data(backend.utf8))
        appendUInt64BE(UInt64(bytes.count), to: &preimage)
        preimage.append(bytes)
        return Data(SHA256.hash(data: preimage))
    }

    private func fields(_ values: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private func sequence(_ values: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private func constVector(_ value: Data) -> Data {
        fields(value.map { Data([$0]) })
    }

    private func fixed32Sequence(_ values: [Data]) -> Data {
        sequence(values)
    }

    private func byteVector(_ value: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private func option(_ value: Data?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeLength(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private func optionalUInt64(_ value: UInt64?) -> Data {
        option(value.map(uint64))
    }

    private func uint16(_ value: UInt16) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    private func uint32(_ value: UInt32) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    private func uint64(_ value: UInt64) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(value)
        return writer.data
    }

    private func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        var bigEndian = value.bigEndian
        withUnsafeBytes(of: &bigEndian) { data.append(contentsOf: $0) }
    }

    private func branch(
        seed: UInt8,
        amount: String,
        path: KagemushaRecursiveSpendBranchPath,
        transitionBinding: Data = Data(repeating: 0xB0, count: 32)
    ) throws -> KagemushaRecursiveSpendInputBranch {
        let tags = try (0..<Int(path.depth)).map { _ in
            try KagemushaRecursiveSpend.transitionTag(for: transitionBinding)
        }
        return try KagemushaRecursiveSpendInputBranch(
            bundleDigest: fixed32(seed),
            inputNote: note(seed: seed &+ 1, amount: amount),
            branchClaims: [try KagemushaRecursiveSpendBranchClaim(
                path: path,
                transitionTags: tags
            )],
            inputRoot: fixed32(seed &+ 4),
            proofStepCount: 1,
            peerHopCount: UInt32(path.depth)
        )
    }

    private func syntheticPeerSplitBundle(
        branch: KagemushaRecursiveSpendBranch,
        operationID: Data,
        requestDigest: Data
    ) throws -> KagemushaRecursiveSpendBundle {
        let peerSplit = fields([
            fixed32(0x50),
            uint32(branch.rawValue),
            requestDigest,
            operationID,
            uint32(1),
            uint32(0),
        ])
        var transition = OfflineCompactNoritoWriter()
        transition.writeUInt32LE(0)
        transition.writeField(peerSplit)

        let statement = fields(
            (0..<9).map { Data([UInt8($0 + 1)]) }
                + [
                    option(transition.data),
                    Data([0xA1]),
                    Data([0xA2]),
                    Data([0xA3]),
                ]
        )
        let archive = noritoEncode(
            typeName: KagemushaRecursiveSpend.bundleWireName,
            payload: fields([statement, Data([0xB0])]),
            flags: NoritoHeader.compactLen
        )
        let claim = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA0)
        )
        let summary = KagemushaRecursiveSpendBundleSummary(
            assetDefinitionID: assetDefinitionID(),
            amount: try KagemushaScaledAmount(atomicUnits: "1", scale: 2),
            noteCommitment: fixed32(0x30),
            spendNullifier: fixed32(0x31),
            hopCount: 1,
            branchClaims: [claim],
            artifactGeneration: "generation-v2-test",
            verifierKeyID: KagemushaRecursiveSpend.semanticCircuitID,
            lineageMode: .semantic,
            bundleDigest: fixed32(0x32)
        )
        return KagemushaRecursiveSpendBundle(archive: archive, summary: summary)
    }

    private func topUpAnchor(
        version: UInt16 = 2,
        chainID: String? = nil,
        amount: String = "1",
        assetID: String? = nil,
        initialRoot: Data? = nil,
        nullifiers: [Data]? = nil,
        operationID: Data? = nil,
        artifactGeneration: String = "generation-v2-test"
    ) throws -> KagemushaRecursiveSpendTopUpAnchor {
        let currentNote = try note(seed: 0xD0, amount: "1")
        let payer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC0))
            .toI105(networkPrefix: 0x02F1)
        return try KagemushaRecursiveSpendTopUpAnchor(
            version: version,
            chainID: chainID ?? currentNote.chainID,
            payer: payer,
            assetID: assetID ?? "\(currentNote.assetDefinitionID)#\(payer)",
            assetScale: 2,
            amount: KagemushaScaledAmount(atomicUnits: amount, scale: 2),
            initialRoot: initialRoot ?? fixed32(0xD2),
            finalizedRoot: fixed32(0xD3),
            topUpAnchorNullifiers: nullifiers ?? [fixed32(0xD4)],
            currentNote: currentNote,
            topUpOperationID: operationID ?? fixed32(0xD5),
            transferVerifierID: "halo2:fixture-transfer",
            transferVerifierCommitment: fixed32(0xD6),
            artifactGeneration: artifactGeneration,
            finalizedHeight: 1,
            finalizedTransactionHash: fixed32(0xD7),
            anchorDigest: fixed32(0xD8),
            archive: Data([1])
        )
    }

    private func topUpAnchorRef() throws -> KagemushaRecursiveSpendTopUpAnchorRef {
        try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: fixed32(0xD5),
            anchorDigest: fixed32(0xA0)
        )
    }

    private func note(seed: UInt8, amount: String) throws -> KagemushaSpendableNoteDescriptor {
        try KagemushaSpendableNoteDescriptor(
            chainID: "swift-kagemusha-v2",
            assetDefinitionID: assetDefinitionID(),
            noteCommitment: fixed32(seed),
            spendNullifier: fixed32(seed &+ 1),
            amount: KagemushaScaledAmount(atomicUnits: amount, scale: 2)
        )
    }

    private func lineageNode(
        seed: UInt8,
        parents: [Data],
        step: UInt32,
        verificationHeight: UInt64? = nil,
        archiveByte: UInt8? = nil
    ) throws -> KagemushaRecursiveSpendLineageNode {
        try KagemushaRecursiveSpendLineageNode(
            resultBundleDigest: fixed32(seed),
            parentBundleDigests: parents,
            proofStepCount: step,
            verifiedAtBlockHeight: verificationHeight ?? (step == 1 ? 100 : 101),
            transitionArchive: Data([archiveByte ?? seed])
        )
    }

    private func path(
        bit: UInt8?,
        lineageRoot: Data? = nil
    ) throws -> KagemushaRecursiveSpendBranchPath {
        var bits = Data(repeating: 0, count: 8)
        if bit == 1 { bits[0] = 0x80 }
        return try KagemushaRecursiveSpendBranchPath(
            lineageRoot: lineageRoot ?? fixed32(0xA0),
            depth: bit == nil ? 0 : 1,
            pathBits: bits
        )
    }

    private func assetDefinitionID() -> String {
        var bytes = Data((0..<16).map { UInt8($0 + 1) })
        bytes[6] = (bytes[6] & 0x0f) | 0x40
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return AssetDefinitionAddress.encode(uuidBytes: bytes)!
    }

    private func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }

    private func framedArchive(typeName: String) -> Data {
        let payload = Data([0xA5])
        var archive = Data()
        archive.append(NoritoHeader.magic)
        archive.append(contentsOf: [NoritoHeader.versionMajor, NoritoHeader.versionMinor])
        archive.append(contentsOf: noritoSchemaHash(forTypeName: typeName))
        archive.append(NoritoCompression.none.rawValue)
        archive.append(contentsOf: withUnsafeBytes(
            of: UInt64(payload.count).littleEndian,
            Array.init
        ))
        archive.append(contentsOf: withUnsafeBytes(
            of: crc64ECMA(payload).littleEndian,
            Array.init
        ))
        archive.append(NoritoHeader.compactLen)
        archive.append(payload)
        return archive
    }

    private static let semanticDAGRustGoldenHex =
        "4e52543000003604117c64ddb476ec54ce10bfd0662f00780100000000000063899240769af2d102d5020300000000000000" +
        "4220101010101010101010101010101010101010101010101010101010101010101008000000000000000004010000000864" +
        "00000000000000090100000000000000a1422020202020202020202020202020202020202020202020202020202020202020" +
        "200800000000000000000401000000086400000000000000090100000000000000b2c5012030303030303030303030303030" +
        "303030303030303030303030303030303030308a010200000000000000400110011001100110011001100110011001100110" +
        "0110011001100110011001100110011001100110011001100110011001100110011001100110011001100110400120012001" +
        "2001200120012001200120012001200120012001200120012001200120012001200120012001200120012001200120012001" +
        "2001200120012001200402000000086500000000000000090100000000000000c32030303030303030303030303030303030" +
        "30303030303030303030303030303030"
}
