import Foundation

#if canImport(Darwin)
import Darwin
#endif

extension NoritoNativeBridge {
    private typealias KagemushaV2SymbolProbeFn = @convention(c) () -> Void
    private typealias KagemushaV2FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias KagemushaV4SecretFreeFn = @convention(c) (
        UnsafeMutablePointer<UInt8>?
    ) -> Void
    private typealias KagemushaV2ArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2ArchiveTimeOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2TwoArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2TwoArchiveTimeOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaRecipientLineageVerifyV1Fn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2TwoArchiveThreeOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2ThreeArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2ThreeArchiveTwoOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2FourArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV4FourArchiveTimeOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV4FrontierBuildFn = @convention(c) (
        UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2KeyReferenceFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV4ArtifactBeginFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt64>?
    ) -> Int32
    private typealias KagemushaV4ArtifactWriteFn = @convention(c) (
        UInt64, UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV4ArtifactHandleFn = @convention(c) (UInt64) -> Int32
    private typealias KagemushaV4ArtifactSetInstallFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt64>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV4ArtifactSetStatusFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias KagemushaV4ArtifactSetUninstallFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV4InstalledManifestDigestFn = @convention(c) (
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    func hasKagemushaRecursiveSpendV4Symbols(_ symbols: [String]) -> Bool {
        #if canImport(Darwin)
        symbols.allSatisfy {
            resolveKagemushaV2Symbol($0, as: KagemushaV2SymbolProbeFn.self) != nil
        }
        #else
        _ = symbols
        return false
        #endif
    }

    private func copyKagemushaV2Output(
        status: Int32,
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let freeFunction = resolveKagemushaV2Symbol(
            "connect_norito_free",
            as: KagemushaV2FreeFn.self
        ) else {
            return nil
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let pointer { freeFunction(pointer) }
            throw error
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: pointer,
            length: length,
            free: freeFunction
        )
        #else
        _ = status
        _ = pointer
        _ = length
        return nil
        #endif
    }

    #if canImport(Darwin)
    static func copyKagemushaNativeSecretArchiveOutput(
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong,
        secureFree: (UnsafeMutablePointer<UInt8>?) -> Void
    ) throws -> Data {
        guard let pointer else {
            throw NativeBridgeError.nullPointer
        }
        defer { secureFree(pointer) }
        guard length > 0,
              length <= CUnsignedLong(
                  KagemushaRecursiveSpend.maximumRedemptionChangePreparationArchiveBytesV4
              ) else {
            throw NativeBridgeError.kagemushaProve
        }
        return Data(bytes: pointer, count: Int(length))
    }
    #endif

    func kagemushaReceiverKeyReferenceV2(
        publicKey: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_receiver_key_reference_v2",
            as: KagemushaV2KeyReferenceFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = publicKey.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                &output,
                &outputLength
            )
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    func kagemushaRecipientOutputDeriveV2(
        requestArchive: Data,
        noteOpeningArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recipient_output_derive_v2",
            first: requestArchive,
            second: noteOpeningArchive
        )
    }

    func kagemushaRecursiveSpendRedemptionChangePrepareV4(
        requestArchive: Data
    ) throws -> Data? {
        guard !requestArchive.isEmpty,
              requestArchive.count
                <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4
                    + KagemushaRecursiveSpend.maximumRedemptionChangePreparationArchiveBytesV4
        else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
            as: KagemushaV2ArchiveOutFn.self
        ), let secureFree = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_secret_free_buffer",
            as: KagemushaV4SecretFreeFn.self
        ) else {
            return nil
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = requestArchive.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                &output,
                &outputLength
            )
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let output { secureFree(output) }
            throw error
        }
        return try Self.copyKagemushaNativeSecretArchiveOutput(
            pointer: output,
            length: outputLength,
            secureFree: secureFree
        )
        #else
        return nil
        #endif
    }

    func kagemushaRecipientPaymentRequestSigningBytesV2(payloadArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
            archive: payloadArchive
        )
    }

    func kagemushaRecipientPaymentRequestCreateV2(
        payloadArchive: Data,
        signature: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recipient_payment_request_create_v2",
            first: payloadArchive,
            second: signature
        )
    }

    func kagemushaRecipientPaymentRequestVerifyV2(
        requestArchive: Data,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data? {
        try callKagemushaV2ArchiveAtTime(
            symbol: "connect_norito_kagemusha_recipient_payment_request_verify_v2",
            archive: requestArchive,
            milliseconds: verifiedAtMilliseconds
        )
    }

    func kagemushaRecipientRegistrationLineageVerifyV1(
        requestArchive: Data,
        lineageArchive: Data,
        verifiedAtMilliseconds: UInt64,
        expectedEvaluatedBlockHeight: UInt64,
        expectedEvaluatedBlockHash: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard verifiedAtMilliseconds > 0,
              expectedEvaluatedBlockHeight > 0,
              expectedEvaluatedBlockHash.count == 32,
              expectedEvaluatedBlockHash.contains(where: { $0 != 0 }) else {
            throw NativeBridgeError.kagemushaProve
        }
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
            as: KagemushaRecipientLineageVerifyV1Fn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = requestArchive.withUnsafeBytes { requestBuffer in
            lineageArchive.withUnsafeBytes { lineageBuffer in
                expectedEvaluatedBlockHash.withUnsafeBytes { hashBuffer in
                    function(
                        requestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(requestBuffer.count),
                        lineageBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(lineageBuffer.count),
                        verifiedAtMilliseconds,
                        expectedEvaluatedBlockHeight,
                        hashBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(hashBuffer.count),
                        &output,
                        &outputLength
                    )
                }
            }
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    func kagemushaRequestAuthorizationSigningBytesV2(
        preparationArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
            archive: preparationArchive
        )
    }

    func kagemushaRequestAuthorizationFinalizeHardwareV2(
        preparationArchive: Data,
        authenticatorData: Data,
        derSignature: Data
    ) throws -> (authorizationArchive: Data, rawSignature: Data)? {
        guard !preparationArchive.isEmpty,
              preparationArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2,
              authenticatorData.count <= 4 * 1024,
              !derSignature.isEmpty,
              derSignature.count <= 72 else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
            as: KagemushaV2ThreeArchiveTwoOutFn.self
        ), let freeFunction = resolveKagemushaV2Symbol(
            "connect_norito_free",
            as: KagemushaV2FreeFn.self
        ) else {
            return nil
        }
        var authorization: UnsafeMutablePointer<UInt8>?
        var authorizationLength: CUnsignedLong = 0
        var rawSignature: UnsafeMutablePointer<UInt8>?
        var rawSignatureLength: CUnsignedLong = 0
        let status = preparationArchive.withUnsafeBytes { preparationBuffer in
            authenticatorData.withUnsafeBytes { authenticatorBuffer in
                derSignature.withUnsafeBytes { signatureBuffer in
                    function(
                        preparationBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(preparationBuffer.count),
                        authenticatorBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(authenticatorBuffer.count),
                        signatureBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(signatureBuffer.count),
                        &authorization,
                        &authorizationLength,
                        &rawSignature,
                        &rawSignatureLength
                    )
                }
            }
        }
        defer {
            if let authorization { freeFunction(authorization) }
            if let rawSignature { freeFunction(rawSignature) }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            throw error
        }
        guard let authorization,
              authorizationLength > 0,
              authorizationLength
                <= CUnsignedLong(KagemushaRecursiveSpend.maximumPeerArchiveBytesV2),
              let rawSignature,
              rawSignatureLength == CUnsignedLong(KagemushaDeviceSignatureV2.rawByteCount) else {
            throw NativeBridgeError.kagemushaProve
        }
        return (
            Data(bytes: authorization, count: Int(authorizationLength)),
            Data(bytes: rawSignature, count: Int(rawSignatureLength))
        )
        #else
        return nil
        #endif
    }

    func kagemushaRequestAuthorizationFinalizeIosAppAttestV2(
        preparationArchive: Data,
        assertionObject: Data
    ) throws -> (
        authorizationArchive: Data,
        rawSignature: Data,
        authenticatorData: Data
    )? {
        guard !preparationArchive.isEmpty,
              preparationArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2,
              !assertionObject.isEmpty,
              assertionObject.count
                <= KagemushaRecursiveSpend.maximumIosAppAttestAssertionObjectBytesV2 else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
            as: KagemushaV2TwoArchiveThreeOutFn.self
        ), let freeFunction = resolveKagemushaV2Symbol(
            "connect_norito_free",
            as: KagemushaV2FreeFn.self
        ) else {
            return nil
        }
        var authorization: UnsafeMutablePointer<UInt8>?
        var authorizationLength: CUnsignedLong = 0
        var rawSignature: UnsafeMutablePointer<UInt8>?
        var rawSignatureLength: CUnsignedLong = 0
        var authenticatorData: UnsafeMutablePointer<UInt8>?
        var authenticatorDataLength: CUnsignedLong = 0
        let status = preparationArchive.withUnsafeBytes { preparationBuffer in
            assertionObject.withUnsafeBytes { assertionBuffer in
                function(
                    preparationBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(preparationBuffer.count),
                    assertionBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(assertionBuffer.count),
                    &authorization,
                    &authorizationLength,
                    &rawSignature,
                    &rawSignatureLength,
                    &authenticatorData,
                    &authenticatorDataLength
                )
            }
        }
        defer {
            if let authorization { freeFunction(authorization) }
            if let rawSignature { freeFunction(rawSignature) }
            if let authenticatorData { freeFunction(authenticatorData) }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            throw error
        }
        guard let authorization,
              authorizationLength > 0,
              authorizationLength
                <= CUnsignedLong(KagemushaRecursiveSpend.maximumPeerArchiveBytesV2),
              let rawSignature,
              rawSignatureLength == CUnsignedLong(KagemushaDeviceSignatureV2.rawByteCount),
              let authenticatorData,
              (37...KagemushaRecursiveSpend.maximumIosAppAttestAuthenticatorDataBytesV2)
                .contains(Int(authenticatorDataLength)) else {
            throw NativeBridgeError.kagemushaProve
        }
        return (
            Data(bytes: authorization, count: Int(authorizationLength)),
            Data(bytes: rawSignature, count: Int(rawSignatureLength)),
            Data(bytes: authenticatorData, count: Int(authenticatorDataLength))
        )
        #else
        return nil
        #endif
    }

    func kagemushaReceiverAcknowledgementPayloadV2(
        requestArchive: Data,
        peerPaymentArchive: Data,
        acceptedAtMilliseconds: UInt64
    ) throws -> Data? {
        try callKagemushaV2TwoArchivesAtTime(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
            first: requestArchive,
            second: peerPaymentArchive,
            milliseconds: acceptedAtMilliseconds
        )
    }

    func kagemushaReceiverAcknowledgementSigningBytesV2(payloadArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
            archive: payloadArchive
        )
    }

    func kagemushaReceiverAcknowledgementCreateV2(
        payloadArchive: Data,
        signature: Data,
        requestArchive: Data,
        peerPaymentArchive: Data
    ) throws -> Data? {
        try callKagemushaV2FourArchives(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
            first: payloadArchive,
            second: signature,
            third: requestArchive,
            fourth: peerPaymentArchive
        )
    }

    func kagemushaReceiverAcknowledgementVerifyV2(
        acknowledgementArchive: Data,
        requestArchive: Data,
        peerPaymentArchive: Data
    ) throws -> Data? {
        try callKagemushaV2ThreeArchives(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
            first: acknowledgementArchive,
            second: requestArchive,
            third: peerPaymentArchive
        )
    }

    func kagemushaRecursiveSpendPeerPaymentFromSplitV4(
        splitResultArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
            archive: splitResultArchive
        )
    }

    func kagemushaRecursiveSpendPeerPaymentValidateV4(
        paymentArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
            archive: paymentArchive
        )
    }

    func kagemushaRecursiveSpendBundleSummaryV4(bundleArchive: Data) throws -> Data? {
        guard !bundleArchive.isEmpty,
              bundleArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4 else {
            throw NativeBridgeError.kagemushaProve
        }
        return try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_bundle_summary_v4",
            archive: bundleArchive
        )
    }

    func kagemushaRecursiveSpendTopUpProvenanceBuildV4(
        bundleArchive: Data,
        rosterArchive: Data,
        anchorArchive: Data,
        finalityProofArchive: Data,
        blockHeight: UInt64
    ) throws -> Data? {
        guard !bundleArchive.isEmpty,
              bundleArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4,
              !rosterArchive.isEmpty,
              rosterArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityRosterMaximumArchiveBytes,
              !anchorArchive.isEmpty,
              anchorArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes,
              !finalityProofArchive.isEmpty,
              finalityProofArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityProofMaximumArchiveBytes,
              blockHeight > 0 else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
            as: KagemushaV4FourArchiveTimeOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = bundleArchive.withUnsafeBytes { bundle in
            rosterArchive.withUnsafeBytes { roster in
                anchorArchive.withUnsafeBytes { anchor in
                    finalityProofArchive.withUnsafeBytes { proof in
                        function(
                            bundle.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(bundle.count),
                            roster.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(roster.count),
                            anchor.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(anchor.count),
                            proof.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(proof.count),
                            blockHeight,
                            &output,
                            &outputLength
                        )
                    }
                }
            }
        }
        let result = try copyKagemushaV2Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        guard result.map({ !$0.isEmpty && $0.count
            <= KagemushaRecursiveSpend.maximumTopUpProvenanceArchiveBytesV4 }) ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        return result
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendTopUpProvenanceValidateV4(
        bundleArchive: Data,
        provenanceArchive: Data,
        blockHeight: UInt64
    ) throws -> Data? {
        guard !bundleArchive.isEmpty,
              bundleArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4,
              !provenanceArchive.isEmpty,
              provenanceArchive.count
                <= KagemushaRecursiveSpend.maximumTopUpProvenanceArchiveBytesV4,
              blockHeight > 0 else {
            throw NativeBridgeError.kagemushaProve
        }
        let result = try callKagemushaV2TwoArchivesAtTime(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
            first: bundleArchive,
            second: provenanceArchive,
            milliseconds: blockHeight
        )
        guard result.map({ !$0.isEmpty && $0.count
            <= KagemushaRecursiveSpend.maximumTopUpProvenanceArchiveBytesV4 }) ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        return result
    }

    func kagemushaOutputMembershipFrontierBuildV4(
        leafIndex: UInt32,
        zeroPath: PrivacyConfidentialMerklePathWitnessV2
    ) throws -> Data? {
        let siblings = Data(zeroPath.siblings.flatMap { $0 })
        guard zeroPath.siblings.count
                == PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2,
              siblings.count
                == PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2 * 32,
              zeroPath.directions.count
                == PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2,
              zeroPath.root.count == 32,
              zeroPath.root.contains(where: { $0 != 0 }) else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_output_membership_frontier_build_v4",
            as: KagemushaV4FrontierBuildFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = siblings.withUnsafeBytes { siblingBytes in
            zeroPath.directions.withUnsafeBytes { directions in
                zeroPath.root.withUnsafeBytes { root in
                    function(
                        leafIndex,
                        siblingBytes.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(siblingBytes.count),
                        directions.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(directions.count),
                        root.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(root.count),
                        &output,
                        &outputLength
                    )
                }
            }
        }
        let result = try copyKagemushaV2Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        guard result.map({ !$0.isEmpty && $0.count
            <= KagemushaRecursiveSpend.maximumOutputMembershipFrontierArchiveBytesV4 })
            ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        return result
        #else
        return nil
        #endif
    }

    func kagemushaOutputMembershipPathsDeriveV4(
        frontierArchive: Data,
        recipientCommitment: Data?,
        changeCommitment: Data?
    ) throws -> Data? {
        guard !frontierArchive.isEmpty,
              frontierArchive.count
                <= KagemushaRecursiveSpend.maximumOutputMembershipFrontierArchiveBytesV4,
              recipientCommitment != nil || changeCommitment != nil,
              recipientCommitment.map({ $0.count == 32 && $0.contains(where: { $0 != 0 }) })
                ?? true,
              changeCommitment.map({ $0.count == 32 && $0.contains(where: { $0 != 0 }) })
                ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_output_membership_paths_derive_v4",
            as: KagemushaV2ThreeArchiveOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let invoke = {
            (recipientPointer: UnsafePointer<UInt8>?, recipientLength: Int,
             changePointer: UnsafePointer<UInt8>?, changeLength: Int) -> Int32 in
            frontierArchive.withUnsafeBytes { frontier in
                function(
                    frontier.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(frontier.count),
                    recipientPointer,
                    CUnsignedLong(recipientLength),
                    changePointer,
                    CUnsignedLong(changeLength),
                    &output,
                    &outputLength
                )
            }
        }
        let status: Int32
        switch (recipientCommitment, changeCommitment) {
        case let (.some(recipient), .some(change)):
            status = recipient.withUnsafeBytes { recipientBytes in
                change.withUnsafeBytes { changeBytes in
                    invoke(
                        recipientBytes.bindMemory(to: UInt8.self).baseAddress,
                        recipientBytes.count,
                        changeBytes.bindMemory(to: UInt8.self).baseAddress,
                        changeBytes.count
                    )
                }
            }
        case let (.some(recipient), nil):
            status = recipient.withUnsafeBytes { recipientBytes in
                invoke(
                    recipientBytes.bindMemory(to: UInt8.self).baseAddress,
                    recipientBytes.count,
                    nil,
                    0
                )
            }
        case let (nil, .some(change)):
            status = change.withUnsafeBytes { changeBytes in
                invoke(
                    nil,
                    0,
                    changeBytes.bindMemory(to: UInt8.self).baseAddress,
                    changeBytes.count
                )
            }
        case (nil, nil):
            throw NativeBridgeError.kagemushaProve
        }
        let result = try copyKagemushaV2Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        guard result.map({ !$0.isEmpty && $0.count
            <= KagemushaRecursiveSpend.maximumOutputMembershipPathsArchiveBytesV4 })
            ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        return result
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendBranchValidateV4(
        bundleArchive: Data,
        provenanceArchive: Data,
        witnessArchive: Data,
        openingArchive: Data,
        blockHeight: UInt64
    ) throws -> Data? {
        guard !bundleArchive.isEmpty,
              bundleArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4,
              !provenanceArchive.isEmpty,
              provenanceArchive.count
                <= KagemushaRecursiveSpend.maximumTopUpProvenanceArchiveBytesV4,
              !witnessArchive.isEmpty,
              witnessArchive.count <= 1 * 1_024 * 1_024,
              !openingArchive.isEmpty,
              openingArchive.count <= 16 * 1_024,
              blockHeight > 0 else {
            throw NativeBridgeError.kagemushaProve
        }
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
            as: KagemushaV4FourArchiveTimeOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = bundleArchive.withUnsafeBytes { bundle in
            provenanceArchive.withUnsafeBytes { provenance in
                witnessArchive.withUnsafeBytes { witness in
                    openingArchive.withUnsafeBytes { opening in
                        function(
                            bundle.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(bundle.count),
                            provenance.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(provenance.count),
                            witness.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(witness.count),
                            opening.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(opening.count),
                            blockHeight,
                            &output,
                            &outputLength
                        )
                    }
                }
            }
        }
        let result = try copyKagemushaV2Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        guard result.map({ !$0.isEmpty && $0.count
            <= KagemushaRecursiveSpend.maximumOutputMembershipFrontierArchiveBytesV4 })
            ?? true else {
            throw NativeBridgeError.kagemushaProve
        }
        return result
        #else
        return nil
        #endif
    }

    func kagemushaTopUpShieldBuildUnsignedV4(
        requestArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendTopUpUnsignedPayloadDigestV4(
        unsignedArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
            archive: unsignedArchive
        )
    }

    func kagemushaRecursiveSpendTopUpFinalizeRequestV4(
        unsignedArchive: Data,
        authorizationArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
            first: unsignedArchive,
            second: authorizationArchive
        )
    }

    func kagemushaRecursiveSpendRedeemUnsignedPayloadDigestV4(
        unsignedArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
            archive: unsignedArchive
        )
    }

    func kagemushaRecursiveSpendRedeemFinalizeRequestV4(
        buildResultArchive: Data,
        authorizationArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
            first: buildResultArchive,
            second: authorizationArchive
        )
    }

    func kagemushaRecursiveSpendArtifactBeginV4(
        manifestArchive: Data,
        expectedManifestSHA256: Data,
        expectedArtifactSHA256: Data
    ) throws -> UInt64? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
            as: KagemushaV4ArtifactBeginFn.self
        ) else { return nil }
        var handle: UInt64 = 0
        let status = manifestArchive.withUnsafeBytes { manifestBuffer in
            expectedManifestSHA256.withUnsafeBytes { manifestDigestBuffer in
                expectedArtifactSHA256.withUnsafeBytes { artifactDigestBuffer in
                    function(
                        manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(manifestBuffer.count),
                        manifestDigestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(manifestDigestBuffer.count),
                        artifactDigestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(artifactDigestBuffer.count),
                        &handle
                    )
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        guard handle != 0 else { throw NativeBridgeError.invalidKagemushaVerifierOutput }
        return handle
        #else
        _ = manifestArchive
        _ = expectedManifestSHA256
        _ = expectedArtifactSHA256
        return nil
        #endif
    }

    func kagemushaRecursiveSpendArtifactWriteV4(handle: UInt64, chunk: Data) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
            as: KagemushaV4ArtifactWriteFn.self
        ) else { return false }
        let status = chunk.withUnsafeBytes { buffer in
            function(
                handle,
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count)
            )
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        _ = handle
        _ = chunk
        return false
        #endif
    }

    func kagemushaRecursiveSpendArtifactFinalizeV4(handle: UInt64) throws -> Bool {
        try callKagemushaV4ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
            handle: handle
        )
    }

    func kagemushaRecursiveSpendArtifactCancelV4(handle: UInt64) throws -> Bool {
        try callKagemushaV4ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
            handle: handle
        )
    }

    func kagemushaRecursiveSpendArtifactSetInstallV4(
        manifestArchive: Data,
        expectedManifestSHA256: Data,
        trustedPolicyArchive: Data,
        releaseAttestationArchive: Data,
        benchmarkEvidence: Data,
        cryptographicReview: Data,
        promotionRecordArchive: Data,
        handles: [UInt64]
    ) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
            as: KagemushaV4ArtifactSetInstallFn.self
        ) else { return false }
        let status = manifestArchive.withUnsafeBytes { manifestBuffer in
            expectedManifestSHA256.withUnsafeBytes { digestBuffer in
                trustedPolicyArchive.withUnsafeBytes { policyBuffer in
                    releaseAttestationArchive.withUnsafeBytes { attestationBuffer in
                        benchmarkEvidence.withUnsafeBytes { benchmarkBuffer in
                            cryptographicReview.withUnsafeBytes { reviewBuffer in
                                promotionRecordArchive.withUnsafeBytes { promotionBuffer in
                                    handles.withUnsafeBufferPointer { handlesBuffer in
                                        function(
                                            manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(manifestBuffer.count),
                                            digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(digestBuffer.count),
                                            policyBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(policyBuffer.count),
                                            attestationBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(attestationBuffer.count),
                                            benchmarkBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(benchmarkBuffer.count),
                                            reviewBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(reviewBuffer.count),
                                            promotionBuffer.bindMemory(to: UInt8.self).baseAddress,
                                            CUnsignedLong(promotionBuffer.count),
                                            handlesBuffer.baseAddress,
                                            CUnsignedLong(handlesBuffer.count)
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        _ = manifestArchive
        _ = expectedManifestSHA256
        _ = trustedPolicyArchive
        _ = releaseAttestationArchive
        _ = benchmarkEvidence
        _ = cryptographicReview
        _ = promotionRecordArchive
        _ = handles
        return false
        #endif
    }

    func kagemushaRecursiveSpendArtifactSetIsInstalledV4(
        manifestArchive: Data,
        expectedManifestSHA256: Data
    ) throws -> Bool? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
            as: KagemushaV4ArtifactSetStatusFn.self
        ) else { return nil }
        var installed: UInt8 = 0
        let status = manifestArchive.withUnsafeBytes { manifestBuffer in
            expectedManifestSHA256.withUnsafeBytes { digestBuffer in
                function(
                    manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(manifestBuffer.count),
                    digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(digestBuffer.count),
                    &installed
                )
            }
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return installed == 1
        #else
        _ = manifestArchive
        _ = expectedManifestSHA256
        return nil
        #endif
    }

    func kagemushaRecursiveSpendArtifactSetUninstallV4(
        expectedManifestSHA256: Data
    ) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
            as: KagemushaV4ArtifactSetUninstallFn.self
        ) else { return false }
        let status = expectedManifestSHA256.withUnsafeBytes { digestBuffer in
            function(
                digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(digestBuffer.count)
            )
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        _ = expectedManifestSHA256
        return false
        #endif
    }

    func kagemushaRecursiveSpendInstalledManifestSHA256V4() throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
            as: KagemushaV4InstalledManifestDigestFn.self
        ) else { return nil }
        var digest = Data(repeating: 0, count: 32)
        let status = digest.withUnsafeMutableBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count)
            )
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        guard digest.contains(where: { $0 != 0 }) else {
            throw NativeBridgeError.invalidKagemushaVerifierOutput
        }
        return digest
        #else
        return nil
        #endif
    }

    private func callKagemushaV2Archive(symbol: String, archive: Data) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2ArchiveOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = archive.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                &output,
                &outputLength
            )
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV2ArchiveAtTime(
        symbol: String,
        archive: Data,
        milliseconds: UInt64
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2ArchiveTimeOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = archive.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                milliseconds,
                &output,
                &outputLength
            )
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV2TwoArchives(
        symbol: String,
        first: Data,
        second: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2TwoArchiveOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                function(
                    firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(firstBuffer.count),
                    secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(secondBuffer.count),
                    &output,
                    &outputLength
                )
            }
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV2TwoArchivesAtTime(
        symbol: String,
        first: Data,
        second: Data,
        milliseconds: UInt64
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2TwoArchiveTimeOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                function(
                    firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(firstBuffer.count),
                    secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(secondBuffer.count),
                    milliseconds,
                    &output,
                    &outputLength
                )
            }
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV2ThreeArchives(
        symbol: String,
        first: Data,
        second: Data,
        third: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2ThreeArchiveOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                third.withUnsafeBytes { thirdBuffer in
                    function(
                        firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(firstBuffer.count),
                        secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(secondBuffer.count),
                        thirdBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(thirdBuffer.count),
                        &output,
                        &outputLength
                    )
                }
            }
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV2FourArchives(
        symbol: String,
        first: Data,
        second: Data,
        third: Data,
        fourth: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2FourArchiveOutFn.self)
        else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                third.withUnsafeBytes { thirdBuffer in
                    fourth.withUnsafeBytes { fourthBuffer in
                        function(
                            firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(firstBuffer.count),
                            secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(secondBuffer.count),
                            thirdBuffer.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(thirdBuffer.count),
                            fourthBuffer.bindMemory(to: UInt8.self).baseAddress,
                            CUnsignedLong(fourthBuffer.count),
                            &output,
                            &outputLength
                        )
                    }
                }
            }
        }
        return try copyKagemushaV2Output(status: status, pointer: output, length: outputLength)
        #else
        return nil
        #endif
    }

    private func callKagemushaV4ArtifactHandle(symbol: String, handle: UInt64) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV4ArtifactHandleFn.self)
        else { return false }
        let status = function(handle)
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        return false
        #endif
    }
}
