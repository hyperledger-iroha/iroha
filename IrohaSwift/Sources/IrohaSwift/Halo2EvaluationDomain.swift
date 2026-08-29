import Foundation

public enum Halo2EvaluationDomainError: Error, Equatable {
    case invalidK(UInt32)
    case kExceedsMaximum(k: UInt32, maximum: UInt32)
    case invalidValueCount(expected: Int, actual: Int)
}

public struct Halo2EvaluationDomain: Equatable, Sendable {
    public let k: UInt32
    public let n: Int
    public let omega: PastaFp
    public let omegaInv: PastaFp
    public let nInv: PastaFp

    public init(k: UInt32, maximumK: UInt32) throws {
        let intrinsicMaximumK = UInt32(PastaFpParameters.twoAdicity)
        guard maximumK <= intrinsicMaximumK else {
            throw Halo2EvaluationDomainError.invalidK(maximumK)
        }
        guard k <= intrinsicMaximumK else {
            throw Halo2EvaluationDomainError.invalidK(k)
        }
        guard k <= maximumK else {
            throw Halo2EvaluationDomainError.kExceedsMaximum(k: k, maximum: maximumK)
        }
        let n = 1 << Int(k)
        let exponent = UInt64(1) << UInt64(PastaFpParameters.twoAdicity - Int(k))
        let omega = PastaFp.rootOfUnity.powVartime([exponent, 0, 0, 0])
        guard let omegaInv = omega.inverted(),
              let nInv = PastaFp(UInt64(n)).inverted() else {
            throw Halo2EvaluationDomainError.invalidK(k)
        }
        self.k = k
        self.n = n
        self.omega = omega
        self.omegaInv = omegaInv
        self.nInv = nInv
    }

    public func coeffToLagrange(_ coefficients: [PastaFp]) throws -> [PastaFp] {
        guard coefficients.count == n else {
            throw Halo2EvaluationDomainError.invalidValueCount(expected: n, actual: coefficients.count)
        }
        return fft(coefficients, root: omega, scale: nil)
    }

    public func lagrangeToCoeff(_ evaluations: [PastaFp]) throws -> [PastaFp] {
        guard evaluations.count == n else {
            throw Halo2EvaluationDomainError.invalidValueCount(expected: n, actual: evaluations.count)
        }
        return fft(evaluations, root: omegaInv, scale: nInv)
    }

    public func vanishingPolynomialEvaluation(at point: PastaFp) -> PastaFp {
        point.powVartime([UInt64(n), 0, 0, 0]) - PastaFp.one
    }

    public func evaluateLagrangeBasisZero(at point: PastaFp) -> PastaFp? {
        if point == .one {
            return .one
        }
        let numerator = vanishingPolynomialEvaluation(at: point)
        if numerator == .zero {
            return .zero
        }
        let denominator = PastaFp(UInt64(n)) * (point - PastaFp.one)
        guard denominator != .zero, let denominatorInv = denominator.inverted() else {
            return nil
        }
        return numerator * denominatorInv
    }

    private func fft(_ input: [PastaFp], root: PastaFp, scale: PastaFp?) -> [PastaFp] {
        var values = input
        bitReverse(&values)

        var m = 1
        while m < n {
            let step = root.powVartime([UInt64(n / (2 * m)), 0, 0, 0])
            for start in stride(from: 0, to: n, by: 2 * m) {
                var w = PastaFp.one
                for offset in 0..<m {
                    let even = values[start + offset]
                    let odd = values[start + offset + m] * w
                    values[start + offset] = even + odd
                    values[start + offset + m] = even - odd
                    w *= step
                }
            }
            m *= 2
        }

        if let scale {
            for idx in values.indices {
                values[idx] *= scale
            }
        }
        return values
    }

    private func bitReverse(_ values: inout [PastaFp]) {
        var j = 0
        for i in 1..<values.count {
            var bit = values.count >> 1
            while (j & bit) != 0 {
                j ^= bit
                bit >>= 1
            }
            j ^= bit
            if i < j {
                values.swapAt(i, j)
            }
        }
    }
}

public enum Halo2ExtendedEvaluationDomainError: Error, Equatable {
    case invalidDegree(Int)
    case invalidK(UInt32)
    case kExceedsMaximum(k: UInt32, maximum: UInt32)
    case extendedKExceedsMaximum(required: UInt32, maximum: UInt32)
    case invalidValueCount(expected: Int, actual: Int)
    case nonInvertibleVanishingEvaluation
}

public struct Halo2ExtendedEvaluationDomain: Equatable, Sendable {
    public let k: UInt32
    public let n: Int
    public let degree: Int
    public let quotientPolynomialDegree: Int
    public let extendedK: UInt32
    public let extendedN: Int
    public let omega: PastaFp
    public let omegaInv: PastaFp
    public let extendedOmega: PastaFp
    public let extendedOmegaInv: PastaFp
    public let nInv: PastaFp
    public let extendedNInv: PastaFp

    private let gCoset: PastaFp
    private let gCosetInv: PastaFp
    private let tEvaluationsInv: [PastaFp]

    public init(
        degree: Int,
        k: UInt32,
        maximumK: UInt32,
        maximumExtendedK: UInt32
    ) throws {
        guard degree >= 2 else {
            throw Halo2ExtendedEvaluationDomainError.invalidDegree(degree)
        }
        let intrinsicMaximumK = UInt32(PastaFpParameters.twoAdicity)
        guard maximumK <= intrinsicMaximumK else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(maximumK)
        }
        guard maximumExtendedK <= intrinsicMaximumK else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(maximumExtendedK)
        }
        guard k <= intrinsicMaximumK else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(k)
        }
        guard k <= maximumK else {
            throw Halo2ExtendedEvaluationDomainError.kExceedsMaximum(k: k, maximum: maximumK)
        }
        guard k <= maximumExtendedK else {
            throw Halo2ExtendedEvaluationDomainError.extendedKExceedsMaximum(
                required: k,
                maximum: maximumExtendedK
            )
        }
        let n = 1 << Int(k)
        let quotientPolynomialDegree = degree - 1
        let (minimumExtendedN, minimumExtendedNOverflow) =
            n.multipliedReportingOverflow(by: quotientPolynomialDegree)
        guard !minimumExtendedNOverflow else {
            throw Halo2ExtendedEvaluationDomainError.invalidDegree(degree)
        }
        var extendedK = k
        var extendedN = n
        while extendedN < minimumExtendedN {
            guard extendedK < intrinsicMaximumK else {
                throw Halo2ExtendedEvaluationDomainError.invalidK(k)
            }
            let requiredExtendedK = extendedK + 1
            guard requiredExtendedK <= maximumExtendedK else {
                throw Halo2ExtendedEvaluationDomainError.extendedKExceedsMaximum(
                    required: requiredExtendedK,
                    maximum: maximumExtendedK
                )
            }
            let (doubledExtendedN, doubledExtendedNOverflow) =
                extendedN.multipliedReportingOverflow(by: 2)
            guard !doubledExtendedNOverflow else {
                throw Halo2ExtendedEvaluationDomainError.invalidK(k)
            }
            extendedN = doubledExtendedN
            extendedK = requiredExtendedK
        }
        guard extendedK <= intrinsicMaximumK else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(k)
        }

        var extendedOmega = PastaFp.rootOfUnity
        for _ in extendedK..<UInt32(PastaFpParameters.twoAdicity) {
            extendedOmega = extendedOmega.squared()
        }
        guard let extendedOmegaInv = extendedOmega.inverted() else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(k)
        }

        var omega = extendedOmega
        for _ in k..<extendedK {
            omega = omega.squared()
        }
        guard let omegaInv = omega.inverted(),
              let nInv = PastaFp(UInt64(n)).inverted(),
              let extendedNInv = PastaFp(UInt64(extendedN)).inverted() else {
            throw Halo2ExtendedEvaluationDomainError.invalidK(k)
        }

        let gCoset = PastaFp.zeta
        let gCosetInv = gCoset.squared()
        let step = extendedOmega.powVartime([UInt64(n), 0, 0, 0])
        let original = gCoset.powVartime([UInt64(n), 0, 0, 0])
        var current = original
        var tEvaluations: [PastaFp] = []
        repeat {
            guard let inverse = (current - PastaFp.one).inverted() else {
                throw Halo2ExtendedEvaluationDomainError.nonInvertibleVanishingEvaluation
            }
            tEvaluations.append(inverse)
            current *= step
        } while current != original

        self.k = k
        self.n = n
        self.degree = degree
        self.quotientPolynomialDegree = quotientPolynomialDegree
        self.extendedK = extendedK
        self.extendedN = extendedN
        self.omega = omega
        self.omegaInv = omegaInv
        self.extendedOmega = extendedOmega
        self.extendedOmegaInv = extendedOmegaInv
        self.nInv = nInv
        self.extendedNInv = extendedNInv
        self.gCoset = gCoset
        self.gCosetInv = gCosetInv
        self.tEvaluationsInv = tEvaluations
    }

    public func lagrangeToCoeff(_ evaluations: [PastaFp]) throws -> [PastaFp] {
        guard evaluations.count == n else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(expected: n, actual: evaluations.count)
        }
        return Self.fft(evaluations, root: omegaInv, scale: nInv)
    }

    public func coeffToLagrange(_ coefficients: [PastaFp]) throws -> [PastaFp] {
        guard coefficients.count == n else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(expected: n, actual: coefficients.count)
        }
        return Self.fft(coefficients, root: omega, scale: nil)
    }

    public func coeffToExtendedPart(_ coefficients: [PastaFp], factor: PastaFp) throws -> [PastaFp] {
        guard coefficients.count == n else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(expected: n, actual: coefficients.count)
        }
        var values = coefficients
        distributePowers(&values, by: gCoset * factor)
        return Self.fft(values, root: omega, scale: nil)
    }

    public func extendedFromParts(_ parts: [[PastaFp]]) throws -> [PastaFp] {
        let expectedParts = extendedN >> Int(k)
        guard parts.count == expectedParts else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(expected: expectedParts, actual: parts.count)
        }
        for part in parts where part.count != n {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(expected: n, actual: part.count)
        }
        var output = [PastaFp]()
        output.reserveCapacity(extendedN)
        for row in 0..<n {
            for part in parts {
                output.append(part[row])
            }
        }
        return output
    }

    public func divideByVanishingPolynomial(_ extendedEvaluations: [PastaFp]) throws -> [PastaFp] {
        guard extendedEvaluations.count == extendedN else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(
                expected: extendedN,
                actual: extendedEvaluations.count
            )
        }
        return extendedEvaluations.enumerated().map { index, value in
            value * tEvaluationsInv[index % tEvaluationsInv.count]
        }
    }

    public func extendedToCoeff(_ extendedEvaluations: [PastaFp]) throws -> [PastaFp] {
        guard extendedEvaluations.count == extendedN else {
            throw Halo2ExtendedEvaluationDomainError.invalidValueCount(
                expected: extendedN,
                actual: extendedEvaluations.count
            )
        }
        var coefficients = Self.fft(extendedEvaluations, root: extendedOmegaInv, scale: extendedNInv)
        distributePowersZeta(&coefficients, intoCoset: false)
        return coefficients
    }

    public func rotateOmega(_ value: PastaFp, by rotation: Int32) -> PastaFp {
        if rotation >= 0 {
            return value * omega.powVartime([UInt64(rotation), 0, 0, 0])
        }
        return value * omegaInv.powVartime([UInt64(rotation.magnitude), 0, 0, 0])
    }

    private func distributePowers(_ values: inout [PastaFp], by factor: PastaFp) {
        var power = PastaFp.one
        for index in values.indices {
            values[index] *= power
            power *= factor
        }
    }

    private func distributePowersZeta(_ values: inout [PastaFp], intoCoset: Bool) {
        let powers = intoCoset ? [gCoset, gCosetInv] : [gCosetInv, gCoset]
        for index in values.indices {
            let slot = index % 3
            if slot != 0 {
                values[index] *= powers[slot - 1]
            }
        }
    }

    private static func fft(_ input: [PastaFp], root: PastaFp, scale: PastaFp?) -> [PastaFp] {
        var values = input
        bitReverse(&values)

        var m = 1
        while m < values.count {
            let step = root.powVartime([UInt64(values.count / (2 * m)), 0, 0, 0])
            for start in stride(from: 0, to: values.count, by: 2 * m) {
                var w = PastaFp.one
                for offset in 0..<m {
                    let even = values[start + offset]
                    let odd = values[start + offset + m] * w
                    values[start + offset] = even + odd
                    values[start + offset + m] = even - odd
                    w *= step
                }
            }
            m *= 2
        }

        if let scale {
            for idx in values.indices {
                values[idx] *= scale
            }
        }
        return values
    }

    private static func bitReverse(_ values: inout [PastaFp]) {
        var j = 0
        for i in 1..<values.count {
            var bit = values.count >> 1
            while (j & bit) != 0 {
                j ^= bit
                bit >>= 1
            }
            j ^= bit
            if i < j {
                values.swapAt(i, j)
            }
        }
    }
}
