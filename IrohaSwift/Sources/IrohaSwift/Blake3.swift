import Foundation

/// Shared dependency-free BLAKE3 owner for canonical protocol domain hashes.
enum Blake3 {
    private static let iv: [UInt32] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
    ]
    private static let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
    private static let chunkStart: UInt32 = 1
    private static let chunkEnd: UInt32 = 2
    private static let parent: UInt32 = 4
    private static let root: UInt32 = 8
    private static let blockLength = 64
    private static let chunkLength = 1_024

    private struct Output {
        let inputCV: [UInt32]
        let blockWords: [UInt32]
        let counter: UInt64
        let blockLength: UInt32
        let flags: UInt32

        func chainingValue() -> [UInt32] {
            Array(Blake3.compress(
                cv: inputCV,
                blockWords: blockWords,
                counter: counter,
                blockLength: blockLength,
                flags: flags
            ).prefix(8))
        }

        func rootBytes() -> Data {
            let words = Blake3.compress(
                cv: inputCV,
                blockWords: blockWords,
                counter: 0,
                blockLength: blockLength,
                flags: flags | Blake3.root
            )
            var data = Data(capacity: 32)
            for word in words.prefix(8) {
                var littleEndian = word.littleEndian
                withUnsafeBytes(of: &littleEndian) { data.append(contentsOf: $0) }
            }
            return data
        }
    }

    static func hash(_ data: Data) -> Data? {
        let bytes = [UInt8](data)
        let chunkCount = max(1, (bytes.count + chunkLength - 1) / chunkLength)
        var cvStack: [[UInt32]] = []
        if chunkCount > 1 {
            for chunkIndex in 0..<(chunkCount - 1) {
                let start = chunkIndex * chunkLength
                let chunk = Array(bytes[start..<(start + chunkLength)])
                var cv = chunkOutput(chunk, counter: UInt64(chunkIndex)).chainingValue()
                var completedChunks = chunkIndex + 1
                while completedChunks & 1 == 0 {
                    guard let left = cvStack.popLast() else { return nil }
                    cv = parentOutput(left: left, right: cv).chainingValue()
                    completedChunks >>= 1
                }
                cvStack.append(cv)
            }
        }
        let finalStart = (chunkCount - 1) * chunkLength
        let finalChunk = finalStart < bytes.count ? Array(bytes[finalStart...]) : []
        var output = chunkOutput(finalChunk, counter: UInt64(chunkCount - 1))
        while let left = cvStack.popLast() {
            output = parentOutput(left: left, right: output.chainingValue())
        }
        return output.rootBytes()
    }

    private static func chunkOutput(_ bytes: [UInt8], counter: UInt64) -> Output {
        let blockCount = max(1, (bytes.count + blockLength - 1) / blockLength)
        var cv = iv
        if blockCount > 1 {
            for blockIndex in 0..<(blockCount - 1) {
                let start = blockIndex * blockLength
                let block = Array(bytes[start..<(start + blockLength)])
                let flags = blockIndex == 0 ? chunkStart : 0
                cv = Array(compress(
                    cv: cv,
                    blockWords: words(block),
                    counter: counter,
                    blockLength: UInt32(blockLength),
                    flags: flags
                ).prefix(8))
            }
        }
        let finalStart = (blockCount - 1) * blockLength
        let final = finalStart < bytes.count ? Array(bytes[finalStart...]) : []
        var flags = chunkEnd
        if blockCount == 1 { flags |= chunkStart }
        return Output(
            inputCV: cv,
            blockWords: words(final),
            counter: counter,
            blockLength: UInt32(final.count),
            flags: flags
        )
    }

    private static func parentOutput(left: [UInt32], right: [UInt32]) -> Output {
        Output(
            inputCV: iv,
            blockWords: left + right,
            counter: 0,
            blockLength: UInt32(blockLength),
            flags: parent
        )
    }

    private static func words(_ bytes: [UInt8]) -> [UInt32] {
        var padded = bytes
        padded.append(contentsOf: repeatElement(0, count: max(0, blockLength - padded.count)))
        return (0..<16).map { index in
            let start = index * 4
            return UInt32(padded[start])
                | (UInt32(padded[start + 1]) << 8)
                | (UInt32(padded[start + 2]) << 16)
                | (UInt32(padded[start + 3]) << 24)
        }
    }

    private static func compress(
        cv: [UInt32],
        blockWords: [UInt32],
        counter: UInt64,
        blockLength: UInt32,
        flags: UInt32
    ) -> [UInt32] {
        var state = cv + Array(iv.prefix(4)) + [
            UInt32(truncatingIfNeeded: counter),
            UInt32(truncatingIfNeeded: counter >> 32),
            blockLength,
            flags,
        ]
        var message = blockWords
        for _ in 0..<7 {
            round(&state, message)
            message = permutation.map { message[$0] }
        }
        var output = [UInt32](repeating: 0, count: 16)
        for index in 0..<8 {
            output[index] = state[index] ^ state[index + 8]
            output[index + 8] = state[index + 8] ^ cv[index]
        }
        return output
    }

    private static func round(_ state: inout [UInt32], _ message: [UInt32]) {
        mix(&state, 0, 4, 8, 12, message[0], message[1])
        mix(&state, 1, 5, 9, 13, message[2], message[3])
        mix(&state, 2, 6, 10, 14, message[4], message[5])
        mix(&state, 3, 7, 11, 15, message[6], message[7])
        mix(&state, 0, 5, 10, 15, message[8], message[9])
        mix(&state, 1, 6, 11, 12, message[10], message[11])
        mix(&state, 2, 7, 8, 13, message[12], message[13])
        mix(&state, 3, 4, 9, 14, message[14], message[15])
    }

    private static func mix(
        _ state: inout [UInt32],
        _ a: Int,
        _ b: Int,
        _ c: Int,
        _ d: Int,
        _ x: UInt32,
        _ y: UInt32
    ) {
        state[a] = state[a] &+ state[b] &+ x
        state[d] = rotateRight(state[d] ^ state[a], by: 16)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 12)
        state[a] = state[a] &+ state[b] &+ y
        state[d] = rotateRight(state[d] ^ state[a], by: 8)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 7)
    }

    private static func rotateRight(_ value: UInt32, by amount: UInt32) -> UInt32 {
        (value >> amount) | (value << (32 - amount))
    }
}

