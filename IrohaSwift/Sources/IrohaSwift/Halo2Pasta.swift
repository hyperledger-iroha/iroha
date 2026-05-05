import Foundation

public protocol PastaFieldParameters: Sendable {
    static var modulus: [UInt64] { get }
    static var montgomeryInv: UInt64 { get }
    static var r: [UInt64] { get }
    static var r2: [UInt64] { get }
    static var r3: [UInt64] { get }
    static var rootOfUnity: [UInt64] { get }
    static var rootOfUnityInv: [UInt64] { get }
    static var zeta: [UInt64] { get }
    static var twoAdicity: Int { get }
    static var t: [UInt64] { get }
    static var tPlusOneOverTwo: [UInt64] { get }
}

public enum PastaFpParameters: PastaFieldParameters {
    public static let modulus: [UInt64] = [
        0x992d30ed00000001,
        0x224698fc094cf91b,
        0x0000000000000000,
        0x4000000000000000,
    ]
    public static let montgomeryInv: UInt64 = 0x992d30ecffffffff
    public static let r: [UInt64] = [
        0x34786d38fffffffd,
        0x992c350be41914ad,
        0xffffffffffffffff,
        0x3fffffffffffffff,
    ]
    public static let r2: [UInt64] = [
        0x8c78ecb30000000f,
        0xd7d30dbd8b0de0e7,
        0x7797a99bc3c95d18,
        0x096d41af7b9cb714,
    ]
    public static let r3: [UInt64] = [
        0xf185a5993a9e10f9,
        0xf6a68f3b6ac5b1d1,
        0xdf8d1014353fd42c,
        0x2ae309222d2d9910,
    ]
    public static let rootOfUnity: [UInt64] = [
        0xbdad6fabd87ea32f,
        0xea322bf2b7bb7584,
        0x362120830561f81a,
        0x2bce74deac30ebda,
    ]
    public static let rootOfUnityInv: [UInt64] = [
        0xf0b87c7db2ce91f6,
        0x84a0a1d8859f066f,
        0xb4ed8e647196dad1,
        0x2cd5282c53116b5c,
    ]
    public static let zeta: [UInt64] = [
        0x1dad5ebdfdfe4ab9,
        0x1d1f8bd237ad3149,
        0x2caad5dc57aab1b0,
        0x12ccca834acdba71,
    ]
    public static let twoAdicity = 32
    public static let t: [UInt64] = [
        0x094cf91b992d30ed,
        0x00000000224698fc,
        0x0000000000000000,
        0x0000000040000000,
    ]
    public static let tPlusOneOverTwo: [UInt64] = [
        0x04a67c8dcc969877,
        0x0000000011234c7e,
        0x0000000000000000,
        0x0000000020000000,
    ]
}

public enum PastaFqParameters: PastaFieldParameters {
    public static let modulus: [UInt64] = [
        0x8c46eb2100000001,
        0x224698fc0994a8dd,
        0x0000000000000000,
        0x4000000000000000,
    ]
    public static let montgomeryInv: UInt64 = 0x8c46eb20ffffffff
    public static let r: [UInt64] = [
        0x5b2b3e9cfffffffd,
        0x992c350be3420567,
        0xffffffffffffffff,
        0x3fffffffffffffff,
    ]
    public static let r2: [UInt64] = [
        0xfc9678ff0000000f,
        0x67bb433d891a16e3,
        0x7fae231004ccf590,
        0x096d41af7ccfdaa9,
    ]
    public static let r3: [UInt64] = [
        0x008b421c249dae4c,
        0xe13bda50dba41326,
        0x88fececb8e15cb63,
        0x07dd97a06e6792c8,
    ]
    public static let rootOfUnity: [UInt64] = [
        0xa70e2c1102b6d05f,
        0x9bb97ea3c106f049,
        0x9e5c4dfd492ae26e,
        0x2de6a9b8746d3f58,
    ]
    public static let rootOfUnityInv: [UInt64] = [
        0x57eecda0a84b6836,
        0x4ad38b9084b8a80c,
        0xf4c8f353124086c1,
        0x2235e1a7415bf936,
    ]
    public static let zeta: [UInt64] = [
        0x2aa9d2e050aa0e4f,
        0x0fed467d47c033af,
        0x511db4d81cf70f5a,
        0x06819a58283e528e,
    ]
    public static let twoAdicity = 32
    public static let t: [UInt64] = [
        0x0994a8dd8c46eb21,
        0x00000000224698fc,
        0x0000000000000000,
        0x0000000040000000,
    ]
    public static let tPlusOneOverTwo: [UInt64] = [
        0x04ca546ec6237591,
        0x0000000011234c7e,
        0x0000000000000000,
        0x0000000020000000,
    ]
}

public typealias PastaFp = PastaField<PastaFpParameters>
public typealias PastaFq = PastaField<PastaFqParameters>

public struct PastaField<Parameters: PastaFieldParameters>: Equatable, Sendable {
    private var limbs: [UInt64]

    private init(montgomeryLimbs: [UInt64]) {
        precondition(montgomeryLimbs.count == 4)
        limbs = montgomeryLimbs
    }

    public static var zero: Self {
        Self(montgomeryLimbs: [0, 0, 0, 0])
    }

    public static var one: Self {
        Self(montgomeryLimbs: Parameters.r)
    }

    public static var rootOfUnity: Self {
        Self.fromCanonicalLimbs(Parameters.rootOfUnity)
    }

    public static var rootOfUnityInv: Self {
        Self.fromCanonicalLimbs(Parameters.rootOfUnityInv)
    }

    public static var zeta: Self {
        Self.fromCanonicalLimbs(Parameters.zeta)
    }

    public init(_ value: UInt64) {
        self = Self.fromCanonicalLimbs([value, 0, 0, 0])
    }

    public static func fromRawLimbs(_ limbs: [UInt64]) -> Self {
        precondition(limbs.count == 4)
        return Self.fromCanonicalLimbs(limbs)
    }

    public static func fromUniformBytes64(_ bytes: Data) -> Self? {
        guard bytes.count == 64 else {
            return nil
        }
        var limbs = [UInt64](repeating: 0, count: 8)
        for idx in 0..<8 {
            let start = idx * 8
            var word: UInt64 = 0
            for offset in 0..<8 {
                word |= UInt64(bytes[start + offset]) << UInt64(offset * 8)
            }
            limbs[idx] = word
        }

        let low = Self(montgomeryLimbs: Array(limbs[0..<4])) * Self(montgomeryLimbs: Parameters.r2)
        let high = Self(montgomeryLimbs: Array(limbs[4..<8])) * Self(montgomeryLimbs: Parameters.r3)
        return low + high
    }

    public static func fromCanonicalBytes(_ bytes: Data) -> Self? {
        guard bytes.count == 32 else {
            return nil
        }
        var parsed = [UInt64](repeating: 0, count: 4)
        for idx in 0..<4 {
            let start = idx * 8
            var word: UInt64 = 0
            for offset in 0..<8 {
                word |= UInt64(bytes[start + offset]) << UInt64(offset * 8)
            }
            parsed[idx] = word
        }
        guard Self.compare(parsed, Parameters.modulus) == .orderedAscending else {
            return nil
        }
        return Self.fromCanonicalLimbs(parsed)
    }

    public func canonicalBytes() -> Data {
        let reduced = canonicalLimbs()
        var out = Data(capacity: 32)
        for limb in reduced {
            var value = limb
            for _ in 0..<8 {
                out.append(UInt8(value & 0xff))
                value >>= 8
            }
        }
        return out
    }

    public func canonicalLimbs() -> [UInt64] {
        Self.montgomeryReduce(
            limbs[0], limbs[1], limbs[2], limbs[3],
            0, 0, 0, 0
        ).limbs
    }

    public func canonicalHex() -> String {
        canonicalBytes().hexEncodedString()
    }

    public var isOdd: Bool {
        (canonicalBytes().first ?? 0) & 1 == 1
    }

    public func bit(at index: Int) -> Bool {
        precondition(index >= 0 && index < 256)
        let limbs = canonicalLimbs()
        return ((limbs[index / 64] >> UInt64(index % 64)) & 1) == 1
    }

    public func squared() -> Self {
        let (r1a, carry1a) = Self.mac(0, limbs[0], limbs[1], 0)
        let (r2a, carry2a) = Self.mac(0, limbs[0], limbs[2], carry1a)
        let (r3a, r4a) = Self.mac(0, limbs[0], limbs[3], carry2a)

        let (r3b, carry3b) = Self.mac(r3a, limbs[1], limbs[2], 0)
        let (r4b, r5a) = Self.mac(r4a, limbs[1], limbs[3], carry3b)

        let (r5b, r6a) = Self.mac(r5a, limbs[2], limbs[3], 0)

        let r7 = r6a >> 63
        let r6 = (r6a << 1) | (r5b >> 63)
        let r5 = (r5b << 1) | (r4b >> 63)
        let r4 = (r4b << 1) | (r3b >> 63)
        let r3 = (r3b << 1) | (r2a >> 63)
        let r2 = (r2a << 1) | (r1a >> 63)
        let r1 = r1a << 1

        let (r0, carry0) = Self.mac(0, limbs[0], limbs[0], 0)
        let (r1b, carry1b) = Self.adc(0, r1, carry0)
        let (r2b, carry2b) = Self.mac(r2, limbs[1], limbs[1], carry1b)
        let (r3c, carry3c) = Self.adc(0, r3, carry2b)
        let (r4c, carry4c) = Self.mac(r4, limbs[2], limbs[2], carry3c)
        let (r5c, carry5c) = Self.adc(0, r5, carry4c)
        let (r6b, carry6b) = Self.mac(r6, limbs[3], limbs[3], carry5c)
        let (r7b, _) = Self.adc(0, r7, carry6b)

        return Self.montgomeryReduce(r0, r1b, r2b, r3c, r4c, r5c, r6b, r7b)
    }

    public func inverted() -> Self? {
        guard self != .zero else {
            return nil
        }
        let exponent = [
            Parameters.modulus[0] &- 2,
            Parameters.modulus[1],
            Parameters.modulus[2],
            Parameters.modulus[3],
        ]
        return powVartime(exponent)
    }

    public func squareRoot() -> Self? {
        if self == .zero {
            return .zero
        }

        var z = Self.rootOfUnity
        var w = powVartime(Parameters.t)
        var x = powVartime(Parameters.tPlusOneOverTwo)
        var v = Parameters.twoAdicity

        while w != .one {
            var k = 1
            var probe = w.squared()
            while probe != .one {
                probe = probe.squared()
                k += 1
                if k >= v {
                    return nil
                }
            }

            var b = z
            let squarings = v - k - 1
            if squarings > 0 {
                for _ in 0..<squarings {
                    b = b.squared()
                }
            }
            x *= b
            z = b.squared()
            w *= z
            v = k
        }

        guard x.squared() == self else {
            return nil
        }
        return x
    }

    public static func sqrtRatio(numerator: Self, denominator: Self) -> (isSquare: Bool, root: Self)? {
        guard denominator != .zero,
              let denominatorInv = denominator.inverted() else {
            return nil
        }
        let ratio = numerator * denominatorInv
        if let root = ratio.squareRoot() {
            return (true, root)
        }
        guard let root = (ratio * Self.rootOfUnity).squareRoot() else {
            return nil
        }
        return (false, root)
    }

    public func powVartime(_ exponent: [UInt64]) -> Self {
        var result = Self.one
        var foundOne = false
        for word in exponent.reversed() {
            for bit in stride(from: 63, through: 0, by: -1) {
                if foundOne {
                    result = result.squared()
                }
                if ((word >> UInt64(bit)) & 1) == 1 {
                    foundOne = true
                    result *= self
                }
            }
        }
        return result
    }

    public static func + (lhs: Self, rhs: Self) -> Self {
        let (d0, c0) = adc(lhs.limbs[0], rhs.limbs[0], 0)
        let (d1, c1) = adc(lhs.limbs[1], rhs.limbs[1], c0)
        let (d2, c2) = adc(lhs.limbs[2], rhs.limbs[2], c1)
        let (d3, _) = adc(lhs.limbs[3], rhs.limbs[3], c2)
        return Self(montgomeryLimbs: [d0, d1, d2, d3]) - Self(montgomeryLimbs: Parameters.modulus)
    }

    public static func - (lhs: Self, rhs: Self) -> Self {
        let (d0a, b0) = sbb(lhs.limbs[0], rhs.limbs[0], 0)
        let (d1a, b1) = sbb(lhs.limbs[1], rhs.limbs[1], b0)
        let (d2a, b2) = sbb(lhs.limbs[2], rhs.limbs[2], b1)
        let (d3a, b3) = sbb(lhs.limbs[3], rhs.limbs[3], b2)

        let (d0, c0) = adc(d0a, Parameters.modulus[0] & b3, 0)
        let (d1, c1) = adc(d1a, Parameters.modulus[1] & b3, c0)
        let (d2, c2) = adc(d2a, Parameters.modulus[2] & b3, c1)
        let (d3, _) = adc(d3a, Parameters.modulus[3] & b3, c2)
        return Self(montgomeryLimbs: [d0, d1, d2, d3])
    }

    public static prefix func - (value: Self) -> Self {
        let (d0a, b0) = sbb(Parameters.modulus[0], value.limbs[0], 0)
        let (d1a, b1) = sbb(Parameters.modulus[1], value.limbs[1], b0)
        let (d2a, b2) = sbb(Parameters.modulus[2], value.limbs[2], b1)
        let (d3a, _) = sbb(Parameters.modulus[3], value.limbs[3], b2)
        let nonzero = value.limbs.reduce(UInt64(0)) { acc, limb in acc | limb }
        let mask: UInt64 = nonzero == 0 ? 0 : UInt64.max
        return Self(montgomeryLimbs: [d0a & mask, d1a & mask, d2a & mask, d3a & mask])
    }

    public static func * (lhs: Self, rhs: Self) -> Self {
        let (r0, carry0) = mac(0, lhs.limbs[0], rhs.limbs[0], 0)
        let (r1a, carry1a) = mac(0, lhs.limbs[0], rhs.limbs[1], carry0)
        let (r2a, carry2a) = mac(0, lhs.limbs[0], rhs.limbs[2], carry1a)
        let (r3a, r4a) = mac(0, lhs.limbs[0], rhs.limbs[3], carry2a)

        let (r1b, carry1b) = mac(r1a, lhs.limbs[1], rhs.limbs[0], 0)
        let (r2b, carry2b) = mac(r2a, lhs.limbs[1], rhs.limbs[1], carry1b)
        let (r3b, carry3b) = mac(r3a, lhs.limbs[1], rhs.limbs[2], carry2b)
        let (r4b, r5a) = mac(r4a, lhs.limbs[1], rhs.limbs[3], carry3b)

        let (r2c, carry2c) = mac(r2b, lhs.limbs[2], rhs.limbs[0], 0)
        let (r3c, carry3c) = mac(r3b, lhs.limbs[2], rhs.limbs[1], carry2c)
        let (r4c, carry4c) = mac(r4b, lhs.limbs[2], rhs.limbs[2], carry3c)
        let (r5b, r6a) = mac(r5a, lhs.limbs[2], rhs.limbs[3], carry4c)

        let (r3d, carry3d) = mac(r3c, lhs.limbs[3], rhs.limbs[0], 0)
        let (r4d, carry4d) = mac(r4c, lhs.limbs[3], rhs.limbs[1], carry3d)
        let (r5c, carry5c) = mac(r5b, lhs.limbs[3], rhs.limbs[2], carry4d)
        let (r6b, r7) = mac(r6a, lhs.limbs[3], rhs.limbs[3], carry5c)

        return montgomeryReduce(r0, r1b, r2c, r3d, r4d, r5c, r6b, r7)
    }

    public static func += (lhs: inout Self, rhs: Self) {
        lhs = lhs + rhs
    }

    public static func -= (lhs: inout Self, rhs: Self) {
        lhs = lhs - rhs
    }

    public static func *= (lhs: inout Self, rhs: Self) {
        lhs = lhs * rhs
    }

    private static func fromCanonicalLimbs(_ limbs: [UInt64]) -> Self {
        Self(montgomeryLimbs: limbs) * Self(montgomeryLimbs: Parameters.r2)
    }

    private static func montgomeryReduce(
        _ r0: UInt64,
        _ r1: UInt64,
        _ r2: UInt64,
        _ r3: UInt64,
        _ r4: UInt64,
        _ r5: UInt64,
        _ r6: UInt64,
        _ r7: UInt64
    ) -> Self {
        let modulus = Parameters.modulus

        var r1 = r1
        var r2 = r2
        var r3 = r3
        var r4 = r4
        var r5 = r5
        var r6 = r6
        var r7 = r7

        var k = r0 &* Parameters.montgomeryInv
        var carry: UInt64
        var carry2: UInt64
        (_, carry) = mac(r0, k, modulus[0], 0)
        (r1, carry) = mac(r1, k, modulus[1], carry)
        (r2, carry) = mac(r2, k, modulus[2], carry)
        (r3, carry) = mac(r3, k, modulus[3], carry)
        (r4, carry2) = adc(r4, 0, carry)

        k = r1 &* Parameters.montgomeryInv
        (_, carry) = mac(r1, k, modulus[0], 0)
        (r2, carry) = mac(r2, k, modulus[1], carry)
        (r3, carry) = mac(r3, k, modulus[2], carry)
        (r4, carry) = mac(r4, k, modulus[3], carry)
        (r5, carry2) = adc(r5, carry2, carry)

        k = r2 &* Parameters.montgomeryInv
        (_, carry) = mac(r2, k, modulus[0], 0)
        (r3, carry) = mac(r3, k, modulus[1], carry)
        (r4, carry) = mac(r4, k, modulus[2], carry)
        (r5, carry) = mac(r5, k, modulus[3], carry)
        (r6, carry2) = adc(r6, carry2, carry)

        k = r3 &* Parameters.montgomeryInv
        (_, carry) = mac(r3, k, modulus[0], 0)
        (r4, carry) = mac(r4, k, modulus[1], carry)
        (r5, carry) = mac(r5, k, modulus[2], carry)
        (r6, carry) = mac(r6, k, modulus[3], carry)
        (r7, _) = adc(r7, carry2, carry)

        return Self(montgomeryLimbs: [r4, r5, r6, r7]) - Self(montgomeryLimbs: modulus)
    }

    private static func compare(_ lhs: [UInt64], _ rhs: [UInt64]) -> ComparisonResult {
        precondition(lhs.count == 4 && rhs.count == 4)
        for idx in stride(from: 3, through: 0, by: -1) {
            if lhs[idx] < rhs[idx] {
                return .orderedAscending
            }
            if lhs[idx] > rhs[idx] {
                return .orderedDescending
            }
        }
        return .orderedSame
    }

    private static func adc(_ a: UInt64, _ b: UInt64, _ carry: UInt64) -> (UInt64, UInt64) {
        let (sum0, overflow0) = a.addingReportingOverflow(b)
        let (sum1, overflow1) = sum0.addingReportingOverflow(carry)
        return (sum1, (overflow0 ? 1 : 0) + (overflow1 ? 1 : 0))
    }

    private static func sbb(_ a: UInt64, _ b: UInt64, _ borrow: UInt64) -> (UInt64, UInt64) {
        let borrowBit = borrow >> 63
        let (diff0, overflow0) = a.subtractingReportingOverflow(b)
        let (diff1, overflow1) = diff0.subtractingReportingOverflow(borrowBit)
        return (diff1, (overflow0 || overflow1) ? UInt64.max : 0)
    }

    private static func mac(_ a: UInt64, _ b: UInt64, _ c: UInt64, _ carry: UInt64) -> (UInt64, UInt64) {
        let product = b.multipliedFullWidth(by: c)
        var low = product.low
        var high = product.high
        let (low0, overflow0) = low.addingReportingOverflow(a)
        low = low0
        high = high &+ (overflow0 ? 1 : 0)
        let (low1, overflow1) = low.addingReportingOverflow(carry)
        low = low1
        high = high &+ (overflow1 ? 1 : 0)
        return (low, high)
    }
}
