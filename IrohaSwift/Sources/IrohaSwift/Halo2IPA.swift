import Foundation

public enum Halo2IPAError: Error, Equatable {
    case invalidK(UInt32)
    case invalidPointCount(expected: Int, actual: Int)
    case invalidPolynomialLength(expected: Int, actual: Int)
    case truncatedParameters
    case invalidPointEncoding
    case nonInvertibleChallenge
}

public struct Halo2IPAParameters: Equatable, Sendable {
    public let k: UInt32
    public let n: Int
    public let g: [VestaAffine]
    public let gLagrange: [VestaAffine]
    public let w: VestaAffine
    public let u: VestaAffine

    public init(
        k: UInt32,
        g: [VestaAffine],
        gLagrange: [VestaAffine],
        w: VestaAffine,
        u: VestaAffine
    ) throws {
        guard k < 32 else {
            throw Halo2IPAError.invalidK(k)
        }
        let n = 1 << Int(k)
        guard g.count == n else {
            throw Halo2IPAError.invalidPointCount(expected: n, actual: g.count)
        }
        guard gLagrange.count == n else {
            throw Halo2IPAError.invalidPointCount(expected: n, actual: gLagrange.count)
        }
        self.k = k
        self.n = n
        self.g = g
        self.gLagrange = gLagrange
        self.w = w
        self.u = u
    }

    public static func read(from data: Data) throws -> Halo2IPAParameters {
        var cursor = 0
        let k = try readUInt32LE(data, cursor: &cursor)
        guard k < 32 else {
            throw Halo2IPAError.invalidK(k)
        }
        let n = 1 << Int(k)
        let g = try (0..<n).map { _ in try readPoint(data, cursor: &cursor) }
        let gLagrange = try (0..<n).map { _ in try readPoint(data, cursor: &cursor) }
        let w = try readPoint(data, cursor: &cursor)
        let u = try readPoint(data, cursor: &cursor)
        guard cursor == data.count else {
            throw Halo2IPAError.invalidPointEncoding
        }
        return try Halo2IPAParameters(k: k, g: g, gLagrange: gLagrange, w: w, u: u)
    }

    public static func generated(k: UInt32) throws -> Halo2IPAParameters {
        guard k < 32 else {
            throw Halo2IPAError.invalidK(k)
        }
        let n = 1 << Int(k)
        var gProjective = [VestaProjective]()
        gProjective.reserveCapacity(n)
        for idx in 0..<n {
            var message = Data(repeating: 0, count: 5)
            var indexLE = UInt32(idx).littleEndian
            withUnsafeBytes(of: &indexLE) { message.replaceSubrange(1..<5, with: $0) }
            gProjective.append(VestaHashToCurve.hash(domainPrefix: "Halo2-Parameters", message: message))
        }
        let g = gProjective.map { $0.toAffine() }
        let gLagrange = try lagrangeBasisGenerators(from: gProjective, k: k)
        let w = VestaHashToCurve.hash(domainPrefix: "Halo2-Parameters", message: Data([1])).toAffine()
        let u = VestaHashToCurve.hash(domainPrefix: "Halo2-Parameters", message: Data([2])).toAffine()
        return try Halo2IPAParameters(k: k, g: g, gLagrange: gLagrange, w: w, u: u)
    }

    public func serialized() -> Data {
        var out = Data(capacity: 4 + (g.count + gLagrange.count + 2) * 32)
        var kLE = k.littleEndian
        withUnsafeBytes(of: &kLE) { out.append(contentsOf: $0) }
        for point in g {
            out.append(point.compressedBytes())
        }
        for point in gLagrange {
            out.append(point.compressedBytes())
        }
        out.append(w.compressedBytes())
        out.append(u.compressedBytes())
        return out
    }

    public func commit(coefficients: [PastaFp], blind: PastaFp) throws -> VestaProjective {
        guard coefficients.count == n else {
            throw Halo2IPAError.invalidPolynomialLength(expected: n, actual: coefficients.count)
        }
        var scalars = coefficients
        var bases = g
        scalars.append(blind)
        bases.append(w)
        return VestaProjective.multiscalarMultiply(scalars: scalars, bases: bases)
    }

    public func commitLagrange(evaluations: [PastaFp], blind: PastaFp) throws -> VestaProjective {
        guard evaluations.count == n else {
            throw Halo2IPAError.invalidPolynomialLength(expected: n, actual: evaluations.count)
        }
        var scalars = evaluations
        var bases = gLagrange
        scalars.append(blind)
        bases.append(w)
        return VestaProjective.multiscalarMultiply(scalars: scalars, bases: bases)
    }

    private static func readUInt32LE(_ data: Data, cursor: inout Int) throws -> UInt32 {
        guard cursor + 4 <= data.count else {
            throw Halo2IPAError.truncatedParameters
        }
        var value: UInt32 = 0
        for offset in 0..<4 {
            value |= UInt32(data[cursor + offset]) << UInt32(offset * 8)
        }
        cursor += 4
        return value
    }

    private static func readPoint(_ data: Data, cursor: inout Int) throws -> VestaAffine {
        guard cursor + 32 <= data.count else {
            throw Halo2IPAError.truncatedParameters
        }
        let bytes = Data(data[cursor..<(cursor + 32)])
        cursor += 32
        guard let point = VestaAffine.fromCompressedBytes(bytes), !point.isIdentity else {
            throw Halo2IPAError.invalidPointEncoding
        }
        return point
    }

    private static func lagrangeBasisGenerators(from coefficientGenerators: [VestaProjective], k: UInt32) throws -> [VestaAffine] {
        let n = 1 << Int(k)
        guard coefficientGenerators.count == n else {
            throw Halo2IPAError.invalidPointCount(expected: n, actual: coefficientGenerators.count)
        }
        var omegaInv = PastaFp.rootOfUnityInv
        for _ in k..<UInt32(PastaFpParameters.twoAdicity) {
            omegaInv = omegaInv.squared()
        }
        var values = coefficientGenerators
        fftProjective(&values, root: omegaInv)
        guard let nInv = PastaFp(UInt64(n)).inverted() else {
            throw Halo2IPAError.invalidK(k)
        }
        return values.map { $0.multiplied(by: nInv).toAffine() }
    }

    private static func fftProjective(_ values: inout [VestaProjective], root: PastaFp) {
        bitReverse(&values)
        var m = 1
        while m < values.count {
            let step = root.powVartime([UInt64(values.count / (2 * m)), 0, 0, 0])
            for start in stride(from: 0, to: values.count, by: 2 * m) {
                var w = PastaFp.one
                for offset in 0..<m {
                    let even = values[start + offset]
                    let odd = values[start + offset + m].multiplied(by: w)
                    values[start + offset] = even + odd
                    values[start + offset + m] = even + (-odd)
                    w *= step
                }
            }
            m *= 2
        }
    }

    private static func bitReverse(_ values: inout [VestaProjective]) {
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

public struct Halo2IPAOpeningProofBundle: Equatable, Sendable {
    public let commitment: VestaAffine
    public let point: PastaFp
    public let value: PastaFp
    public let proof: Data
}

public struct Halo2IPAProverQuery: Equatable, Sendable {
    public let point: PastaFp
    public let polynomial: [PastaFp]
    public let blind: PastaFp

    public init(point: PastaFp, polynomial: [PastaFp], blind: PastaFp) {
        self.point = point
        self.polynomial = polynomial
        self.blind = blind
    }
}

public enum Halo2IPAOpeningProof {
    public static func create(
        params: Halo2IPAParameters,
        polynomial: [PastaFp],
        blind: PastaFp,
        point: PastaFp
    ) throws -> Halo2IPAOpeningProofBundle {
        var rng = SystemRandomNumberGenerator()
        return try create(params: params, polynomial: polynomial, blind: blind, point: point, rng: &rng)
    }

    public static func create<R: RandomNumberGenerator>(
        params: Halo2IPAParameters,
        polynomial: [PastaFp],
        blind: PastaFp,
        point: PastaFp,
        rng: inout R
    ) throws -> Halo2IPAOpeningProofBundle {
        guard polynomial.count == params.n else {
            throw Halo2IPAError.invalidPolynomialLength(expected: params.n, actual: polynomial.count)
        }
        let commitment = try params.commit(coefficients: polynomial, blind: blind).toAffine()
        let value = evaluatePolynomial(polynomial, at: point)
        var transcript = Halo2Blake2bWriteTranscript()
        try transcript.commonPoint(commitment)
        transcript.commonScalar(value)
        transcript.commonScalar(point)
        try appendProof(
            params: params,
            transcript: &transcript,
            polynomial: polynomial,
            blind: blind,
            point: point,
            rng: &rng
        )
        return Halo2IPAOpeningProofBundle(
            commitment: commitment,
            point: point,
            value: value,
            proof: transcript.proofBytes
        )
    }

    public static func verify(
        params: Halo2IPAParameters,
        commitment: VestaAffine,
        point: PastaFp,
        value: PastaFp,
        proof: Data
    ) throws -> Bool {
        var transcript = Halo2Blake2bReadTranscript(proof: proof)
        try transcript.commonPoint(commitment)
        transcript.commonScalar(value)
        transcript.commonScalar(point)

        var scalars = [PastaFp.one, -value]
        var bases = [commitment, params.g[0]]

        let sCommitment = try transcript.readPoint()
        let xi = transcript.squeezeChallenge().scalar
        scalars.append(xi)
        bases.append(sCommitment)
        let z = transcript.squeezeChallenge().scalar

        var roundChallenges: [PastaFp] = []
        for _ in 0..<Int(params.k) {
            let l = try transcript.readPoint()
            let r = try transcript.readPoint()
            let challenge = transcript.squeezeChallenge().scalar
            guard let challengeInv = challenge.inverted() else {
                throw Halo2IPAError.nonInvertibleChallenge
            }
            scalars.append(challengeInv)
            bases.append(l)
            scalars.append(challenge)
            bases.append(r)
            roundChallenges.append(challenge)
        }

        let c = try transcript.readScalar()
        let f = try transcript.readScalar()
        guard transcript.remainingBytes == 0 else {
            return false
        }

        let b = computeB(point: point, challenges: roundChallenges)
        scalars.append((-c) * b * z)
        bases.append(params.u)
        scalars.append(-f)
        bases.append(params.w)

        let gScalars = computeS(challenges: roundChallenges, initial: -c)
        scalars.append(contentsOf: gScalars)
        bases.append(contentsOf: params.g)

        return VestaProjective.multiscalarMultiply(scalars: scalars, bases: bases).toAffine() == .identity
    }

    public static func verifyInTranscript(
        params: Halo2IPAParameters,
        commitment: VestaProjective,
        point: PastaFp,
        value: PastaFp,
        transcript: inout Halo2Blake2bReadTranscript
    ) throws -> Bool {
        var accumulator = commitment + params.g[0].projective.multiplied(by: -value)

        let sCommitment = try transcript.readPoint()
        let xi = transcript.squeezeChallenge().scalar
        accumulator = accumulator + sCommitment.projective.multiplied(by: xi)
        let z = transcript.squeezeChallenge().scalar

        var roundChallenges: [PastaFp] = []
        for _ in 0..<Int(params.k) {
            let l = try transcript.readPoint()
            let r = try transcript.readPoint()
            let challenge = transcript.squeezeChallenge().scalar
            guard let challengeInv = challenge.inverted() else {
                throw Halo2IPAError.nonInvertibleChallenge
            }
            accumulator = accumulator + l.projective.multiplied(by: challengeInv)
            accumulator = accumulator + r.projective.multiplied(by: challenge)
            roundChallenges.append(challenge)
        }

        let c = try transcript.readScalar()
        let f = try transcript.readScalar()
        let b = computeB(point: point, challenges: roundChallenges)
        accumulator = accumulator + params.u.projective.multiplied(by: (-c) * b * z)
        accumulator = accumulator + params.w.projective.multiplied(by: -f)

        let gScalars = computeS(challenges: roundChallenges, initial: -c)
        for (scalar, base) in zip(gScalars, params.g) where scalar != .zero {
            accumulator = accumulator + base.projective.multiplied(by: scalar)
        }

        return accumulator.toAffine() == .identity
    }

    public static func appendProof<R: RandomNumberGenerator>(
        params: Halo2IPAParameters,
        transcript: inout Halo2Blake2bWriteTranscript,
        polynomial: [PastaFp],
        blind: PastaFp,
        point: PastaFp,
        rng: inout R
    ) throws {
        guard polynomial.count == params.n else {
            throw Halo2IPAError.invalidPolynomialLength(expected: params.n, actual: polynomial.count)
        }

        var sPolynomial = polynomial.map { _ in randomScalar(rng: &rng) }
        let sAtPoint = evaluatePolynomial(sPolynomial, at: point)
        sPolynomial[0] -= sAtPoint
        let sBlind = randomScalar(rng: &rng)
        let sCommitment = try params.commit(coefficients: sPolynomial, blind: sBlind).toAffine()
        try transcript.writePoint(sCommitment)

        let xi = transcript.squeezeChallenge().scalar
        let z = transcript.squeezeChallenge().scalar

        var pPrime = zip(sPolynomial, polynomial).map { s, p in s * xi + p }
        let v = evaluatePolynomial(pPrime, at: point)
        pPrime[0] -= v
        var f = sBlind * xi + blind

        var b = powers(of: point, count: params.n)
        var gPrime = params.g

        for round in 0..<Int(params.k) {
            let half = 1 << (Int(params.k) - round - 1)
            let pLo = Array(pPrime[0..<half])
            let pHi = Array(pPrime[half..<(half * 2)])
            let bLo = Array(b[0..<half])
            let bHi = Array(b[half..<(half * 2)])
            let gLo = Array(gPrime[0..<half])
            let gHi = Array(gPrime[half..<(half * 2)])

            let valueL = innerProduct(pHi, bLo)
            let valueR = innerProduct(pLo, bHi)
            let lRandomness = randomScalar(rng: &rng)
            let rRandomness = randomScalar(rng: &rng)

            var lScalars = pHi
            var lBases = gLo
            lScalars.append(valueL * z)
            lBases.append(params.u)
            lScalars.append(lRandomness)
            lBases.append(params.w)

            var rScalars = pLo
            var rBases = gHi
            rScalars.append(valueR * z)
            rBases.append(params.u)
            rScalars.append(rRandomness)
            rBases.append(params.w)

            try transcript.writePoint(VestaProjective.multiscalarMultiply(scalars: lScalars, bases: lBases).toAffine())
            try transcript.writePoint(VestaProjective.multiscalarMultiply(scalars: rScalars, bases: rBases).toAffine())

            let challenge = transcript.squeezeChallenge().scalar
            guard let challengeInv = challenge.inverted() else {
                throw Halo2IPAError.nonInvertibleChallenge
            }

            for idx in 0..<half {
                pPrime[idx] += pPrime[idx + half] * challengeInv
                b[idx] += b[idx + half] * challenge
                gPrime[idx] = (gPrime[idx].projective + gPrime[idx + half].projective.multiplied(by: challenge)).toAffine()
            }
            pPrime.removeSubrange(half..<pPrime.count)
            b.removeSubrange(half..<b.count)
            gPrime.removeSubrange(half..<gPrime.count)

            f += lRandomness * challengeInv
            f += rRandomness * challenge
        }

        transcript.writeScalar(pPrime[0])
        transcript.writeScalar(f)
    }

    public static func appendSamePointMultiOpeningProof<R: RandomNumberGenerator>(
        params: Halo2IPAParameters,
        transcript: inout Halo2Blake2bWriteTranscript,
        queries: [Halo2IPAProverQuery],
        rng: inout R
    ) throws {
        guard let first = queries.first else {
            throw Halo2IPAError.invalidPolynomialLength(expected: params.n, actual: 0)
        }
        guard first.polynomial.count == params.n else {
            throw Halo2IPAError.invalidPolynomialLength(expected: params.n, actual: first.polynomial.count)
        }
        for query in queries {
            guard query.point == first.point else {
                throw Halo2IPAError.nonInvertibleChallenge
            }
            guard query.polynomial.count == params.n else {
                throw Halo2IPAError.invalidPolynomialLength(expected: params.n, actual: query.polynomial.count)
            }
        }

        let x1 = transcript.squeezeChallenge().scalar
        _ = transcript.squeezeChallenge()

        var qPolynomial = first.polynomial
        var qBlind = first.blind
        for query in queries.dropFirst() {
            qPolynomial = add(scale(qPolynomial, by: x1), query.polynomial)
            qBlind = qBlind * x1 + query.blind
        }

        var qPrimePolynomial = kateDivision(qPolynomial, by: first.point)
        qPrimePolynomial.append(contentsOf: repeatElement(PastaFp.zero, count: params.n - qPrimePolynomial.count))
        let qPrimeBlind = randomScalar(rng: &rng)
        let qPrimeCommitment = try params.commit(coefficients: qPrimePolynomial, blind: qPrimeBlind).toAffine()
        try transcript.writePoint(qPrimeCommitment)

        let x3 = transcript.squeezeChallenge().scalar
        transcript.writeScalar(evaluatePolynomial(qPolynomial, at: x3))

        let x4 = transcript.squeezeChallenge().scalar
        let pPolynomial = add(scale(qPrimePolynomial, by: x4), qPolynomial)
        let pBlind = qPrimeBlind * x4 + qBlind

        try appendProof(
            params: params,
            transcript: &transcript,
            polynomial: pPolynomial,
            blind: pBlind,
            point: x3,
            rng: &rng
        )
    }

    private static func randomScalar<R: RandomNumberGenerator>(rng: inout R) -> PastaFp {
        var bytes = Data(capacity: 64)
        for _ in 0..<8 {
            var word = rng.next().littleEndian
            withUnsafeBytes(of: &word) { bytes.append(contentsOf: $0) }
        }
        return PastaFp.fromUniformBytes64(bytes)!
    }

    private static func evaluatePolynomial(_ polynomial: [PastaFp], at point: PastaFp) -> PastaFp {
        polynomial.reversed().reduce(PastaFp.zero) { accumulator, coefficient in
            accumulator * point + coefficient
        }
    }

    private static func kateDivision(_ polynomial: [PastaFp], by point: PastaFp) -> [PastaFp] {
        precondition(!polynomial.isEmpty)
        guard polynomial.count > 1 else {
            return []
        }
        let negPoint = -point
        var quotient = [PastaFp](repeating: .zero, count: polynomial.count - 1)
        var tmp = PastaFp.zero
        for (quotientIndex, coefficient) in zip(quotient.indices.reversed(), polynomial.dropFirst().reversed()) {
            let leadCoefficient = coefficient - tmp
            quotient[quotientIndex] = leadCoefficient
            tmp = leadCoefficient * negPoint
        }
        return quotient
    }

    private static func add(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> [PastaFp] {
        precondition(lhs.count == rhs.count)
        return zip(lhs, rhs).map(+)
    }

    private static func scale(_ values: [PastaFp], by scalar: PastaFp) -> [PastaFp] {
        values.map { $0 * scalar }
    }

    private static func innerProduct(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> PastaFp {
        precondition(lhs.count == rhs.count)
        return zip(lhs, rhs).reduce(PastaFp.zero) { accumulator, pair in
            accumulator + pair.0 * pair.1
        }
    }

    private static func powers(of point: PastaFp, count: Int) -> [PastaFp] {
        var powers: [PastaFp] = []
        powers.reserveCapacity(count)
        var current = PastaFp.one
        for _ in 0..<count {
            powers.append(current)
            current *= point
        }
        return powers
    }

    private static func computeB(point: PastaFp, challenges: [PastaFp]) -> PastaFp {
        var result = PastaFp.one
        var current = point
        for challenge in challenges.reversed() {
            result *= PastaFp.one + challenge * current
            current *= current
        }
        return result
    }

    private static func computeS(challenges: [PastaFp], initial: PastaFp) -> [PastaFp] {
        guard !challenges.isEmpty else {
            return [initial]
        }
        var values = [PastaFp](repeating: .zero, count: 1 << challenges.count)
        values[0] = initial
        for (idx, challenge) in challenges.reversed().enumerated() {
            let len = 1 << idx
            for slot in 0..<len {
                values[len + slot] = values[slot] * challenge
            }
        }
        return values
    }
}
