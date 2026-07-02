import XCTest
@testable import IrohaSwift

final class ToriiOfflineCashAPIModelsTests: XCTestCase {
    func testEndpointConstantsUseCurrentOfflineNoteRoutes() {
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.keyRefill.path, "/v1/offline/v2/keys/refill")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteIssue.path, "/v1/offline/v2/notes/issue")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteRedeem.path, "/v1/offline/v2/notes/redeem")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.audit.path, "/v1/offline/v2/audit")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.revocationBundle.path, "/v1/offline/revocations/bundle")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.telemetry.path, "/v1/offline/telemetry")
    }

    func testIssuerEndpointConstantsDoNotRegressToRetiredRoutes() {
        let retiredRoutes = Set([
            "/v1/offline/keys/refill",
            "/v1/offline/notes/issue",
            "/v1/offline/notes/redeem",
            "/v1/offline/audit",
        ])
        let issuerRoutes = [
            ToriiOfflineCashAPI.Endpoint.keyRefill.path,
            ToriiOfflineCashAPI.Endpoint.noteIssue.path,
            ToriiOfflineCashAPI.Endpoint.noteRedeem.path,
            ToriiOfflineCashAPI.Endpoint.audit.path,
        ]

        XCTAssertEqual(Set(issuerRoutes).count, issuerRoutes.count)
        for route in issuerRoutes {
            XCTAssertTrue(route.hasPrefix("/v1/offline/v2/"))
            XCTAssertFalse(retiredRoutes.contains(route))
        }
    }

    func testCompactKeyCertificateDefaultsUseCanonicalAttestationProfiles() throws {
        let iosCertificate = try Self.certificate(platform: "ios-appattest").offlineNoteKeyCertificate()
        XCTAssertEqual(iosCertificate.assertionScheme, "apple-appattest-counter-v1")
        XCTAssertEqual(iosCertificate.assertionKeyAlgorithm, "app-attest-p256")
        XCTAssertNil(iosCertificate.assertionUsageCountLimit)

        let androidCertificate = try Self.certificate(
            platform: "android-keymint",
            assertionUsageCountLimit: 1
        ).offlineNoteKeyCertificate()
        XCTAssertEqual(androidCertificate.assertionScheme, "android-keymint-ecdsa-p256-usage-limit-v1")
        XCTAssertEqual(androidCertificate.assertionKeyAlgorithm, "ecdsa-p256-sha256")
        XCTAssertEqual(androidCertificate.assertionUsageCountLimit, 1)
    }

    func testCompactKeyCertificateRejectsNonCanonicalCertificateFields() throws {
        XCTAssertThrowsError(try Self.certificate(
            platform: "android-keymint",
            assertionScheme: "android-keymint-ecdsa-p256-usage-limit",
            assertionUsageCountLimit: 1
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("assertion_scheme"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionKeyAlgorithm: "ed25519"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("assertion_key_algorithm"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: "issuer-signature"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issuer_signature_base64"))
        }

        for invalidPlatform in ["android", "android-keymint ", "Android-keymint", "ios-appattest-android"] {
            XCTAssertThrowsError(try Self.certificate(
                platform: invalidPlatform,
                assertionScheme: "android-keymint-ecdsa-p256-usage-limit-v1",
                assertionKeyAlgorithm: "ecdsa-p256-sha256",
                assertionUsageCountLimit: 1
            ).offlineNoteKeyCertificate()) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
            }
        }
    }

    func testCompactKeyCertificateRejectsRetiredAssertionPublicKeyAlias() throws {
        let retiredAlias = Data(repeating: 3, count: 65).base64EncodedString()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: nil,
            appAttestPublicKeyBase64: retiredAlias
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("app_attest_public_key_base64"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: nil
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("assertion_public_key"))
        }
    }

    func testCompactKeyCertificateRejectsNonCanonicalBase64Encodings() throws {
        let hexPublicKey = Data(repeating: 1, count: 33).map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            publicKey: hexPublicKey
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("public_key"))
        }

        let urlSafeAssertionKey = Data(repeating: 0xFF, count: 32)
            .base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: urlSafeAssertionKey
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("assertion_public_key"))
        }

        let canonicalSignature = Data(repeating: 2, count: 64).base64EncodedString()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: " \(canonicalSignature)"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issuer_signature_base64"))
        }
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: canonicalSignature.replacingOccurrences(of: "=", with: "")
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issuer_signature_base64"))
        }
    }

    func testKeyRefillRequestEncodesSnakeCaseAndRejectsRetiredAttestKeyAlias() throws {
        let deviceProof = try Self.proof()
        let request = try ToriiOfflineKeyRefillRequest(
            operationId: "op-refill",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            attestationKeyId: "attest-key",
            assetDefinitionId: "pkr#sbp",
            existingLineageId: "lineage-1",
            lineageState: try Self.lineageState(),
            localRevision: 3,
            localStateHash: Self.hashHex(3),
            deviceBinding: try Self.binding(),
            deviceProof: deviceProof
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-refill")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-refill")
        XCTAssertNil(json["app_attest_key_id"])
        XCTAssertEqual(json["attestation_key_id"] as? String, "attest-key")
        let keyCertificateBindings = try XCTUnwrap(json["key_certificate_bindings"] as? [[String: Any]])
        XCTAssertEqual(keyCertificateBindings.count, 1)
        XCTAssertEqual(keyCertificateBindings.first?["attestation_key_id"] as? String, "attest-key")
        XCTAssertEqual(keyCertificateBindings.first?["assertion_public_key"] as? String, "assertion-public-key")
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["operationId"])
        let proof = try XCTUnwrap(json["device_proof"] as? [String: Any])
        XCTAssertEqual(proof["challenge_hash_hex"] as? String, deviceProof.challengeHashHex)
        XCTAssertNil(proof["challengeHashHex"])

        let retiredAliasPayload = """
        {
          "operation_id":"op-refill",
          "account_id":"alice@hbl.sbp",
          "device_id":"device-1",
          "offline_public_key":"offline-public-key",
          "app_attest_key_id":"retired-attest-key",
          "asset_definition_id":"pkr#sbp",
          "local_revision":3,
          "local_state_hash":"\(Self.hashHex(3))",
          "device_binding":\(try Self.jsonString(try Self.binding())),
          "device_proof":\(try Self.jsonString(deviceProof))
        }
        """
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineKeyRefillRequest.self,
            from: Data(retiredAliasPayload.utf8)
        )) { error in
            guard case DecodingError.keyNotFound(let key, _) = error else {
                return XCTFail("expected missing attestation_key_id, got \(error)")
            }
            XCTAssertEqual(key.stringValue, "attestation_key_id")
        }
    }

    func testKeyRefillRequestRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.keyRefillRequest(localStateHash: "").localStateHash, "")
        XCTAssertEqual(try Self.keyRefillRequest().localStateHash, Self.hashHex(3))

        func assertInvalidKeyRefill(
            operationId: String = "op-refill",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            offlinePublicKey: String = "offline-public-key",
            attestationKeyId: String = "attest-key",
            assetDefinitionId: String = "pkr#sbp",
            existingLineageId: String? = "lineage-1",
            localStateHash: String = Self.hashHex(3),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.keyRefillRequest(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                attestationKeyId: attestationKeyId,
                assetDefinitionId: assetDefinitionId,
                existingLineageId: existingLineageId,
                localStateHash: localStateHash
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try Self.keyRefillRequestPayload(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                attestationKeyId: attestationKeyId,
                assetDefinitionId: assetDefinitionId,
                existingLineageId: existingLineageId,
                localStateHash: localStateHash
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineKeyRefillRequest.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidKeyRefill(operationId: " op-refill", expectedField: "operation_id")
        try assertInvalidKeyRefill(accountId: "", expectedField: "account_id")
        try assertInvalidKeyRefill(deviceId: "device-1\n", expectedField: "device_id")
        try assertInvalidKeyRefill(offlinePublicKey: " offline-public-key", expectedField: "offline_public_key")
        try assertInvalidKeyRefill(attestationKeyId: "attest-key ", expectedField: "attestation_key_id")
        try assertInvalidKeyRefill(assetDefinitionId: "pkr#sbp ", expectedField: "asset_definition_id")
        try assertInvalidKeyRefill(existingLineageId: " lineage-1", expectedField: "existing_lineage_id")
        try assertInvalidKeyRefill(localStateHash: "state-3", expectedField: "local_state_hash")
        try assertInvalidKeyRefill(
            localStateHash: Self.hashHex(0xab).uppercased(),
            expectedField: "local_state_hash"
        )
    }

    func testDeviceProofRejectsNonCanonicalFields() throws {
        let canonicalChallenge = Self.hashHex(0xab)
        let canonicalAssertion = Data("assertion".utf8).base64EncodedString()
        XCTAssertEqual(try Self.proof().challengeHashHex, canonicalChallenge)
        XCTAssertEqual(try Self.proof(platform: "android").platform, "android")

        func assertInvalidProof(
            platform: String = OfflineNoteV2Constants.iosPlatform,
            attestationKeyId: String = "attest-key",
            challengeHashHex: String = canonicalChallenge,
            assertionBase64: String = canonicalAssertion,
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try ToriiOfflineDeviceProof(
                platform: platform,
                attestationKeyId: attestationKeyId,
                challengeHashHex: challengeHashHex,
                assertionBase64: assertionBase64,
                counter: 1
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try JSONSerialization.data(withJSONObject: [
                "platform": platform,
                "attestation_key_id": attestationKeyId,
                "challenge_hash_hex": challengeHashHex,
                "assertion_base64": assertionBase64,
                "counter": 1,
            ])
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineDeviceProof.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        for invalidPlatform in ["ios-appattest", "android-keymint", "android-keymint ", "Android"] {
            try assertInvalidProof(platform: invalidPlatform, expectedField: "platform")
        }
        for invalidKeyId in ["", " attest-key", "attest-key\n"] {
            try assertInvalidProof(attestationKeyId: invalidKeyId, expectedField: "attestation_key_id")
        }
        for invalidChallenge in Self.nonExactHashHexVariants(canonicalChallenge) {
            try assertInvalidProof(challengeHashHex: invalidChallenge, expectedField: "challenge_hash_hex")
        }
        for invalidAssertion in [
            "",
            " \(canonicalAssertion)",
            "\(canonicalAssertion)\n",
            Data(repeating: 0xff, count: 4).base64EncodedString().replacingOccurrences(of: "/", with: "_"),
            Data([0xff]).base64EncodedString().replacingOccurrences(of: "=", with: ""),
        ] {
            try assertInvalidProof(assertionBase64: invalidAssertion, expectedField: "assertion_base64")
        }
    }

    func testDeviceBindingRejectsNonCanonicalIdentityFields() throws {
        XCTAssertEqual(try Self.binding().offlinePublicKey, "offline-public-key")
        XCTAssertEqual(try Self.binding(platform: "android").platform, "android")
        XCTAssertEqual(
            try Self.binding(platform: OfflineNoteV2Constants.iosPlatform, iosEnvironment: "production").iosEnvironment,
            "production"
        )
        XCTAssertEqual(
            try Self.binding(platform: OfflineNoteV2Constants.iosPlatform, iosEnvironment: "development").iosEnvironment,
            "development"
        )

        func assertInvalidBinding(
            platform: String = OfflineNoteV2Constants.iosPlatform,
            attestationKeyId: String = "attest-key",
            deviceId: String = "device-1",
            offlinePublicKey: String = "offline-public-key",
            assertionPublicKey: String? = "assertion-public-key",
            attestationReportBase64: String = "report",
            iosTeamId: String? = nil,
            iosBundleId: String? = nil,
            iosEnvironment: String? = nil,
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
                platform: platform,
                attestationKeyId: attestationKeyId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                assertionPublicKey: assertionPublicKey,
                attestationReportBase64: attestationReportBase64,
                iosTeamId: iosTeamId,
                iosBundleId: iosBundleId,
                iosEnvironment: iosEnvironment
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            var payload: [String: Any] = [
                "platform": platform,
                "attestation_key_id": attestationKeyId,
                "device_id": deviceId,
                "offline_public_key": offlinePublicKey,
                "attestation_report_base64": attestationReportBase64,
            ]
            if let assertionPublicKey {
                payload["assertion_public_key"] = assertionPublicKey
            }
            if let iosTeamId {
                payload["ios_team_id"] = iosTeamId
            }
            if let iosBundleId {
                payload["ios_bundle_id"] = iosBundleId
            }
            if let iosEnvironment {
                payload["ios_environment"] = iosEnvironment
            }
            let data = try JSONSerialization.data(withJSONObject: payload)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineDeviceBinding.self, from: data),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        for invalidPlatform in ["ios-appattest", "ios-app-attest", "android-keymint", "android-keymint ", "Android"] {
            try assertInvalidBinding(platform: invalidPlatform, expectedField: "platform")
        }
        for (field, expectedField, applyInvalid) in [
            ("attestation_key_id", "attestation_key_id", { try assertInvalidBinding(attestationKeyId: " attest-key", expectedField: "attestation_key_id") }),
            ("device_id", "device_id", { try assertInvalidBinding(deviceId: "device-1\n", expectedField: "device_id") }),
            ("offline_public_key", "offline_public_key", { try assertInvalidBinding(offlinePublicKey: "", expectedField: "offline_public_key") }),
            ("assertion_public_key", "assertion_public_key", { try assertInvalidBinding(assertionPublicKey: " assertion-public-key", expectedField: "assertion_public_key") }),
            ("ios_team_id", "ios_team_id", { try assertInvalidBinding(iosTeamId: " TEAMID1234", expectedField: "ios_team_id") }),
            ("ios_bundle_id", "ios_bundle_id", { try assertInvalidBinding(iosBundleId: "jp.co.soramitsu.iroha.offline ", expectedField: "ios_bundle_id") }),
        ] {
            XCTAssertNoThrow(try applyInvalid(), "expected \(field) to reject as \(expectedField)")
        }
        for invalidEnvironment in ["Production", " production", "sandbox", ""] {
            try assertInvalidBinding(iosEnvironment: invalidEnvironment, expectedField: "ios_environment")
        }
    }

    func testSpendAuthorizationRejectsNonCanonicalFields() throws {
        XCTAssertEqual(try Self.authorization().issuerSignatureBase64, Self.issuerSignatureBase64())

        func assertInvalidAuthorization(
            authorizationId: String = "authorization-1",
            lineageId: String = "lineage-1",
            accountId: String = "alice@hbl.sbp",
            verdictId: String = "verdict-1",
            policyMaxBalance: String = "1000",
            policyMaxTxValue: String = "250",
            issuedAtMs: UInt64 = 1_700_000_000_000,
            refreshAtMs: UInt64 = 1_700_000_100_000,
            expiresAtMs: UInt64 = 1_700_000_200_000,
            issuerSignatureBase64: String = Self.issuerSignatureBase64(),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.authorization(
                authorizationId: authorizationId,
                lineageId: lineageId,
                accountId: accountId,
                verdictId: verdictId,
                policyMaxBalance: policyMaxBalance,
                policyMaxTxValue: policyMaxTxValue,
                issuedAtMs: issuedAtMs,
                refreshAtMs: refreshAtMs,
                expiresAtMs: expiresAtMs,
                issuerSignatureBase64: issuerSignatureBase64
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidAuthorization(authorizationId: " authorization-1", expectedField: "authorization_id")
        try assertInvalidAuthorization(lineageId: "lineage-1\n", expectedField: "lineage_id")
        try assertInvalidAuthorization(accountId: "", expectedField: "account_id")
        try assertInvalidAuthorization(verdictId: " verdict-1", expectedField: "verdict_id")
        try assertInvalidAuthorization(policyMaxBalance: "-1", expectedField: "max_balance")
        try assertInvalidAuthorization(policyMaxTxValue: "-0.01", expectedField: "max_tx_value")
        try assertInvalidAuthorization(refreshAtMs: 1_699_999_999_999, expectedField: "refresh_at_ms")
        try assertInvalidAuthorization(expiresAtMs: 1_700_000_000_000, expectedField: "expires_at_ms")
        try assertInvalidAuthorization(
            issuerSignatureBase64: " \(Self.issuerSignatureBase64())",
            expectedField: "issuer_signature_base64"
        )

        var payload = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.authorization()))
        payload["issuer_signature_base64"] = Self.issuerSignatureBase64().replacingOccurrences(of: "=", with: "")
        let data = try JSONSerialization.data(withJSONObject: payload)
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineSpendAuthorization.self, from: data)) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issuer_signature_base64"))
        }
    }

    func testCashStateRejectsNonCanonicalFields() throws {
        XCTAssertEqual(try Self.lineageState().serverStateHash, Self.hashHex(4))

        func assertInvalidState(
            lineageId: String = "lineage-1",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            offlinePublicKey: String = "offline-public-key",
            assetDefinitionId: String = "pkr#sbp",
            balance: String = "100.00",
            lockedBalance: String = "0",
            serverStateHash: String = Self.hashHex(4),
            issuerSignatureBase64: String = Self.issuerSignatureBase64(),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try ToriiOfflineCashState(
                lineageId: lineageId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                assetDefinitionId: assetDefinitionId,
                balance: balance,
                lockedBalance: lockedBalance,
                serverRevision: 4,
                serverStateHash: serverStateHash,
                pendingLocalRevision: 4,
                authorization: try Self.authorization(),
                issuerSignatureBase64: issuerSignatureBase64
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidState(lineageId: " lineage-1", expectedField: "lineage_id")
        try assertInvalidState(accountId: "", expectedField: "account_id")
        try assertInvalidState(deviceId: "device-1\n", expectedField: "device_id")
        try assertInvalidState(offlinePublicKey: "", expectedField: "offline_public_key")
        try assertInvalidState(assetDefinitionId: "pkr#sbp ", expectedField: "asset_definition_id")
        try assertInvalidState(balance: "-1", expectedField: "balance")
        try assertInvalidState(lockedBalance: "-0.01", expectedField: "locked_balance")
        try assertInvalidState(serverStateHash: "server-state-4", expectedField: "server_state_hash")
        try assertInvalidState(
            issuerSignatureBase64: Self.issuerSignatureBase64().replacingOccurrences(of: "=", with: ""),
            expectedField: "issuer_signature_base64"
        )

        var payload = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.lineageState()))
        payload["server_state_hash"] = Self.hashHex(0xab).uppercased()
        let data = try JSONSerialization.data(withJSONObject: payload)
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineCashState.self, from: data)) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("server_state_hash"))
        }
    }

    func testTransferReceiptRejectsNonCanonicalFields() throws {
        XCTAssertEqual(try Self.transferReceipt().preStateHash, Self.hashHex(1))

        func assertInvalidReceipt(
            version: Int = 1,
            transferId: String = "transfer-1",
            lineageId: String = "lineage-1",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            offlinePublicKey: String = "offline-public-key",
            preBalance: String = "100",
            postBalance: String = "90",
            preLockedBalance: String = "0",
            postLockedBalance: String = "0",
            preStateHash: String = Self.hashHex(1),
            postStateHash: String = Self.hashHex(2),
            counterpartyLineageId: String = "lineage-2",
            counterpartyAccountId: String = "bob@hbl.sbp",
            counterpartyDeviceId: String = "device-2",
            counterpartyOfflinePublicKey: String = "counterparty-offline-public-key",
            amount: String = "10",
            senderSignatureBase64: String = Self.issuerSignatureBase64(4),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.transferReceipt(
                version: version,
                transferId: transferId,
                lineageId: lineageId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                preBalance: preBalance,
                postBalance: postBalance,
                preLockedBalance: preLockedBalance,
                postLockedBalance: postLockedBalance,
                preStateHash: preStateHash,
                postStateHash: postStateHash,
                counterpartyLineageId: counterpartyLineageId,
                counterpartyAccountId: counterpartyAccountId,
                counterpartyDeviceId: counterpartyDeviceId,
                counterpartyOfflinePublicKey: counterpartyOfflinePublicKey,
                amount: amount,
                senderSignatureBase64: senderSignatureBase64
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidReceipt(version: 2, expectedField: "version")
        try assertInvalidReceipt(transferId: " transfer-1", expectedField: "transfer_id")
        try assertInvalidReceipt(lineageId: "lineage-1\n", expectedField: "lineage_id")
        try assertInvalidReceipt(accountId: "", expectedField: "account_id")
        try assertInvalidReceipt(deviceId: "device-1 ", expectedField: "device_id")
        try assertInvalidReceipt(offlinePublicKey: "", expectedField: "offline_public_key")
        try assertInvalidReceipt(preBalance: "-1", expectedField: "pre_balance")
        try assertInvalidReceipt(postBalance: "-1", expectedField: "post_balance")
        try assertInvalidReceipt(preLockedBalance: "-1", expectedField: "pre_locked_balance")
        try assertInvalidReceipt(postLockedBalance: "-1", expectedField: "post_locked_balance")
        try assertInvalidReceipt(preStateHash: Self.hashHex(0xab).uppercased(), expectedField: "pre_state_hash")
        try assertInvalidReceipt(postStateHash: "post-state", expectedField: "post_state_hash")
        try assertInvalidReceipt(counterpartyLineageId: "", expectedField: "counterparty_lineage_id")
        try assertInvalidReceipt(counterpartyAccountId: " bob@hbl.sbp", expectedField: "counterparty_account_id")
        try assertInvalidReceipt(counterpartyDeviceId: "device-2\n", expectedField: "counterparty_device_id")
        try assertInvalidReceipt(
            counterpartyOfflinePublicKey: "",
            expectedField: "counterparty_offline_public_key"
        )
        try assertInvalidReceipt(amount: "-10", expectedField: "amount")
        try assertInvalidReceipt(
            senderSignatureBase64: " \(Self.issuerSignatureBase64(4))",
            expectedField: "sender_signature_base64"
        )

        var payload = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.transferReceipt()))
        payload["sender_signature_base64"] = Self.issuerSignatureBase64(4).replacingOccurrences(of: "=", with: "")
        let data = try JSONSerialization.data(withJSONObject: payload)
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineTransferReceipt.self, from: data)) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("sender_signature_base64"))
        }
    }

    func testIssueSettlementRequestCarriesLineageState() throws {
        let request = try ToriiOfflineNoteIssueSettlementRequest(
            operationId: "op-issue",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            lineageId: "lineage-1",
            assetDefinitionId: "pkr#sbp",
            amount: "50.00",
            noteCommitment: Self.hashHex(9),
            lineageState: try Self.lineageState(),
            localBalance: "100.00",
            localRevision: 4,
            localStateHash: Self.hashHex(4),
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof()
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-issue")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-issue")
        XCTAssertEqual(json["offline_public_key"] as? String, "offline-public-key")
        XCTAssertEqual(json["note_commitment"] as? String, Self.hashHex(9))
        let keyCertificateBindings = try XCTUnwrap(json["key_certificate_bindings"] as? [[String: Any]])
        XCTAssertEqual(keyCertificateBindings.count, 1)
        XCTAssertEqual(keyCertificateBindings.first?["attestation_key_id"] as? String, "attest-key")
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["offlinePublicKey"])
        XCTAssertNil(json["lineageState"])
    }

    func testIssueSettlementRequestRejectsNonExactNoteCommitmentHex() throws {
        let canonical = Self.hashHex(0xab)
        let request = try Self.issueSettlementRequest(noteCommitment: canonical)
        XCTAssertEqual(request.noteCommitment, canonical)

        for invalid in Self.nonExactHashHexVariants(canonical) {
            XCTAssertThrowsError(
                try Self.issueSettlementRequest(noteCommitment: invalid),
                "accepted non-exact note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("note_commitment"))
            }
        }
    }

    func testIssueSettlementRequestRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.issueSettlementRequest().localStateHash, Self.hashHex(4))

        func assertInvalidIssueRequest(
            operationId: String = "op-issue",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            offlinePublicKey: String = "offline-public-key",
            lineageId: String = "lineage-1",
            assetDefinitionId: String = "pkr#sbp",
            amount: String = "50.00",
            localBalance: String = "100.00",
            localStateHash: String = Self.hashHex(4),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.issueSettlementRequest(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                lineageId: lineageId,
                assetDefinitionId: assetDefinitionId,
                amount: amount,
                localBalance: localBalance,
                localStateHash: localStateHash
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try Self.issueSettlementRequestPayload(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                offlinePublicKey: offlinePublicKey,
                lineageId: lineageId,
                assetDefinitionId: assetDefinitionId,
                amount: amount,
                localBalance: localBalance,
                localStateHash: localStateHash
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineNoteIssueSettlementRequest.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidIssueRequest(operationId: " op-issue", expectedField: "operation_id")
        try assertInvalidIssueRequest(accountId: "", expectedField: "account_id")
        try assertInvalidIssueRequest(deviceId: "device-1\n", expectedField: "device_id")
        try assertInvalidIssueRequest(offlinePublicKey: " offline-public-key", expectedField: "offline_public_key")
        try assertInvalidIssueRequest(lineageId: "lineage-1 ", expectedField: "lineage_id")
        try assertInvalidIssueRequest(assetDefinitionId: "pkr#sbp ", expectedField: "asset_definition_id")
        try assertInvalidIssueRequest(amount: "-50.00", expectedField: "amount")
        try assertInvalidIssueRequest(localBalance: "-1", expectedField: "local_balance")
        try assertInvalidIssueRequest(
            localStateHash: Self.hashHex(0xab).uppercased(),
            expectedField: "local_state_hash"
        )
    }

    func testSettlementProofRejectsNonExactNoteCommitmentHex() throws {
        let canonical = Self.hashHex(0xab)
        let proof = try Self.settlementProof(noteCommitment: canonical)
        XCTAssertEqual(proof.noteCommitment, canonical)
        XCTAssertNoThrow(try Self.settlementProof(noteCommitment: nil))

        for invalid in Self.nonExactHashHexVariants(canonical) {
            XCTAssertThrowsError(
                try Self.settlementProof(noteCommitment: invalid),
                "accepted non-exact settlement note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("note_commitment"))
            }

            let payload = try JSONSerialization.data(withJSONObject: [
                "operation_id": "op-issue",
                "kind": "load",
                "account_id": "alice@hbl.sbp",
                "device_id": "device-1",
                "asset_definition_id": "pkr#sbp",
                "amount": "50.00",
                "pre_balance": "100.00",
                "post_balance": "150.00",
                "entry_hash": Self.hashHex(0x07),
                "chain_tx_hash": Self.hashHex(0x08),
                "block_height": 7,
                "issued_at_ms": 1_700_000_000_000,
                "note_commitment": invalid,
                "issuer_signature_base64": Self.issuerSignatureBase64(),
            ])
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineSettlementProof.self, from: payload),
                "decoded non-exact settlement note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("note_commitment"))
            }
        }
    }

    func testSettlementProofRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.settlementProof(noteCommitment: nil).entryHash, Self.hashHex(0x07))

        func assertInvalidSettlement(
            operationId: String = "op-issue",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            assetDefinitionId: String = "pkr#sbp",
            amount: String = "50.00",
            preBalance: String = "100.00",
            postBalance: String = "150.00",
            entryHash: String = Self.hashHex(0x07),
            chainTxHash: String = Self.hashHex(0x08),
            issuerSignatureBase64: String = Self.issuerSignatureBase64(),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) {
            XCTAssertThrowsError(try ToriiOfflineSettlementProof(
                operationId: operationId,
                kind: .load,
                accountId: accountId,
                deviceId: deviceId,
                assetDefinitionId: assetDefinitionId,
                amount: amount,
                preBalance: preBalance,
                postBalance: postBalance,
                entryHash: entryHash,
                chainTxHash: chainTxHash,
                blockHeight: 7,
                issuedAtMs: 1_700_000_000_000,
                noteCommitment: nil,
                issuerSignatureBase64: issuerSignatureBase64
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        assertInvalidSettlement(operationId: " op-issue", expectedField: "operation_id")
        assertInvalidSettlement(accountId: "", expectedField: "account_id")
        assertInvalidSettlement(deviceId: "device-1\n", expectedField: "device_id")
        assertInvalidSettlement(assetDefinitionId: "pkr#sbp ", expectedField: "asset_definition_id")
        assertInvalidSettlement(amount: "-50.00", expectedField: "amount")
        assertInvalidSettlement(preBalance: "-1", expectedField: "pre_balance")
        assertInvalidSettlement(postBalance: "-1", expectedField: "post_balance")
        assertInvalidSettlement(entryHash: Self.hashHex(0xab).uppercased(), expectedField: "entry_hash")
        assertInvalidSettlement(chainTxHash: "chain-tx", expectedField: "chain_tx_hash")
        assertInvalidSettlement(
            issuerSignatureBase64: Self.issuerSignatureBase64().replacingOccurrences(of: "=", with: ""),
            expectedField: "issuer_signature_base64"
        )

        let payload = try JSONSerialization.data(withJSONObject: [
            "operation_id": "op-issue",
            "kind": "load",
            "account_id": "alice@hbl.sbp",
            "device_id": "device-1",
            "asset_definition_id": "pkr#sbp",
            "amount": "50.00",
            "pre_balance": "100.00",
            "post_balance": "150.00",
            "entry_hash": Self.hashHex(0x07),
            "chain_tx_hash": Self.hashHex(0x08),
            "block_height": 7,
            "issued_at_ms": 1_700_000_000_000,
            "issuer_signature_base64": " \(Self.issuerSignatureBase64())",
        ])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineSettlementProof.self, from: payload)) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issuer_signature_base64"))
        }
    }

    func testIssueSettlementResponseRejectsNonExactIssuedNoteCommitmentHex() throws {
        let canonical = Self.hashHex(0xab)
        let response = try Self.issueSettlementResponse(issuedNoteCommitment: canonical)
        XCTAssertEqual(response.issuedNoteCommitment, canonical)
        XCTAssertNoThrow(try Self.issueSettlementResponse(issuedNoteCommitment: nil))

        for invalid in Self.nonExactHashHexVariants(canonical) {
            XCTAssertThrowsError(
                try Self.issueSettlementResponse(issuedNoteCommitment: invalid),
                "accepted non-exact issued_note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issued_note_commitment"))
            }

            let payload = try Self.issueSettlementResponsePayload(issuedNoteCommitment: invalid)
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineNoteIssueSettlementResponse.self, from: payload),
                "decoded non-exact issued_note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("issued_note_commitment"))
            }
        }
    }

    func testRedeemSettlementRequestEncodesSnakeCaseContract() throws {
        let redemption = try ToriiOfflineRedemptionProof(
            sourceNoteCommitment: Self.hashHex(1),
            inputNullifiers: [Self.hashHex(3)],
            senderKeyCertificate: try Self.certificate(),
            recipientAccountId: "alice@hbl.sbp",
            assetDefinitionId: "pkr#sbp",
            amount: "25.00",
            recursiveProof: OfflineRecursiveProof(
                publicInputsHashHex: Self.hashHex(5),
                proofBytesBase64: Data("proof".utf8).base64EncodedString()
            )
        )
        let request = try ToriiOfflineNoteRedeemSettlementRequest(
            operationId: "op-redeem",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            lineageId: "lineage-1",
            assetDefinitionId: "pkr#sbp",
            amount: "25.00",
            localBalance: "100.00",
            localRevision: 4,
            localStateHash: Self.hashHex(4),
            pendingReceipts: [],
            paymentTokens: [],
            paymentTokensNoritoBase64: ["native-token"],
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof(),
            redemption: redemption
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-redeem")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-redeem")
        XCTAssertEqual(json["amount"] as? String, "25.00")
        XCTAssertEqual(json["local_balance"] as? String, "100.00")
        XCTAssertEqual(json["payment_tokens_norito_base64"] as? [String], ["native-token"])
        XCTAssertNil(json["operationId"])
        XCTAssertNil(json["localBalance"])
        XCTAssertNil(json["paymentTokensNoritoBase64"])
        let redemptionJSON = try XCTUnwrap(json["redemption"] as? [String: Any])
        XCTAssertEqual(redemptionJSON["source_note_commitment"] as? String, Self.hashHex(1))
        XCTAssertNil(redemptionJSON["sourceNoteCommitment"])
        let recursiveProof = try XCTUnwrap(redemptionJSON["recursive_proof"] as? [String: Any])
        XCTAssertEqual(recursiveProof["public_inputs_hash_hex"] as? String, Self.hashHex(5))
    }

    func testRedeemSettlementRequestRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.redeemSettlementRequest().localStateHash, Self.hashHex(4))

        func assertInvalidRedeemRequest(
            operationId: String = "op-redeem",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            lineageId: String = "lineage-1",
            assetDefinitionId: String = "pkr#sbp",
            amount: String = "25.00",
            localBalance: String = "100.00",
            localStateHash: String = Self.hashHex(4),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.redeemSettlementRequest(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                lineageId: lineageId,
                assetDefinitionId: assetDefinitionId,
                amount: amount,
                localBalance: localBalance,
                localStateHash: localStateHash
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try Self.redeemSettlementRequestPayload(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                lineageId: lineageId,
                assetDefinitionId: assetDefinitionId,
                amount: amount,
                localBalance: localBalance,
                localStateHash: localStateHash
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineNoteRedeemSettlementRequest.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidRedeemRequest(operationId: " op-redeem", expectedField: "operation_id")
        try assertInvalidRedeemRequest(accountId: "", expectedField: "account_id")
        try assertInvalidRedeemRequest(deviceId: "device-1\n", expectedField: "device_id")
        try assertInvalidRedeemRequest(lineageId: "lineage-1 ", expectedField: "lineage_id")
        try assertInvalidRedeemRequest(assetDefinitionId: "pkr#sbp ", expectedField: "asset_definition_id")
        try assertInvalidRedeemRequest(amount: "-25.00", expectedField: "amount")
        try assertInvalidRedeemRequest(localBalance: "-1", expectedField: "local_balance")
        try assertInvalidRedeemRequest(
            localStateHash: Self.hashHex(0xab).uppercased(),
            expectedField: "local_state_hash"
        )
    }

    func testRedemptionProofRejectsNonExactHashFields() throws {
        let sourceCommitment = Self.hashHex(0xab)
        let inputNullifier = Self.hashHex(0xcd)
        let proof = try Self.redemptionProof(
            sourceNoteCommitment: sourceCommitment,
            inputNullifiers: [inputNullifier]
        )
        XCTAssertEqual(proof.sourceNoteCommitment, sourceCommitment)
        XCTAssertEqual(proof.inputNullifiers, [inputNullifier])

        for invalid in Self.nonExactHashHexVariants(sourceCommitment) {
            XCTAssertThrowsError(
                try Self.redemptionProof(sourceNoteCommitment: invalid, inputNullifiers: [inputNullifier]),
                "accepted non-exact source_note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("source_note_commitment"))
            }

            let payload = try Self.redemptionProofPayload(
                sourceNoteCommitment: invalid,
                inputNullifiers: [inputNullifier]
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineRedemptionProof.self, from: payload),
                "decoded non-exact source_note_commitment \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("source_note_commitment"))
            }
        }

        for invalid in Self.nonExactHashHexVariants(inputNullifier) {
            XCTAssertThrowsError(
                try Self.redemptionProof(sourceNoteCommitment: sourceCommitment, inputNullifiers: [invalid]),
                "accepted non-exact input_nullifiers value \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("input_nullifiers"))
            }

            let payload = try Self.redemptionProofPayload(
                sourceNoteCommitment: sourceCommitment,
                inputNullifiers: [invalid]
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineRedemptionProof.self, from: payload),
                "decoded non-exact input_nullifiers value \(invalid.debugDescription)"
            ) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("input_nullifiers"))
            }
        }
    }

    func testRedemptionProofRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.redemptionProof().amount, "25.00")

        func assertInvalidRedemption(
            sourceNoteCommitment: String = Self.hashHex(1),
            inputNullifiers: [String] = [Self.hashHex(3)],
            recipientAccountId: String = "alice@hbl.sbp",
            assetDefinitionId: String = "pkr#sbp",
            amount: String = "25.00",
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.redemptionProof(
                sourceNoteCommitment: sourceNoteCommitment,
                inputNullifiers: inputNullifiers,
                recipientAccountId: recipientAccountId,
                assetDefinitionId: assetDefinitionId,
                amount: amount
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try Self.redemptionProofPayload(
                sourceNoteCommitment: sourceNoteCommitment,
                inputNullifiers: inputNullifiers,
                recipientAccountId: recipientAccountId,
                assetDefinitionId: assetDefinitionId,
                amount: amount
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineRedemptionProof.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidRedemption(inputNullifiers: [], expectedField: "input_nullifiers")
        try assertInvalidRedemption(recipientAccountId: " alice@hbl.sbp", expectedField: "recipient_account_id")
        try assertInvalidRedemption(assetDefinitionId: "pkr#sbp\n", expectedField: "asset_definition_id")
        try assertInvalidRedemption(amount: "-25.00", expectedField: "amount")
    }

    func testRecursiveProofRejectsNonCanonicalBase64Encodings() throws {
        let canonicalProofBytes = Data(repeating: 3, count: 64).base64EncodedString()
        XCTAssertNoThrow(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: canonicalProofBytes
        ).offlineNoteRecursiveProof())

        let hexProofBytes = Data(repeating: 4, count: 33).map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: hexProofBytes
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("proof_bytes_base64"))
        }

        let urlSafeProofBytes = Data(repeating: 0xFF, count: 64)
            .base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: urlSafeProofBytes
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("proof_bytes_base64"))
        }

        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: " \(canonicalProofBytes)"
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("proof_bytes_base64"))
        }

        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: canonicalProofBytes.replacingOccurrences(of: "=", with: "")
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("proof_bytes_base64"))
        }
    }

    func testSettlementResponseDecodesCanonicalAmounts() throws {
        let response = try JSONDecoder().decode(
            ToriiOfflineNoteRedeemSettlementResponse.self,
            from: Data("""
            {
              "operation_id":"op-redeem",
              "settlement":{
                "operation_id":"op-redeem",
                "kind":"redeem",
                "account_id":"alice@hbl.sbp",
                "device_id":"device-1",
                "asset_definition_id":"pkr#sbp",
                "amount":"25.00",
                "pre_balance":"100.00",
                "post_balance":"75.00",
                "entry_hash":"\(Self.hashHex(0x07))",
                "chain_tx_hash":"\(Self.hashHex(0x08))",
                "block_height":7,
                "issued_at_ms":1700000000000,
                "issuer_signature_base64":"\(Self.issuerSignatureBase64())"
              },
              "local_balance":"75.00",
              "locked_balance":"0",
              "local_revision":5,
              "local_state_hash":"\(Self.hashHex(5))",
              "accepted_receipt_ids":["receipt-1"]
            }
            """.utf8)
        )

        XCTAssertEqual(response.operationId, "op-redeem")
        XCTAssertEqual(response.settlement.kind, .redeem)
        XCTAssertEqual(response.settlement.amount, "25.00")
        XCTAssertEqual(response.settlement.postBalance, "75.00")
        XCTAssertEqual(response.acceptedReceiptIds, ["receipt-1"])
    }

    func testResponseDtosRejectNonCanonicalStateFields() throws {
        XCTAssertThrowsError(try ToriiOfflineKeyRefillResponse(operationId: " op-refill")) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("operation_id"))
        }
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineKeyRefillResponse.self,
            from: Data(#"{"operation_id":" op-refill"}"#.utf8)
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("operation_id"))
        }

        XCTAssertThrowsError(try ToriiOfflineNoteIssueSettlementResponse(
            operationId: "op-issue",
            settlement: Self.settlementProof(noteCommitment: nil),
            localBalance: "-1",
            localStateHash: Self.hashHex(5)
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("local_balance"))
        }
        let invalidIssueResponse = try JSONSerialization.data(withJSONObject: [
            "operation_id": "op-issue",
            "settlement": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(Self.settlementProof(noteCommitment: nil))
            ),
            "local_balance": "150.00",
            "local_state_hash": "state-5",
        ])
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineNoteIssueSettlementResponse.self,
            from: invalidIssueResponse
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("local_state_hash"))
        }

        XCTAssertThrowsError(try ToriiOfflineNoteRedeemSettlementResponse(
            operationId: "op-redeem",
            settlement: Self.settlementProof(noteCommitment: nil),
            lockedBalance: "-1",
            localStateHash: Self.hashHex(6),
            acceptedReceiptIds: ["receipt-1"]
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("locked_balance"))
        }
        let invalidRedeemResponse = try JSONSerialization.data(withJSONObject: [
            "operation_id": "op-redeem",
            "settlement": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(Self.settlementProof(noteCommitment: nil))
            ),
            "local_state_hash": Self.hashHex(6),
            "accepted_receipt_ids": [" receipt-1"],
        ])
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineNoteRedeemSettlementResponse.self,
            from: invalidRedeemResponse
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("accepted_receipt_ids"))
        }

        XCTAssertThrowsError(try ToriiOfflineAuditResponse(
            operationId: "op-audit",
            acceptedReceiptIds: ["receipt-1\n"]
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("accepted_receipt_ids"))
        }
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineAuditResponse.self,
            from: Data(#"{"operation_id":"op-audit","accepted_receipt_ids":[""]}"#.utf8)
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("accepted_receipt_ids"))
        }
    }

    func testAuditRequestRejectsNonCanonicalSignedFields() throws {
        XCTAssertEqual(try Self.auditRequest().localStateHash, Self.hashHex(6))

        func assertInvalidAuditRequest(
            operationId: String = "op-audit",
            accountId: String = "alice@hbl.sbp",
            deviceId: String = "device-1",
            lineageId: String = "lineage-1",
            localStateHash: String = Self.hashHex(6),
            expectedField: String,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            XCTAssertThrowsError(try Self.auditRequest(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                lineageId: lineageId,
                localStateHash: localStateHash
            ), file: file, line: line) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }

            let payload = try Self.auditRequestPayload(
                operationId: operationId,
                accountId: accountId,
                deviceId: deviceId,
                lineageId: lineageId,
                localStateHash: localStateHash
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiOfflineAuditRequest.self, from: payload),
                file: file,
                line: line
            ) { error in
                XCTAssertEqual(
                    error as? OfflineNotePayloadError,
                    .invalidField(expectedField),
                    file: file,
                    line: line
                )
            }
        }

        try assertInvalidAuditRequest(operationId: " op-audit", expectedField: "operation_id")
        try assertInvalidAuditRequest(accountId: "", expectedField: "account_id")
        try assertInvalidAuditRequest(deviceId: "device-1\n", expectedField: "device_id")
        try assertInvalidAuditRequest(lineageId: "lineage-1 ", expectedField: "lineage_id")
        try assertInvalidAuditRequest(localStateHash: "state-6", expectedField: "local_state_hash")
        try assertInvalidAuditRequest(
            localStateHash: Self.hashHex(0xab).uppercased(),
            expectedField: "local_state_hash"
        )
    }

    private static func binding(
        platform: String = OfflineNoteV2Constants.iosPlatform,
        attestationKeyId: String = "attest-key",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        assertionPublicKey: String? = "assertion-public-key",
        attestationReportBase64: String = "report",
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil
    ) throws -> ToriiOfflineDeviceBinding {
        try ToriiOfflineDeviceBinding(
            platform: platform,
            attestationKeyId: attestationKeyId,
            deviceId: deviceId,
            offlinePublicKey: offlinePublicKey,
            assertionPublicKey: assertionPublicKey,
            attestationReportBase64: attestationReportBase64,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment
        )
    }

    private static func proof(
        platform: String = OfflineNoteV2Constants.iosPlatform,
        attestationKeyId: String = "attest-key",
        challengeHashHex: String? = nil,
        assertionBase64: String = Data("assertion".utf8).base64EncodedString(),
        counter: UInt64? = 1
    ) throws -> ToriiOfflineDeviceProof {
        try ToriiOfflineDeviceProof(
            platform: platform,
            attestationKeyId: attestationKeyId,
            challengeHashHex: challengeHashHex ?? Self.hashHex(0xab),
            assertionBase64: assertionBase64,
            counter: counter
        )
    }

    private static func certificate(
        platform: String = "ios-appattest",
        assertionScheme: String? = nil,
        assertionKeyAlgorithm: String? = nil,
        assertionUsageCountLimit: Int? = nil,
        publicKey: String = Data(repeating: 1, count: 32).base64EncodedString(),
        assertionPublicKey: String? = Data(repeating: 2, count: 65).base64EncodedString(),
        appAttestPublicKeyBase64: String? = nil,
        issuerSignatureBase64: String = Data(repeating: 2, count: 64).base64EncodedString()
    ) throws -> OfflineCompactKeyCertificate {
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        return OfflineCompactKeyCertificate(
            platform: platform,
            keyId: "attest-key",
            deviceId: "device-1",
            accountId: AccountId.make(publicKey: keypair.publicKey),
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit,
            appAttestPublicKeyBase64: appAttestPublicKeyBase64,
            issuerSignatureBase64: issuerSignatureBase64
        )
    }

    private static func lineageState() throws -> ToriiOfflineCashState {
        try ToriiOfflineCashState(
            lineageId: "lineage-1",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: "pkr#sbp",
            balance: "100.00",
            lockedBalance: "0",
            serverRevision: 4,
            serverStateHash: Self.hashHex(4),
            pendingLocalRevision: 4,
            authorization: try Self.authorization(),
            issuerSignatureBase64: Self.issuerSignatureBase64()
        )
    }

    private static func authorization(
        authorizationId: String = "authorization-1",
        lineageId: String = "lineage-1",
        accountId: String = "alice@hbl.sbp",
        verdictId: String = "verdict-1",
        policyMaxBalance: String = "1000",
        policyMaxTxValue: String = "250",
        issuedAtMs: UInt64 = 1_700_000_000_000,
        refreshAtMs: UInt64 = 1_700_000_100_000,
        expiresAtMs: UInt64 = 1_700_000_200_000,
        deviceBinding: ToriiOfflineDeviceBinding? = nil,
        issuerSignatureBase64: String? = nil
    ) throws -> ToriiOfflineSpendAuthorization {
        try ToriiOfflineSpendAuthorization(
            authorizationId: authorizationId,
            lineageId: lineageId,
            accountId: accountId,
            verdictId: verdictId,
            policyMaxBalance: policyMaxBalance,
            policyMaxTxValue: policyMaxTxValue,
            issuedAtMs: issuedAtMs,
            refreshAtMs: refreshAtMs,
            expiresAtMs: expiresAtMs,
            deviceBinding: deviceBinding ?? Self.binding(),
            issuerSignatureBase64: issuerSignatureBase64 ?? Self.issuerSignatureBase64()
        )
    }

    private static func transferReceipt(
        version: Int = 1,
        transferId: String = "transfer-1",
        direction: ToriiOfflineTransferDirection = .outgoing,
        lineageId: String = "lineage-1",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        preBalance: String = "100",
        postBalance: String = "90",
        preLockedBalance: String = "0",
        postLockedBalance: String = "0",
        preStateHash: String? = nil,
        postStateHash: String? = nil,
        localRevision: UInt64 = 5,
        counterpartyLineageId: String = "lineage-2",
        counterpartyAccountId: String = "bob@hbl.sbp",
        counterpartyDeviceId: String = "device-2",
        counterpartyOfflinePublicKey: String = "counterparty-offline-public-key",
        amount: String = "10",
        authorization: ToriiOfflineSpendAuthorization? = nil,
        deviceProof: ToriiOfflineDeviceProof? = nil,
        senderSignatureBase64: String? = nil,
        createdAtMs: UInt64 = 1_700_000_300_000
    ) throws -> ToriiOfflineTransferReceipt {
        try ToriiOfflineTransferReceipt(
            version: version,
            transferId: transferId,
            direction: direction,
            lineageId: lineageId,
            accountId: accountId,
            deviceId: deviceId,
            offlinePublicKey: offlinePublicKey,
            preBalance: preBalance,
            postBalance: postBalance,
            preLockedBalance: preLockedBalance,
            postLockedBalance: postLockedBalance,
            preStateHash: preStateHash ?? Self.hashHex(1),
            postStateHash: postStateHash ?? Self.hashHex(2),
            localRevision: localRevision,
            counterpartyLineageId: counterpartyLineageId,
            counterpartyAccountId: counterpartyAccountId,
            counterpartyDeviceId: counterpartyDeviceId,
            counterpartyOfflinePublicKey: counterpartyOfflinePublicKey,
            amount: amount,
            authorization: authorization,
            deviceProof: deviceProof ?? Self.proof(),
            senderSignatureBase64: senderSignatureBase64 ?? Self.issuerSignatureBase64(4),
            createdAtMs: createdAtMs
        )
    }

    private static func keyRefillRequest(
        operationId: String = "op-refill",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        attestationKeyId: String = "attest-key",
        assetDefinitionId: String = "pkr#sbp",
        existingLineageId: String? = "lineage-1",
        localStateHash: String? = nil
    ) throws -> ToriiOfflineKeyRefillRequest {
        try ToriiOfflineKeyRefillRequest(
            operationId: operationId,
            accountId: accountId,
            deviceId: deviceId,
            offlinePublicKey: offlinePublicKey,
            attestationKeyId: attestationKeyId,
            assetDefinitionId: assetDefinitionId,
            existingLineageId: existingLineageId,
            lineageState: try Self.lineageState(),
            localRevision: 3,
            localStateHash: localStateHash ?? Self.hashHex(3),
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof()
        )
    }

    private static func keyRefillRequestPayload(
        operationId: String = "op-refill",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        attestationKeyId: String = "attest-key",
        assetDefinitionId: String = "pkr#sbp",
        existingLineageId: String? = "lineage-1",
        localStateHash: String? = nil
    ) throws -> Data {
        var object: [String: Any] = [
            "operation_id": operationId,
            "account_id": accountId,
            "device_id": deviceId,
            "offline_public_key": offlinePublicKey,
            "attestation_key_id": attestationKeyId,
            "asset_definition_id": assetDefinitionId,
            "lineage_state": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(try Self.lineageState())
            ),
            "local_revision": 3,
            "local_state_hash": localStateHash ?? Self.hashHex(3),
            "device_binding": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.binding())),
            "device_proof": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.proof())),
        ]
        object["existing_lineage_id"] = existingLineageId
        return try JSONSerialization.data(withJSONObject: object)
    }

    private static func issueSettlementRequest(
        operationId: String = "op-issue",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        lineageId: String = "lineage-1",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "50.00",
        noteCommitment: String? = nil,
        localBalance: String = "100.00",
        localStateHash: String? = nil
    ) throws -> ToriiOfflineNoteIssueSettlementRequest {
        try ToriiOfflineNoteIssueSettlementRequest(
            operationId: operationId,
            accountId: accountId,
            deviceId: deviceId,
            offlinePublicKey: offlinePublicKey,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            noteCommitment: noteCommitment ?? Self.hashHex(9),
            lineageState: try Self.lineageState(),
            localBalance: localBalance,
            localRevision: 4,
            localStateHash: localStateHash ?? Self.hashHex(4),
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof()
        )
    }

    private static func issueSettlementRequestPayload(
        operationId: String = "op-issue",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        lineageId: String = "lineage-1",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "50.00",
        noteCommitment: String? = nil,
        localBalance: String = "100.00",
        localStateHash: String? = nil
    ) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "operation_id": operationId,
            "account_id": accountId,
            "device_id": deviceId,
            "offline_public_key": offlinePublicKey,
            "lineage_id": lineageId,
            "asset_definition_id": assetDefinitionId,
            "amount": amount,
            "note_commitment": noteCommitment ?? Self.hashHex(9),
            "lineage_state": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(try Self.lineageState())
            ),
            "local_balance": localBalance,
            "local_revision": 4,
            "local_state_hash": localStateHash ?? Self.hashHex(4),
            "device_binding": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.binding())),
            "device_proof": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.proof())),
        ])
    }

    private static func settlementProof(noteCommitment: String?) throws -> ToriiOfflineSettlementProof {
        try ToriiOfflineSettlementProof(
            operationId: "op-issue",
            kind: .load,
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            assetDefinitionId: "pkr#sbp",
            amount: "50.00",
            preBalance: "100.00",
            postBalance: "150.00",
            entryHash: Self.hashHex(0x07),
            chainTxHash: Self.hashHex(0x08),
            blockHeight: 7,
            issuedAtMs: 1_700_000_000_000,
            noteCommitment: noteCommitment,
            issuerSignatureBase64: Self.issuerSignatureBase64()
        )
    }

    private static func issueSettlementResponse(
        issuedNoteCommitment: String?
    ) throws -> ToriiOfflineNoteIssueSettlementResponse {
        try ToriiOfflineNoteIssueSettlementResponse(
            operationId: "op-issue",
            settlement: Self.settlementProof(noteCommitment: nil),
            issuedNoteCommitment: issuedNoteCommitment,
            localBalance: "150.00",
            localRevision: 5,
            localStateHash: Self.hashHex(5)
        )
    }

    private static func issueSettlementResponsePayload(
        issuedNoteCommitment: String?
    ) throws -> Data {
        var object: [String: Any] = [
            "operation_id": "op-issue",
            "settlement": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(Self.settlementProof(noteCommitment: nil))
            ),
            "local_balance": "150.00",
            "local_revision": 5,
            "local_state_hash": Self.hashHex(5),
        ]
        object["issued_note_commitment"] = issuedNoteCommitment
        return try JSONSerialization.data(withJSONObject: object)
    }

    private static func redemptionProof(
        sourceNoteCommitment: String? = nil,
        inputNullifiers: [String]? = nil,
        recipientAccountId: String = "alice@hbl.sbp",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "25.00"
    ) throws -> ToriiOfflineRedemptionProof {
        try ToriiOfflineRedemptionProof(
            sourceNoteCommitment: sourceNoteCommitment ?? Self.hashHex(1),
            inputNullifiers: inputNullifiers ?? [Self.hashHex(3)],
            senderKeyCertificate: try Self.certificate(),
            recipientAccountId: recipientAccountId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            recursiveProof: Self.recursiveProof()
        )
    }

    private static func redemptionProofPayload(
        sourceNoteCommitment: String? = nil,
        inputNullifiers: [String]? = nil,
        recipientAccountId: String = "alice@hbl.sbp",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "25.00"
    ) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "source_note_commitment": sourceNoteCommitment ?? Self.hashHex(1),
            "input_nullifiers": inputNullifiers ?? [Self.hashHex(3)],
            "sender_key_certificate": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(try Self.certificate())
            ),
            "recipient_account_id": recipientAccountId,
            "asset_definition_id": assetDefinitionId,
            "amount": amount,
            "recursive_proof": try Self.jsonObject(
                ToriiOfflineCashAPI.canonicalBody(Self.recursiveProof())
            ),
        ])
    }

    private static func redeemSettlementRequest(
        operationId: String = "op-redeem",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        lineageId: String = "lineage-1",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "25.00",
        localBalance: String = "100.00",
        localStateHash: String? = nil
    ) throws -> ToriiOfflineNoteRedeemSettlementRequest {
        try ToriiOfflineNoteRedeemSettlementRequest(
            operationId: operationId,
            accountId: accountId,
            deviceId: deviceId,
            lineageId: lineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            localBalance: localBalance,
            localRevision: 4,
            localStateHash: localStateHash ?? Self.hashHex(4),
            pendingReceipts: [],
            paymentTokens: [],
            paymentTokensNoritoBase64: ["native-token"],
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof(),
            redemption: try Self.redemptionProof()
        )
    }

    private static func redeemSettlementRequestPayload(
        operationId: String = "op-redeem",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        lineageId: String = "lineage-1",
        assetDefinitionId: String = "pkr#sbp",
        amount: String = "25.00",
        localBalance: String = "100.00",
        localStateHash: String? = nil
    ) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "operation_id": operationId,
            "account_id": accountId,
            "device_id": deviceId,
            "lineage_id": lineageId,
            "asset_definition_id": assetDefinitionId,
            "amount": amount,
            "local_balance": localBalance,
            "local_revision": 4,
            "local_state_hash": localStateHash ?? Self.hashHex(4),
            "pending_receipts": [],
            "payment_tokens": [],
            "payment_tokens_norito_base64": ["native-token"],
            "device_binding": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.binding())),
            "device_proof": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.proof())),
            "redemption": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.redemptionProof())),
        ])
    }

    private static func auditRequest(
        operationId: String = "op-audit",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        lineageId: String = "lineage-1",
        localStateHash: String? = nil
    ) throws -> ToriiOfflineAuditRequest {
        try ToriiOfflineAuditRequest(
            operationId: operationId,
            accountId: accountId,
            deviceId: deviceId,
            lineageId: lineageId,
            localRevision: 6,
            localStateHash: localStateHash ?? Self.hashHex(6),
            receipts: [],
            paymentTokens: [],
            paymentTokensNoritoBase64: [],
            deviceBinding: try Self.binding(),
            deviceProof: try Self.proof()
        )
    }

    private static func auditRequestPayload(
        operationId: String = "op-audit",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        lineageId: String = "lineage-1",
        localStateHash: String? = nil
    ) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "operation_id": operationId,
            "account_id": accountId,
            "device_id": deviceId,
            "lineage_id": lineageId,
            "local_revision": 6,
            "local_state_hash": localStateHash ?? Self.hashHex(6),
            "receipts": [],
            "payment_tokens": [],
            "payment_tokens_norito_base64": [],
            "device_binding": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.binding())),
            "device_proof": try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(try Self.proof())),
        ])
    }

    private static func recursiveProof() -> OfflineRecursiveProof {
        OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(5),
            proofBytesBase64: Data("proof".utf8).base64EncodedString()
        )
    }

    private static func nonExactHashHexVariants(_ canonical: String) -> [String] {
        [
            " \(canonical)",
            "\(canonical)\n",
            canonical.uppercased(),
            "0x\(canonical)",
            String(canonical.dropLast()),
            String(repeating: "g", count: 64),
            "",
        ]
    }

    private static func hashHex(_ lastByte: UInt8) -> String {
        (Data(repeating: 0, count: 31) + Data([lastByte])).map { String(format: "%02x", $0) }.joined()
    }

    private static func issuerSignatureBase64(_ byte: UInt8 = 3) -> String {
        Data(repeating: byte, count: 64).base64EncodedString()
    }

    private static func jsonObject(_ data: Data) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private static func jsonString<T: Encodable>(_ value: T) throws -> String {
        String(data: try ToriiOfflineCashAPI.canonicalBody(value), encoding: .utf8) ?? "{}"
    }
}
