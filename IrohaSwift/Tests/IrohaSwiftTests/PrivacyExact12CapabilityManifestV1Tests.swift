import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyExact12CapabilityManifestV1Tests: XCTestCase {
    private let executionModes: [UInt32] = [0, 1, 2, 3, 4, 4, 2, 4, 5, 1, 5, 5]
    private let featureMasks: [UInt8] = [0, 6, 1, 2, 2, 2, 0, 2, 7, 2, 7, 31]
    private let networkBytes = Data(repeating: 0xd1, count: 32)

    func testNativeConstructionRejectsEmptyInputsWithoutMintingAdmission() throws {
        let network = try NetworkId(bytes: networkBytes)
        for (manifest, instruction) in [(Data(), Data([1])), (Data([1]), Data())] {
            XCTAssertThrowsError(try PrivacyNativeBridge.requireExact12SubmitProofConstructionV1(
                manifestArchive: manifest, protocolId: .zkAcePqAuthorizationV1,
                instructionArchive: instruction, expectedNetworkId: network)) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                               .invalidArchive("construction input is empty or exceeds its transport bound"))
            }
        }
    }

    func testSecurityClaimCanonicalArchiveMatchesCurrentRustOwner() throws {
        // Exact canonical archive emitted by the Rust fixture owner.
        let claim = securityClaim(protocolId: .zkAcePqAuthorizationV1, digestByte: 0x31)
        XCTAssertEqual(exact12CatalogCommitment().count, 32)
        let expected = try XCTUnwrap(Data(hexString:
            "4e525430000087368b9da35faa6d2dee8dd5dbdf70cf00b900000000000000d355b690039f4e5b02205bcf366f" +
            "b18d2e4361486c015da5b41a3fb00edf05611042ccc452120d176eb00400000000040000000002800002800021" +
            "203131313131313131313131313131313131313131313131313131313131313131212031313131313131313131" +
            "313131313131313131313131313131313131313131312120e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1e1" +
            "e1e1e1e1e1e1e1e1e1e1e12120e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2e2"
        ))
        XCTAssertEqual(
            noritoEncode(
                typeName: "iroha_data_model::privacy::PrivacySecurityClaimV1",
                payload: claim.claim,
                flags: NoritoHeader.compactLen,
                payloadAlignment: 2
            ),
            expected
        )
        XCTAssertEqual(
            claim.digest,
            try XCTUnwrap(Data(hexString: "fdfe96fbb683002b794b5ad83d6ce637359d145b6e8aea5275a82cfe6ab89efd"))
        )
    }

    func testManagedProjectionPreservesFieldsWithoutGrantingNativeAuthority() throws {
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
            1, 2, 9 * 1024 * 1024, 9_699_840, 9_699_840,
            16_711_680, 256 * 1024, 8, 8, 2_048,
        ]
        XCTAssertEqual(
            manifest.consensusPolicy.currentLimits.orderedValues,
            expectedConsensusLimits
        )
        let consensusUpdate = try XCTUnwrap(
            manifest.consensusPolicy.pendingUpdate
        )
        XCTAssertEqual(consensusUpdate.scheduledAtHeight, 2)
        XCTAssertEqual(consensusUpdate.effectiveAtHeight, 302)
        XCTAssertEqual(consensusUpdate.nextLimits.retainedRootCount, 1_024)
        XCTAssertEqual(
            manifest.consensusPolicy.canonicalNorito,
            consensusPolicy(includePendingState: true)
        )

        let first = manifest.row(for: .zkAcePqAuthorizationV1)
        XCTAssertTrue(first.localCompiledTupleMatches)
        XCTAssertTrue(first.isNetworkAvailable)
        XCTAssertEqual(first.readiness, .available)
        guard case let .available(profile) = first.compiledProfile else {
            return XCTFail("available row lost its complete canonical profile")
        }
        XCTAssertEqual(profile.protocolId, .zkAcePqAuthorizationV1)
        XCTAssertEqual(profile.proofSystemId, .starkFriSha3_384Goldilocks)
        XCTAssertEqual(profile.engineId, .nativeGoldilocksSha3_384StarkFri)
        XCTAssertEqual(profile.parameterId, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(profile.parameterDigest, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(profile.verifierDigest, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(profile.statementSchemaDigest, Data(repeating: 0x31, count: 32))
        XCTAssertEqual(profile.engineManifestDigest, Data(repeating: 0x31, count: 32))
        let policy = try XCTUnwrap(first.policy)
        XCTAssertEqual(policy.protocolId, .zkAcePqAuthorizationV1)
        XCTAssertEqual(policy.currentLimits.values, [])
        XCTAssertNil(policy.pendingUpdate)
        XCTAssertEqual(manifest.networkId.bytes, networkBytes)
        XCTAssertEqual(policy.canonicalNorito, policyForProfileZero())
        XCTAssertEqual(first.effectiveProtocolLimits, profile.protocolLimits)

        let jindo = manifest.row(for: .irohaJindoPolynomialCommitmentV1)
        XCTAssertEqual(jindo.readiness, .available)
        XCTAssertTrue(jindo.isNetworkAvailable)
        let jindoPolicy = try XCTUnwrap(jindo.policy)
        XCTAssertEqual(jindoPolicy.currentLimits.values, [4])
        let jindoUpdate = try XCTUnwrap(
            jindoPolicy.pendingUpdate
        )
        XCTAssertEqual(jindoUpdate.scheduledAtHeight, 2)
        XCTAssertEqual(jindoUpdate.effectiveAtHeight, 302)
        XCTAssertEqual(jindoUpdate.nextLimits.values, [3])
        XCTAssertEqual(
            jindoPolicy.canonicalNorito,
            policyForJindoWithPendingUpdate()
        )
    }

    func testPendingUpdateAcceptsNextBlockAndRejectsInvalidSnapshotHeights() throws {
        let cases: [(UInt64, UInt64, UInt64, Bool)] = [
            (3, 4, 3, true), (3, 5, 3, true),
            (UInt64.max - 1, UInt64.max, UInt64.max - 1, true),
            (0, 4, 3, false), (3, 3, 3, false), (3, 2, 3, false),
            (4, 5, 3, false), (3, 4, 4, false),
            (UInt64.max, UInt64.max, UInt64.max, false),
        ]
        for consensus in [true, false] {
            for (scheduled, effective, committed, accepted) in cases {
                let valid = (committed, committed == UInt64.max ? UInt64.max : committed + 1)
                let fixture = makeFixture(
                    committedHeight: committed,
                    includePendingState: true,
                    consensusSchedule: consensus ? (scheduled, effective) : valid,
                    protocolSchedule: consensus ? valid : (scheduled, effective)
                )
                if accepted {
                    let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
                        fixture.manifest, nativeCatalogArchive: fixture.catalog
                    )
                    XCTAssertEqual(manifest.canonicalBytes(), fixture.manifest)
                    XCTAssertTrue(manifest.row(for: .irohaJindoPolynomialCommitmentV1).isNetworkAvailable)
                } else {
                    XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                        fixture.manifest, nativeCatalogArchive: fixture.catalog
                    ))
                }
            }
        }
    }

    func testPolicyAbsenceNeverDisablesCanonicalImplementation() throws {
        for height in [UInt64(0), UInt64(1), UInt64(3), UInt64.max] {
            let fixture = makeFixture(omitRowZeroPolicy: true, committedHeight: height)
            let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog)
            let row = manifest.row(for: .zkAcePqAuthorizationV1)
            XCTAssertNil(row.policy)
            XCTAssertEqual(row.readiness, .available)
            guard case let .available(profile) = row.compiledProfile else {
                return XCTFail("canonical profile was lost")
            }
            XCTAssertEqual(row.effectiveProtocolLimits, profile.protocolLimits)
            XCTAssertNoThrow(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
                manifest, protocolId: row.protocolId))
        }
    }

    func testProtocolUpdatesAllowChangedBoundedValuesInEitherDirection() throws {
        for (current, next, accepted) in [(4, 3, true), (3, 4, true), (3, 3, false),
                                         (4, 5, false), (0, 3, false), (3, 0, false)] {
            let policy = policyForJindoWithPendingUpdate(current: UInt32(current), next: UInt32(next))
            let fixture = makeFixture(jindoPolicyOverride: policy)
            if accepted {
                let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
                    fixture.manifest, nativeCatalogArchive: fixture.catalog)
                let row = manifest.row(for: .irohaJindoPolynomialCommitmentV1)
                XCTAssertEqual(row.effectiveProtocolLimits?.values, [UInt32(current)])
                XCTAssertEqual(row.policy?.pendingUpdate?.nextLimits.values, [UInt32(next)])
            } else {
                XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                    fixture.manifest, nativeCatalogArchive: fixture.catalog))
            }
        }
    }

    func testRetiredLifecycleReasonsAndActivationRecordAreRejected() throws {
        for tag in UInt32(1)...UInt32(4) {
            let fixture = makeFixture(rowZeroReadiness: enumValue(1, enumValue(tag)))
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog))
        }
        let digest = structure(Data(repeating: 0x31, count: 32))
        let retiredActivation = structure(
            enumValue(0), enumValue(0), enumValue(0), digest, digest, digest, digest, digest,
            enumValue(1, structure(u64(1), u64(2), u64(2))), enumValue(0), option(nil))
        for malformed in [retiredActivation, policyForProfileZero() + Data([0]),
                          structure(enumValue(0), enumValue(1, structure(u32(16), u32(1))), option(nil))] {
            let fixture = makeFixture(rowZeroPolicy: malformed)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog))
        }
    }

    func testConsensusUpdatesAllowChangedBoundedValuesInEitherDirection() throws {
        for (current, next, accepted) in [(2_048, 1_024, true), (1_024, 2_048, true),
                                         (1_024, 1_024, false), (1_024, 2_049, false)] {
            let policy = consensusPolicy(includePendingState: true,
                                         currentRetained: UInt32(current), nextRetained: UInt32(next))
            let fixture = makeFixture(consensusPolicyOverride: policy)
            if accepted {
                let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
                    fixture.manifest, nativeCatalogArchive: fixture.catalog)
                XCTAssertEqual(manifest.consensusPolicy.currentLimits.retainedRootCount, UInt32(current))
                XCTAssertEqual(manifest.consensusPolicy.pendingUpdate?.nextLimits.retainedRootCount, UInt32(next))
            } else {
                XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                    fixture.manifest, nativeCatalogArchive: fixture.catalog))
            }
        }
    }

    func testSupportedProtocolIsAvailableWhileAnotherConcreteImplementationIsUnavailable() throws {
        let fixture = makeFixture(includePendingState: true)
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: fixture.catalog
        )
        XCTAssertEqual(manifest.networkId.bytes, networkBytes)
        XCTAssertEqual(manifest.row(for: .zkAcePqAuthorizationV1).readiness, .available)
        XCTAssertEqual(manifest.row(for: .irohaJindoPolynomialCommitmentV1).readiness, .available)
        let x509 = manifest.row(for: .irohaZkX509StarkP256V1)
        XCTAssertEqual(x509.compiledProfile, .unavailable(.jointSecurityReductionIncomplete))
        XCTAssertEqual(x509.readiness, .unavailable(.compiledProfile(.jointSecurityReductionIncomplete)))
        XCTAssertFalse(x509.isNetworkAvailable)
        XCTAssertNil(x509.policy)
        // Managed projection remains a byte check, even for an independently available row.
        XCTAssertThrowsError(try PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
            manifest, protocolId: .zkAcePqAuthorizationV1
        ))
    }

    func testRemovedDocumentaryFieldAndArchiveOverflowAreRejected() throws {
        for removed in [Data([0]), Data([1]), Data(repeating: 0, count: 130 * 1024)] {
            let fixture = makeFixture(removedQualificationField: removed)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            ))
        }
        let fixture = makeFixture()
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            Data(repeating: 0, count: PrivacyExact12CapabilityManifestV1.maximumArchiveBytes + 1),
            nativeCatalogArchive: fixture.catalog
        ))
    }

    func testStandaloneProjectionCannotMintAuthenticatedNetworkAdmission() throws {
        let fixture = makeFixture()
        let projection = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest,
            nativeCatalogArchive: fixture.catalog
        )
        XCTAssertThrowsError(try PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
            projection,
            protocolId: .zkAcePqAuthorizationV1
        )) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1, .invalidAdmission)
        }
    }

    func testRetiredDocumentaryReadinessReasonsAndForgedUnavailableRowsAreRejected() throws {
        for reason in [UInt32(5), 6] {
            let fixture = makeFixture(rowZeroReadiness: enumValue(1, enumValue(reason)))
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            ))
        }
        for reason in [enumValue(3, Data([0])), enumValue(4)] {
            let fixture = makeFixture(x509UnavailableReason: reason)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            ))
        }
    }

    func testManifestRejectsElevenAndThirteenRowsBeforeProjection() throws {
        for count in [11, 13] {
            let fixture = makeFixture(manifestRowCount: count)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            )) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                               .invalidArchive("protocols must contain exactly 12 rows"))
            }
        }
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
            ("policy protocol", makeFixture(policyProtocolTag: 1)),
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

    func testUnavailableImplementationStaysUnavailableWithOrdinaryPolicy() throws {
        let available = makeFixture(x509Readiness: enumValue(0))
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            available.manifest, nativeCatalogArchive: available.catalog)) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                           .invalidArchive("readiness was not derived from the canonical implementation"))
        }
        let policy = structure(enumValue(5), enumValue(5), option(nil))
        let fixture = makeFixture(x509Policy: policy)
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: fixture.catalog)
        let row = manifest.row(for: .irohaZkX509StarkP256V1)
        XCTAssertEqual(row.policy?.canonicalNorito, policy)
        XCTAssertEqual(row.readiness, .unavailable(.compiledProfile(.jointSecurityReductionIncomplete)))
        XCTAssertNil(row.effectiveProtocolLimits)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
            manifest, protocolId: .irohaZkX509StarkP256V1))
        let wrongPolicy = makeFixture(x509Policy: policyForProfileZero())
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
            wrongPolicy.manifest, nativeCatalogArchive: wrongPolicy.catalog))
    }

    func testCatalogSubstitutionAndDigestShellsFailClosed() throws {
        let fixture = makeFixture()
        let substituted = makeFixture(catalogDigestByte: 0x32)
        let projection = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: substituted.catalog
        )
        XCTAssertFalse(projection.row(for: .zkAcePqAuthorizationV1).localCompiledTupleMatches)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
            projection, protocolId: .zkAcePqAuthorizationV1
        )) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                           .compiledTupleMismatch(.zkAcePqAuthorizationV1))
        }

        for digest in [Data(repeating: 0, count: 32), Data(repeating: 0xa5, count: 32)] {
            let shell = makeFixture(embeddedDigest: digest)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                shell.manifest,
                nativeCatalogArchive: shell.catalog
            ))
        }
    }

    func testSelectedProtocolDoesNotDependOnAnUnrelatedLocalTuple() throws {
        let fixture = makeFixture(includePendingState: true)
        let otherLocalCatalog = makeFixture(catalogDigestByte: 0x32).catalog
        let projection = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: otherLocalCatalog
        )
        let jindo = try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
            projection, protocolId: .irohaJindoPolynomialCommitmentV1
        )
        XCTAssertEqual(jindo.protocolId, .irohaJindoPolynomialCommitmentV1)
        XCTAssertTrue(jindo.localCompiledTupleMatches)
        XCTAssertEqual(jindo.readiness, .available)
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
            projection, protocolId: .zkAcePqAuthorizationV1
        )) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                           .compiledTupleMismatch(.zkAcePqAuthorizationV1))
        }
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
            projection, protocolId: .irohaZkX509StarkP256V1
        )) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                           .unavailableProtocol(.irohaZkX509StarkP256V1))
        }
        // The same selected-row predicate runs after authenticated native validation;
        // calling this projection helper alone cannot issue the opaque admission.
        XCTAssertThrowsError(try PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
            projection, protocolId: .irohaJindoPolynomialCommitmentV1
        )) { error in
            XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1, .invalidAdmission)
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
            consensusLimits: limits,
            expectedNetworkId: try NetworkId(bytes: networkBytes)
        )

        let hostile = [
            submitProofInstruction(protocolTag: 1),
            submitProofInstruction(proofSystemTag: 1),
            submitProofInstruction(engineTag: 1),
            submitProofInstruction(parameterIdByte: 0x32),
            submitProofInstruction(statementDigestByte: 0),
            submitProofInstruction(statementTag: 1),
            submitProofInstruction(proofTag: 1),
            submitProofInstruction(statementNetworkBytes: Data(repeating: 0xd3, count: 32)),
            submitProofInstruction(wireMagic: Data(repeating: 0, count: 8)),
            submitProofInstruction(catalogCommitment: Data(repeating: 0, count: 48)),
            submitProofInstruction(retiredElevenFieldEnvelope: true),
        ]
        for instruction in hostile {
            XCTAssertThrowsError(
                try PrivacyExact12CapabilityManifestCodecV1
                    .requireSubmitProofInstruction(
                        instruction,
                        row: row,
                        consensusLimits: limits,
                        expectedNetworkId: try NetworkId(bytes: networkBytes)
                    )
            )
        }
    }

    func testSubmitProofChargesCanonicalEnvelopeWithoutItsInstructionWrapper() throws {
        let fixture = makeFixture()
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: fixture.catalog
        )
        let archive = submitProofInstruction()
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        var prefixBytes = 0
        for byte in frame.payload {
            prefixBytes += 1
            if byte & 0x80 == 0 { break }
        }
        // The synthetic projection has no valid proof; this checks framing only.
        let envelopeCharge = UInt32(archive.count - prefixBytes)
        let defaults = manifest.consensusPolicy.currentLimits
        func limits(_ actionBytes: UInt32) -> PrivacyConsensusLimitsV1 {
            PrivacyConsensusLimitsV1(
                maxActionsPerTransaction: 1, maxActionsPerBlock: 2,
                maxProofBytesPerAction: 1, maxActionBytes: actionBytes,
                maxPrivacyBytesPerTransaction: defaults.maxPrivacyBytesPerTransaction,
                maxPrivacyBytesPerBlock: defaults.maxPrivacyBytesPerBlock,
                maxStatementAndEncryptedOutputBytesPerTransaction: 1,
                maxNullifiersPerAction: 8, maxCommitmentsPerAction: 8, retainedRootCount: 2048
            )
        }
        let row = manifest.row(for: .zkAcePqAuthorizationV1)
        let network = try NetworkId(bytes: networkBytes)
        XCTAssertGreaterThan(archive.count, Int(envelopeCharge))
        try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
            archive, row: row, consensusLimits: limits(envelopeCharge), expectedNetworkId: network
        )
        XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
            archive, row: row, consensusLimits: limits(envelopeCharge - 1), expectedNetworkId: network
        ))
    }

    func testFinalEnvelopeRequiresEveryWireMarkerAndCatalogByte() throws {
        let fixture = makeFixture()
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: fixture.catalog
        )
        let expected = try NetworkId(bytes: networkBytes)
        for index in 0..<32 {
            var catalog = exact12CatalogCommitment()
            catalog[index] ^= 1
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
                submitProofInstruction(catalogCommitment: catalog),
                row: manifest.row(for: .zkAcePqAuthorizationV1),
                consensusLimits: manifest.consensusPolicy.currentLimits,
                expectedNetworkId: expected
            ))
        }
        let canonical = exact12CatalogCommitment()
        XCTAssertEqual(canonical.count, 32)
        for hostile in [
            Data(canonical.dropLast()), canonical + Data([0]),
            canonical + Data(repeating: 0, count: 16),
            Data([0xe0, 0x37, 0xf1, 0x39, 0x04, 0xa0, 0x30, 0x7c, 0x00, 0xdb, 0x15, 0xd8, 0x5c, 0xfb, 0x40, 0x6b, 0xd7, 0x97, 0x72, 0xd2, 0x01, 0x44, 0xa9, 0x49, 0xde, 0xf0, 0xf3, 0xfd, 0xa7, 0x8e, 0x34, 0x2e, 0x74, 0x7f, 0x65, 0x78, 0x7c, 0xbf, 0xbf, 0xfa, 0xc9, 0x4f, 0x11, 0xc3, 0x69, 0xe2, 0xbb, 0xff]),
        ] {
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
                submitProofInstruction(catalogCommitment: hostile),
                row: manifest.row(for: .zkAcePqAuthorizationV1),
                consensusLimits: manifest.consensusPolicy.currentLimits,
                expectedNetworkId: expected
            ))
        }
        for index in 0..<8 {
            var marker = Data([0x49, 0x52, 0x48, 0x5a, 0x4b, 0x31, 0xa5, 0x5a])
            marker[index] ^= 1
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
                submitProofInstruction(wireMagic: marker),
                row: manifest.row(for: .zkAcePqAuthorizationV1),
                consensusLimits: manifest.consensusPolicy.currentLimits,
                expectedNetworkId: expected
            ))
        }
    }

    func testAuthoritativeSnapshotBindsEveryNetworkByteWithoutGrantingAuthority() throws {
        let expected = try NetworkId(bytes: networkBytes)
        let fixture = makeFixture()
        let manifest = try PrivacyExact12CapabilityManifestCodecV1.decode(
            fixture.manifest, nativeCatalogArchive: fixture.catalog
        )
        try PrivacyExact12CapabilityManifestCodecV1.requireNetworkV1(manifest, expectedNetworkId: expected)
        // Recomputing the unsigned manifest digest never changes the expected network.
        for index in 0..<32 {
            var changed = networkBytes
            changed[index] ^= 2
            let other = try NetworkId(bytes: changed)
            let moved = makeFixture(manifestNetworkBytes: changed)
            let projection = try PrivacyExact12CapabilityManifestCodecV1.decode(
                moved.manifest, nativeCatalogArchive: moved.catalog
            )
            XCTAssertEqual(projection.networkId, other)
            XCTAssertNotEqual(projection.manifestDigest, manifest.manifestDigest)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireNetworkV1(
                projection, expectedNetworkId: expected
            ))
            XCTAssertThrowsError(try PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
                projection, protocolId: .zkAcePqAuthorizationV1
            )) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1, .invalidAdmission)
            }
        }
    }

    func testSnapshotRejectsMissingOrNoncanonicalNetworkAndOldManifestLayout() throws {
        for invalid in [Data(), Data(repeating: 0xd1, count: 31),
                        Data(repeating: 0xd1, count: 33), structure(networkBytes),
                        Data(repeating: 0, count: 32), Data(repeating: 0xd0, count: 32)] {
            let fixture = makeFixture(manifestNetworkBytes: invalid)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            ))
        }
        for fixture in [makeFixture(omitNetwork: true),
                        makeFixture(removedQualificationField: Data([0]), omitNetwork: true)] {
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.decode(
                fixture.manifest, nativeCatalogArchive: fixture.catalog
            ))
        }
    }

    func testAllTwelveStatementContextsRequireExactExpectedNetwork() throws {
        let expected = try NetworkId(bytes: networkBytes)
        for protocolId in PrivacyProtocolIdV1.allCases {
            try PrivacyExact12CapabilityManifestCodecV1.requireStatementNetworkV1(
                syntheticStatement(tag: protocolId.noritoDiscriminant, network: networkBytes),
                protocolId: protocolId, expectedNetworkId: expected
            )
            for index in 0..<32 {
                var changed = networkBytes
                changed[index] ^= 2
                XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireStatementNetworkV1(
                    syntheticStatement(tag: protocolId.noritoDiscriminant, network: changed),
                    protocolId: protocolId, expectedNetworkId: expected
                ))
            }
            for invalid in [Data(), Data(repeating: 0xd1, count: 31),
                            Data(repeating: 0xd1, count: 33), structure(networkBytes),
                            Data(repeating: 0xd0, count: 32)] {
                XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireStatementNetworkV1(
                    syntheticStatement(tag: protocolId.noritoDiscriminant, network: invalid),
                    protocolId: protocolId, expectedNetworkId: expected
                ))
            }
            for malformed in [
                enumValue(protocolId.noritoDiscriminant, Data([1])),
                enumValue(protocolId.noritoDiscriminant, structure(structure(networkBytes))),
                syntheticStatement(tag: protocolId.noritoDiscriminant, network: networkBytes,
                                   contextSuffix: Data([0])),
                syntheticStatement(tag: (protocolId.noritoDiscriminant + 1) % 12, network: networkBytes),
            ] {
                XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireStatementNetworkV1(
                    malformed, protocolId: protocolId, expectedNetworkId: expected
                ))
            }
        }
    }

    func testGenericInstructionConstructionCannotBypassPrivacyAdmission() throws {
        let frame = noritoEncode(
            typeName: "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
            payload: Data(),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
        for wire in [PrivacyExact12FixtureCodecV1.submitProofWireId,
                     PrivacyExact12FixtureCodecV1.submitX509WireId] {
            XCTAssertThrowsError(try TransactionInstructionFrame(
                wireName: wire,
                framedPayload: frame
            )) { error in
                XCTAssertEqual(
                    error as? ExecutableBatchInputError,
                    .privacyExact12CapabilityAdmissionRequired
                )
            }
        }

        let ordinary = try TransactionInstructionFrame(
            wireName: "iroha.test.non_privacy.v1",
            framedPayload: frame
        )
        let ordinaryBytes = try ordinary.compactInstructionBoxPayload()
        XCTAssertFalse(ordinaryBytes.isEmpty)
        XCTAssertEqual(
            try ordinary.compactInstructionBoxPayload(expectedNetworkId: NetworkId(bytes: networkBytes)),
            ordinaryBytes
        )
    }

    func testCapabilityRequestBypassesCachesAndPreservesExactCanonicalAuthentication() throws {
        let network = try NetworkId(bytes: networkBytes)
        let privateKey = Data(repeating: 0x11, count: 32)
        let account = try Keypair(privateKeyBytes: privateKey)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
        let auth = ToriiCanonicalRequestAuth(
            accountId: account, privateKey: privateKey
        )
        let client = ToriiClient(
            baseURL: URL(string: "https://example.invalid")!,
            defaultHeaders: ["cache-control": "max-age=3600", "Accept": "application/json"],
            localSigningContext: ToriiLocalSigningContext(networkId: network),
            canonicalRequestFreshness: ToriiCanonicalRequestFreshness(
                timestampMs: { 1_700_000_000_000 },
                nonce: { "privacy-cache-policy" }
            )
        )
        // This returns only a request, never a validated manifest or admission.
        let request = try client.makePrivacyExact12CapabilityRequestV1(canonicalAuth: auth)
        let url = try XCTUnwrap(request.url)
        XCTAssertEqual(url.absoluteString, "https://example.invalid/v1/privacy/capabilities")
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertNil(request.httpBody)
        XCTAssertEqual(request.cachePolicy, .reloadIgnoringLocalCacheData)
        XCTAssertEqual(request.value(forHTTPHeaderField: "Cache-Control"), "no-cache, no-store")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
        let expectedHeaders = try ToriiCanonicalRequest.buildHeaders(
            method: "GET", url: url, body: nil, accountId: account, privateKey: privateKey,
            networkId: network, timestampMs: 1_700_000_000_000, nonce: "privacy-cache-policy"
        )
        for (header, value) in expectedHeaders where header != ToriiCanonicalRequest.headerSignature {
            XCTAssertEqual(request.value(forHTTPHeaderField: header), value, header)
        }
        let signature = try XCTUnwrap(Data(base64Encoded: XCTUnwrap(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
        )))
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKey).publicKey
        let message = try ToriiCanonicalRequest.signatureMessage(
            networkId: network, method: "GET", url: url, body: nil,
            timestampMs: 1_700_000_000_000, nonce: "privacy-cache-policy"
        )
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
        let changedBodyMessage = try ToriiCanonicalRequest.signatureMessage(
            networkId: network, method: "GET", url: url, body: Data([1]),
            timestampMs: 1_700_000_000_000, nonce: "privacy-cache-policy"
        )
        XCTAssertFalse(publicKey.isValidSignature(signature, for: changedBodyMessage))
        let changedNetworkMessage = try ToriiCanonicalRequest.signatureMessage(
            networkId: NetworkId(bytes: Data(repeating: 0xd3, count: 32)),
            method: "GET", url: url, body: nil,
            timestampMs: 1_700_000_000_000, nonce: "privacy-cache-policy"
        )
        XCTAssertFalse(publicKey.isValidSignature(signature, for: changedNetworkMessage))
    }

    func testToriiManifestRequiresLocalSigningNetworkBeforeNativeOrTransport() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [UnexpectedPrivacyCapabilityTransport.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let client = ToriiClient(baseURL: URL(string: "https://example.invalid")!, session: session)
        // The guard must run before authentication, native validation, or transport.
        let auth = ToriiCanonicalRequestAuth(
            accountId: "unused-before-network-guard", privateKey: Data(repeating: 1, count: 32)
        )
        do {
            _ = try await client.getPrivacyExact12CapabilityManifestV1(canonicalAuth: auth)
            XCTFail("missing expected network must reject")
        } catch ToriiClientError.invalidPayload(let message) {
            XCTAssertEqual(message, "privacy capability authority requires the expected local signing network")
        }
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

    func testNativeProjectionCannotAdmitFabricatedSelectedTuples() throws {
        let fixture = makeFixture()
        if PrivacyNativeBridge.isNativeAvailable {
            // Structural native validation accepts this canonical model; the actual
            // local catalog exposes the selected tuple mismatch without gating others.
            let projection = try PrivacyNativeBridge.validateExact12CapabilityManifestV1(fixture.manifest)
            XCTAssertFalse(projection.row(for: .zkAcePqAuthorizationV1).localCompiledTupleMatches)
            XCTAssertThrowsError(try PrivacyExact12CapabilityManifestCodecV1.requireAvailableLocalRowV1(
                projection, protocolId: .zkAcePqAuthorizationV1
            )) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1,
                               .compiledTupleMismatch(.zkAcePqAuthorizationV1))
            }
            XCTAssertThrowsError(try PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
                projection, protocolId: .zkAcePqAuthorizationV1
            )) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1, .invalidAdmission)
            }
        } else {
            XCTAssertThrowsError(try PrivacyNativeBridge.validateExact12CapabilityManifestV1(fixture.manifest)) { error in
                XCTAssertEqual(error as? PrivacyExact12CapabilityManifestErrorV1, .nativeUnavailable)
            }
        }
    }

    private func makeFixture(
        rowZeroReadiness: Data? = nil,
        rowZeroPolicy: Data? = nil,
        omitRowZeroPolicy: Bool = false,
        maxActionsPerTransaction: UInt32 = 1,
        policyProtocolTag: UInt32 = 0,
        catalogDigestByte: UInt8 = 0x31,
        committedHeight: UInt64 = 3,
        useRetiredNineFieldRows: Bool = false,
        useRetiredExperimentalAssurance: Bool = false,
        swapFirstRows: Bool = false,
        manifestRowCount: Int = 12,
        embeddedDigest: Data? = nil,
        includePendingState: Bool = false,
        consensusSchedule: (UInt64, UInt64) = (2, 302),
        protocolSchedule: (UInt64, UInt64) = (2, 302),
        manifestNetworkBytes: Data? = nil,
        removedQualificationField: Data? = nil,
        omitNetwork: Bool = false,
        x509UnavailableReason: Data? = nil,
        x509Readiness: Data? = nil,
        x509Policy: Data? = nil,
        jindoPolicyOverride: Data? = nil,
        consensusPolicyOverride: Data? = nil
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

        let x509Reason = x509UnavailableReason ?? enumValue(3)
        profiles[5] = enumValue(1, x509Reason)

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
                readiness = rowZeroReadiness ?? enumValue(0)
            } else if index == 5 {
                readiness = x509Readiness ?? enumValue(1, enumValue(0, x509Reason))
            } else if index == 6 {
                readiness = enumValue(0)
            } else {
                readiness = enumValue(1, enumValue(0, enumValue(0)))
            }
            let policy: Data
            if index == 0 {
                policy = option(omitRowZeroPolicy ? nil : rowZeroPolicy ?? policyForProfileZero(
                    protocolTag: policyProtocolTag,
                    useRetiredExperimentalAssurance: useRetiredExperimentalAssurance))
            } else if index == 5 {
                policy = option(x509Policy)
            } else if index == 6, let override = jindoPolicyOverride {
                policy = option(override)
            } else if index == 6, includePendingState {
                policy = option(policyForJindoWithPendingUpdate(schedule: protocolSchedule))
            } else {
                policy = option(nil)
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
                appendField(policy, to: &encodedRow)
                appendField(option(nil), to: &encodedRow)
            } else {
                appendField(policy, to: &encodedRow)
            }
            rows.append(encodedRow)
        }
        if swapFirstRows { rows.swapAt(0, 1) }
        precondition((11...13).contains(manifestRowCount))
        if manifestRowCount == 11 { rows.removeLast() }
        if manifestRowCount == 13 { rows.append(rows[11]) }

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
        if let embeddedDigest {
            digest = embeddedDigest
        } else {
            let normalized = buildManifest(
                rows: rows,
                network: manifestNetworkBytes ?? networkBytes,
                removedQualificationField: removedQualificationField,
                omitNetwork: omitNetwork,
                digest: Data(repeating: 0, count: 32),
                maxActionsPerTransaction: maxActionsPerTransaction,
                includePendingState: includePendingState,
                schedule: consensusSchedule,
                committedHeight: committedHeight,
                consensusPolicyOverride: consensusPolicyOverride
            )
            var preimage = Data("iroha:privacy:exact12-capability-manifest:v1".utf8)
            preimage.append(u64(UInt64(normalized.count)))
            preimage.append(normalized)
            digest = Data(SHA256.hash(data: preimage))
        }
        return Fixture(
            manifest: buildManifest(
                rows: rows,
                network: manifestNetworkBytes ?? networkBytes,
                removedQualificationField: removedQualificationField,
                omitNetwork: omitNetwork,
                digest: digest,
                maxActionsPerTransaction: maxActionsPerTransaction,
                includePendingState: includePendingState,
                schedule: consensusSchedule,
                committedHeight: committedHeight,
                consensusPolicyOverride: consensusPolicyOverride
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

    private func policyForProfileZero(
        protocolTag: UInt32 = 0,
        useRetiredExperimentalAssurance: Bool = false
    ) -> Data {
        var policy = structure(enumValue(protocolTag), enumValue(0), option(nil))
        if useRetiredExperimentalAssurance { appendField(enumValue(0), to: &policy) }
        return policy
    }

    private func securityClaim(
        protocolId: PrivacyProtocolIdV1,
        digestByte: UInt8
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
            typeName: "iroha_data_model::privacy::PrivacySecurityClaimV1",
            payload: claim,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 2
        )
        var preimage = Data("iroha:privacy:security-claim:v1".utf8)
        preimage.append(u64(UInt64(canonicalClaim.count)))
        preimage.append(canonicalClaim)
        let claimDigest = Data(SHA256.hash(data: preimage))
        return (claim, claimDigest)
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
        Data([
            0x5b, 0xcf, 0x36, 0x6f, 0xb1, 0x8d, 0x2e, 0x43,
            0x61, 0x48, 0x6c, 0x01, 0x5d, 0xa5, 0xb4, 0x1a,
            0x3f, 0xb0, 0x0e, 0xdf, 0x05, 0x61, 0x10, 0x42,
            0xcc, 0xc4, 0x52, 0x12, 0x0d, 0x17, 0x6e, 0xb0,
        ])
    }

    private func submitProofInstruction(
        protocolTag: UInt32 = 0,
        proofSystemTag: UInt32 = 0,
        engineTag: UInt32 = 0,
        parameterIdByte: UInt8 = 0x31,
        statementDigestByte: UInt8 = 0x71,
        statementTag: UInt32 = 0,
        proofTag: UInt32 = 0,
        statementNetworkBytes: Data? = nil,
        wireMagic: Data = Data([0x49, 0x52, 0x48, 0x5a, 0x4b, 0x31, 0xa5, 0x5a]),
        catalogCommitment: Data? = nil,
        retiredElevenFieldEnvelope: Bool = false
    ) -> Data {
        let profileDigest = structure(Data(repeating: 0x31, count: 32))
        let parameterId = structure(Data(repeating: parameterIdByte, count: 32))
        let statementDigest = structure(
            Data(repeating: statementDigestByte, count: 32)
        )
        let retainedFields = structure(
            enumValue(protocolTag),
            enumValue(proofSystemTag),
            enumValue(engineTag),
            parameterId,
            profileDigest,
            profileDigest,
            profileDigest,
            profileDigest,
            statementDigest,
            syntheticStatement(tag: statementTag, network: statementNetworkBytes ?? networkBytes),
            enumValue(proofTag, Data([1]))
        )
        let envelope = retiredElevenFieldEnvelope
            ? retainedFields
            : structure(wireMagic, catalogCommitment ?? exact12CatalogCommitment()) + retainedFields
        return noritoEncode(
            typeName: "iroha_data_model::isi::privacy::SubmitPrivacyProofV1",
            payload: structure(envelope),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
    }

    /// Projection-only statement shell; it has no proof or native authority.
    private func syntheticStatement(tag: UInt32, network: Data, contextSuffix: Data = Data()) -> Data {
        let digest = structure(Data(repeating: 0x31, count: 32))
        let context = structure(network, u32(0), digest, digest, digest, digest, digest, digest)
            + contextSuffix
        return enumValue(tag, structure(context, Data([1])))
    }

    private func policyForJindoWithPendingUpdate(
        schedule: (UInt64, UInt64) = (2, 302),
        current: UInt32 = 4,
        next: UInt32 = 3
    ) -> Data {
        structure(enumValue(6), enumValue(6, structure(u32(current))),
                  option(structure(u64(schedule.0), u64(schedule.1), enumValue(6, structure(u32(next))))))
    }

    private func consensusPolicy(
        maxActionsPerTransaction: UInt32 = 1,
        includePendingState: Bool = false,
        schedule: (UInt64, UInt64) = (2, 302),
        currentRetained: UInt32 = 2_048,
        nextRetained: UInt32 = 1_024
    ) -> Data {
        let current = structure(
            u32(maxActionsPerTransaction), u32(2),
            u32(9 * 1024 * 1024), u32(9_699_840),
            u32(9_699_840), u32(16_711_680),
            u32(256 * 1024), u32(8), u32(8), u32(currentRetained)
        )
        let pending: Data?
        if includePendingState {
            pending = structure(
                u64(schedule.0),
                u64(schedule.1),
                structure(
                    u32(maxActionsPerTransaction), u32(2),
                    u32(9 * 1024 * 1024), u32(9_699_840),
                    u32(9_699_840), u32(16_711_680),
                    u32(256 * 1024), u32(8), u32(8), u32(nextRetained)
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
        network: Data,
        removedQualificationField: Data?,
        omitNetwork: Bool,
        digest: Data,
        maxActionsPerTransaction: UInt32,
        includePendingState: Bool,
        schedule: (UInt64, UInt64),
        committedHeight: UInt64,
        consensusPolicyOverride: Data?
    ) -> Data {
        var payload = structure(u32(1))
        if !omitNetwork { appendField(network, to: &payload) }
        appendField(u64(committedHeight), to: &payload)
        appendField(consensusPolicyOverride ?? consensusPolicy(
            maxActionsPerTransaction: maxActionsPerTransaction,
            includePendingState: includePendingState,
            schedule: schedule
        ), to: &payload)
        if let removedQualificationField { appendField(removedQualificationField, to: &payload) }
        appendField(sequence(rows), to: &payload)
        appendField(structure(digest), to: &payload)
        return manifestFrame(payload)
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

private final class UnexpectedPrivacyCapabilityTransport: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        XCTFail("privacy admission without an expected network must never reach transport")
        client?.urlProtocol(self, didFailWithError: URLError(.cancelled))
    }
    override func stopLoading() {}
}
