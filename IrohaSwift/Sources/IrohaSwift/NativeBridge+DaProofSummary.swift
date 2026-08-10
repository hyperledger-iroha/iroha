import Foundation

extension NoritoNativeBridge {
    typealias DaProofSummaryFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        CUnsignedLong, UInt64,
        UnsafePointer<CUnsignedLong>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32

    func daProofSummary(
        manifest: Data,
        payload: Data,
        options: ToriiDaProofSummaryOptions
    ) -> Data? {
        #if canImport(Darwin)
        guard let daProofSummaryFn = daProofSummaryFn,
              let freeFn = freeFn else {
            return nil
        }
        guard !manifest.isEmpty, !payload.isEmpty else {
            return nil
        }

        var normalizedLeafIndexes = [CUnsignedLong]()
        normalizedLeafIndexes.reserveCapacity(options.leafIndexes.count)
        for index in options.leafIndexes {
            guard index >= 0 else { return nil }
            normalizedLeafIndexes.append(CUnsignedLong(index))
        }

        var outputPtr: UnsafeMutablePointer<UInt8>? = nil
        var outputLen: CUnsignedLong = 0
        let status = manifest.withUnsafeBytes { manifestBuffer -> Int32 in
            guard let manifestPtr = manifestBuffer.bindMemory(to: UInt8.self).baseAddress else {
                return -1
            }
            return payload.withUnsafeBytes { payloadBuffer -> Int32 in
                guard let payloadPtr = payloadBuffer.bindMemory(to: UInt8.self).baseAddress else {
                    return -1
                }
                return normalizedLeafIndexes.withUnsafeBufferPointer { indexesBuffer -> Int32 in
                    let indexesPtr = indexesBuffer.baseAddress
                    let indexesLen = CUnsignedLong(indexesBuffer.count)
                    return daProofSummaryFn(
                        manifestPtr,
                        CUnsignedLong(manifest.count),
                        payloadPtr,
                        CUnsignedLong(payload.count),
                        CUnsignedLong(max(options.sampleCount, 0)),
                        options.sampleSeed,
                        indexesPtr,
                        indexesLen,
                        &outputPtr,
                        &outputLen
                    )
                }
            }
        }

        guard status == 0, let summaryPtr = outputPtr else {
            if let outputPtr {
                freeFn(outputPtr)
            }
            return nil
        }
        let data = Data(bytes: summaryPtr, count: Int(outputLen))
        freeFn(summaryPtr)
        return data
        #else
        return nil
        #endif
    }

}
