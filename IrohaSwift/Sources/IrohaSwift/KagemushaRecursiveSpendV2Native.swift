import Foundation

#if canImport(Darwin)
import Darwin
#endif

extension NoritoNativeBridge {
    private typealias KagemushaV2FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
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
    private typealias KagemushaV2ArtifactBeginFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt32, UnsafeMutablePointer<UInt64>?
    ) -> Int32
    private typealias KagemushaV2ArtifactWriteFn = @convention(c) (
        UInt64, UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32
    private typealias KagemushaV2ArtifactHandleFn = @convention(c) (UInt64) -> Int32

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
        recipientBundleArchive: Data,
        acceptedAtMilliseconds: UInt64
    ) throws -> Data? {
        try callKagemushaV2TwoArchivesAtTime(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
            first: requestArchive,
            second: recipientBundleArchive,
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
        recipientBundleArchive: Data
    ) throws -> Data? {
        try callKagemushaV2FourArchives(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
            first: payloadArchive,
            second: signature,
            third: requestArchive,
            fourth: recipientBundleArchive
        )
    }

    func kagemushaReceiverAcknowledgementVerifyV2(
        acknowledgementArchive: Data,
        requestArchive: Data,
        recipientBundleArchive: Data
    ) throws -> Data? {
        try callKagemushaV2ThreeArchives(
            symbol: "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
            first: acknowledgementArchive,
            second: requestArchive,
            third: recipientBundleArchive
        )
    }

    func kagemushaRecursiveSpendBundleSummaryV2(bundleArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
            archive: bundleArchive
        )
    }

    func kagemushaRecursiveSpendInitV2(
        requestArchive: Data,
        topUpAnchorArchive: Data
    ) throws -> Data? {
        try callKagemushaV2TwoArchives(
            symbol: "connect_norito_kagemusha_recursive_spend_init_v2",
            first: requestArchive,
            second: topUpAnchorArchive
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

    func kagemushaRecursiveSpendRedeemChangeV2(requestArchive: Data) throws -> Data? {
        try callKagemushaV2Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendArtifactBeginV2(
        referenceArchive: Data,
        expectedRole: UInt32
    ) throws -> UInt64? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v2",
            as: KagemushaV2ArtifactBeginFn.self
        ) else { return nil }
        var handle: UInt64 = 0
        let status = referenceArchive.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                expectedRole,
                &handle
            )
        }
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        guard handle != 0 else { throw NativeBridgeError.invalidKagemushaVerifierOutput }
        return handle
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendArtifactWriteV2(handle: UInt64, chunk: Data) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_artifact_write_v2",
            as: KagemushaV2ArtifactWriteFn.self
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
        return false
        #endif
    }

    func kagemushaRecursiveSpendArtifactFinalizeV2(handle: UInt64) throws -> Bool {
        try callKagemushaV2ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_finalize_v2",
            handle: handle
        )
    }

    func kagemushaRecursiveSpendArtifactCancelV2(handle: UInt64) throws -> Bool {
        try callKagemushaV2ArtifactHandle(
            symbol: "connect_norito_kagemusha_recursive_spend_artifact_cancel_v2",
            handle: handle
        )
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

    private func callKagemushaV2ArtifactHandle(symbol: String, handle: UInt64) throws -> Bool {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(symbol, as: KagemushaV2ArtifactHandleFn.self)
        else { return false }
        let status = function(handle)
        if let error = NativeBridgeError.fromStatus(status) { throw error }
        return true
        #else
        return false
        #endif
    }
}
