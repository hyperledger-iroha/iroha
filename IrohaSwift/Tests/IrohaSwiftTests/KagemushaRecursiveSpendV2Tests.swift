import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendTests: XCTestCase {
    func testNativeBusyStatusIsRetryableAtThePublicLifecycleBoundary() {
        XCTAssertEqual(NativeBridgeError.fromStatus(-318), .kagemushaBusy)
        XCTAssertEqual(
            KagemushaRecursiveSpendError.proofWorkerBusy.errorDescription,
            "Another Kagemusha proof operation is active; retry after it completes."
        )
    }

    func testTopUpShieldBusyStatusMapsToRetryableError() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot
                .appendingPathComponent("Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift"),
            encoding: .utf8
        )
        let start = try XCTUnwrap(
            source.range(of: "    public func buildUnsigned() throws")
        )
        let tail = source[start.lowerBound...]
        let end = try XCTUnwrap(
            tail.range(of: "/// Canonical unsigned ABI22 online-to-offline request fields.")
        )
        let implementation = String(tail[..<end.lowerBound])

        XCTAssertTrue(implementation.contains("catch NativeBridgeError.kagemushaBusy"))
        XCTAssertTrue(implementation.contains(
            "throw KagemushaRecursiveSpendError.proofWorkerBusy"
        ))
    }

    func testTopUpWitnessBindingKeepsTheSingleAuthenticatedSnapshot() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot
                .appendingPathComponent("Sources/IrohaSwift/TxBuilder.swift"),
            encoding: .utf8
        )
        let start = try XCTUnwrap(source.range(of: "package func prepareKagemushaTopUpShield("))
        let tail = source[start.lowerBound...]
        let end = try XCTUnwrap(tail.range(of: "    /// Generates a new signing key"))
        let implementation = String(tail[..<end.lowerBound])

        XCTAssertTrue(implementation.contains("getZkAssetMerklePathSnapshot("))
        XCTAssertTrue(implementation.contains("canonicalAuth: canonicalAuth"))
        XCTAssertTrue(implementation.contains(
            "snapshot.evaluatedBlockHeight >= expectedReadiness.minimumEvaluatedBlockHeight"
        ))
        XCTAssertTrue(implementation.contains(
            "verifier.activationHeight <= snapshot.evaluatedBlockHeight"
        ))
        XCTAssertTrue(implementation.contains(
            "evaluatedBlockHeight: snapshot.evaluatedBlockHeight"
        ))
        XCTAssertTrue(implementation.contains(
            "evaluatedBlockHash: snapshot.evaluatedBlockHash"
        ))
        XCTAssertFalse(implementation.contains("let currentReadiness"))
        XCTAssertFalse(implementation.contains("getKagemushaReadiness"))
    }

    func testReleaseQualifiedStepEqVerifierKeyIDBindsExactManifest() throws {
        let manifest = Data(repeating: 0x42, count: 32)
        XCTAssertEqual(
            try KagemushaRecursiveSpend.releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: manifest
            ),
            "\(KagemushaRecursiveSpend.pastaCycleBackendV4):"
                + "\(KagemushaRecursiveSpend.stepEqCircuitIDV4)-"
                + String(repeating: "42", count: 32)
        )
        XCTAssertNotEqual(
            try KagemushaRecursiveSpend.releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: manifest
            ),
            try KagemushaRecursiveSpend.releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: Data(repeating: 0x43, count: 32)
            )
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpend.releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: Data(repeating: 0, count: 32)
            )
        )
    }

    func testVerifyResultV4DecoderRequiresExactVerifierWindowAndManifestBinding() throws {
        let atActivation = try KagemushaRecursiveSpendCodecs.decodeVerifyResultV4(
            verifyResultV4Archive(blockHeight: 10)
        )
        XCTAssertEqual(atActivation.verifierActivationHeight, 10)
        XCTAssertEqual(atActivation.verifierWithdrawHeight, 20)
        XCTAssertEqual(atActivation.verifiedAtBlockHeight, 10)

        let beforeWithdrawal = try KagemushaRecursiveSpendCodecs.decodeVerifyResultV4(
            verifyResultV4Archive(blockHeight: 19)
        )
        XCTAssertEqual(beforeWithdrawal.verifiedAtBlockHeight, 19)

        let invalidWindows: [(
            activation: UInt64?,
            withdrawal: UInt64?,
            blockHeight: UInt64
        )] = [
            (nil, 20, 10),
            (10, nil, 10),
            (0, 20, 10),
            (10, 10, 10),
            (20, 10, 10),
            (10, 20, 9),
            (10, 20, 20),
        ]
        for window in invalidWindows {
            XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodeVerifyResultV4(
                verifyResultV4Archive(
                    activation: window.activation,
                    withdrawal: window.withdrawal,
                    blockHeight: window.blockHeight
                )
            ))
        }

        let manifest = fixed32(0xB8)
        let staleVerifierKeyID = try KagemushaRecursiveSpend
            .releaseQualifiedStepEqVerifierKeyIDV4(manifestSHA256: fixed32(0xB9))
        for verifierKeyID in [
            "\(KagemushaRecursiveSpend.pastaCycleBackendV4):"
                + KagemushaRecursiveSpend.stepEqCircuitIDV4,
            staleVerifierKeyID,
        ] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodeVerifyResultV4(
                verifyResultV4Archive(
                    manifest: manifest,
                    verifierKeyID: verifierKeyID
                )
            ))
        }
    }

    func testKagemushaDeviceAttestationIsCanonicalAndRejectsAdversarialInputs() throws {
        let registration = try kagemushaDeviceAttestation()
        XCTAssertEqual(
            try registration.canonicalChallengeHash(),
            registration.challengeHash
        )
        XCTAssertFalse(registration.attestationReport.isEmpty)
        XCTAssertEqual(
            Data(registration.evidence.prefix(
                KagemushaDeviceAttestation.deviceAttestationEvidencePrefix.utf8.count
            )),
            Data(KagemushaDeviceAttestation.deviceAttestationEvidencePrefix.utf8)
        )
        XCTAssertFalse(try registration.noritoEncoded().isEmpty)

        XCTAssertThrowsError(try kagemushaDeviceAttestation(version: 2)) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationError,
                .invalidRegistrationVersion(2)
            )
        }
        XCTAssertThrowsError(try kagemushaDeviceAttestation(oneUse: false)) { error in
            XCTAssertEqual(
                error as? KagemushaDeviceAttestationError,
                .authorityMustBeOneUse
            )
        }
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            authorityPublicKey: Data(repeating: 0x41, count: 31)
        ))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(platform: "ios"))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(keyID: " credential "))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(deviceID: "device-1\n"))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(assertionUsageCountLimit: 1))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            assertionPublicKey: Data(repeating: 0, count: 65)
        ))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(attestationReport: Data()))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(recentBlockHeight: 0))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(expiresAtMilliseconds: 0))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            recentBlockHash: Data(repeating: 1, count: 31)
        ))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            challengeHash: fixed32(0xA1)
        ))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            attestationReportHash: fixed32(0xA1)
        ))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(evidence: Data([0x00])))
        XCTAssertThrowsError(try kagemushaDeviceAttestation(
            evidenceHash: fixed32(0xA1)
        ))
    }

    func testNoteOpeningCodecRoundTripsOnlyCanonicalNonzeroArchives() throws {
        let opening = try KagemushaNoteOpening(
            spendKey: fixed32(0x31),
            rho: fixed32(0x32),
            diversifier: fixed32(0x33)
        )
        let archive = try KagemushaRecursiveSpendCodecs.encodeNoteOpening(opening)
        XCTAssertEqual(try XCTUnwrap(noritoDecodeFrame(archive)).paddingLength, 0)
        XCTAssertEqual(
            try KagemushaRecursiveSpendCodecs.decodeNoteOpening(archive),
            opening
        )

        var padded = archive
        padded.insert(
            contentsOf: Data(repeating: 0, count: 8),
            at: NoritoHeader.encodedLength
        )
        XCTAssertEqual(try XCTUnwrap(noritoDecodeFrame(padded)).paddingLength, 8)
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeNoteOpening(padded)
        )

        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeNoteOpening(Data(archive.dropLast()))
        )
        var extended = archive
        extended.append(0)
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeNoteOpening(extended)
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeNoteOpening(
                Data(repeating: 0, count: archive.count)
            )
        )
    }

    func testMembershipWitnessCodecRoundTripsAndRejectsPathSubstitution() throws {
        let leafIndex: UInt32 = 5
        let inputDirections = Data((0..<16).map {
            UInt8((UInt64(leafIndex) >> UInt64($0)) & 1)
        })
        let root = fixed32(0x44)
        let inputPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: (0..<16).map { fixed32(UInt8($0 + 1)) },
            directions: inputDirections,
            root: root
        )
        let dummyPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: (0..<16).map { fixed32(UInt8($0 + 33)) },
            directions: Data(repeating: 0, count: 16),
            root: root
        )
        let witness = try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: inputPath,
            dummyInputPath: dummyPath
        )
        let archive = try witness.noritoEncoded()
        XCTAssertEqual(try KagemushaNoteMembershipWitness.decode(archive), witness)

        XCTAssertThrowsError(try KagemushaNoteMembershipWitness.decode(
            Data(archive.dropLast())
        ))
        var extended = archive
        extended.append(0)
        XCTAssertThrowsError(try KagemushaNoteMembershipWitness.decode(extended))

        let otherRootPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: dummyPath.siblings,
            directions: dummyPath.directions,
            root: fixed32(0x45)
        )
        XCTAssertThrowsError(try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: inputPath,
            dummyInputPath: otherRootPath
        ))

        let wrongDirections = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: inputPath.siblings,
            directions: Data(repeating: 0, count: 16),
            root: root
        )
        XCTAssertThrowsError(try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: wrongDirections,
            dummyInputPath: dummyPath
        ))
        XCTAssertThrowsError(try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: inputPath,
            dummyInputPath: inputPath
        ))
        let zeroRootPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: inputPath.siblings,
            directions: inputPath.directions,
            root: Data(repeating: 0, count: 32)
        )
        XCTAssertThrowsError(try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: zeroRootPath,
            dummyInputPath: zeroRootPath
        ))
    }

    func testV4TwoOutputMembershipAcceptsIntermediateChangeRootAndRequiresExactFrontier() throws {
        func path(_ index: UInt32, root: Data, seed: UInt8) throws
            -> PrivacyConfidentialMerklePathWitnessV2
        {
            try PrivacyConfidentialMerklePathWitnessV2(
                siblings: (0..<16).map { fixed32(seed &+ UInt8($0)) },
                directions: Data((0..<16).map {
                    UInt8((UInt64(index) >> UInt64($0)) & 1)
                }),
                root: root
            )
        }

        let initialRoot = fixed32(0x71)
        let intermediateRoot = fixed32(0x72)
        let finalRoot = fixed32(0x73)
        let recipient = try KagemushaOutputMembershipLeafPathsV4(
            leafIndex: 7,
            updatePath: path(7, root: initialRoot, seed: 0x10),
            membershipPath: path(7, root: finalRoot, seed: 0x20)
        )
        let change = try KagemushaOutputMembershipLeafPathsV4(
            leafIndex: 8,
            updatePath: path(8, root: intermediateRoot, seed: 0x30),
            membershipPath: path(8, root: finalRoot, seed: 0x40)
        )
        let canonical = try KagemushaOutputMembershipPathsV4(
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            recipient: recipient,
            change: change,
            dummyLeafIndex: 9,
            dummyPath: path(9, root: finalRoot, seed: 0x50)
        )
        XCTAssertEqual(canonical.change?.updatePath.root, intermediateRoot)

        XCTAssertThrowsError(try KagemushaOutputMembershipPathsV4(
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            recipient: recipient,
            change: change,
            dummyLeafIndex: 10,
            dummyPath: path(10, root: finalRoot, seed: 0x60)
        ))

        let skippedChange = try KagemushaOutputMembershipLeafPathsV4(
            leafIndex: 9,
            updatePath: path(9, root: intermediateRoot, seed: 0x70),
            membershipPath: path(9, root: finalRoot, seed: 0x80)
        )
        XCTAssertThrowsError(try KagemushaOutputMembershipPathsV4(
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            recipient: recipient,
            change: skippedChange,
            dummyLeafIndex: 10,
            dummyPath: path(10, root: finalRoot, seed: 0x90)
        ))
    }

    func testTopUpReadinessExpectationRejectsRoleSubstitutionAndExpiredVerifier() throws {
        func binding(
            backend: String = "halo2/ipa",
            circuitID: String = KagemushaRecursiveSpend.topUpShieldCircuitID,
            commitment: String = String(repeating: "11", count: 32),
            schema: String = String(repeating: "22", count: 32),
            maximumProofBytes: UInt32 = 192 * 1024,
            activationHeight: UInt64 = 40,
            withdrawalHeight: UInt64? = 80
        ) throws -> KagemushaTopUpShieldVerifierBinding {
            try KagemushaTopUpShieldVerifierBinding(
                backend: backend,
                name: "kagemusha-topup-shield-v2",
                version: 3,
                circuitID: circuitID,
                commitment: commitment,
                publicInputsSchemaHash: schema,
                maximumProofBytes: maximumProofBytes,
                activationHeight: activationHeight,
                withdrawalHeight: withdrawalHeight
            )
        }

        let verifier = try binding()
        let expected = try KagemushaTopUpShieldReadinessExpectation(
            assetDefinitionID: assetDefinitionID(),
            assetScale: 9,
            minimumEvaluatedBlockHeight: 42,
            verifier: verifier
        )
        XCTAssertEqual(expected.verifier, verifier)

        XCTAssertThrowsError(try binding(backend: "halo2/kzg"))
        XCTAssertThrowsError(try binding(circuitID: "confidential-transfer-v2"))
        XCTAssertThrowsError(try binding(commitment: String(repeating: "00", count: 32)))
        XCTAssertThrowsError(try binding(schema: "ab"))
        XCTAssertThrowsError(try binding(maximumProofBytes: 192 * 1024 + 1))
        XCTAssertThrowsError(try binding(activationHeight: 80, withdrawalHeight: 80))
        XCTAssertThrowsError(try KagemushaTopUpShieldReadinessExpectation(
            assetDefinitionID: assetDefinitionID(),
            assetScale: 9,
            minimumEvaluatedBlockHeight: 39,
            verifier: verifier
        ))
        XCTAssertThrowsError(try KagemushaTopUpShieldReadinessExpectation(
            assetDefinitionID: assetDefinitionID(),
            assetScale: 9,
            minimumEvaluatedBlockHeight: 80,
            verifier: verifier
        ))
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

    func testBranchClaimConflictVerifierAllowsOnlyConsistentSiblings() throws {
        let transitionTag = Data(
            repeating: 0x41,
            count: KagemushaRecursiveSpend.transitionTagBytes
        )
        let recipient = try KagemushaRecursiveSpendBranchClaim(
            path: path(bit: 0),
            transitionTags: [transitionTag]
        )
        let change = try KagemushaRecursiveSpendBranchClaim(
            path: path(bit: 1),
            transitionTags: [transitionTag]
        )
        XCTAssertFalse(recipient.conflicts(with: change))
        XCTAssertFalse(change.conflicts(with: recipient))

        let root = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xA0)
        )
        XCTAssertTrue(root.conflicts(with: recipient))
        XCTAssertTrue(recipient.conflicts(with: root))
        XCTAssertTrue(recipient.conflicts(with: recipient))

        let alternative = try KagemushaRecursiveSpendBranchClaim(
            path: path(bit: 1),
            transitionTags: [Data(
                repeating: 0x42,
                count: KagemushaRecursiveSpend.transitionTagBytes
            )]
        )
        XCTAssertTrue(recipient.conflicts(with: alternative))
        XCTAssertTrue(alternative.conflicts(with: recipient))

        let independent = try KagemushaRecursiveSpendBranchClaim(
            path: path(bit: 0, lineageRoot: fixed32(0xB0)),
            transitionTags: [transitionTag]
        )
        XCTAssertFalse(recipient.conflicts(with: independent))
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

        var claimReader = CanonicalNoritoReader(data: encoded)
        let encodedPath = try claimReader.readCompactField()
        let encodedTags = try claimReader.readCompactField()
        XCTAssertEqual(claimReader.remaining(), 0)
        var flattened = firstTag
        flattened.append(secondTag)
        var tagsReader = CanonicalNoritoReader(data: encodedTags)
        XCTAssertEqual(try tagsReader.readUInt64LE(), UInt64(flattened.count))
        XCTAssertEqual(try tagsReader.readBytes(flattened.count), flattened)
        XCTAssertEqual(tagsReader.remaining(), 0)

        let nonCanonicalNestedTags = sequence([
            constVector(firstTag),
            constVector(secondTag),
        ])
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodeBranchClaim(
            fields([encodedPath, nonCanonicalNestedTags])
        ))
    }

    func testV4InventoryRequiresExplicitFailClosedCapabilities() {
        XCTAssertEqual(KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion, 22)
        XCTAssertEqual(
            KagemushaRecursiveSpend.artifactManifestSchemaV4,
            "kagemusha.offline.recursive_spend.artifact_manifest.v4"
        )
        XCTAssertEqual(KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4, 5)
        XCTAssertEqual(KagemushaRecursiveSpend.localWitnessVersionV4, 4)
        XCTAssertEqual(KagemushaRecursiveSpend.artifactRolesV4.count, 8)
        XCTAssertEqual(
            KagemushaRecursiveSpend.artifactRolesV4,
            [
                "step_eq_params_ipa",
                "step_eq_proving_key",
                "step_eq_verifying_key",
                "step_eq_bootstrap_witness",
                "step_ep_params_ipa",
                "step_ep_proving_key",
                "step_ep_verifying_key",
                "step_ep_bootstrap_witness",
            ]
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendArtifactRoleV4.allCases.map(\.fileName),
            KagemushaRecursiveSpend.artifactFileNamesV4
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.VerifierRole.recursiveStepEq.registryName,
            "kagemusha_recursive_step_eq_v4_verifier_record"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.VerifierRole.recursiveStepEq.circuitID,
            KagemushaRecursiveSpend.stepEqCircuitIDV4
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.VerifierRole.recursiveStepEp.registryName,
            "kagemusha_recursive_step_ep_v4_verifier_record"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.VerifierRole.recursiveStepEp.circuitID,
            KagemushaRecursiveSpend.stepEpCircuitIDV4
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.VerifierRole.unshield.registryName,
            "confidential_unshield_v3_verifier_record"
        )
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerArchiveBytesV4, 32 * 1_024 * 1_024)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerArchiveBytes, 32 * 1_024 * 1_024)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumInputsPerTransition, 2)
        XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerHops, 8)
        XCTAssertFalse(KagemushaRecursiveSpend.isProductionAvailable)
        XCTAssertTrue(
            KagemushaRecursiveSpend.requiredNativeSymbols.allSatisfy {
                !$0.hasSuffix("_v3")
            }
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpend.ensureProofBackendAvailableV4())
    }

    #if os(macOS)
    func testLinkedPrivacyRuntimeReportsOneExactFailClosedProductionGate() throws {
        let capabilities = try KagemushaRecursiveSpend.nativeCapabilitiesV4()

        XCTAssertEqual(capabilities.bridgeABIVersion, 22)
        XCTAssertEqual(capabilities.proofEnvelopeVersion, 5)
        XCTAssertFalse(capabilities.proofBackendAvailable)
        switch (
            capabilities.missingGates,
            KagemushaRecursiveSpend.isProductionCompiledAndLinked
        ) {
        case (["authenticated-v4-artifact-installation"], true),
             (["authenticated-production-promotion"], false):
            break
        default:
            XCTFail(
                "unexpected bridge-ABI-22 production gate state: "
                    + "\(capabilities.missingGates), "
                    + "compiled=\(KagemushaRecursiveSpend.isProductionCompiledAndLinked)"
            )
        }
        XCTAssertFalse(KagemushaRecursiveSpend.isProductionAvailable)
    }
    #endif

    func testNativeLinkageProbeDoesNotCachePrePromotionUnavailability() throws {
        var promoted = false
        var probeCount = 0
        let probe = {
            probeCount += 1
            return promoted
        }

        XCTAssertFalse(KagemushaRecursiveSpend.productionAvailability(
            hasRequiredNativeSymbols: true,
            probe: probe
        ))
        promoted = true
        XCTAssertTrue(KagemushaRecursiveSpend.productionAvailability(
            hasRequiredNativeSymbols: true,
            probe: probe
        ))
        XCTAssertEqual(probeCount, 2, "artifact readiness must be probed again after promotion")

        XCTAssertFalse(KagemushaRecursiveSpend.productionAvailability(
            hasRequiredNativeSymbols: false,
            probe: { XCTFail("an absent bridge ABI must not be invoked"); return true }
        ))

        #if canImport(Darwin)
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-22 bridge is not linked in this test host"
        )
        // Portable offer projection is protocol parsing, not proof-backend
        // readiness. It must remain callable while the production gate is
        // deliberately closed before artifact promotion.
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        XCTAssertFalse(
            try offer.project(
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ).request.archive.isEmpty
        )
        #endif
    }

    func testProductionCompilationProbeBreaksArtifactBootstrapCycleWithoutOpeningMoneyGate() {
        var installed = false
        var probeCount = 0
        let productionProbe = {
            probeCount += 1
            return (
                proofBackendAvailable: installed,
                missingGates: installed ? [] : ["authenticated-v4-artifact-installation"]
            )
        }
        XCTAssertTrue(KagemushaRecursiveSpend.productionCompilationAvailability(
            hasRequiredNativeSymbols: true,
            probe: productionProbe
        ))
        installed = true
        XCTAssertTrue(KagemushaRecursiveSpend.productionCompilationAvailability(
            hasRequiredNativeSymbols: true,
            probe: productionProbe
        ))
        XCTAssertEqual(probeCount, 2, "production compilation must be probed without caching")

        XCTAssertFalse(KagemushaRecursiveSpend.productionCompilationAvailability(
            hasRequiredNativeSymbols: true,
            probe: {
                (
                    proofBackendAvailable: false,
                    missingGates: ["authenticated-production-promotion"]
                )
            }
        ))
        XCTAssertFalse(KagemushaRecursiveSpend.productionCompilationAvailability(
            hasRequiredNativeSymbols: false,
            probe: { XCTFail("an absent bridge ABI must not be invoked"); return (true, []) }
        ))
    }

    func testNativeCapabilitiesV4RequireExactBridgeABI22EightRoleContract() throws {
        let gates = [
            "authenticated-v4-artifact-installation",
            "independent-cryptographic-review",
            "physical-device-benchmark",
            "production-recursive-prover-linkage",
        ]
        let maximumProofBytes = KagemushaRecursiveSpend.absoluteMaximumProofPairBytesV4
        XCTAssertEqual(maximumProofBytes, 384 * 1024)
        let archive = KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.nativeCapabilitiesWireNameV4,
            payload: fields([
                uint32(KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion),
                CompactNorito.encodeString(KagemushaRecursiveSpend.artifactManifestSchemaV4),
                CompactNorito.encodeString(KagemushaRecursiveSpend.pastaCycleBackendV4),
                CompactNorito.encodeString(KagemushaRecursiveSpend.pastaCycleTranscriptV4),
                uint16(KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4),
                CompactNorito.encodeString(KagemushaRecursiveSpend.stepEqCircuitIDV4),
                CompactNorito.encodeString(KagemushaRecursiveSpend.stepEpCircuitIDV4),
                sequence(KagemushaRecursiveSpend.artifactRolesV4.map(
                    CompactNorito.encodeString
                )),
                uint32(maximumProofBytes),
                Data([0]),
                sequence(gates.map(CompactNorito.encodeString)),
            ])
        )
        let capabilities = try KagemushaRecursiveSpendCodecs
            .decodeNativeCapabilitiesV4(archive)
        XCTAssertEqual(capabilities.bridgeABIVersion, 22)
        XCTAssertEqual(capabilities.artifactRoles, KagemushaRecursiveSpend.artifactRolesV4)
        XCTAssertEqual(capabilities.maxProofBytes, maximumProofBytes)
        XCTAssertEqual(capabilities.missingGates, gates)
        XCTAssertFalse(capabilities.proofBackendAvailable)

        // ABI 21 is intentionally retained only as a retired negative vector.
        XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilitiesV4(
            bridgeABIVersion: 21,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchemaV4,
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackendV4,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscriptV4,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4,
            stepEqCircuitID: KagemushaRecursiveSpend.stepEqCircuitIDV4,
            stepEpCircuitID: KagemushaRecursiveSpend.stepEpCircuitIDV4,
            artifactRoles: KagemushaRecursiveSpend.artifactRolesV4,
            maxProofBytes: maximumProofBytes,
            proofBackendAvailable: false,
            missingGates: gates
        ))

        var missingRole = KagemushaRecursiveSpend.artifactRolesV4
        missingRole.removeLast()
        XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilitiesV4(
            bridgeABIVersion: 22,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchemaV4,
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackendV4,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscriptV4,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4,
            stepEqCircuitID: KagemushaRecursiveSpend.stepEqCircuitIDV4,
            stepEpCircuitID: KagemushaRecursiveSpend.stepEpCircuitIDV4,
            artifactRoles: missingRole,
            maxProofBytes: maximumProofBytes,
            proofBackendAvailable: false,
            missingGates: gates
        ))

        XCTAssertThrowsError(try KagemushaRecursiveSpendNativeCapabilitiesV4(
            bridgeABIVersion: 22,
            artifactManifestSchema: KagemushaRecursiveSpend.artifactManifestSchemaV4,
            proofBackend: KagemushaRecursiveSpend.pastaCycleBackendV4,
            transcriptProfile: KagemushaRecursiveSpend.pastaCycleTranscriptV4,
            proofEnvelopeVersion: KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4,
            stepEqCircuitID: KagemushaRecursiveSpend.stepEqCircuitIDV4,
            stepEpCircuitID: KagemushaRecursiveSpend.stepEpCircuitIDV4,
            artifactRoles: KagemushaRecursiveSpend.artifactRolesV4,
            maxProofBytes: maximumProofBytes,
            proofBackendAvailable: false,
            missingGates: Array(gates.reversed())
        ))
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

    func testV4ArtifactManifestArchivePinsSchemaAndDigest() throws {
        let manifestArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName
        )
        let nonManifestArchive = framedArchive(
            typeName: "invalid::KagemushaRecursiveSpendArtifactManifest"
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
            "iroha_data_model::offline::model::KagemushaRecursiveSpendArtifactManifestV4"
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

    func testV4ArtifactInstallSessionValidatesLocallyAndCannotReopenAfterCancel() throws {
        let manifestArchive = framedArchive(
            typeName: KagemushaRecursiveSpend.artifactManifestWireName
        )
        let manifest = try KagemushaRecursiveSpendArtifactManifestArchive(
            noritoArchive: manifestArchive,
            expectedSHA256: Data(SHA256.hash(data: manifestArchive))
        )
        let session = try KagemushaRecursiveSpendArtifactInstallSessionV4(
            manifest: manifest,
            binding: KagemushaRecursiveSpendArtifactBindingV4(
                generation: "generation-v4-test",
                manifestSHA256: manifest.sha256
            ),
            authentication: try KagemushaRecursiveSpendReleaseAuthenticationV4(
                trustedPolicyNorito: Data([0x01]),
                releaseAttestationNorito: Data([0x02]),
                internalValidationReceiptNorito: Data([0x03]),
                benchmarkEvidence: Data([0x04]),
                cryptographicReview: Data([0x05]),
                promotionRecordNorito: Data([0x06])
            )
        )
        XCTAssertEqual(session.manifest, manifest)

        XCTAssertThrowsError(try session.beginArtifact(
            role: .stepEqParamsIpa,
            expectedArtifactSHA256: Data(repeating: 0, count: 32)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifact.sha256")
            )
        }

        // An empty pending session cancels without resolving native symbols.
        try session.cancel()
        XCTAssertThrowsError(try session.beginArtifact(
            role: .stepEqParamsIpa,
            expectedArtifactSHA256: fixed32(0xA5)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("artifactSet.state")
            )
        }
    }

    func testV4ReleaseAuthenticationIsMandatoryAndBounded() throws {
        XCTAssertNoThrow(try KagemushaRecursiveSpendReleaseAuthenticationV4(
            trustedPolicyNorito: Data([0x01]),
            releaseAttestationNorito: Data([0x02]),
            internalValidationReceiptNorito: Data([0x03]),
            benchmarkEvidence: Data([0x04]),
            cryptographicReview: Data([0x05]),
            promotionRecordNorito: Data([0x06])
        ))
        for field in 0..<6 {
            var values = [
                Data([0x01]), Data([0x02]), Data([0x03]), Data([0x04]), Data([0x05]),
                Data([0x06]),
            ]
            values[field] = Data()
            XCTAssertThrowsError(try KagemushaRecursiveSpendReleaseAuthenticationV4(
                trustedPolicyNorito: values[0],
                releaseAttestationNorito: values[1],
                internalValidationReceiptNorito: values[2],
                benchmarkEvidence: values[3],
                cryptographicReview: values[4],
                promotionRecordNorito: values[5]
            ))
        }
        XCTAssertThrowsError(try KagemushaRecursiveSpendReleaseAuthenticationV4(
            trustedPolicyNorito: Data(repeating: 0x01, count: 64 * 1_024 + 1),
            releaseAttestationNorito: Data([0x02]),
            internalValidationReceiptNorito: Data([0x03]),
            benchmarkEvidence: Data([0x04]),
            cryptographicReview: Data([0x05]),
            promotionRecordNorito: Data([0x06])
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendReleaseAuthenticationV4(
            trustedPolicyNorito: Data([0x01]),
            releaseAttestationNorito: Data([0x02]),
            internalValidationReceiptNorito: Data(
                repeating: 0x03,
                count: KagemushaRecursiveSpend.maximumInternalValidationReceiptBytesV4 + 1
            ),
            benchmarkEvidence: Data([0x04]),
            cryptographicReview: Data([0x05]),
            promotionRecordNorito: Data([0x06])
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendReleaseAuthenticationV4(
            trustedPolicyNorito: Data([0x01]),
            releaseAttestationNorito: Data([0x02]),
            internalValidationReceiptNorito: Data([0x03]),
            benchmarkEvidence: Data([0x04]),
            cryptographicReview: Data(
                repeating: 0x05,
                count: KagemushaRecursiveSpend.maximumCryptographicReviewBytesV4 + 1
            ),
            promotionRecordNorito: Data([0x06])
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendReleaseAuthenticationV4(
            trustedPolicyNorito: Data([0x01]),
            releaseAttestationNorito: Data([0x02]),
            internalValidationReceiptNorito: Data([0x03]),
            benchmarkEvidence: Data([0x04]),
            cryptographicReview: Data([0x05]),
            promotionRecordNorito: Data(
                repeating: 0x06,
                count: KagemushaRecursiveSpend.maximumPromotionRecordBytesV4 + 1
            )
        ))
    }

    func testV4OpaqueCarriersRejectFrozenSchemas() throws {
        let frozenBundleSchema =
            "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV2"
        XCTAssertNoThrow(try KagemushaRecursiveSpendBundleV4(
            noritoArchive: framedArchive(
                typeName: KagemushaRecursiveSpend.bundleWireNameV4
            )
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendBundleV4(
            noritoArchive: framedArchive(typeName: frozenBundleSchema)
        ))

        let frozenSchemas: [(String, String, (Data) throws -> Void)] = [
            (
                KagemushaRecursiveSpend.initResultWireNameV4,
                "iroha_data_model::offline::model::KagemushaRecursiveSpendInitResultV2",
                { _ = try KagemushaRecursiveSpendInitResultV4(noritoArchive: $0) }
            ),
            (
                KagemushaRecursiveSpend.splitResultWireNameV4,
                "iroha_data_model::offline::model::KagemushaRecursiveSpendSplitResultV2",
                { _ = try KagemushaRecursiveSpendSplitResultV4(noritoArchive: $0) }
            ),
            (
                KagemushaRecursiveSpend.verifyResultWireNameV4,
                "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV2",
                { _ = try KagemushaRecursiveSpendVerifyResultV4(noritoArchive: $0) }
            ),
            (
                KagemushaRecursiveSpend.redeemBuildResultWireNameV4,
                "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemBuildResultV2",
                { _ = try KagemushaRecursiveSpendRedeemBuildResultV4(noritoArchive: $0) }
            ),
        ]
        for (v4Schema, frozenSchema, construct) in frozenSchemas {
            // These result types validate their full payload, not only the
            // frame schema; the one-byte fixture is intentionally truncated.
            XCTAssertThrowsError(try construct(framedArchive(typeName: v4Schema)))
            XCTAssertThrowsError(try construct(framedArchive(typeName: frozenSchema)))
        }

        let bindingV4 = try KagemushaRecursiveSpendArtifactBindingV4(
            generation: "swift-v4-binding",
            manifestSHA256: fixed32(0xB8)
        )
        let v4Frame = try XCTUnwrap(noritoDecodeFrame(bindingV4.noritoEncoded()))
        XCTAssertEqual(
            v4Frame.header.schema,
            noritoSchemaHash(forTypeName: KagemushaRecursiveSpend.artifactBindingWireNameV4)
        )
        let frozenBinding = framedArchive(
            typeName:
                "iroha_data_model::offline::model::KagemushaRecursiveSpendArtifactBindingV3"
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpend.requireArchive(
            frozenBinding,
            schema: KagemushaRecursiveSpend.artifactBindingWireNameV4,
            field: "artifactBindingV4"
        ))
    }

    func testArtifactBindingRejectsMalformedAndGenerationSubstitution() throws {
        for generation in [
            "", " ", "release\n2", " release-2", "release-2 ",
            ".release", "release.", "release/name", "rélease", "CON",
            "com1.keys", String(repeating: "a", count: 129),
        ] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactBindingV4(
                generation: generation,
                manifestSHA256: fixed32(0xA7)
            ), generation)
        }
        for digest in [
            Data(repeating: 0x41, count: 31),
            Data(repeating: 0x41, count: 33),
            Data(repeating: 0, count: 32),
        ] {
            XCTAssertThrowsError(try KagemushaRecursiveSpendArtifactBindingV4(
                generation: "release-2",
                manifestSHA256: digest
            ))
        }

        let expected = try KagemushaRecursiveSpendArtifactBindingV4(
            generation: "release-2",
            manifestSHA256: fixed32(0xA7)
        )
        let substituted = try KagemushaRecursiveSpendArtifactBindingV4(
            generation: "release-3",
            manifestSHA256: expected.manifestSHA256
        )
        XCTAssertNotEqual(expected, substituted)
    }

    func testRecipientPublicKeyUsesFixedUncompressedP256Bytes() throws {
        let publicKeyBytes = try P256.Signing.PrivateKey(
            rawRepresentation: fixed32(0x02)
        ).publicKey.x963Representation
        let publicKey = try KagemushaDevicePublicKeyV2(sec1Bytes: publicKeyBytes)
        let recipient = try AccountAddress
            .fromAccount(publicKey: fixed32(0xC3))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let output = try note(seed: 0xE0, amount: "1")
        let payload = try KagemushaRecipientPaymentRequestSigningPayload(
            networkID: output.networkID,
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
            senderOutputProverMaterial: Data([0xE4])
        )

        let archive = try KagemushaRecursiveSpendCodecs
            .encodeRecipientRequestPayload(payload)
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        XCTAssertEqual(frame.paddingLength, 8)
        var payloadReader = CanonicalNoritoReader(data: frame.payload)
        for _ in 0..<6 {
            _ = try payloadReader.readCompactField()
        }
        let encodedPublicKey = try payloadReader.readCompactField()
        XCTAssertEqual(encodedPublicKey, publicKeyBytes)
    }

    func testRecipientRequestRejectsMissingOrExcessHeaderPadding() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        XCTAssertEqual(
            try XCTUnwrap(noritoDecodeFrame(request.archive)).paddingLength,
            8
        )
        XCTAssertNoThrow(
            try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
                request.archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        )

        var missingPadding = request.archive
        missingPadding.removeSubrange(
            NoritoHeader.encodedLength..<(NoritoHeader.encodedLength + 8)
        )
        XCTAssertEqual(
            try XCTUnwrap(noritoDecodeFrame(missingPadding)).paddingLength,
            0
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
                missingPadding,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        )

        var excessPadding = request.archive
        excessPadding.insert(
            contentsOf: Data(repeating: 0, count: 8),
            at: NoritoHeader.encodedLength
        )
        XCTAssertEqual(
            try XCTUnwrap(noritoDecodeFrame(excessPadding)).paddingLength,
            16
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
                excessPadding,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        )
    }

    func testRecipientRequestPreservesExplicitTairaContextAndRejectsWrongContext() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let decoded = try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
            request.archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )

        XCTAssertEqual(decoded.archive, request.archive)
        XCTAssertEqual(decoded.payload.recipient, request.payload.recipient)
        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(
                decoded.payload.recipient
            ).chainDiscriminant,
            SccpV1.tairaI105DiscriminantV1
        )

        let wrongContext = try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
            request.archive,
            chainDiscriminant: AccountId.defaultNetworkPrefix
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpend.canonicalAccountAddress(
            wrongContext.payload.recipient,
            field: "recipient",
            expectedChainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    #if canImport(Darwin)
    func testStaticProcessHandleReplacesDynamicHandleForKagemushaResolution() throws {
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

    private func fields(_ values: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private func sequence(_ values: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private func constVector(_ value: Data) -> Data {
        fields(value.map { Data([$0]) })
    }

    private func uint16(_ value: UInt16) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    private func uint32(_ value: UInt32) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    private func uint64(_ value: UInt64) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(value)
        return writer.data
    }

    private func optionalUInt64(_ value: UInt64?) -> Data {
        var writer = CompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(uint64(value))
        return writer.data
    }

    private func encodedVerifierKeyID(_ value: String) throws -> Data {
        let separator = try XCTUnwrap(value.firstIndex(of: ":"))
        return fields([
            CompactNorito.encodeString(String(value[..<separator])),
            CompactNorito.encodeString(String(value[value.index(after: separator)...])),
        ])
    }

    private func verifyResultV4Archive(
        manifest: Data? = nil,
        verifierKeyID: String? = nil,
        activation: UInt64? = 10,
        withdrawal: UInt64? = 20,
        blockHeight: UInt64 = 10
    ) throws -> Data {
        let manifest = manifest ?? fixed32(0xB8)
        let resolvedVerifierKeyID: String
        if let verifierKeyID {
            resolvedVerifierKeyID = verifierKeyID
        } else {
            resolvedVerifierKeyID = try KagemushaRecursiveSpend
                .releaseQualifiedStepEqVerifierKeyIDV4(
                    manifestSHA256: manifest
                )
        }
        let assetDefinitionBytes = try XCTUnwrap(
            AssetDefinitionAddress.decode(assetDefinitionID())
        )
        var atomicUnits = Data(repeating: 0, count: 16)
        atomicUnits[0] = 1
        let branchClaim = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(0xD1)
        )
        let summary = fields([
            constVector(assetDefinitionBytes),
            fields([atomicUnits, uint32(0)]),
            fixed32(0xC1),
            fixed32(0xC2),
            uint32(0),
            uint32(1),
            sequence([try KagemushaRecursiveSpendCodecs.encodeBranchClaim(branchClaim)]),
            fields([
                uint16(KagemushaRecursiveSpend.wireVersionV4),
                CompactNorito.encodeString("test-release"),
                manifest,
            ]),
            try encodedVerifierKeyID(resolvedVerifierKeyID),
            fixed32(0xC3),
        ])
        return KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.verifyResultWireNameV4,
            payload: fields([
                Data([1]),
                Data([1]),
                Data([1]),
                Data([1]),
                summary,
                fixed32(0xC4),
                fixed32(0xC5),
                try encodedVerifierKeyID(resolvedVerifierKeyID),
                CompactNorito.encodeString(KagemushaRecursiveSpend.stepEqCircuitIDV4),
                optionalUInt64(activation),
                optionalUInt64(withdrawal),
                uint64(blockHeight),
                uint64(1_000),
            ])
        )
    }

    private func kagemushaDeviceAttestation(
        version: UInt16 = KagemushaDeviceAttestation.registrationVersion,
        platform: String = KagemushaDeviceAttestation.iosAppAttestPlatform,
        keyID: String? = nil,
        deviceID: String = "ios-device-1",
        authorityPublicKey: Data? = nil,
        assertionPublicKey: Data? = nil,
        assertionUsageCountLimit: UInt32? = nil,
        oneUse: Bool = true,
        challengeHash: Data? = nil,
        attestationReportHash: Data? = nil,
        attestationReport: Data = Data("app-attest-object".utf8),
        evidenceHash: Data? = nil,
        evidence: Data? = nil,
        recentBlockHeight: UInt64 = 42,
        recentBlockHash: Data? = nil,
        expiresAtMilliseconds: UInt64 = 10_000
    ) throws -> KagemushaDeviceAttestationRegistration {
        let authorityKey = authorityPublicKey ?? fixed32(0xA5)
        let accountID = try AccountAddress
            .fromAccount(publicKey: authorityKey)
            .toI105(networkPrefix: 0x02F1)
        let assertionKey = try assertionPublicKey
            ?? P256.Signing.PrivateKey(
                rawRepresentation: Data(repeating: 1, count: 32)
            ).publicKey.x963Representation
        let deviceAuthorityKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: P256.Signing.PrivateKey(
                rawRepresentation: Data(repeating: 2, count: 32)
            ).publicKey.x963Representation
        )
        let resolvedKeyID = keyID ?? Data("app-attest-credential".utf8).base64EncodedString()
        let reportHash = IrohaHash.hash(attestationReport)
        let resolvedEvidence = evidence ?? (
            Data(KagemushaDeviceAttestation.deviceAttestationEvidencePrefix.utf8)
                + reportHash
        )
        return try KagemushaDeviceAttestationRegistration(
            version: version,
            platform: platform,
            keyId: resolvedKeyID,
            deviceId: deviceID,
            accountId: accountID,
            publicKey: deviceAuthorityKey,
            assertionScheme: KagemushaDeviceAttestation.iosAppAttestAssertionScheme,
            assertionKeyAlgorithm:
                KagemushaDeviceAttestation.iosAppAttestAssertionKeyAlgorithm,
            assertionPublicKey: assertionKey,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            challengeHash: challengeHash,
            attestationReportHash: attestationReportHash,
            attestationReport: attestationReport,
            evidenceHash: evidenceHash,
            evidence: resolvedEvidence,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash ?? IrohaHash.hash(Data("block-42".utf8)),
            expiresAtMs: expiresAtMilliseconds
        )
    }

    private func note(seed: UInt8, amount: String) throws -> KagemushaSpendableNoteDescriptor {
        try KagemushaSpendableNoteDescriptor(
            networkID: TestNetworkIds.canonical,
            assetDefinitionID: assetDefinitionID(),
            noteCommitment: fixed32(seed),
            spendNullifier: fixed32(seed &+ 1),
            amount: KagemushaScaledAmount(atomicUnits: amount, scale: 2)
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
        if KagemushaRecursiveSpend.archivedPayloadAlignment(forWireName: typeName) != nil {
            return KagemushaRecursiveSpend.frameArchive(
                schema: typeName,
                payload: payload
            )
        }
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

}
