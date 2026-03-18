import Foundation

enum Blake2s {
    private static let iv: [UInt32] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ]

    private static let sigma: [[UInt8]] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    ]

    @inline(__always) private static func rotr32(_ value: UInt32, by shift: UInt32) -> UInt32 {
        (value >> shift) | (value << (32 - shift))
    }

    private static func compress(
        _ h: inout [UInt32],
        block: UnsafePointer<UInt8>,
        t0: UInt32,
        t1: UInt32,
        f0: UInt32
    ) {
        var m = [UInt32](repeating: 0, count: 16)
        for i in 0..<16 {
            let offset = i &* 4
            var word: UInt32 = 0
            for j in 0..<4 {
                word |= UInt32(block.advanced(by: offset + j).pointee) << (UInt32(j) * 8)
            }
            m[i] = word
        }

        var v = [UInt32](repeating: 0, count: 16)
        for i in 0..<8 { v[i] = h[i] }
        for i in 0..<8 { v[i + 8] = iv[i] }
        v[12] ^= t0
        v[13] ^= t1
        v[14] ^= f0

        func G(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt32, _ y: UInt32) {
            v[a] = v[a] &+ v[b] &+ x
            v[d] = rotr32(v[d] ^ v[a], by: 16)
            v[c] = v[c] &+ v[d]
            v[b] = rotr32(v[b] ^ v[c], by: 12)
            v[a] = v[a] &+ v[b] &+ y
            v[d] = rotr32(v[d] ^ v[a], by: 8)
            v[c] = v[c] &+ v[d]
            v[b] = rotr32(v[b] ^ v[c], by: 7)
        }

        for round in 0..<10 {
            let s = sigma[round]
            G(0, 4, 8, 12, m[Int(s[0])], m[Int(s[1])])
            G(1, 5, 9, 13, m[Int(s[2])], m[Int(s[3])])
            G(2, 6, 10, 14, m[Int(s[4])], m[Int(s[5])])
            G(3, 7, 11, 15, m[Int(s[6])], m[Int(s[7])])
            G(0, 5, 10, 15, m[Int(s[8])], m[Int(s[9])])
            G(1, 6, 11, 12, m[Int(s[10])], m[Int(s[11])])
            G(2, 7, 8, 13, m[Int(s[12])], m[Int(s[13])])
            G(3, 4, 9, 14, m[Int(s[14])], m[Int(s[15])])
        }

        for i in 0..<8 {
            h[i] ^= v[i] ^ v[i + 8]
        }
    }

    static func hash(data: Data, key: Data = Data(), outputLength: Int = 32) -> Data {
        precondition(outputLength > 0 && outputLength <= 32, "BLAKE2s supports up to 32 output bytes")
        precondition(key.count <= 32, "BLAKE2s key must be at most 32 bytes")

        var h = iv
        let param = UInt32(outputLength) | (UInt32(key.count) << 8) | (1 << 16) | (1 << 24)
        h[0] ^= param

        let blockLen = 64
        var t0: UInt32 = 0
        var t1: UInt32 = 0

        func incrementCounter(by amount: Int) {
            let inc = UInt32(amount)
            t0 = t0 &+ inc
            if t0 < inc { t1 = t1 &+ 1 }
        }

        if !key.isEmpty {
            var block = [UInt8](repeating: 0, count: blockLen)
            key.copyBytes(to: &block, count: key.count)
            incrementCounter(by: blockLen)
            block.withUnsafeBufferPointer { buf in
                let flag: UInt32 = data.isEmpty ? ~UInt32(0) : 0
                compress(&h, block: buf.baseAddress!, t0: t0, t1: t1, f0: flag)
            }
        }

        let total = data.count
        if total > 0 {
            data.withUnsafeBytes { rawBuf in
                guard let base = rawBuf.bindMemory(to: UInt8.self).baseAddress else { return }
                var offset = 0
                let fullBlocks = total / blockLen
                let remainder = total % blockLen

                for blockIndex in 0..<fullBlocks {
                    let ptr = base.advanced(by: offset)
                    offset += blockLen
                    incrementCounter(by: blockLen)
                    let isLastFull = (blockIndex == fullBlocks - 1) && (remainder == 0)
                    let flag: UInt32 = isLastFull ? ~UInt32(0) : 0
                    compress(&h, block: ptr, t0: t0, t1: t1, f0: flag)
                }

                if remainder > 0 {
                    var lastBlock = [UInt8](repeating: 0, count: blockLen)
                    for i in 0..<remainder {
                        lastBlock[i] = base.advanced(by: offset + i).pointee
                    }
                    incrementCounter(by: remainder)
                    lastBlock.withUnsafeBufferPointer { buf in
                        compress(&h, block: buf.baseAddress!, t0: t0, t1: t1, f0: ~UInt32(0))
                    }
                }
            }
        } else if key.isEmpty {
            let zeroBlock = [UInt8](repeating: 0, count: blockLen)
            zeroBlock.withUnsafeBufferPointer { buf in
                compress(&h, block: buf.baseAddress!, t0: 0, t1: 0, f0: ~UInt32(0))
            }
        }

        return output(from: h, outputLength: outputLength)
    }

    private static func output(from h: [UInt32], outputLength: Int) -> Data {
        var out = Data(count: outputLength)
        out.withUnsafeMutableBytes { outBuf in
            var idx = 0
            for word in h {
                var le = word.littleEndian
                withUnsafeBytes(of: &le) { wptr in
                    let count = min(4, outputLength - idx)
                    guard count > 0 else { return }
                    outBuf.baseAddress!.advanced(by: idx).copyMemory(from: wptr.baseAddress!, byteCount: count)
                    idx += count
                }
                if idx >= outputLength { break }
            }
        }
        return out
    }
}
