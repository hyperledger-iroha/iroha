import CoreFoundation
import Foundation
import XCTest
@testable import IrohaSwift

final class MusubiInstructionsV1Tests: XCTestCase {
    func testTypedInstructionsMatchRustOwnedNoritoFixture() throws {
        XCTAssertThrowsError(try fixtureUInt64(true))
        XCTAssertEqual(try fixtureUInt64(NSNumber(value: 1)), 1)

        let fixture = try loadFixture()
        XCTAssertEqual(fixture["format"] as? String, "iroha-musubi-instructions-v1")
        XCTAssertEqual(fixture["fixture_version"] as? Int, 1)
        XCTAssertEqual(fixture["rust_owner"] as? String, "iroha_data_model::isi::musubi")

        let boxSchema = try XCTUnwrap(fixture["instruction_box_schema_name"] as? String)
        XCTAssertEqual(boxSchema, "(alloc::string::String, alloc::vec::Vec<u8>)")
        XCTAssertEqual(
            Data(noritoSchemaHash(forTypeName: boxSchema)).hexEncodedString(),
            fixture["instruction_box_schema_hash"] as? String
        )

        let cases = try XCTUnwrap(fixture["cases"] as? [[String: Any]])
        XCTAssertEqual(cases.count, 19)
        XCTAssertEqual(
            try cases.map { try XCTUnwrap($0["id"] as? String) },
            [
                "accept-root-max-revision",
                "revoke-domain-invitation",
                "register-alias-domain-target",
                "assert-prerelease-digest",
                "retire-location-max-revision",
                "unyank-domain-release-high-revision",
                "remove-root-maintainer-high-revision",
                "register-domain-namespace-max-generation",
                "invite-domain-maintainer-max-expiry",
                "promote-root-member-to-owner-high-revision",
                "recover-domain-package-three-owners",
                "retarget-one-character-alias-high-revision",
                "takedown-max-major-prerelease",
                "register-archive-max-bounds-signed-receipt",
                "register-provider-bundle-attestation",
                "add-location-three-signed-providers",
                "publish-delegated-domain-release",
                "replace-domain-metadata-high-revision",
                "set-allowlisted-policy-repriced-aliases",
            ]
        )
        for fixtureCase in cases {
            try requireKeys(
                fixtureCase,
                [
                    "id",
                    "semantic",
                    "wire_id",
                    "concrete_schema_name",
                    "concrete_schema_hash",
                    "header_flags",
                    "bare_payload_hex",
                    "concrete_frame_hex",
                    "instruction_box_pair_hex",
                    "standalone_instruction_box_frame_hex",
                ]
            )
            let identifier = try XCTUnwrap(fixtureCase["id"] as? String)
            let instruction = try instruction(for: fixtureCase)
            XCTAssertEqual(instruction.wireID, fixtureCase["wire_id"] as? String, identifier)
            XCTAssertEqual(
                instruction.concreteSchemaName,
                fixtureCase["concrete_schema_name"] as? String,
                identifier
            )
            XCTAssertEqual(
                Data(noritoSchemaHash(forTypeName: instruction.concreteSchemaName))
                    .hexEncodedString(),
                fixtureCase["concrete_schema_hash"] as? String,
                identifier
            )

            let barePayload = try instruction.barePayload()
            XCTAssertEqual(
                barePayload.hexEncodedString(),
                fixtureCase["bare_payload_hex"] as? String,
                identifier
            )

            let transactionFrame = try instruction.transactionInstructionFrame()
            XCTAssertEqual(
                transactionFrame.framedPayload.hexEncodedString(),
                fixtureCase["concrete_frame_hex"] as? String,
                identifier
            )
            let concreteFrame = try XCTUnwrap(noritoDecodeFrame(transactionFrame.framedPayload))
            XCTAssertEqual(concreteFrame.header.flags, UInt8(try fixtureUInt64(fixtureCase, "header_flags")))
            XCTAssertEqual(concreteFrame.payload, barePayload, identifier)
            XCTAssertEqual(
                try transactionFrame.compactInstructionBoxPayload().hexEncodedString(),
                fixtureCase["instruction_box_pair_hex"] as? String,
                identifier
            )

            let standalone = try instruction.standaloneInstructionBoxFrame()
            XCTAssertEqual(
                standalone.hexEncodedString(),
                fixtureCase["standalone_instruction_box_frame_hex"] as? String,
                identifier
            )
            let standaloneFrame = try XCTUnwrap(noritoDecodeFrame(standalone))
            XCTAssertEqual(
                standaloneFrame.payload.hexEncodedString(),
                fixtureCase["instruction_box_pair_hex"] as? String,
                identifier
            )
        }
    }

    func testTypedInstructionsEmbedExactFixturePairsInCanonicalNetworkSignedBatch() throws {
        let fixture = try loadFixture()
        let cases = try XCTUnwrap(fixture["cases"] as? [[String: Any]])
        XCTAssertEqual(cases.count, 19)
        let instructions = try cases.map { try instruction(for: $0) }
        let frames = try instructions.map { try $0.transactionInstructionFrame() }
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = AccountId.make(publicKey: try signingKey.publicKey())
        let networkId = TestNetworkIds.canonical
        let sdk = IrohaSDK(
            baseURL: URL(string: "https://torii.example")!,
            creationTimeProvider: { 1_700_000_000_000 }
        )

        let envelope = try sdk.buildSignedExecutableBatch(
            networkId: networkId,
            authority: authority,
            entries: frames.map { TransactionBatchEntry.instruction($0) },
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: 60,
            nonce: 7,
            signingKey: signingKey
        )

        var signedReader = CanonicalNoritoReader(data: envelope.signedTransaction)
        let transactionSignature = try signedReader.readCompactField()
        var transactionSignatureReader = CanonicalNoritoReader(data: transactionSignature)
        let signaturePayload = try transactionSignatureReader.readCompactField()
        XCTAssertEqual(transactionSignatureReader.remaining(), 0)
        var signatureReader = CanonicalNoritoReader(data: signaturePayload)
        XCTAssertEqual(try signatureReader.readUInt64LE(), 64)
        for _ in 0..<64 {
            XCTAssertEqual(try signatureReader.readCompactField().count, 1)
        }
        XCTAssertEqual(signatureReader.remaining(), 0)

        let payload = try signedReader.readCompactField()
        XCTAssertEqual(try signedReader.readCompactField(), Data([0]))
        XCTAssertEqual(signedReader.remaining(), 0)
        var payloadReader = CanonicalNoritoReader(data: payload)
        var payloadFields: [Data] = []
        for _ in 0..<10 {
            payloadFields.append(try payloadReader.readCompactField())
        }
        XCTAssertEqual(payloadReader.remaining(), 0)

        var admissionIntentReader = CanonicalNoritoReader(data: payloadFields[7])
        XCTAssertEqual(
            try admissionIntentReader.readUInt32LE(),
            TransactionAdmissionIntentV1.queuePlanSynced.rawValue
        )
        XCTAssertEqual(admissionIntentReader.remaining(), 0)

        var domainReader = CanonicalNoritoReader(data: payloadFields[0])
        XCTAssertEqual(try domainReader.readUInt32LE(), 0)
        XCTAssertEqual(try domainReader.readCompactField(), networkId.bytes)
        XCTAssertEqual(domainReader.remaining(), 0)

        var executableReader = CanonicalNoritoReader(data: payloadFields[3])
        XCTAssertEqual(try executableReader.readUInt32LE(), 4)
        let sequence = try executableReader.readCompactField()
        XCTAssertEqual(executableReader.remaining(), 0)
        var sequenceReader = CanonicalNoritoReader(data: sequence)
        XCTAssertEqual(try sequenceReader.readUInt64LE(), UInt64(cases.count))
        for fixtureCase in cases {
            let identifier = try XCTUnwrap(fixtureCase["id"] as? String)
            var itemReader = CanonicalNoritoReader(
                data: try sequenceReader.readCompactField()
            )
            XCTAssertEqual(try itemReader.readUInt32LE(), 0, identifier)
            XCTAssertEqual(
                try itemReader.readCompactField().hexEncodedString(),
                fixtureCase["instruction_box_pair_hex"] as? String,
                identifier
            )
            XCTAssertEqual(itemReader.remaining(), 0, identifier)
        }
        XCTAssertEqual(sequenceReader.remaining(), 0)
    }

    func testAliasNameEnforcesPermanentAliasGrammarAndBound() throws {
        XCTAssertEqual(try MusubiAliasNameV1("a").value, "a")
        XCTAssertEqual(
            try MusubiAliasNameV1(String(repeating: "a", count: 32)).value.count,
            32
        )
        for rejected in [
            "",
            "Oracle-tools",
            "oracle_tools",
            "-oracle",
            "oracle-",
            "oracle--tools",
            String(repeating: "a", count: 33),
        ] {
            XCTAssertThrowsError(try MusubiAliasNameV1(rejected), rejected)
        }
    }

    func testReasonEnforcesCanonicalTextAndUtf8Bound() throws {
        XCTAssertEqual(try MusubiReasonV1("reviewed transition").value, "reviewed transition")
        XCTAssertEqual(
            try MusubiReasonV1("reviewed 👩‍💻 transition").value,
            "reviewed 👩‍💻 transition"
        )
        XCTAssertEqual(try MusubiReasonV1(String(repeating: "a", count: 1_024)).value.utf8.count, 1_024)
        for rejected in [
            "",
            " leading",
            "trailing ",
            "line\nbreak",
            String(repeating: "a", count: 1_025),
            String(repeating: "é", count: 513),
        ] {
            XCTAssertThrowsError(try MusubiReasonV1(rejected), rejected)
        }
    }

    func testMutationValuesRejectNoncanonicalDomainAndPrereleaseCases() throws {
        let name = try MusubiPackageNameV1("package")
        XCTAssertThrowsError(
            try MusubiPackageIdV1(homeDataspace: 1, scope: .domain(""), name: name)
        )
        XCTAssertThrowsError(
            try MusubiPackageIdV1(homeDataspace: 1, scope: .domain("bad domain"), name: name)
        )
        XCTAssertThrowsError(
            try MusubiPackageIdV1(
                homeDataspace: 1,
                scope: .domain("safe\u{202e}name"),
                name: name
            )
        )
        for invalid in ["", "7", "007", "has space", String(repeating: "a", count: 65)] {
            XCTAssertThrowsError(
                try MusubiVersionV1(
                    major: 1,
                    minor: 0,
                    patch: 0,
                    prerelease: [.alphaNumeric(invalid)]
                ),
                invalid
            )
        }
    }

    func testReleaseExportsUseRustUnsignedUtf8NameOrdering() throws {
        let cases = try XCTUnwrap(loadFixture()["cases"] as? [[String: Any]])
        let fixtureCase = try XCTUnwrap(
            cases.first { $0["id"] as? String == "publish-delegated-domain-release" }
        )
        let semantic = try fixtureObject(fixtureCase["semantic"])
        let publicationObject = try fixtureObject(semantic["publication"])
        let manifest = try releaseManifest(publicationObject["manifest"])
        let utf8SortedNames = ["zeta", "éclair"]

        let accepted = try MusubiReleaseManifestV1(
            release: manifest.release,
            edition: manifest.edition,
            abi: manifest.abi,
            dependencies: manifest.dependencies,
            exports: utf8SortedNames,
            interfaceDigest: manifest.interfaceDigest,
            metadata: manifest.metadata,
            archiveID: manifest.archiveID,
            verificationLockDigest: manifest.verificationLockDigest
        )
        XCTAssertEqual(accepted.exports, utf8SortedNames)
        XCTAssertThrowsError(
            try MusubiReleaseManifestV1(
                release: manifest.release,
                edition: manifest.edition,
                abi: manifest.abi,
                dependencies: manifest.dependencies,
                exports: Array(utf8SortedNames.reversed()),
                interfaceDigest: manifest.interfaceDigest,
                metadata: manifest.metadata,
                archiveID: manifest.archiveID,
                verificationLockDigest: manifest.verificationLockDigest
            )
        )
    }

    func testBlake3SpansManyChunks() throws {
        let digest = try XCTUnwrap(MusubiBlake3V1.hash(Data(repeating: 0, count: 100_000)))
        XCTAssertEqual(
            digest.hexEncodedString(),
            "b1fc3c3bf473596bc8ac1f5c86f77c2fc0e0186a872b88adf841716fe9140a50"
        )
    }

    func testNewMutationBuildersRejectZeroRevisions() throws {
        let cases = try XCTUnwrap(loadFixture()["cases"] as? [[String: Any]])
        func fixtureCase(_ identifier: String) throws -> [String: Any] {
            try XCTUnwrap(cases.first { $0["id"] as? String == identifier })
        }

        let register = try XCTUnwrap(
            try instruction(
                for: fixtureCase("register-archive-max-bounds-signed-receipt")
            ) as? RegisterMusubiArchiveV1
        )
        XCTAssertThrowsError(
            try RegisterMusubiArchiveV1(
                commitment: register.commitment,
                stagingReceipt: register.stagingReceipt,
                expectedPolicyRevision: 0
            )
        )

        let attestationRegistration = try XCTUnwrap(
            try instruction(
                for: fixtureCase("register-provider-bundle-attestation")
            ) as? RegisterMusubiProviderBundleAttestationV1
        )
        XCTAssertThrowsError(
            try RegisterMusubiProviderBundleAttestationV1(
                attestation: attestationRegistration.attestation,
                expectedLocationRevision: 0
            )
        )

        let location = try XCTUnwrap(
            try instruction(
                for: fixtureCase("add-location-three-signed-providers")
            ) as? AddMusubiArchiveLocationV1
        )
        XCTAssertThrowsError(
            try AddMusubiArchiveLocationV1(
                archiveID: location.archiveID,
                locationID: location.locationID,
                pinManifest: location.pinManifest,
                replicationOrder: location.replicationOrder,
                providerAttestationSetDigest: location.providerAttestationSetDigest,
                renewAfterEpoch: location.renewAfterEpoch,
                expiresAtEpoch: location.expiresAtEpoch,
                expectedLocationRevision: 0
            )
        )
        XCTAssertThrowsError(
            try AddMusubiArchiveLocationV1(
                archiveID: location.archiveID,
                locationID: location.locationID,
                pinManifest: location.pinManifest,
                replicationOrder: location.replicationOrder,
                providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1(
                    bytes: [UInt8](repeating: 0, count: 32)
                ),
                renewAfterEpoch: location.renewAfterEpoch,
                expiresAtEpoch: location.expiresAtEpoch,
                expectedLocationRevision: location.expectedLocationRevision
            )
        )

        let publish = try XCTUnwrap(
            try instruction(
                for: fixtureCase("publish-delegated-domain-release")
            ) as? PublishMusubiReleaseV1
        )
        XCTAssertThrowsError(
            try PublishMusubiReleaseV1(
                namespace: publish.namespace,
                publication: publish.publication,
                namespaceDelegation: publish.namespaceDelegation,
                expectedPolicyRevision: 0,
                expectedGovernanceRevision: publish.expectedGovernanceRevision
            )
        )
        XCTAssertThrowsError(
            try PublishMusubiReleaseV1(
                namespace: publish.namespace,
                publication: publish.publication,
                namespaceDelegation: publish.namespaceDelegation,
                expectedPolicyRevision: publish.expectedPolicyRevision,
                expectedGovernanceRevision: 0
            )
        )

        let metadata = try XCTUnwrap(
            try instruction(
                for: fixtureCase("replace-domain-metadata-high-revision")
            ) as? SetMusubiPackageMetadataV1
        )
        XCTAssertThrowsError(
            try SetMusubiPackageMetadataV1(
                package: metadata.package,
                metadata: metadata.metadata,
                expectedMetadataRevision: 0
            )
        )

        let policy = try XCTUnwrap(
            try instruction(
                for: fixtureCase("set-allowlisted-policy-repriced-aliases")
            ) as? SetMusubiRegistryPolicyV1
        )
        XCTAssertThrowsError(
            try SetMusubiRegistryPolicyV1(
                decision: policy.decision,
                policy: policy.policy,
                expectedPolicyRevision: 0
            )
        )
    }

    func testMaintainerMutationValuesRejectInvalidRoleInviteAndAccount() throws {
        XCTAssertThrowsError(
            try MusubiMaintainerPermissionsV1(
                publish: false,
                yank: false,
                metadata: false,
                archiveLocations: false
            )
        )

        let cases = try XCTUnwrap(loadFixture()["cases"] as? [[String: Any]])
        let invite = try XCTUnwrap(
            cases.first { $0["id"] as? String == "invite-domain-maintainer-max-expiry" }
        )
        let semantic = try fixtureObject(invite["semantic"])
        let package = try packageID(semantic["package"])
        let role = try packageRole(semantic["role"])
        let account = try XCTUnwrap(semantic["invited_account"] as? String)
        let revision = try fixtureUInt64(semantic, "expected_governance_revision")
        let nonzeroInviteID = try digest32(semantic["invite_id"])

        XCTAssertThrowsError(
            try InviteMusubiPackageMaintainerV1(
                package: package,
                inviteID: MusubiDigest32V1(bytes: [UInt8](repeating: 0, count: 32)),
                invitedAccount: account,
                role: role,
                expiresAtHeight: UInt64.max,
                expectedGovernanceRevision: revision
            )
        )
        XCTAssertThrowsError(
            try InviteMusubiPackageMaintainerV1(
                package: package,
                inviteID: nonzeroInviteID,
                invitedAccount: account,
                role: role,
                expiresAtHeight: 0,
                expectedGovernanceRevision: revision
            )
        )
        XCTAssertThrowsError(
            try InviteMusubiPackageMaintainerV1(
                package: package,
                inviteID: nonzeroInviteID,
                invitedAccount: " \(account)",
                role: role,
                expiresAtHeight: UInt64.max,
                expectedGovernanceRevision: revision
            )
        )
        XCTAssertThrowsError(
            try SetMusubiPackageMaintainerRoleV1(
                package: package,
                account: "\(account) ",
                role: .owner,
                expectedGovernanceRevision: revision
            )
        )
    }

    func testParliamentMutationsRejectInvalidDecisionOwnersAndRevisions() throws {
        let cases = try XCTUnwrap(loadFixture()["cases"] as? [[String: Any]])
        let recoverSemantic = try fixtureObject(
            XCTUnwrap(
                cases.first { $0["id"] as? String == "recover-domain-package-three-owners" }
            )["semantic"]
        )
        let validDecision = try governanceDecision(recoverSemantic["decision"])
        let package = try packageID(recoverSemantic["package"])
        let owners = try fixtureArray(recoverSemantic["owners"]).map {
            try XCTUnwrap($0 as? String)
        }
        let actionDigest = validDecision.actionDigest

        XCTAssertThrowsError(
            try MusubiGovernanceDecisionV1(
                decisionID: [UInt8](repeating: 0, count: 32),
                actionDigest: actionDigest,
                enactedAtHeight: 1,
                executeAfterHeight: 2
            )
        )
        XCTAssertThrowsError(
            try MusubiGovernanceDecisionV1(
                decisionID: validDecision.decisionID,
                actionDigest: MusubiDigest32V1(bytes: [UInt8](repeating: 0, count: 32)),
                enactedAtHeight: 1,
                executeAfterHeight: 2
            )
        )
        XCTAssertThrowsError(
            try MusubiGovernanceDecisionV1(
                decisionID: validDecision.decisionID,
                actionDigest: actionDigest,
                enactedAtHeight: 0,
                executeAfterHeight: 2
            )
        )
        XCTAssertThrowsError(
            try MusubiGovernanceDecisionV1(
                decisionID: validDecision.decisionID,
                actionDigest: actionDigest,
                enactedAtHeight: 2,
                executeAfterHeight: 2
            )
        )

        for rejectedOwners in [
            [],
            Array(owners.reversed()),
            [owners[0], owners[0]],
            Array(repeating: owners[0], count: 65),
        ] {
            XCTAssertThrowsError(
                try RecoverMusubiPackageV1(
                    decision: validDecision,
                    package: package,
                    owners: rejectedOwners,
                    expectedGovernanceRevision: 1
                )
            )
        }
        XCTAssertThrowsError(
            try RecoverMusubiPackageV1(
                decision: validDecision,
                package: package,
                owners: owners,
                expectedGovernanceRevision: 0
            )
        )

        let retargetSemantic = try fixtureObject(
            XCTUnwrap(
                cases.first {
                    $0["id"] as? String == "retarget-one-character-alias-high-revision"
                }
            )["semantic"]
        )
        XCTAssertThrowsError(
            try RetargetMusubiAliasV1(
                decision: governanceDecision(retargetSemantic["decision"]),
                alias: MusubiAliasNameV1(newtypeText(retargetSemantic["alias"])),
                target: packageID(retargetSemantic["target"]),
                expectedHistoryRevision: 0
            )
        )

        let takedownSemantic = try fixtureObject(
            XCTUnwrap(
                cases.first { $0["id"] as? String == "takedown-max-major-prerelease" }
            )["semantic"]
        )
        XCTAssertThrowsError(
            try SetMusubiArtifactTakedownV1(
                decision: governanceDecision(takedownSemantic["decision"]),
                release: releaseID(takedownSemantic["release"]),
                reason: MusubiReasonV1(newtypeText(takedownSemantic["reason"])),
                expectedArtifactGovernanceRevision: 0
            )
        )
    }

    func testRecoveryNormalizesMultisigOwnersBeforeDistinctnessAndEncoding() throws {
        let fixture = try loadFixture()
        let cases = try XCTUnwrap(fixture["cases"] as? [[String: Any]])
        let recoverCase = try XCTUnwrap(
            cases.first { $0["id"] as? String == "recover-domain-package-three-owners" }
        )
        let semantic = try fixtureObject(recoverCase["semantic"])
        let decision = try governanceDecision(semantic["decision"])
        let package = try packageID(semantic["package"])
        let sortedBytes = try XCTUnwrap(
            Data(
                hexString:
                    "0a010100020002" +
                    "01000100205c9c6df261c9cb840475776aaefcd944b405328fab28f9b3a95ef40490d3de84" +
                    "0100020020d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737"
            )
        )
        let reversedBytes = try XCTUnwrap(
            Data(
                hexString:
                    "0a010100020002" +
                    "0100020020d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737" +
                    "01000100205c9c6df261c9cb840475776aaefcd944b405328fab28f9b3a95ef40490d3de84"
            )
        )
        let sortedOwner = try AccountAddress.fromCanonicalBytes(sortedBytes)
            .toI105(networkPrefix: 753)
        let reversedOwner = try AccountAddress.fromCanonicalBytes(reversedBytes)
            .toI105(networkPrefix: 753)
        XCTAssertNotEqual(sortedOwner, reversedOwner)

        let sorted = try RecoverMusubiPackageV1(
            decision: decision,
            package: package,
            owners: [sortedOwner],
            expectedGovernanceRevision: 1
        )
        let reversed = try RecoverMusubiPackageV1(
            decision: decision,
            package: package,
            owners: [reversedOwner],
            expectedGovernanceRevision: 1
        )
        XCTAssertEqual(try sorted.barePayload(), try reversed.barePayload())
        XCTAssertThrowsError(
            try RecoverMusubiPackageV1(
                decision: decision,
                package: package,
                owners: [sortedOwner, reversedOwner],
                expectedGovernanceRevision: 1
            )
        )
    }

    private func instruction(
        for fixtureCase: [String: Any]
    ) throws -> any MusubiInstructionV1 {
        let identifier = try XCTUnwrap(fixtureCase["id"] as? String)
        let semantic = try fixtureObject(fixtureCase["semantic"])
        switch identifier {
        case "accept-root-max-revision":
            try requireKeys(
                semantic,
                ["package", "invite_id", "expected_governance_revision"]
            )
            return AcceptMusubiPackageMaintainerV1(
                package: try packageID(semantic["package"]),
                inviteID: try digest32(semantic["invite_id"]),
                expectedGovernanceRevision: try fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "revoke-domain-invitation":
            try requireKeys(
                semantic,
                ["package", "invite_id", "expected_governance_revision"]
            )
            return RevokeMusubiPackageMaintainerInvitationV1(
                package: try packageID(semantic["package"]),
                inviteID: try digest32(semantic["invite_id"]),
                expectedGovernanceRevision: try fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "register-alias-domain-target":
            try requireKeys(
                semantic,
                ["alias", "target", "expected_pricing_revision"]
            )
            return RegisterMusubiAliasV1(
                alias: try MusubiAliasNameV1(newtypeText(semantic["alias"])),
                target: try packageID(semantic["target"]),
                expectedPricingRevision: try fixtureUInt64(
                    semantic,
                    "expected_pricing_revision"
                )
            )
        case "assert-prerelease-digest":
            try requireKeys(semantic, ["release", "expected_digest"])
            return AssertMusubiReleaseDigestV1(
                release: try releaseID(semantic["release"]),
                expectedDigest: try digest32(semantic["expected_digest"])
            )
        case "retire-location-max-revision":
            try requireKeys(
                semantic,
                ["archive_id", "location_id", "expected_location_revision", "reason"]
            )
            return RetireMusubiArchiveLocationV1(
                archiveID: try digest32(semantic["archive_id"]),
                locationID: try digest32(semantic["location_id"]),
                expectedLocationRevision: try fixtureUInt64(
                    semantic,
                    "expected_location_revision"
                ),
                reason: try MusubiReasonV1(newtypeText(semantic["reason"]))
            )
        case "unyank-domain-release-high-revision":
            try requireKeys(
                semantic,
                ["release", "yanked", "reason", "expected_yank_revision"]
            )
            return SetMusubiReleaseYankV1(
                release: try releaseID(semantic["release"]),
                yanked: try XCTUnwrap(semantic["yanked"] as? Bool),
                reason: try MusubiReasonV1(newtypeText(semantic["reason"])),
                expectedYankRevision: try fixtureUInt64(
                    semantic,
                    "expected_yank_revision"
                )
            )
        case "remove-root-maintainer-high-revision":
            try requireKeys(
                semantic,
                ["package", "account", "expected_governance_revision"]
            )
            return try RemoveMusubiPackageMaintainerV1(
                package: packageID(semantic["package"]),
                account: XCTUnwrap(semantic["account"] as? String),
                expectedGovernanceRevision: fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "register-domain-namespace-max-generation":
            try requireKeys(semantic, ["binding", "expected_policy_revision"])
            return RegisterMusubiNamespaceBindingV1(
                binding: try namespaceBinding(semantic["binding"]),
                expectedPolicyRevision: try fixtureUInt64(
                    semantic,
                    "expected_policy_revision"
                )
            )
        case "invite-domain-maintainer-max-expiry":
            try requireKeys(
                semantic,
                [
                    "package", "invite_id", "invited_account", "role",
                    "expires_at_height", "expected_governance_revision",
                ]
            )
            return try InviteMusubiPackageMaintainerV1(
                package: packageID(semantic["package"]),
                inviteID: digest32(semantic["invite_id"]),
                invitedAccount: XCTUnwrap(semantic["invited_account"] as? String),
                role: packageRole(semantic["role"]),
                expiresAtHeight: fixtureUInt64(semantic, "expires_at_height"),
                expectedGovernanceRevision: fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "promote-root-member-to-owner-high-revision":
            try requireKeys(
                semantic,
                ["package", "account", "role", "expected_governance_revision"]
            )
            return try SetMusubiPackageMaintainerRoleV1(
                package: packageID(semantic["package"]),
                account: XCTUnwrap(semantic["account"] as? String),
                role: packageRole(semantic["role"]),
                expectedGovernanceRevision: fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "recover-domain-package-three-owners":
            try requireKeys(
                semantic,
                ["decision", "package", "owners", "expected_governance_revision"]
            )
            return try RecoverMusubiPackageV1(
                decision: governanceDecision(semantic["decision"]),
                package: packageID(semantic["package"]),
                owners: fixtureArray(semantic["owners"]).map {
                    try XCTUnwrap($0 as? String)
                },
                expectedGovernanceRevision: fixtureUInt64(
                    semantic,
                    "expected_governance_revision"
                )
            )
        case "retarget-one-character-alias-high-revision":
            try requireKeys(
                semantic,
                ["decision", "alias", "target", "expected_history_revision"]
            )
            return try RetargetMusubiAliasV1(
                decision: governanceDecision(semantic["decision"]),
                alias: MusubiAliasNameV1(newtypeText(semantic["alias"])),
                target: packageID(semantic["target"]),
                expectedHistoryRevision: fixtureUInt64(
                    semantic,
                    "expected_history_revision"
                )
            )
        case "takedown-max-major-prerelease":
            try requireKeys(
                semantic,
                [
                    "decision", "release", "reason",
                    "expected_artifact_governance_revision",
                ]
            )
            return try SetMusubiArtifactTakedownV1(
                decision: governanceDecision(semantic["decision"]),
                release: releaseID(semantic["release"]),
                reason: MusubiReasonV1(newtypeText(semantic["reason"])),
                expectedArtifactGovernanceRevision: fixtureUInt64(
                    semantic,
                    "expected_artifact_governance_revision"
                )
            )
        case "register-archive-max-bounds-signed-receipt":
            try requireKeys(
                semantic,
                ["commitment", "staging_receipt", "expected_policy_revision"]
            )
            return try RegisterMusubiArchiveV1(
                commitment: decodeSemantic(
                    semantic["commitment"], as: MusubiArchiveCommitmentV1.self
                ),
                stagingReceipt: decodeSemantic(
                    semantic["staging_receipt"], as: MusubiSeedIngressReceiptV1.self
                ),
                expectedPolicyRevision: fixtureUInt64(
                    semantic, "expected_policy_revision"
                )
            )
        case "register-provider-bundle-attestation":
            try requireKeys(
                semantic,
                ["attestation", "expected_location_revision"]
            )
            return try RegisterMusubiProviderBundleAttestationV1(
                attestation: providerAttestation(XCTUnwrap(semantic["attestation"])),
                expectedLocationRevision: fixtureUInt64(
                    semantic, "expected_location_revision"
                )
            )
        case "add-location-three-signed-providers":
            try requireKeys(
                semantic,
                [
                    "archive_id", "location_id", "pin_manifest", "replication_order",
                    "provider_attestation_set_digest", "renew_after_epoch", "expires_at_epoch",
                    "expected_location_revision",
                ]
            )
            return try AddMusubiArchiveLocationV1(
                archiveID: digest32(semantic["archive_id"]),
                locationID: digest32(semantic["location_id"]),
                pinManifest: digest32(semantic["pin_manifest"]),
                replicationOrder: digest32(semantic["replication_order"]),
                providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1(
                    bytes: digest32(semantic["provider_attestation_set_digest"]).bytes
                ),
                renewAfterEpoch: fixtureUInt64(semantic, "renew_after_epoch"),
                expiresAtEpoch: fixtureUInt64(semantic, "expires_at_epoch"),
                expectedLocationRevision: fixtureUInt64(
                    semantic, "expected_location_revision"
                )
            )
        case "publish-delegated-domain-release":
            try requireKeys(
                semantic,
                [
                    "namespace", "publication", "namespace_delegation",
                    "expected_policy_revision", "expected_governance_revision",
                ]
            )
            XCTAssertTrue(semantic["expected_governance_revision"] is NSNull)
            return try PublishMusubiReleaseV1(
                namespace: MusubiNamespaceV1(newtypeText(semantic["namespace"])),
                publication: publication(semantic["publication"]),
                namespaceDelegation: namespaceDelegation(
                    semantic["namespace_delegation"]
                ),
                expectedPolicyRevision: fixtureUInt64(
                    semantic, "expected_policy_revision"
                ),
                expectedGovernanceRevision: nil
            )
        case "replace-domain-metadata-high-revision":
            try requireKeys(
                semantic,
                ["package", "metadata", "expected_metadata_revision"]
            )
            return try SetMusubiPackageMetadataV1(
                package: packageID(semantic["package"]),
                metadata: releaseMetadata(semantic["metadata"]),
                expectedMetadataRevision: fixtureUInt64(
                    semantic, "expected_metadata_revision"
                )
            )
        case "set-allowlisted-policy-repriced-aliases":
            try requireKeys(
                semantic,
                ["decision", "policy", "expected_policy_revision"]
            )
            return try SetMusubiRegistryPolicyV1(
                decision: governanceDecision(semantic["decision"]),
                policy: registryPolicy(semantic["policy"]),
                expectedPolicyRevision: fixtureUInt64(
                    semantic, "expected_policy_revision"
                )
            )
        default:
            throw MusubiV1Error.invalidValue("Unknown mutation fixture case \(identifier).")
        }
    }

    private func packageID(_ raw: Any?) throws -> MusubiPackageIdV1 {
        let package = try fixtureObject(raw)
        try requireKeys(package, ["home_dataspace", "scope", "name"])
        let scope = try packageScope(package["scope"])
        return try MusubiPackageIdV1(
            homeDataspace: fixtureUInt64(package, "home_dataspace"),
            scope: scope,
            name: MusubiPackageNameV1(newtypeText(package["name"]))
        )
    }

    private func packageScope(_ raw: Any?) throws -> MusubiPackageScopeV1 {
        let scopeObject = try fixtureObject(raw)
        try requireKeys(scopeObject, ["kind", "value"])
        switch try XCTUnwrap(scopeObject["kind"] as? String) {
        case "DataspaceRoot":
            XCTAssertTrue(scopeObject["value"] is NSNull)
            return .dataspaceRoot
        case "Domain":
            return .domain(try XCTUnwrap(scopeObject["value"] as? String))
        default:
            throw MusubiV1Error.invalidValue("Unknown package-scope fixture variant.")
        }
    }

    private func namespaceBinding(_ raw: Any?) throws -> MusubiNamespaceBindingV1 {
        let binding = try fixtureObject(raw)
        try requireKeys(binding, ["namespace", "home_dataspace", "scope", "generation"])
        return try MusubiNamespaceBindingV1(
            namespace: MusubiNamespaceV1(newtypeText(binding["namespace"])),
            homeDataspace: fixtureUInt64(binding, "home_dataspace"),
            scope: packageScope(binding["scope"]),
            generation: fixtureUInt64(binding, "generation")
        )
    }

    private func packageRole(_ raw: Any?) throws -> MusubiPackageRoleV1 {
        let role = try fixtureObject(raw)
        try requireKeys(role, ["kind", "value"])
        switch try XCTUnwrap(role["kind"] as? String) {
        case "Owner":
            XCTAssertTrue(role["value"] is NSNull)
            return .owner
        case "Maintainer":
            let permissions = try fixtureObject(role["value"])
            try requireKeys(
                permissions,
                ["publish", "yank", "metadata", "archive_locations"]
            )
            return .maintainer(
                try MusubiMaintainerPermissionsV1(
                    publish: XCTUnwrap(permissions["publish"] as? Bool),
                    yank: XCTUnwrap(permissions["yank"] as? Bool),
                    metadata: XCTUnwrap(permissions["metadata"] as? Bool),
                    archiveLocations: XCTUnwrap(permissions["archive_locations"] as? Bool)
                )
            )
        default:
            throw MusubiV1Error.invalidValue("Unknown package-role fixture variant.")
        }
    }

    private func releaseID(_ raw: Any?) throws -> MusubiReleaseIdV1 {
        let release = try fixtureObject(raw)
        try requireKeys(release, ["package", "version"])
        return MusubiReleaseIdV1(
            package: try packageID(release["package"]),
            version: try version(release["version"])
        )
    }

    private func version(_ raw: Any?) throws -> MusubiVersionV1 {
        let version = try fixtureObject(raw)
        try requireKeys(version, ["major", "minor", "patch", "prerelease"])
        let prerelease = try fixtureArray(version["prerelease"]).map { raw in
            let identifier = try fixtureObject(raw)
            try requireKeys(identifier, ["kind", "value"])
            switch try XCTUnwrap(identifier["kind"] as? String) {
            case "Numeric":
                return MusubiPrereleaseIdentifierV1.numeric(
                    try fixtureUInt64(identifier, "value")
                )
            case "AlphaNumeric":
                return MusubiPrereleaseIdentifierV1.alphaNumeric(
                    try XCTUnwrap(identifier["value"] as? String)
                )
            default:
                throw MusubiV1Error.invalidValue("Unknown prerelease fixture variant.")
            }
        }
        return try MusubiVersionV1(
            major: fixtureUInt64(version, "major"),
            minor: fixtureUInt64(version, "minor"),
            patch: fixtureUInt64(version, "patch"),
            prerelease: prerelease
        )
    }

    private func versionRequirement(_ raw: Any?) throws -> MusubiVersionReqV1 {
        let requirement = try fixtureObject(raw)
        try requireKeys(requirement, ["kind", "value"])
        let kind = try XCTUnwrap(requirement["kind"] as? String)
        switch kind {
        case "Any":
            XCTAssertTrue(requirement["value"] is NSNull)
            return .any
        case "Caret": return .caret(try version(requirement["value"]))
        case "Tilde": return .tilde(try version(requirement["value"]))
        case "MajorWildcard": return .majorWildcard(try fixtureUInt64(requirement["value"]))
        case "MinorWildcard":
            let wildcard = try fixtureObject(requirement["value"])
            try requireKeys(wildcard, ["major", "minor"])
            return .minorWildcard(
                major: try fixtureUInt64(wildcard, "major"),
                minor: try fixtureUInt64(wildcard, "minor")
            )
        case "Exact": return .exact(try version(requirement["value"]))
        case "Comparators":
            return .comparators(
                try fixtureArray(requirement["value"]).map { raw in
                    let comparator = try fixtureObject(raw)
                    try requireKeys(comparator, ["op", "version"])
                    return MusubiVersionComparatorV1(
                        op: try comparatorOperator(comparator["op"]),
                        version: try version(comparator["version"])
                    )
                }
            )
        default:
            throw MusubiV1Error.invalidValue("Unknown version-requirement fixture variant.")
        }
    }

    private func comparatorOperator(_ raw: Any?) throws -> MusubiComparatorOpV1 {
        let operation = try fixtureObject(raw)
        try requireKeys(operation, ["kind", "value"])
        XCTAssertTrue(operation["value"] is NSNull)
        switch try XCTUnwrap(operation["kind"] as? String) {
        case "Greater": return .greater
        case "GreaterOrEqual": return .greaterOrEqual
        case "Less": return .less
        case "LessOrEqual": return .lessOrEqual
        case "Equal": return .equal
        default:
            throw MusubiV1Error.invalidValue("Unknown comparator fixture variant.")
        }
    }

    private func controllerApproval(_ raw: Any?) throws -> MusubiControllerApprovalV1 {
        let approval = try fixtureObject(raw)
        try requireKeys(approval, ["public_key", "signature"])
        return try MusubiControllerApprovalV1(
            publicKey: XCTUnwrap(approval["public_key"] as? String),
            signature: XCTUnwrap(approval["signature"] as? String)
        )
    }

    private func providerAttestation(
        _ raw: Any
    ) throws -> MusubiProviderBundleVerificationAttestationV1 {
        let attestation = try fixtureObject(raw)
        try requireKeys(attestation, ["payload", "approvals"])
        let payload = try fixtureObject(attestation["payload"])
        try requireKeys(payload, ["version", "binding"])
        let binding = try providerBinding(payload["binding"])
        return try MusubiProviderBundleVerificationAttestationV1(
            payload: MusubiProviderBundleVerificationPayloadV1(
                version: try XCTUnwrap(UInt8(exactly: fixtureUInt64(payload, "version"))),
                binding: binding
            ),
            approvals: fixtureArray(attestation["approvals"]).map(controllerApproval)
        )
    }

    private func providerBinding(
        _ raw: Any?
    ) throws -> MusubiProviderBundleVerificationBindingV1 {
        let binding = try fixtureObject(raw)
        try requireKeys(
            binding,
            [
                "network_id", "provider_id", "completed_by",
                "completion_authority", "replication_order", "assignment_revision",
                "completion_epoch", "finalized_anchor", "archive_id", "bundle_digest",
                "descriptor_digest", "semantic_release_manifest_digest",
                "verification_lock_digest", "source_tree_digest",
            ]
        )
        let authority = try fixtureObject(binding["completion_authority"])
        try requireKeys(authority, ["provider_owner", "signer_policy"])
        let policy = try fixtureObject(authority["signer_policy"])
        try requireKeys(
            policy,
            ["policy_id", "revision", "predecessor_digest", "policy_digest"]
        )
        let predecessor: [UInt8]?
        if policy["predecessor_digest"] is NSNull {
            predecessor = nil
        } else {
            predecessor = try fixedBytes32(policy["predecessor_digest"])
        }
        let completionAuthority = try MusubiProviderIngestCompletionAuthorityV1(
            providerOwner: XCTUnwrap(authority["provider_owner"] as? String),
            signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1(
                policyID: fixedBytes32(policy["policy_id"]),
                revision: fixtureUInt64(policy, "revision"),
                predecessorDigest: predecessor,
                policyDigest: fixedBytes32(policy["policy_digest"])
            )
        )
        let anchor = try fixtureObject(binding["finalized_anchor"])
        try requireKeys(anchor, ["height", "block_hash"])
        return try MusubiProviderBundleVerificationBindingV1(
            networkId: NetworkId(
                literal: try XCTUnwrap(binding["network_id"] as? String)
            ),
            providerID: digestFromNewtypeHex(binding["provider_id"]),
            completedBy: XCTUnwrap(binding["completed_by"] as? String),
            completionAuthority: completionAuthority,
            replicationOrder: digest32(binding["replication_order"]),
            assignmentRevision: fixtureUInt64(binding, "assignment_revision"),
            completionEpoch: fixtureUInt64(binding, "completion_epoch"),
            finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1(
                height: fixtureUInt64(anchor, "height"),
                blockHash: fixedBytes32(anchor["block_hash"])
            ),
            archiveID: digest32(binding["archive_id"]),
            bundleDigest: digest32(binding["bundle_digest"]),
            descriptorDigest: digest32(binding["descriptor_digest"]),
            semanticReleaseManifestDigest: digest32(
                binding["semantic_release_manifest_digest"]
            ),
            verificationLockDigest: digest32(binding["verification_lock_digest"]),
            sourceTreeDigest: digest32(binding["source_tree_digest"])
        )
    }

    private func publication(_ raw: Any?) throws -> MusubiPublicationV1 {
        let publication = try fixtureObject(raw)
        try requireKeys(publication, ["manifest", "resolution"])
        return try MusubiPublicationV1(
            manifest: releaseManifest(publication["manifest"]),
            resolution: resolutionProof(publication["resolution"])
        )
    }

    private func releaseManifest(_ raw: Any?) throws -> MusubiReleaseManifestV1 {
        let manifest = try fixtureObject(raw)
        try requireKeys(
            manifest,
            [
                "release", "edition", "abi", "dependencies", "exports",
                "interface_digest", "metadata", "archive_id",
                "verification_lock_digest",
            ]
        )
        let edition = try fixtureObject(manifest["edition"])
        try requireKeys(edition, ["kind", "value"])
        XCTAssertEqual(edition["kind"] as? String, "V1")
        XCTAssertTrue(edition["value"] is NSNull)
        return try MusubiReleaseManifestV1(
            release: releaseID(manifest["release"]),
            edition: .v1,
            abi: abiBinding(manifest["abi"]),
            dependencies: fixtureArray(manifest["dependencies"]).map(dependencyReq),
            exports: fixtureArray(manifest["exports"]).map { try XCTUnwrap($0 as? String) },
            interfaceDigest: digest32(manifest["interface_digest"]),
            metadata: releaseMetadata(manifest["metadata"]),
            archiveID: digest32(manifest["archive_id"]),
            verificationLockDigest: digest32(manifest["verification_lock_digest"])
        )
    }

    private func abiBinding(_ raw: Any?) throws -> MusubiAbiBindingV1 {
        let abi = try fixtureObject(raw)
        try requireKeys(abi, ["abi_version", "abi_hash"])
        return try MusubiAbiBindingV1(
            abiVersion: XCTUnwrap(UInt16(exactly: fixtureUInt64(abi, "abi_version"))),
            abiHash: fixedBytes32(abi["abi_hash"])
        )
    }

    private func dependencyReq(_ raw: Any) throws -> MusubiDependencyReqV1 {
        let dependency = try fixtureObject(raw)
        try requireKeys(dependency, ["alias", "package", "requirement"])
        return try MusubiDependencyReqV1(
            alias: XCTUnwrap(dependency["alias"] as? String),
            package: packageID(dependency["package"]),
            requirement: versionRequirement(dependency["requirement"])
        )
    }

    private func exactDependency(_ raw: Any) throws -> MusubiExactDependencyEdgeV1 {
        let dependency = try fixtureObject(raw)
        try requireKeys(
            dependency, ["alias", "kind", "package", "requirement", "selected"]
        )
        let kind = try fixtureObject(dependency["kind"])
        try requireKeys(kind, ["kind", "value"])
        XCTAssertTrue(kind["value"] is NSNull)
        let dependencyKind: MusubiDependencyKindV1
        switch try XCTUnwrap(kind["kind"] as? String) {
        case "Normal": dependencyKind = .normal
        case "Development": dependencyKind = .development
        default: throw MusubiV1Error.invalidValue("Unknown dependency-kind fixture variant.")
        }
        return try MusubiExactDependencyEdgeV1(
            alias: XCTUnwrap(dependency["alias"] as? String),
            kind: dependencyKind,
            package: packageID(dependency["package"]),
            requirement: versionRequirement(dependency["requirement"]),
            selected: releaseID(dependency["selected"])
        )
    }

    private func resolutionProof(_ raw: Any?) throws -> MusubiResolutionProofV1 {
        let proof = try fixtureObject(raw)
        try requireKeys(proof, ["snapshot", "lock"])
        let snapshot = try fixtureObject(proof["snapshot"])
        try requireKeys(
            snapshot, ["finalized_height", "finalized_block_hash", "index_revision"]
        )
        return MusubiResolutionProofV1(
            snapshot: try MusubiRegistrySnapshotV1(
                finalizedHeight: fixtureUInt64(snapshot, "finalized_height"),
                finalizedBlockHash: fixedBytes32(snapshot["finalized_block_hash"]),
                indexRevision: fixtureUInt64(snapshot, "index_revision")
            ),
            lock: try verificationLock(proof["lock"])
        )
    }

    private func verificationLock(_ raw: Any?) throws -> MusubiVerificationLockV1 {
        let lock = try fixtureObject(raw)
        try requireKeys(lock, ["schema", "version", "root", "root_dependencies", "nodes"])
        return try MusubiVerificationLockV1(
            schema: XCTUnwrap(lock["schema"] as? String),
            version: XCTUnwrap(UInt8(exactly: fixtureUInt64(lock, "version"))),
            root: releaseID(lock["root"]),
            rootDependencies: fixtureArray(lock["root_dependencies"]).map(exactDependency),
            nodes: fixtureArray(lock["nodes"]).map(verificationNode)
        )
    }

    private func verificationNode(_ raw: Any) throws -> MusubiVerificationNodeV1 {
        let node = try fixtureObject(raw)
        try requireKeys(
            node,
            [
                "release", "release_digest", "archive_id", "source_digest",
                "interface_digest", "abi", "dependencies",
            ]
        )
        return try MusubiVerificationNodeV1(
            release: releaseID(node["release"]),
            releaseDigest: digest32(node["release_digest"]),
            archiveID: digest32(node["archive_id"]),
            sourceDigest: digest32(node["source_digest"]),
            interfaceDigest: digest32(node["interface_digest"]),
            abi: abiBinding(node["abi"]),
            dependencies: fixtureArray(node["dependencies"]).map(exactDependency)
        )
    }

    private func releaseMetadata(_ raw: Any?) throws -> MusubiReleaseMetadataV1 {
        let metadata = try fixtureObject(raw)
        try requireKeys(
            metadata, ["description", "readme", "license", "repository", "keywords"]
        )
        return try MusubiReleaseMetadataV1(
            description: optionalNewtypeText(metadata["description"]),
            readme: optionalNewtypeText(metadata["readme"]),
            license: optionalNewtypeText(metadata["license"]),
            repository: optionalNewtypeText(metadata["repository"]),
            keywords: fixtureArray(metadata["keywords"]).map(newtypeText)
        )
    }

    private func namespaceDelegation(_ raw: Any?) throws -> MusubiNamespaceDelegationV1 {
        let delegation = try fixtureObject(raw)
        try requireKeys(delegation, ["payload", "approvals"])
        let payload = try fixtureObject(delegation["payload"])
        try requireKeys(
            payload,
            [
                "version", "namespace_binding", "owner_generation", "owner",
                "delegate", "expires_at_height",
            ]
        )
        return try MusubiNamespaceDelegationV1(
            payload: MusubiNamespaceDelegationPayloadV1(
                version: XCTUnwrap(UInt8(exactly: fixtureUInt64(payload, "version"))),
                namespaceBinding: digest32(payload["namespace_binding"]),
                ownerGeneration: fixtureUInt64(payload, "owner_generation"),
                owner: XCTUnwrap(payload["owner"] as? String),
                delegate: XCTUnwrap(payload["delegate"] as? String),
                expiresAtHeight: fixtureUInt64(payload, "expires_at_height")
            ),
            approvals: fixtureArray(delegation["approvals"]).map(controllerApproval)
        )
    }

    private func registryPolicy(_ raw: Any?) throws -> MusubiRegistryPolicyV1 {
        let policy = try fixtureObject(raw)
        try requireKeys(
            policy,
            ["version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing"]
        )
        let mode = try fixtureObject(policy["mode"])
        try requireKeys(mode, ["kind", "value"])
        XCTAssertTrue(mode["value"] is NSNull)
        let admissionMode: MusubiRegistryAdmissionModeV1
        switch try XCTUnwrap(mode["kind"] as? String) {
        case "Closed": admissionMode = .closed
        case "Allowlisted": admissionMode = .allowlisted
        case "Open": admissionMode = .open
        default: throw MusubiV1Error.invalidValue("Unknown registry-mode fixture variant.")
        }
        let pricing = try fixtureObject(policy["alias_pricing"])
        try requireKeys(
            pricing,
            [
                "revision", "length_1_xor", "length_2_xor", "length_3_xor",
                "length_4_xor", "length_5_to_32_xor",
            ]
        )
        return try MusubiRegistryPolicyV1(
            version: XCTUnwrap(UInt8(exactly: fixtureUInt64(policy, "version"))),
            revision: fixtureUInt64(policy, "revision"),
            mode: admissionMode,
            allowlistedDataspaces: fixtureArray(policy["allowlisted_dataspaces"])
                .map(fixtureUInt64),
            aliasPricing: MusubiAliasPricingPolicyV1(
                revision: fixtureUInt64(pricing, "revision"),
                length1Xor: fixtureUInt64(pricing, "length_1_xor"),
                length2Xor: fixtureUInt64(pricing, "length_2_xor"),
                length3Xor: fixtureUInt64(pricing, "length_3_xor"),
                length4Xor: fixtureUInt64(pricing, "length_4_xor"),
                length5To32Xor: fixtureUInt64(pricing, "length_5_to_32_xor")
            )
        )
    }

    private func digestFromNewtypeHex(_ raw: Any?) throws -> MusubiDigest32V1 {
        let text = try newtypeText(raw)
        let data = try XCTUnwrap(Data(hexString: text))
        return try MusubiDigest32V1(bytes: [UInt8](data))
    }

    private func optionalNewtypeText(_ raw: Any?) throws -> String? {
        if raw is NSNull { return nil }
        let wrapper = try fixtureArray(raw)
        if wrapper.isEmpty { return nil }
        XCTAssertEqual(wrapper.count, 1)
        return try XCTUnwrap(wrapper.first as? String)
    }

    private func decodeSemantic<T: Decodable>(_ raw: Any?, as type: T.Type) throws -> T {
        let value = try XCTUnwrap(raw)
        let data = try JSONSerialization.data(withJSONObject: value)
        return try JSONDecoder().decode(type, from: data)
    }

    private func digest32(_ raw: Any?) throws -> MusubiDigest32V1 {
        let wrapper = try fixtureArray(raw)
        XCTAssertEqual(wrapper.count, 1)
        let octets = try fixtureArray(try XCTUnwrap(wrapper.first))
        let bytes = try octets.map { raw -> UInt8 in
            let value = try fixtureUInt64(raw)
            return try XCTUnwrap(UInt8(exactly: value))
        }
        return try MusubiDigest32V1(bytes: bytes)
    }

    private func governanceDecision(_ raw: Any?) throws -> MusubiGovernanceDecisionV1 {
        let decision = try fixtureObject(raw)
        try requireKeys(
            decision,
            ["decision_id", "action_digest", "enacted_at_height", "execute_after_height"]
        )
        return try MusubiGovernanceDecisionV1(
            decisionID: fixedBytes32(decision["decision_id"]),
            actionDigest: digest32(decision["action_digest"]),
            enactedAtHeight: fixtureUInt64(decision, "enacted_at_height"),
            executeAfterHeight: fixtureUInt64(decision, "execute_after_height")
        )
    }

    private func fixedBytes32(_ raw: Any?) throws -> [UInt8] {
        let octets = try fixtureArray(raw)
        XCTAssertEqual(octets.count, 32)
        return try octets.map { raw -> UInt8 in
            try XCTUnwrap(UInt8(exactly: fixtureUInt64(raw)))
        }
    }

    private func newtypeText(_ raw: Any?) throws -> String {
        let wrapper = try fixtureArray(raw)
        XCTAssertEqual(wrapper.count, 1)
        return try XCTUnwrap(wrapper.first as? String)
    }

    private func loadFixture() throws -> [String: Any] {
        let data = try Data(contentsOf: fixtureURL())
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func fixtureURL() throws -> URL {
        var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = current.appendingPathComponent("fixtures/musubi/instructions_v1.json")
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            current.deleteLastPathComponent()
        }
        throw MusubiV1Error.invalidValue(
            "fixtures/musubi/instructions_v1.json was not found."
        )
    }

    private func fixtureUInt64(_ object: [String: Any], _ key: String) throws -> UInt64 {
        try fixtureUInt64(object[key])
    }

    private func fixtureUInt64(_ raw: Any?) throws -> UInt64 {
        let number = try XCTUnwrap(raw as? NSNumber)
        guard CFGetTypeID(number) != CFBooleanGetTypeID() else {
            throw MusubiV1Error.invalidValue("Fixture unsigned integers must not be booleans.")
        }
        return try XCTUnwrap(UInt64(number.stringValue))
    }

    private func fixtureObject(_ raw: Any?) throws -> [String: Any] {
        try XCTUnwrap(raw as? [String: Any])
    }

    private func fixtureArray(_ raw: Any?) throws -> [Any] {
        try XCTUnwrap(raw as? [Any])
    }

    private func requireKeys(_ object: [String: Any], _ keys: Set<String>) throws {
        XCTAssertEqual(Set(object.keys), keys)
    }
}
