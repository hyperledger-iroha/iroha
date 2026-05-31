import Foundation

private let sccpGroth16Bn254BaseFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"
)!

private let sccpGroth16Bn254ScalarFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"
)!

private let sccpGroth16Bn254G2BC0 = Data(hexString:
    "2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5"
)!

private let sccpGroth16Bn254G2BC1 = Data(hexString:
    "009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2"
)!

private typealias SccpGroth16Fq2 = (c0: Data, c1: Data)

private struct SccpGroth16G2ProjectivePoint {
    let x: SccpGroth16Fq2
    let y: SccpGroth16Fq2
    let z: SccpGroth16Fq2
    let isInfinity: Bool

    static var infinity: SccpGroth16G2ProjectivePoint {
        SccpGroth16G2ProjectivePoint(
            x: sccpGroth16Fq2Zero(),
            y: sccpGroth16Fq2One(),
            z: sccpGroth16Fq2Zero(),
            isInfinity: true
        )
    }
}

private final class SccpGroth16G2SubgroupCache {
    private let lock = NSLock()
    private var values: [Data: Bool] = [:]

    func value(for key: Data, compute: () -> Bool) -> Bool {
        lock.lock()
        if let cached = values[key] {
            lock.unlock()
            return cached
        }
        lock.unlock()

        let result = compute()
        lock.lock()
        values[key] = result
        lock.unlock()
        return result
    }
}

private let sccpGroth16G2SubgroupCache = SccpGroth16G2SubgroupCache()

func sccpGroth16Bn254ProofTupleInvalidField(_ proofBytes: Data) -> String? {
    if sccpGroth16ProofWord(proofBytes, index: 0) != sccpGroth16AbiWord(1) {
        return "proofBytes.version"
    }
    if sccpGroth16ProofWordIsZero(proofBytes, index: 1) {
        return "proofBytes.messageId"
    }
    if !sccpGroth16ProofWord(proofBytes, index: 2).prefix(28).allSatisfy({ $0 == 0 }) {
        return "proofBytes.sourceDomain"
    }
    if sccpGroth16ProofWordIsZero(proofBytes, index: 3) {
        return "proofBytes.commitmentRoot"
    }
    for (offset, field) in ["a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y"].enumerated() {
        if sccpGroth16CompareBytes(
            sccpGroth16ProofWord(proofBytes, index: 4 + offset),
            sccpGroth16Bn254BaseFieldModulus
        ) != .orderedAscending {
            return "proofBytes.\(field)"
        }
    }
    if !sccpGroth16G1PointIsValid(
        x: sccpGroth16ProofWord(proofBytes, index: 4),
        y: sccpGroth16ProofWord(proofBytes, index: 5)
    ) {
        return "proofBytes.a"
    }
    if !sccpGroth16G2PointIsValid(
        x0: sccpGroth16ProofWord(proofBytes, index: 6),
        x1: sccpGroth16ProofWord(proofBytes, index: 7),
        y0: sccpGroth16ProofWord(proofBytes, index: 8),
        y1: sccpGroth16ProofWord(proofBytes, index: 9)
    ) {
        return "proofBytes.b"
    }
    if !sccpGroth16G1PointIsValid(
        x: sccpGroth16ProofWord(proofBytes, index: 10),
        y: sccpGroth16ProofWord(proofBytes, index: 11)
    ) {
        return "proofBytes.c"
    }
    return nil
}

private func sccpGroth16ProofWord(_ proofBytes: Data, index: Int) -> Data {
    let start = index * 32
    return Data(proofBytes[start..<(start + 32)])
}

private func sccpGroth16ProofWordIsZero(_ proofBytes: Data, index: Int) -> Bool {
    !sccpGroth16ProofWord(proofBytes, index: index).contains { $0 != 0 }
}

private func sccpGroth16AbiWord(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    var working = value
    for index in stride(from: 31, through: 0, by: -1) {
        out[index] = UInt8(working & 0xff)
        working >>= 8
        if working == 0 {
            break
        }
    }
    return out
}

private func sccpGroth16G1PointIsValid(x: Data, y: Data) -> Bool {
    guard x.contains(where: { $0 != 0 }) || y.contains(where: { $0 != 0 }) else {
        return false
    }
    let left = sccpGroth16FqMul(y, y)
    let x2 = sccpGroth16FqMul(x, x)
    let right = sccpGroth16FqAdd(sccpGroth16FqMul(x2, x), sccpGroth16AbiWord(3))
    return left == right
}

private func sccpGroth16G2PointIsValid(x0: Data, x1: Data, y0: Data, y1: Data) -> Bool {
    guard [x0, x1, y0, y1].contains(where: { $0.contains(where: { $0 != 0 }) }) else {
        return false
    }
    let y: SccpGroth16Fq2 = (c0: y0, c1: y1)
    let x: SccpGroth16Fq2 = (c0: x0, c1: x1)
    let left = sccpGroth16Fq2Mul(y, y)
    let x2 = sccpGroth16Fq2Mul(x, x)
    let right = sccpGroth16Fq2Add(
        sccpGroth16Fq2Mul(x2, x),
        (c0: sccpGroth16Bn254G2BC0, c1: sccpGroth16Bn254G2BC1)
    )
    return sccpGroth16Fq2Equal(left, right) && sccpGroth16G2PointIsInPrimeSubgroup(x: x, y: y)
}

private func sccpGroth16Fq2Add(
    _ left: SccpGroth16Fq2,
    _ right: SccpGroth16Fq2
) -> SccpGroth16Fq2 {
    (
        c0: sccpGroth16FqAdd(left.c0, right.c0),
        c1: sccpGroth16FqAdd(left.c1, right.c1)
    )
}

private func sccpGroth16Fq2Sub(
    _ left: SccpGroth16Fq2,
    _ right: SccpGroth16Fq2
) -> SccpGroth16Fq2 {
    (
        c0: sccpGroth16FqSub(left.c0, right.c0),
        c1: sccpGroth16FqSub(left.c1, right.c1)
    )
}

private func sccpGroth16Fq2Scale(_ left: SccpGroth16Fq2, by scalar: UInt64) -> SccpGroth16Fq2 {
    let word = sccpGroth16AbiWord(scalar)
    return (
        c0: sccpGroth16FqMul(left.c0, word),
        c1: sccpGroth16FqMul(left.c1, word)
    )
}

private func sccpGroth16Fq2Mul(
    _ left: SccpGroth16Fq2,
    _ right: SccpGroth16Fq2
) -> SccpGroth16Fq2 {
    let c0 = sccpGroth16FqSub(
        sccpGroth16FqMul(left.c0, right.c0),
        sccpGroth16FqMul(left.c1, right.c1)
    )
    let c1 = sccpGroth16FqAdd(
        sccpGroth16FqMul(left.c0, right.c1),
        sccpGroth16FqMul(left.c1, right.c0)
    )
    return (c0: c0, c1: c1)
}

private func sccpGroth16Fq2Equal(_ left: SccpGroth16Fq2, _ right: SccpGroth16Fq2) -> Bool {
    left.c0 == right.c0 && left.c1 == right.c1
}

private func sccpGroth16Fq2IsZero(_ value: SccpGroth16Fq2) -> Bool {
    !value.c0.contains { $0 != 0 } && !value.c1.contains { $0 != 0 }
}

private func sccpGroth16Fq2Zero() -> SccpGroth16Fq2 {
    (c0: Data(repeating: 0, count: 32), c1: Data(repeating: 0, count: 32))
}

private func sccpGroth16Fq2One() -> SccpGroth16Fq2 {
    (c0: sccpGroth16AbiWord(1), c1: Data(repeating: 0, count: 32))
}

private func sccpGroth16G2ProjectiveIsInfinity(_ point: SccpGroth16G2ProjectivePoint) -> Bool {
    point.isInfinity || sccpGroth16Fq2IsZero(point.z)
}

private func sccpGroth16G2AffineProjective(
    x: SccpGroth16Fq2,
    y: SccpGroth16Fq2
) -> SccpGroth16G2ProjectivePoint {
    SccpGroth16G2ProjectivePoint(x: x, y: y, z: sccpGroth16Fq2One(), isInfinity: false)
}

private func sccpGroth16G2ProjectiveDouble(
    _ point: SccpGroth16G2ProjectivePoint
) -> SccpGroth16G2ProjectivePoint {
    guard !sccpGroth16G2ProjectiveIsInfinity(point), !sccpGroth16Fq2IsZero(point.y) else {
        return .infinity
    }
    let xx = sccpGroth16Fq2Mul(point.x, point.x)
    let yy = sccpGroth16Fq2Mul(point.y, point.y)
    let yyyy = sccpGroth16Fq2Mul(yy, yy)
    let s = sccpGroth16Fq2Scale(
        sccpGroth16Fq2Sub(
            sccpGroth16Fq2Sub(
                sccpGroth16Fq2Mul(sccpGroth16Fq2Add(point.x, yy), sccpGroth16Fq2Add(point.x, yy)),
                xx
            ),
            yyyy
        ),
        by: 2
    )
    let m = sccpGroth16Fq2Scale(xx, by: 3)
    let x3 = sccpGroth16Fq2Sub(sccpGroth16Fq2Mul(m, m), sccpGroth16Fq2Scale(s, by: 2))
    let y3 = sccpGroth16Fq2Sub(
        sccpGroth16Fq2Mul(m, sccpGroth16Fq2Sub(s, x3)),
        sccpGroth16Fq2Scale(yyyy, by: 8)
    )
    let z3 = sccpGroth16Fq2Scale(sccpGroth16Fq2Mul(point.y, point.z), by: 2)
    return SccpGroth16G2ProjectivePoint(x: x3, y: y3, z: z3, isInfinity: false)
}

private func sccpGroth16G2ProjectiveAddAffine(
    _ point: SccpGroth16G2ProjectivePoint,
    x affineX: SccpGroth16Fq2,
    y affineY: SccpGroth16Fq2
) -> SccpGroth16G2ProjectivePoint {
    guard !sccpGroth16G2ProjectiveIsInfinity(point) else {
        return sccpGroth16G2AffineProjective(x: affineX, y: affineY)
    }
    let z1z1 = sccpGroth16Fq2Mul(point.z, point.z)
    let u2 = sccpGroth16Fq2Mul(affineX, z1z1)
    let s2 = sccpGroth16Fq2Mul(affineY, sccpGroth16Fq2Mul(point.z, z1z1))
    let h = sccpGroth16Fq2Sub(u2, point.x)
    if sccpGroth16Fq2IsZero(h) {
        if sccpGroth16Fq2Equal(s2, point.y) {
            return sccpGroth16G2ProjectiveDouble(point)
        }
        return .infinity
    }
    let hh = sccpGroth16Fq2Mul(h, h)
    let i = sccpGroth16Fq2Scale(hh, by: 4)
    let j = sccpGroth16Fq2Mul(h, i)
    let r = sccpGroth16Fq2Scale(sccpGroth16Fq2Sub(s2, point.y), by: 2)
    let v = sccpGroth16Fq2Mul(point.x, i)
    let x3 = sccpGroth16Fq2Sub(
        sccpGroth16Fq2Sub(sccpGroth16Fq2Mul(r, r), j),
        sccpGroth16Fq2Scale(v, by: 2)
    )
    let y3 = sccpGroth16Fq2Sub(
        sccpGroth16Fq2Mul(r, sccpGroth16Fq2Sub(v, x3)),
        sccpGroth16Fq2Scale(sccpGroth16Fq2Mul(point.y, j), by: 2)
    )
    let z3 = sccpGroth16Fq2Sub(
        sccpGroth16Fq2Sub(
            sccpGroth16Fq2Mul(sccpGroth16Fq2Add(point.z, h), sccpGroth16Fq2Add(point.z, h)),
            z1z1
        ),
        hh
    )
    return SccpGroth16G2ProjectivePoint(x: x3, y: y3, z: z3, isInfinity: false)
}

private func sccpGroth16G2PointIsInPrimeSubgroup(
    x: SccpGroth16Fq2,
    y: SccpGroth16Fq2
) -> Bool {
    var cacheKey = Data()
    cacheKey.append(x.c0)
    cacheKey.append(x.c1)
    cacheKey.append(y.c0)
    cacheKey.append(y.c1)
    return sccpGroth16G2SubgroupCache.value(for: cacheKey) {
        sccpGroth16G2PointIsInPrimeSubgroupUncached(x: x, y: y)
    }
}

private func sccpGroth16G2PointIsInPrimeSubgroupUncached(
    x: SccpGroth16Fq2,
    y: SccpGroth16Fq2
) -> Bool {
    var acc = SccpGroth16G2ProjectivePoint.infinity
    for byte in sccpGroth16Bn254ScalarFieldModulus {
        for bit in stride(from: 7, through: 0, by: -1) {
            acc = sccpGroth16G2ProjectiveDouble(acc)
            if (byte & UInt8(1 << bit)) != 0 {
                acc = sccpGroth16G2ProjectiveAddAffine(acc, x: x, y: y)
            }
        }
    }
    return sccpGroth16G2ProjectiveIsInfinity(acc)
}

private func sccpGroth16FqAdd(_ left: Data, _ right: Data) -> Data {
    var out = Data(repeating: 0, count: 32)
    var carry = 0
    for index in stride(from: 31, through: 0, by: -1) {
        let sum = Int(left[index]) + Int(right[index]) + carry
        out[index] = UInt8(sum & 0xff)
        carry = sum >> 8
    }
    if sccpGroth16CompareBytes(out, sccpGroth16Bn254BaseFieldModulus) != .orderedAscending {
        out = sccpGroth16SubNoUnderflow(out, sccpGroth16Bn254BaseFieldModulus)
    }
    return out
}

private func sccpGroth16FqSub(_ left: Data, _ right: Data) -> Data {
    if sccpGroth16CompareBytes(left, right) != .orderedAscending {
        return sccpGroth16SubNoUnderflow(left, right)
    }
    return sccpGroth16SubNoUnderflow(
        sccpGroth16FqAddNoReduce(left, sccpGroth16Bn254BaseFieldModulus),
        right
    )
}

private func sccpGroth16FqMul(_ left: Data, _ right: Data) -> Data {
    var result = Data(repeating: 0, count: 32)
    for byte in right {
        for bit in stride(from: 7, through: 0, by: -1) {
            result = sccpGroth16FqAdd(result, result)
            if (byte & UInt8(1 << bit)) != 0 {
                result = sccpGroth16FqAdd(result, left)
            }
        }
    }
    return result
}

private func sccpGroth16FqAddNoReduce(_ left: Data, _ right: Data) -> Data {
    var out = Data(repeating: 0, count: 32)
    var carry = 0
    for index in stride(from: 31, through: 0, by: -1) {
        let sum = Int(left[index]) + Int(right[index]) + carry
        out[index] = UInt8(sum & 0xff)
        carry = sum >> 8
    }
    return out
}

private func sccpGroth16SubNoUnderflow(_ left: Data, _ right: Data) -> Data {
    var out = Data(repeating: 0, count: 32)
    var borrow = 0
    for index in stride(from: 31, through: 0, by: -1) {
        let diff = Int(left[index]) - Int(right[index]) - borrow
        if diff < 0 {
            out[index] = UInt8(diff + 256)
            borrow = 1
        } else {
            out[index] = UInt8(diff)
            borrow = 0
        }
    }
    return out
}

private func sccpGroth16CompareBytes(_ lhs: Data, _ rhs: Data) -> ComparisonResult {
    for (left, right) in zip(lhs, rhs) {
        if left < right {
            return .orderedAscending
        }
        if left > right {
            return .orderedDescending
        }
    }
    return .orderedSame
}
