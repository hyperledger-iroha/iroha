import Foundation

public enum Halo2TranscriptError: Error, Equatable {
    case pointAtInfinity
    case invalidPointEncoding
    case invalidScalarEncoding
    case truncatedProof
}

public struct Halo2Challenge255: Equatable, Sendable {
    public let scalar: PastaFp
    public let encodedBytes: Data
}

public struct Halo2Blake2bWriteTranscript: Sendable {
    private static let personal = Data("Halo2-Transcript".utf8)
    private static let prefixChallenge: UInt8 = 0
    private static let prefixPoint: UInt8 = 1
    private static let prefixScalar: UInt8 = 2

    private var state = Data()
    private var proof = Data()

    public init() {}

    public var proofBytes: Data {
        proof
    }

    public mutating func commonPoint(_ point: VestaAffine) throws {
        guard !point.isIdentity else {
            throw Halo2TranscriptError.pointAtInfinity
        }
        state.append(Self.prefixPoint)
        state.append(point.x.canonicalBytes())
        state.append(point.y.canonicalBytes())
    }

    public mutating func commonScalar(_ scalar: PastaFp) {
        state.append(Self.prefixScalar)
        state.append(scalar.canonicalBytes())
    }

    public mutating func writePoint(_ point: VestaAffine) throws {
        try commonPoint(point)
        proof.append(point.compressedBytes())
    }

    public mutating func writeScalar(_ scalar: PastaFp) {
        commonScalar(scalar)
        proof.append(scalar.canonicalBytes())
    }

    public mutating func squeezeChallenge() -> Halo2Challenge255 {
        state.append(Self.prefixChallenge)
        let digest = Blake2b.hash512(state, personal: Self.personal)
        let scalar = PastaFp.fromUniformBytes64(digest)!
        return Halo2Challenge255(scalar: scalar, encodedBytes: scalar.canonicalBytes())
    }
}

public struct Halo2Blake2bReadTranscript: Sendable {
    private static let personal = Data("Halo2-Transcript".utf8)
    private static let prefixChallenge: UInt8 = 0
    private static let prefixPoint: UInt8 = 1
    private static let prefixScalar: UInt8 = 2

    private let proof: Data
    private var offset = 0
    private var state = Data()

    public init(proof: Data) {
        self.proof = proof
    }

    public var remainingBytes: Int {
        proof.count - offset
    }

    public mutating func commonPoint(_ point: VestaAffine) throws {
        guard !point.isIdentity else {
            throw Halo2TranscriptError.pointAtInfinity
        }
        state.append(Self.prefixPoint)
        state.append(point.x.canonicalBytes())
        state.append(point.y.canonicalBytes())
    }

    public mutating func commonScalar(_ scalar: PastaFp) {
        state.append(Self.prefixScalar)
        state.append(scalar.canonicalBytes())
    }

    public mutating func readPoint() throws -> VestaAffine {
        let bytes = try read(count: 32)
        guard let point = VestaAffine.fromCompressedBytes(bytes), !point.isIdentity else {
            throw Halo2TranscriptError.invalidPointEncoding
        }
        try commonPoint(point)
        return point
    }

    public mutating func readScalar() throws -> PastaFp {
        let bytes = try read(count: 32)
        guard let scalar = PastaFp.fromCanonicalBytes(bytes) else {
            throw Halo2TranscriptError.invalidScalarEncoding
        }
        commonScalar(scalar)
        return scalar
    }

    public mutating func squeezeChallenge() -> Halo2Challenge255 {
        state.append(Self.prefixChallenge)
        let digest = Blake2b.hash512(state, personal: Self.personal)
        let scalar = PastaFp.fromUniformBytes64(digest)!
        return Halo2Challenge255(scalar: scalar, encodedBytes: scalar.canonicalBytes())
    }

    private mutating func read(count: Int) throws -> Data {
        guard offset + count <= proof.count else {
            throw Halo2TranscriptError.truncatedProof
        }
        let start = offset
        offset += count
        return Data(proof[start..<offset])
    }
}
