import Foundation

// Blake2b-256 (32-byte digest), no key, one-shot implementation.
public enum Blake2b {
    private static let iv: [UInt64] = [
        0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
        0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179
    ]

    private static let sigma: [[Int]] = [
        [ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15],
        [14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3],
        [11, 8,12, 0, 5, 2,15,13,10,14, 3, 6, 7, 1, 9, 4],
        [ 7, 9, 3, 1,13,12,11,14, 2, 6, 5,10, 4, 0,15, 8],
        [ 9, 0, 5, 7, 2, 4,10,15,14, 1,11,12, 6, 8, 3,13],
        [ 2,12, 6,10, 0,11, 8, 3, 4,13, 7, 5,15,14, 1, 9],
        [12, 5, 1,15,14,13, 4,10, 0, 7, 6, 3, 9, 2, 8,11],
        [13,11, 7,14,12, 1, 3, 9, 5, 0,15, 4, 8, 6, 2,10],
        [ 6,15,14, 9,11, 3, 0, 8,12, 2,13, 7, 1, 4,10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5,15,11, 9,14, 3,12,13, 0],
        [ 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15],
        [14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3],
    ]

    @inline(__always) private static func rotr64(_ x: UInt64, _ c: UInt64) -> UInt64 { (x >> c) | (x << (64 - c)) }

    private static func compress(
        _ h: inout [UInt64],
        block: UnsafePointer<UInt8>,
        t0: UInt64,
        t1: UInt64,
        f0: UInt64
    ) {
        var m = [UInt64](repeating: 0, count: 16)
        for i in 0..<16 {
            let off = i &* 8
            var value: UInt64 = 0
            for j in 0..<8 {
                value |= UInt64(block.advanced(by: off + j).pointee) << (UInt64(j) * 8)
            }
            m[i] = value
        }
        var v = [UInt64](repeating: 0, count: 16)
        for i in 0..<8 { v[i] = h[i] }
        for i in 0..<8 { v[i+8] = iv[i] }
        v[12] ^= t0
        v[13] ^= t1
        v[14] ^= f0

        v.withUnsafeMutableBufferPointer { buf in
            guard let p = buf.baseAddress else { return }

            func G(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt64, _ y: UInt64) {
                p[a] = p[a] &+ p[b] &+ x
                p[d] = rotr64(p[d] ^ p[a], 32)
                p[c] = p[c] &+ p[d]
                p[b] = rotr64(p[b] ^ p[c], 24)
                p[a] = p[a] &+ p[b] &+ y
                p[d] = rotr64(p[d] ^ p[a], 16)
                p[c] = p[c] &+ p[d]
                p[b] = rotr64(p[b] ^ p[c], 63)
            }

            for r in 0..<12 {
                let s = sigma[r]
                G(0, 4, 8, 12, m[s[0]], m[s[1]])
                G(1, 5, 9, 13, m[s[2]], m[s[3]])
                G(2, 6, 10, 14, m[s[4]], m[s[5]])
                G(3, 7, 11, 15, m[s[6]], m[s[7]])
                G(0, 5, 10, 15, m[s[8]], m[s[9]])
                G(1, 6, 11, 12, m[s[10]], m[s[11]])
                G(2, 7, 8, 13, m[s[12]], m[s[13]])
                G(3, 4, 9, 14, m[s[14]], m[s[15]])
            }
        }
        for i in 0..<8 { h[i] ^= v[i] ^ v[i+8] }
    }

    @inline(__always) private static func hash(
        data: Data,
        outputLength: Int,
        personal: Data? = nil
    ) -> Data {
        precondition(outputLength > 0 && outputLength <= 64, "BLAKE2b supports up to 64 output bytes")
        var h = iv
        let outlen = UInt8(outputLength)
        h[0] ^= 0x01010000 ^ UInt64(outlen)

        if let personal = personal, !personal.isEmpty {
            var buffer = [UInt8](repeating: 0, count: 16)
            personal.copyBytes(to: &buffer, count: min(personal.count, buffer.count))
            let p0 = buffer[0..<8].enumerated().reduce(UInt64(0)) { acc, element in
                let (index, value) = element
                return acc | (UInt64(value) << (UInt64(index) * 8))
            }
            let p1 = buffer[8..<16].enumerated().reduce(UInt64(0)) { acc, element in
                let (index, value) = element
                return acc | (UInt64(value) << (UInt64(index) * 8))
            }
            h[6] ^= p0
            h[7] ^= p1
        }

        let blockLen = 128
        var t0: UInt64 = 0
        var t1: UInt64 = 0
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
                    t0 = t0 &+ UInt64(blockLen)
                    if t0 < UInt64(blockLen) { t1 = t1 &+ 1 }
                    let isLastFull = (blockIndex == fullBlocks - 1) && (remainder == 0)
                    let flag: UInt64 = isLastFull ? ~UInt64(0) : 0
                    compress(&h, block: ptr, t0: t0, t1: t1, f0: flag)
                }

                if remainder > 0 {
                    var lastBlock = [UInt8](repeating: 0, count: blockLen)
                    for i in 0..<remainder {
                        lastBlock[i] = base.advanced(by: offset + i).pointee
                    }
                    t0 = t0 &+ UInt64(remainder)
                    if t0 < UInt64(remainder) { t1 = t1 &+ 1 }
                    lastBlock.withUnsafeBufferPointer { buf in
                        compress(&h, block: buf.baseAddress!, t0: t0, t1: t1, f0: ~UInt64(0))
                    }
                }
            }
        } else {
            let zeroBlock = [UInt8](repeating: 0, count: blockLen)
            zeroBlock.withUnsafeBufferPointer { buf in
                compress(&h, block: buf.baseAddress!, t0: 0, t1: 0, f0: ~UInt64(0))
            }
        }

        var out = Data(count: outputLength)
        out.withUnsafeMutableBytes { outBuf in
            var idx = 0
            for word in h {
                var wle = word.littleEndian
                withUnsafeBytes(of: &wle) { wptr in
                    let count = min(8, outputLength - idx)
                    guard count > 0 else { return }
                    outBuf.baseAddress!.advanced(by: idx).copyMemory(from: wptr.baseAddress!, byteCount: count)
                    idx += count
                }
                if idx >= outputLength { break }
            }
        }
        return out
    }

    public static func hash256(_ data: Data, personal: Data? = nil) -> Data {
        hash(data: data, outputLength: 32, personal: personal)
    }

    public static func hash512(_ data: Data) -> Data {
        hash(data: data, outputLength: 64)
    }
}
