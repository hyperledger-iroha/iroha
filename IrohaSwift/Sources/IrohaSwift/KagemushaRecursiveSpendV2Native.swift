import Foundation

#if canImport(Darwin)
import Darwin
#endif

extension NoritoNativeBridge {
    private typealias KagemushaV2SymbolProbeFn = @convention(c) () -> Void
    private typealias KagemushaV2FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias KagemushaV2ArchiveOnlyOutFn = @convention(c) (
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
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
    private typealias KagemushaV2ThreeArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2FourArchiveOutFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV2KeyReferenceFn = @convention(c) (
        UInt8, UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaV3ArtifactBeginFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt64>?
    ) -> Int32
    private typealias KagemushaV3ArtifactWriteFn = @convention(c) (
        UInt64, UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV3ArtifactHandleFn = @convention(c) (UInt64) -> Int32
    private typealias KagemushaV3ArtifactSetInstallFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt64>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV3ArtifactSetStatusFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias KagemushaV3ArtifactSetUninstallFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV2TopUpFinalityVerifyFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    func hasKagemushaRecursiveSpendV2Symbols(_ symbols: [String]) -> Bool {
        #if canImport(Darwin)
        symbols.allSatisfy {
            resolveKagemushaV2Symbol($0, as: KagemushaV2SymbolProbeFn.self) != nil
        }
        #else
        _ = symbols
        return false
        #endif
    }

    func kagemushaRecursiveSpendCapabilitiesV3() throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_capabilities_v3",
            as: KagemushaV2ArchiveOnlyOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = function(&output, &outputLength)
        return try copyKagemushaV2Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendTopUpV2(requestArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendVerifyV2(requestArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_verify_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendRedeemV2(requestArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_v2",
            archive: requestArchive
        )
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

    func kagemushaReceiverKeyReferenceV2(
        algorithm: UInt8,
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
                algorithm,
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

    func kagemushaRequestAuthorizationSigningBytesV2(templateArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
            archive: templateArchive
        )
    }

    func kagemushaRequestAuthorizationCreateV2(
        templateArchive: Data,
        signature: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_request_authorization_create_v2",
            first: templateArchive,
            second: signature
        )
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

    func kagemushaRecursiveSpendPeerPaymentFromSplitV2(
        splitResultArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
            archive: splitResultArchive
        )
    }

    func kagemushaRecursiveSpendPeerPaymentValidateV2(
        paymentArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
            archive: paymentArchive
        )
    }

    func kagemushaRecursiveSpendBundleSummaryV2(bundleArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
            archive: bundleArchive
        )
    }

    func kagemushaRecursiveSpendBuildSplitIntentV2(
        requestArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendInitV2(requestArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_init_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendAppendV2(
        requestArchive: Data,
        recipientRequestArchive: Data,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data? {
        try callKagemushaV2TwoArchivesAtTime(
            symbol: "connect_norito_kagemusha_recursive_spend_append_v2",
            first: requestArchive,
            second: recipientRequestArchive,
            milliseconds: verifiedAtMilliseconds
        )
    }

    func kagemushaRecursiveSpendTopUpUnsignedPayloadDigestV2(
        unsignedArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
            archive: unsignedArchive
        )
    }

    func kagemushaTopUpShieldBuildUnsignedV2(
        requestArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendTopUpFinalizeRequestV2(
        unsignedArchive: Data,
        authorizationArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
            first: unsignedArchive,
            second: authorizationArchive
        )
    }

    func kagemushaRecursiveSpendRedeemUnsignedPayloadDigestV2(
        unsignedArchive: Data
    ) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
            archive: unsignedArchive
        )
    }

    func kagemushaRecursiveSpendRedeemFinalizeRequestV2(
        buildResultArchive: Data,
        authorizationArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
            first: buildResultArchive,
            second: authorizationArchive
        )
    }

    func kagemushaTopUpFinalityVerifyV2(
        proofArchive: Data,
        rosterArtifactArchive: Data,
        anchorArchive: Data,
        manifestArchive: Data,
        expectedManifestSHA256: Data
    ) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_topup_finality_verify_v2",
            as: KagemushaV2TopUpFinalityVerifyFn.self
        ) else { return false }
        let status = proofArchive.withUnsafeBytes { proofBuffer in
            rosterArtifactArchive.withUnsafeBytes { rosterBuffer in
                anchorArchive.withUnsafeBytes { anchorBuffer in
                    manifestArchive.withUnsafeBytes { manifestBuffer in
                        expectedManifestSHA256.withUnsafeBytes { digestBuffer in
                            function(
                                proofBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(proofBuffer.count),
                                rosterBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(rosterBuffer.count),
                                anchorBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(anchorBuffer.count),
                                manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(manifestBuffer.count),
                                digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(digestBuffer.count)
                            )
                        }
                    }
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        _ = proofArchive
        _ = rosterArtifactArchive
        _ = anchorArchive
        _ = manifestArchive
        _ = expectedManifestSHA256
        return false
        #endif
    }

    func kagemushaRecursiveSpendArtifactBeginV3(
        manifestArchive: Data,
        expectedManifestSHA256: Data,
        expectedArtifactSHA256: Data
    ) throws -> UInt64? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
            as: KagemushaV3ArtifactBeginFn.self
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

    func kagemushaRecursiveSpendArtifactWriteV3(handle: UInt64, chunk: Data) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
            as: KagemushaV3ArtifactWriteFn.self
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

    func kagemushaRecursiveSpendArtifactFinalizeV3(handle: UInt64) throws -> Bool {
        try callKagemushaV3ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
            handle: handle
        )
    }

    func kagemushaRecursiveSpendArtifactCancelV3(handle: UInt64) throws -> Bool {
        try callKagemushaV3ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
            handle: handle
        )
    }

    func kagemushaRecursiveSpendArtifactSetInstallV3(
        manifestArchive: Data,
        expectedManifestSHA256: Data,
        handles: [UInt64]
    ) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
            as: KagemushaV3ArtifactSetInstallFn.self
        ) else { return false }
        let status = manifestArchive.withUnsafeBytes { manifestBuffer in
            expectedManifestSHA256.withUnsafeBytes { digestBuffer in
                handles.withUnsafeBufferPointer { handlesBuffer in
                    function(
                        manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(manifestBuffer.count),
                        digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(digestBuffer.count),
                        handlesBuffer.baseAddress,
                        CUnsignedLong(handlesBuffer.count)
                    )
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        _ = manifestArchive
        _ = expectedManifestSHA256
        _ = handles
        return false
        #endif
    }

    func kagemushaRecursiveSpendArtifactSetIsInstalledV3(
        manifestArchive: Data,
        expectedManifestSHA256: Data
    ) throws -> Bool? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
            as: KagemushaV3ArtifactSetStatusFn.self
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

    func kagemushaRecursiveSpendArtifactSetUninstallV3(
        expectedManifestSHA256: Data
    ) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
            as: KagemushaV3ArtifactSetUninstallFn.self
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

    private func callKagemushaV3ArtifactHandle(symbol: String, handle: UInt64) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV3ArtifactHandleFn.self)
        else { return false }
        let status = function(handle)
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        return false
        #endif
    }
}
