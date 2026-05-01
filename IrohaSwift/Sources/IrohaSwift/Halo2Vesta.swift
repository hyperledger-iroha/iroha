import Foundation

public struct VestaAffine: Equatable, Sendable {
    public let x: PastaFq
    public let y: PastaFq
    public let isIdentity: Bool

    public static let identity = VestaAffine(x: .zero, y: .zero, isIdentity: true)
    public static let generator = VestaAffine(x: -PastaFq.one, y: PastaFq(2), isIdentity: false)

    public init?(x: PastaFq, y: PastaFq) {
        guard y.squared() == x.squared() * x + PastaFq(5) else {
            return nil
        }
        self.init(x: x, y: y, isIdentity: false)
    }

    private init(x: PastaFq, y: PastaFq, isIdentity: Bool) {
        self.x = x
        self.y = y
        self.isIdentity = isIdentity
    }

    public static func fromCompressedBytes(_ bytes: Data) -> VestaAffine? {
        guard bytes.count == 32 else {
            return nil
        }
        var xBytes = bytes
        let ySign = (xBytes[31] >> 7) == 1
        xBytes[31] &= 0x7f

        guard let x = PastaFq.fromCanonicalBytes(xBytes) else {
            return nil
        }
        if x == .zero, !ySign {
            return .identity
        }

        let rhs = x.squared() * x + PastaFq(5)
        guard var y = rhs.squareRoot() else {
            return nil
        }
        if y.isOdd != ySign {
            y = -y
        }
        return VestaAffine(x: x, y: y)
    }

    public func compressedBytes() -> Data {
        guard !isIdentity else {
            return Data(repeating: 0, count: 32)
        }
        var bytes = x.canonicalBytes()
        if y.isOdd {
            bytes[31] |= 0x80
        }
        return bytes
    }

    public var projective: VestaProjective {
        VestaProjective(self)
    }

    public func negated() -> VestaAffine {
        guard !isIdentity else {
            return .identity
        }
        return VestaAffine(x: x, y: -y)!
    }

    public func doubled() -> VestaAffine {
        projective.doubled().toAffine()
    }

    public func added(_ rhs: VestaAffine) -> VestaAffine {
        projective.mixedAdded(rhs).toAffine()
    }

    public func multiplied(by scalar: PastaFp) -> VestaAffine {
        projective.multiplied(by: scalar).toAffine()
    }
}

public struct VestaProjective: Equatable, Sendable {
    public let x: PastaFq
    public let y: PastaFq
    public let z: PastaFq

    public static let identity = VestaProjective(x: .zero, y: PastaFq.one, z: .zero)
    public static let generator = VestaAffine.generator.projective

    public var isIdentity: Bool {
        z == .zero
    }

    public init(_ affine: VestaAffine) {
        if affine.isIdentity {
            self = .identity
        } else {
            self.init(x: affine.x, y: affine.y, z: .one)
        }
    }

    public init(x: PastaFq, y: PastaFq, z: PastaFq) {
        self.x = x
        self.y = y
        self.z = z
    }

    public func toAffine() -> VestaAffine {
        guard !isIdentity else {
            return .identity
        }
        guard let zInv = z.inverted() else {
            return .identity
        }
        let z2 = zInv.squared()
        let z3 = z2 * zInv
        return VestaAffine(x: x * z2, y: y * z3)!
    }

    public func negated() -> VestaProjective {
        guard !isIdentity else {
            return .identity
        }
        return VestaProjective(x: x, y: -y, z: z)
    }

    public func doubled() -> VestaProjective {
        guard !isIdentity, y != .zero else {
            return .identity
        }

        let a = x.squared()
        let b = y.squared()
        let c = b.squared()
        let xPlusB = x + b
        let xPlusBSquared = xPlusB.squared()
        let d0 = xPlusBSquared - a - c
        let d = d0 + d0
        let e = a + a + a
        let f = e.squared()
        let x3 = f - d - d
        let y3 = e * (d - x3) - PastaFq(8) * c
        let z3 = (y + y) * z
        return VestaProjective(x: x3, y: y3, z: z3)
    }

    public func mixedAdded(_ rhs: VestaAffine) -> VestaProjective {
        if rhs.isIdentity {
            return self
        }
        if isIdentity {
            return VestaProjective(rhs)
        }

        let z2 = z.squared()
        let u2 = rhs.x * z2
        let s2 = rhs.y * z * z2
        let h = u2 - x
        let r0 = s2 - y

        if h == .zero {
            return r0 == .zero ? doubled() : .identity
        }

        let hh = h.squared()
        let i = hh + hh + hh + hh
        let j = h * i
        let r = r0 + r0
        let v = x * i
        let x3 = r.squared() - j - v - v
        let y3 = r * (v - x3) - PastaFq(2) * y * j
        let z3 = (z + h).squared() - z2 - hh
        return VestaProjective(x: x3, y: y3, z: z3)
    }

    public func added(_ rhs: VestaProjective) -> VestaProjective {
        if rhs.isIdentity {
            return self
        }
        if isIdentity {
            return rhs
        }

        let z1z1 = z.squared()
        let z2z2 = rhs.z.squared()
        let u1 = x * z2z2
        let u2 = rhs.x * z1z1
        let s1 = y * rhs.z * z2z2
        let s2 = rhs.y * z * z1z1
        let h = u2 - u1
        let r0 = s2 - s1

        if h == .zero {
            return r0 == .zero ? doubled() : .identity
        }

        let i = (h + h).squared()
        let j = h * i
        let r = r0 + r0
        let v = u1 * i
        let x3 = r.squared() - j - v - v
        let y3 = r * (v - x3) - PastaFq(2) * s1 * j
        let z3 = ((z + rhs.z).squared() - z1z1 - z2z2) * h
        return VestaProjective(x: x3, y: y3, z: z3)
    }

    public func multiplied(by scalar: PastaFp) -> VestaProjective {
        guard scalar != .zero, !isIdentity else {
            return .identity
        }

        let scalarLimbs = scalar.canonicalLimbs()
        var table = [VestaProjective](repeating: .identity, count: 16)
        table[1] = self
        for index in 2..<table.count {
            table[index] = table[index - 1].added(self)
        }

        var result = VestaProjective.identity
        for window in stride(from: 63, through: 0, by: -1) {
            if !result.isIdentity {
                result = result.doubled().doubled().doubled().doubled()
            }
            let limb = scalarLimbs[window / 16]
            let nibble = Int((limb >> UInt64((window % 16) * 4)) & 0x0f)
            if nibble != 0 {
                result = result.added(table[nibble])
            }
        }
        return result
    }

    public static func + (lhs: VestaProjective, rhs: VestaProjective) -> VestaProjective {
        lhs.added(rhs)
    }

    public static func += (lhs: inout VestaProjective, rhs: VestaProjective) {
        lhs = lhs + rhs
    }

    public static prefix func - (value: VestaProjective) -> VestaProjective {
        value.negated()
    }

    public static func multiscalarMultiply(scalars: [PastaFp], bases: [VestaAffine]) -> VestaProjective {
        precondition(scalars.count == bases.count)
        return multiscalarMultiply(
            scalars: scalars,
            bases: bases,
            additionalScalar: nil,
            additionalBase: nil
        )
    }

    public static func multiscalarMultiply(
        scalars: [PastaFp],
        bases: [VestaAffine],
        additionalScalar: PastaFp,
        additionalBase: VestaAffine
    ) -> VestaProjective {
        precondition(scalars.count == bases.count)
        return multiscalarMultiply(
            scalars: scalars,
            bases: bases,
            additionalScalar: Optional(additionalScalar),
            additionalBase: Optional(additionalBase)
        )
    }

    private static func multiscalarMultiply(
        scalars: [PastaFp],
        bases: [VestaAffine],
        additionalScalar: PastaFp?,
        additionalBase: VestaAffine?
    ) -> VestaProjective {
        var accumulator = VestaProjective.identity
        var prepared: [(scalar: ScalarLimbs, base: VestaAffine)] = []
        prepared.reserveCapacity(scalars.count + (additionalScalar == nil ? 0 : 1))
        for (scalar, base) in zip(scalars, bases) where scalar != .zero && !base.isIdentity {
            prepared.append((ScalarLimbs(scalar), base))
        }
        if let additionalScalar, let additionalBase, additionalScalar != .zero, !additionalBase.isIdentity {
            prepared.append((ScalarLimbs(additionalScalar), additionalBase))
        }
        guard prepared.count >= 8 else {
            for item in prepared {
                accumulator += item.base.projective.multiplied(by: item.scalar.value)
            }
            return accumulator
        }

        for window in stride(from: 63, through: 0, by: -1) {
            if !accumulator.isIdentity {
                accumulator = accumulator.doubled().doubled().doubled().doubled()
            }
            var buckets = [VestaProjective?](repeating: nil, count: 16)
            for item in prepared {
                let nibble = item.scalar.nibble(at: window)
                if nibble != 0 {
                    buckets[nibble] = buckets[nibble]?.mixedAdded(item.base) ?? item.base.projective
                }
            }
            var running = VestaProjective.identity
            for bucket in stride(from: buckets.count - 1, through: 1, by: -1) {
                if let value = buckets[bucket] {
                    running += value
                }
                if !running.isIdentity {
                    accumulator += running
                }
            }
        }
        return accumulator
    }

    private struct ScalarLimbs {
        let value: PastaFp
        let limb0: UInt64
        let limb1: UInt64
        let limb2: UInt64
        let limb3: UInt64

        init(_ value: PastaFp) {
            let limbs = value.canonicalLimbs()
            self.value = value
            self.limb0 = limbs[0]
            self.limb1 = limbs[1]
            self.limb2 = limbs[2]
            self.limb3 = limbs[3]
        }

        func nibble(at window: Int) -> Int {
            let limb: UInt64
            switch window / 16 {
            case 0:
                limb = limb0
            case 1:
                limb = limb1
            case 2:
                limb = limb2
            default:
                limb = limb3
            }
            return Int((limb >> UInt64((window % 16) * 4)) & 0x0f)
        }
    }
}
