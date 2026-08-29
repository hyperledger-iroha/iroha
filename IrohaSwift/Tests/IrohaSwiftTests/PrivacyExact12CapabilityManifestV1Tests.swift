import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyExact12CapabilityManifestV1Tests: XCTestCase {
    private let executionModes: [UInt32] = [0, 1, 2, 3, 4, 4, 2, 4, 5, 1, 5, 5]
    private let featureMasks: [UInt8] = [0, 6, 1, 2, 2, 2, 0, 2, 7, 2, 7, 31]

    func testStrictDecodePreservesCanonicalConsensusAndActivation() throws {
        let fixture = makeFixture(includePendingState: true)
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest,
            nativeCatalogArchive: fixture.catalog
        )

        XCTAssertEqual(manifest.version, 1)
        XCTAssertEqual(manifest.committedHeight, 3)
        XCTAssertEqual(manifest.canonicalBytes(), fixture.manifest)
        XCTAssertEqual(manifest.protocols.count, 12)
        let expectedConsensusLimits: [UInt32] = [
            1, 2, 9 * 1024 * 1024, 9 * 1024 * 1024, 9 * 1024 * 1024,
            18 * 1024 * 1024, 256 * 1024, 8, 8, 2_048,
        ]
        XCTAssertEqual(
            manifest.consensusPolicy.currentLimits.orderedValues,
            expectedConsensusLimits
        )
        let consensusTightening = try XCTUnwrap(
            manifest.consensusPolicy.pendingTightening
        )
        XCTAssertEqual(consensusTightening.scheduledAtHeight, 2)
        XCTAssertEqual(consensusTightening.effectiveAtHeight, 302)
        XCTAssertEqual(consensusTightening.nextLimits.retainedRootCount, 1_024)
        XCTAssertEqual(
            manifest.consensusPolicy.canonicalNorito,
            consensusPolicy(includePendingState: true)
        )

        let first = manifest.row(for: .zkAcePqAuthorizationV1)
        XCTAssertTrue(first.localCompiledTupleMatches)
        XCTAssertTrue(first.isNetworkAvailable)
        XCTAssertEqual(first.readiness, .productionQualified)
        guard let activation = first.activation else {
            return XCTFail("active row lost its complete committed activation")
        }
        XCTAssertEqual(activation.protocolId, .zkAcePqAuthorizationV1)
        XCTAssertEqual(activation.proofSystemId, .starkFriPoseidonX7Goldilocks6x64V1)
        XCTAssertEqual(activation.engineId, .nativeGoldilocksPoseidonX7StarkFri6x64V1)
        XCTAssertEqual(activation.parameterId, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(activation.parameterDigest, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(activation.verifierDigest, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(
            activation.statementSchemaDigest,
            Data(repeating: 0x31, count: 32)
        )
        XCTAssertEqual(
            activation.engineManifestDigest,
            Data(repeating: 0x31, count: 32)
        )
        XCTAssertEqual(activation.protocolLimits.values, [])
        XCTAssertNil(activation.pendingProtocolLimitsTightening)
        let qualification = try XCTUnwrap(manifest.qualification)
        let releaseBinding = qualification.releaseManifest.protocols[0]
        XCTAssertEqual(releaseBinding.securityClaim.protocolId, .zkAcePqAuthorizationV1)
        XCTAssertEqual(releaseBinding.securityClaim.securityModel, .postQuantumQrom)
        XCTAssertEqual(releaseBinding.securityClaim.targetSecurityBits, 128)
        XCTAssertEqual(releaseBinding.securityClaim.achievedSecurityBits, 128)
        XCTAssertEqual(
            releaseBinding.securityClaim.catalogCommitment,
            exact12CatalogCommitment()
        )
        let expectedClaimDigest = try XCTUnwrap(Data(
            hexString: "810fdc49f332423016cb7dbb7383dd03ccae74bf605202e0b8de7101439878a2"
        ))
        XCTAssertEqual(
            releaseBinding.securityClaimDigest,
            expectedClaimDigest
        )
        XCTAssertEqual(
            qualification.releaseManifest.manifestDigest,
            Data(repeating: 0xe3, count: 32)
        )
        XCTAssertEqual(
            qualification.deploymentQualification.releaseManifestDigest,
            Data(repeating: 0xe3, count: 32)
        )
        XCTAssertEqual(
            qualification.deploymentQualification.qualificationDigest,
            Data(repeating: 0xe4, count: 32)
        )
        XCTAssertEqual(activation.canonicalNorito, activationForProfileZero())
        guard case let .active(proposed, activated, since) = activation.lifecycle else {
            return XCTFail("active lifecycle projection was not retained")
        }
        XCTAssertEqual([proposed, activated, since], [1, 2, 2])

        let jindo = manifest.row(for: .irohaJindoPolynomialCommitmentV1)
        XCTAssertEqual(jindo.readiness, .productionQualified)
        XCTAssertTrue(jindo.isNetworkAvailable)
        let jindoActivation = try XCTUnwrap(jindo.activation)
        XCTAssertEqual(jindoActivation.protocolLimits.values, [4])
        let jindoTightening = try XCTUnwrap(
            jindoActivation.pendingProtocolLimitsTightening
        )
        XCTAssertEqual(jindoTightening.scheduledAtHeight, 2)
        XCTAssertEqual(jindoTightening.effectiveAtHeight, 302)
        XCTAssertEqual(jindoTightening.nextLimits.values, [3])
        XCTAssertEqual(
            jindoActivation.canonicalNorito,
            activationForJindoWithPendingTightening()
        )
    }

    func testActiveProtocolWithoutSingletonEvidenceIsUnavailable() throws {
        let fixture = makeFixture(includeQualification: false)
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest,
            nativeCatalogArchive: fixture.catalog
        )
        XCTAssertNil(manifest.qualification)
        XCTAssertEqual(
            manifest.row(for: .zkAcePqAuthorizationV1).readiness,
            .unavailable(.missingProductionQualification)
        )
    }

    func testMismatchedSingletonEvidenceCanOnlyDeriveInvalidReadiness() throws {
        let invalid = makeFixture(
            rowZeroReadiness: enumValue(1, enumValue(6)),
            committedHeight: 4,
            qualificationActivationHeight: 3,
            qualificationConvergenceHeight: 4
        )
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            invalid.manifest,
            nativeCatalogArchive: invalid.catalog
        )
        XCTAssertEqual(
            manifest.row(for: .zkAcePqAuthorizationV1).readiness,
            .unavailable(.invalidProductionQualification)
        )

        let forged = makeFixture(
            committedHeight: 4,
            qualificationActivationHeight: 3,
            qualificationConvergenceHeight: 4
        )
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            forged.manifest,
            nativeCatalogArchive: forged.catalog
        ))
    }

    func testEveryTruncationAndOneByteSuffixFailClosed() throws {
        let fixture = makeFixture()
        for end in 0..<fixture.manifest.count {
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                Data(fixture.manifest.prefix(end)),
                nativeCatalogArchive: fixture.catalog
            ), "accepted truncation at byte \(end)")
        }
        for suffix in UInt8.min...UInt8.max {
            var hostile = fixture.manifest
            hostile.append(suffix)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                hostile,
                nativeCatalogArchive: fixture.catalog
            ), "accepted suffix byte \(suffix)")
        }
    }

    func testSemanticShellsWithRecomputedOuterFramingFailClosed() throws {
        let cases: [(String, Fixture)] = [
            (
                "readiness projection",
                makeFixture(rowZeroReadiness: enumValue(1, enumValue(5)))
            ),
            ("experimental readiness", makeFixture(rowZeroReadiness: enumValue(1))),
            ("consensus ceiling", makeFixture(maxActionsPerTransaction: 2)),
            ("activation tuple", makeFixture(activationDigestByte: 0x32)),
            ("security claim digest", makeFixture(corruptSecurityClaimDigest: true)),
            ("reordered rows", makeFixture(swapFirstRows: true)),
        ]
        for (label, fixture) in cases {
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest,
                nativeCatalogArchive: fixture.catalog
            ), "accepted hostile \(label)")
        }

        let fixture = makeFixture()
        let frame = try XCTUnwrap(noritoDecodeFrame(fixture.manifest))
        var overlong = frame.payload
        XCTAssertEqual(overlong.removeFirst(), 4)
        overlong.insert(contentsOf: [0x84, 0x00], at: overlong.startIndex)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            manifestFrame(overlong),
            nativeCatalogArchive: fixture.catalog
        ))

        var nestedSuffix = frame.payload
        nestedSuffix.append(0)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            manifestFrame(nestedSuffix),
            nativeCatalogArchive: fixture.catalog
        ))
    }

    func testOldNineFieldRowsAndExperimentalAssuranceFailClosed() throws {
        for fixture in [
            makeFixture(useRetiredNineFieldRows: true),
            makeFixture(useRetiredExperimentalAssurance: true),
        ] {
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest,
                nativeCatalogArchive: fixture.catalog
            ))
        }
    }

    func testCatalogSubstitutionAndDigestShellsFailClosed() throws {
        let fixture = makeFixture()
        let substituted = makeFixture(catalogDigestByte: 0x32)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest,
            nativeCatalogArchive: substituted.catalog
        )) { error in
            XCTAssertEqual(
                error as? PrivacyExact12CapabilityManifestErrorV1,
                .compiledTupleMismatch(.zkAcePqAuthorizationV1)
            )
        }

        for digest in [Data(repeating: 0, count: 32), Data(repeating: 0xa5, count: 32)] {
            let shell = makeFixture(embeddedDigest: digest)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                shell.manifest,
                nativeCatalogArchive: shell.catalog
            ))
        }
    }

    func testSubmitProofConstructionBindsProtocolAndFullCompiledTuple() throws {
        let fixture = makeFixture()
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest,
            nativeCatalogArchive: fixture.catalog
        )
        let row = manifest.row(for: .zkAcePqAuthorizationV1)
        let limits = manifest.consensusPolicy.currentLimits
        try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
            submitProofInstruction(),
            row: row,
            consensusLimits: limits
        )

        let hostile = [
            submitProofInstruction(protocolTag: 1),
            submitProofInstruction(proofSystemTag: 1),
            submitProofInstruction(engineTag: 1),
            submitProofInstruction(parameterIdByte: 0x32),
            submitProofInstruction(statementDigestByte: 0),
            submitProofInstruction(statementTag: 1),
            submitProofInstruction(proofTag: 1),
        ]
        for instruction in hostile {
            XCTAssertThrowsError(
                try PrivacyExact12CapabilityManifestCodecV1
                    .requireSubmitProofInstruction(
                        instruction,
                        row: row,
                        consensusLimits: limits
                    )
            )
        }
    }

    func testGenericInstructionConstructionCannotBypassPrivacyAdmission() throws {
        let frame = noritoEncode(
            typeName: "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
            payload: Data(),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
        XCTAssertThrowsError(try TransactionInstructionFrame(
            wireName: PrivacyExact12FixtureCodecV1.submitProofWireId,
            framedPayload: frame
        )) { error in
            XCTAssertEqual(
                error as? ExecutableBatchInputError,
                .privacyExact12CapabilityAdmissionRequired
            )
        }

        let ordinary = try TransactionInstructionFrame(
            wireName: "iroha.test.non_privacy.v1",
            framedPayload: frame
        )
        XCTAssertFalse(try ordinary.compactInstructionBoxPayload().isEmpty)
    }

    func testMissingOrStaleNativeArtifactRejectsManifestValidation() throws {
        guard !PrivacyNativeBridge.isNativeAvailable else { return }
        let fixture = makeFixture()
        XCTAssertThrowsError(try PrivacyNativeBridge.validateExact12CapabilityManifestV1(
            fixture.manifest
        )) { error in
            XCTAssertEqual(
                error as? PrivacyExact12CapabilityManifestErrorV1,
                .nativeUnavailable
            )
        }
    }

    private func makeFixture(
        rowZeroReadiness: Data? = nil,
        maxActionsPerTransaction: UInt32 = 1,
        activationDigestByte: UInt8 = 0x31,
        catalogDigestByte: UInt8 = 0x31,
        includeQualification: Bool = true,
        corruptSecurityClaimDigest: Bool = false,
        committedHeight: UInt64 = 3,
        qualificationActivationHeight: UInt64 = 2,
        qualificationConvergenceHeight: UInt64 = 3,
        useRetiredNineFieldRows: Bool = false,
        useRetiredExperimentalAssurance: Bool = false,
        swapFirstRows: Bool = false,
        embeddedDigest: Data? = nil,
        includePendingState: Bool = false
    ) -> Fixture {
        var profiles = (0..<12).map { _ in enumValue(1, enumValue(0)) }
        profiles[0] = availableProfile(
            protocolTag: 0,
            proofSystemTag: 0,
            engineTag: 0,
            digestByte: 0x31,
            limits: enumValue(0)
        )
        profiles[6] = availableProfile(
            protocolTag: 6,
            proofSystemTag: 5,
            engineTag: 5,
            digestByte: 0x61,
            limits: enumValue(6, structure(u32(4)))
        )

        var catalogProfiles = profiles
        if catalogDigestByte != 0x31 {
            catalogProfiles[0] = availableProfile(
                protocolTag: 0,
                proofSystemTag: 0,
                engineTag: 0,
                digestByte: catalogDigestByte,
                limits: enumValue(0)
            )
        }

        var rows: [Data] = []
        for index in 0..<12 {
            let readiness: Data
            if index == 0 {
                readiness = rowZeroReadiness ?? (
                    includeQualification
                        ? enumValue(0)
                        : enumValue(1, enumValue(5))
                )
            } else if index == 6 {
                readiness = includePendingState
                    ? (includeQualification ? enumValue(0) : enumValue(1, enumValue(5)))
                    : enumValue(1, enumValue(1))
            } else {
                readiness = enumValue(1, enumValue(0, enumValue(0)))
            }
            let activation: Data
            if index == 0 {
                activation = option(activationForProfileZero(
                    digestByte: activationDigestByte,
                    useRetiredExperimentalAssurance: useRetiredExperimentalAssurance
                ))
            } else if index == 6, includePendingState {
                activation = option(activationForJindoWithPendingTightening())
            } else {
                activation = option(nil)
            }
            let commonFields = structure(
                enumValue(UInt32(index)),
                enumValue(UInt32(index)),
                enumValue(executionModes[index]),
                structure(Data([featureMasks[index]])),
                profiles[index],
                readiness
            )
            var encodedRow = commonFields
            if useRetiredNineFieldRows {
                appendField(
                    enumValue(index == 0 || (index == 6 && includePendingState) ? 2 : 0),
                    to: &encodedRow
                )
                appendField(activation, to: &encodedRow)
                appendField(option(nil), to: &encodedRow)
            } else {
                appendField(activation, to: &encodedRow)
            }
            rows.append(encodedRow)
        }
        if swapFirstRows { rows.swapAt(0, 1) }

        let catalogRows = catalogProfiles.enumerated().map { index, profile in
            structure(enumValue(UInt32(index)), profile)
        }
        let catalog = noritoEncode(
            typeName: "iroha.privacy.compiled-profile-catalog.v1",
            payload: structure(u32(1), sequence(catalogRows)),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )

        let digest: Data
        let qualification = includeQualification
            ? exact12Qualification(
                corruptSecurityClaimDigest: corruptSecurityClaimDigest,
                firstActivationHeight: qualificationActivationHeight,
                convergenceHeight: qualificationConvergenceHeight
            )
            : nil
        if let embeddedDigest {
            digest = embeddedDigest
        } else {
            let normalized = buildManifest(
                rows: rows,
                qualification: qualification,
                digest: Data(repeating: 0, count: 32),
                maxActionsPerTransaction: maxActionsPerTransaction,
                includePendingState: includePendingState,
                committedHeight: committedHeight
            )
            var preimage = Data("iroha:privacy:exact12-capability-manifest:v1".utf8)
            preimage.append(u64(UInt64(normalized.count)))
            preimage.append(normalized)
            digest = Data(SHA256.hash(data: preimage))
        }
        return Fixture(
            manifest: buildManifest(
                rows: rows,
                qualification: qualification,
                digest: digest,
                maxActionsPerTransaction: maxActionsPerTransaction,
                includePendingState: includePendingState,
                committedHeight: committedHeight
            ),
            catalog: catalog
        )
    }

    private func availableProfile(
        protocolTag: UInt32,
        proofSystemTag: UInt32,
        engineTag: UInt32,
        digestByte: UInt8,
        limits: Data
    ) -> Data {
        let digest = structure(Data(repeating: digestByte, count: 32))
        return enumValue(0, structure(
            enumValue(protocolTag), enumValue(proofSystemTag), enumValue(engineTag),
            digest, digest, digest, digest, digest, limits
        ))
    }

    private func activationForProfileZero(
        digestByte: UInt8 = 0x31,
        useRetiredExperimentalAssurance: Bool = false
    ) -> Data {
        let digest = structure(Data(repeating: digestByte, count: 32))
        var activation = structure(
            enumValue(0), enumValue(0), enumValue(0),
            digest, digest, digest, digest, digest,
            enumValue(1, structure(u64(1), u64(2), u64(2))),
            enumValue(0), option(nil)
        )
        if useRetiredExperimentalAssurance {
            appendField(enumValue(0), to: &activation)
        }
        return activation
    }

    private func securityClaim(
        protocolId: PrivacyProtocolIdV1,
        digestByte: UInt8,
        corruptDigest: Bool = false
    ) -> (claim: Data, digest: Data) {
        let bindingDigest = structure(Data(repeating: digestByte, count: 32))
        let claim = structure(
            exact12CatalogCommitment(),
            enumValue(protocolId.noritoDiscriminant),
            enumValue(securityModelTag(protocolId)),
            u16(128),
            u16(128),
            bindingDigest,
            bindingDigest,
            structure(Data(repeating: 0xe1, count: 32)),
            structure(Data(repeating: 0xe2, count: 32))
        )
        let canonicalClaim = noritoEncode(
            typeName: "iroha_data_model::privacy::protocol::PrivacySecurityClaimV1",
            payload: claim,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 2
        )
        var preimage = Data("iroha:privacy:security-claim:v1".utf8)
        preimage.append(u64(UInt64(canonicalClaim.count)))
        preimage.append(canonicalClaim)
        var claimDigest = Data(SHA256.hash(data: preimage))
        if corruptDigest {
            claimDigest[claimDigest.startIndex] ^= 0xff
        }
        return (claim, claimDigest)
    }

    private func exact12Qualification(
        corruptSecurityClaimDigest: Bool,
        firstActivationHeight: UInt64,
        convergenceHeight: UInt64
    ) -> Data {
        let releaseDigest = Data(repeating: 0xe3, count: 32)
        let releaseBindings = PrivacyProtocolIdV1.allCases.enumerated().map { index, protocolId in
            releaseBinding(
                protocolId,
                digestByte: profileDigestByte(index),
                corruptSecurityClaimDigest: corruptSecurityClaimDigest && index == 0
            )
        }
        let source = structure(
            structure(Data(repeating: 0xa1, count: 32)),
            Data([1]),
            compactString("swift-test-toolchain"),
            structure(Data(repeating: 0xa2, count: 32)),
            structure(Data(repeating: 0xa3, count: 32))
        )
        let release = structure(
            u16(1),
            compactString("iroha-privacy-exact12-v1"),
            exact12CatalogCommitment(),
            source,
            u16(1),
            structure(Data(repeating: 0xa4, count: 32)),
            structure(Data(repeating: 0xa5, count: 32)),
            sequence([]),
            sequence(releaseBindings),
            sequence([]),
            sequence([]),
            sequence([]),
            sequence([]),
            structure(Data(repeating: 0xa6, count: 32)),
            sequence([]),
            structure(Data(repeating: 0xe2, count: 32)),
            sequence([]),
            structure(releaseDigest)
        )
        let activations = PrivacyProtocolIdV1.allCases.enumerated().map { index, protocolId in
            structure(
                enumValue(protocolId.noritoDiscriminant),
                u64(index == 0 ? firstActivationHeight : 2)
            )
        }
        let deployment = structure(
            u16(1),
            compactString("swift-test-chain"),
            structure(Data(repeating: 0xd0, count: 32)),
            structure(Data(repeating: 0xd0, count: 32)),
            structure(releaseDigest),
            structure(Data(repeating: 0xd1, count: 32)),
            sequence(activations),
            structure(Data(repeating: 0xd2, count: 32)),
            compactString("v1"),
            u64(convergenceHeight),
            structure(Data(repeating: 0xd3, count: 32)),
            sequence([]),
            sequence([]),
            structure(Data(repeating: 0xe4, count: 32))
        )
        return structure(release, deployment)
    }

    private func releaseBinding(
        _ protocolId: PrivacyProtocolIdV1,
        digestByte: UInt8,
        corruptSecurityClaimDigest: Bool
    ) -> Data {
        let digest = structure(Data(repeating: digestByte, count: 32))
        let claim = securityClaim(
            protocolId: protocolId,
            digestByte: digestByte,
            corruptDigest: corruptSecurityClaimDigest
        )
        return structure(
            enumValue(protocolId.noritoDiscriminant),
            enumValue(protocolId.expectedProofSystem.rawValue),
            enumValue(protocolId.expectedEngine.rawValue),
            digest,
            digest,
            digest,
            digest,
            digest,
            claim.claim,
            structure(claim.digest)
        )
    }

    private func profileDigestByte(_ index: Int) -> UInt8 {
        switch index {
        case 0: return 0x31
        case 6: return 0x61
        default: return UInt8(0x40 + index)
        }
    }

    private func securityModelTag(_ protocolId: PrivacyProtocolIdV1) -> UInt32 {
        switch protocolId {
        case .zkAcePqAuthorizationV1, .irohaJindoPolynomialCommitmentV1,
             .irohaBootleLanternAnoncredV1, .irohaIvmPrivateNoteStarkV1,
             .pqMaspStarkV1:
            return 0
        default:
            return 1
        }
    }

    private func exact12CatalogCommitment() -> Data {
        let words: [UInt64] = [
            0x7c30_a004_39f1_37e0,
            0x6b40_fb5c_d815_db00,
            0x49a9_4401_d272_97d7,
            0x2e34_8ea7_fdf3_f0de,
            0xfabf_bf7c_7865_7f74,
            0xffbb_e269_c311_4fc9,
        ]
        return words.reduce(into: Data()) { output, word in
            output.append(u64(word))
        }
    }

    private func submitProofInstruction(
        protocolTag: UInt32 = 0,
        proofSystemTag: UInt32 = 0,
        engineTag: UInt32 = 0,
        parameterIdByte: UInt8 = 0x31,
        statementDigestByte: UInt8 = 0x71,
        statementTag: UInt32 = 0,
        proofTag: UInt32 = 0
    ) -> Data {
        let profileDigest = structure(Data(repeating: 0x31, count: 32))
        let parameterId = structure(Data(repeating: parameterIdByte, count: 32))
        let statementDigest = structure(
            Data(repeating: statementDigestByte, count: 32)
        )
        let envelope = structure(
            enumValue(protocolTag),
            enumValue(proofSystemTag),
            enumValue(engineTag),
            parameterId,
            profileDigest,
            profileDigest,
            profileDigest,
            profileDigest,
            statementDigest,
            enumValue(statementTag, Data([1])),
            enumValue(proofTag, Data([1]))
        )
        return noritoEncode(
            typeName: "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
            payload: structure(envelope),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
    }

    private func activationForJindoWithPendingTightening() -> Data {
        let digest = structure(Data(repeating: 0x61, count: 32))
        return structure(
            enumValue(6), enumValue(5), enumValue(5),
            digest, digest, digest, digest, digest,
            enumValue(1, structure(u64(1), u64(2), u64(2))),
            enumValue(6, structure(u32(4))),
            option(structure(
                u64(2), u64(302), enumValue(6, structure(u32(3)))
            ))
        )
    }

    private func consensusPolicy(
        maxActionsPerTransaction: UInt32 = 1,
        includePendingState: Bool = false
    ) -> Data {
        let current = structure(
            u32(maxActionsPerTransaction), u32(2),
            u32(9 * 1024 * 1024), u32(9 * 1024 * 1024),
            u32(9 * 1024 * 1024), u32(18 * 1024 * 1024),
            u32(256 * 1024), u32(8), u32(8), u32(2_048)
        )
        let pending: Data?
        if includePendingState {
            pending = structure(
                u64(2),
                u64(302),
                structure(
                    u32(maxActionsPerTransaction), u32(2),
                    u32(9 * 1024 * 1024), u32(9 * 1024 * 1024),
                    u32(9 * 1024 * 1024), u32(18 * 1024 * 1024),
                    u32(256 * 1024), u32(8), u32(8), u32(1_024)
                )
            )
        } else {
            pending = nil
        }
        return structure(
            current,
            option(pending)
        )
    }

    private func buildManifest(
        rows: [Data],
        qualification: Data?,
        digest: Data,
        maxActionsPerTransaction: UInt32,
        includePendingState: Bool,
        committedHeight: UInt64
    ) -> Data {
        manifestFrame(structure(
            u32(1), u64(committedHeight),
            consensusPolicy(
                maxActionsPerTransaction: maxActionsPerTransaction,
                includePendingState: includePendingState
            ),
            option(qualification),
            sequence(rows), structure(digest)
        ))
    }

    private func manifestFrame(_ payload: Data) -> Data {
        noritoEncode(
            typeName: "iroha.privacy.exact12-capability-manifest.v1",
            payload: payload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
    }

    private func structure(_ fields: Data...) -> Data {
        var output = Data()
        for field in fields { appendField(field, to: &output) }
        return output
    }

    private func sequence(_ values: [Data]) -> Data {
        var output = u64(UInt64(values.count))
        for value in values { appendField(value, to: &output) }
        return output
    }

    private func enumValue(_ tag: UInt32, _ payload: Data? = nil) -> Data {
        var output = u32(tag)
        if let payload { appendField(payload, to: &output) }
        return output
    }

    private func option(_ value: Data?) -> Data {
        var output = Data([value == nil ? 0 : 1])
        if let value { appendField(value, to: &output) }
        return output
    }

    private func u32(_ value: UInt32) -> Data {
        var value = value.littleEndian
        return Data(bytes: &value, count: 4)
    }

    private func u16(_ value: UInt16) -> Data {
        var value = value.littleEndian
        return Data(bytes: &value, count: 2)
    }

    private func u64(_ value: UInt64) -> Data {
        var value = value.littleEndian
        return Data(bytes: &value, count: 8)
    }

    private func appendField(_ field: Data, to output: inout Data) {
        appendCompactLength(UInt64(field.count), to: &output)
        output.append(field)
    }

    private func compactString(_ value: String) -> Data {
        let bytes = Data(value.utf8)
        var output = Data()
        appendCompactLength(UInt64(bytes.count), to: &output)
        output.append(bytes)
        return output
    }

    private func appendCompactLength(_ value: UInt64, to output: inout Data) {
        var length = value
        while length >= 0x80 {
            output.append(UInt8(length & 0x7f) | 0x80)
            length >>= 7
        }
        output.append(UInt8(length))
    }

    private struct Fixture {
        let manifest: Data
        let catalog: Data
    }
}
