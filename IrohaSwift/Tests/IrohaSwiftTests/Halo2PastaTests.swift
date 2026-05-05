import Foundation
import XCTest
@testable import IrohaSwift

final class Halo2PastaTests: XCTestCase {
    func testFpSmallArithmeticUsesCanonicalPastaField() throws {
        XCTAssertEqual(PastaFp.zero.canonicalBytes(), Data(repeating: 0, count: 32))
        XCTAssertEqual(PastaFp.one.canonicalBytes(), Self.scalarBytes(1))
        XCTAssertEqual(PastaFp(3) + PastaFp(5), PastaFp(8))
        XCTAssertEqual(PastaFp(3) * PastaFp(5), PastaFp(15))
        XCTAssertEqual(PastaFp(5).squared(), PastaFp(25))
        let inverse = try XCTUnwrap(PastaFp(7).inverted())
        XCTAssertEqual(PastaFp(7) * inverse, PastaFp.one)
    }

    func testFqSmallArithmeticUsesCanonicalPastaField() throws {
        XCTAssertEqual(PastaFq.one.canonicalBytes(), Self.scalarBytes(1))
        XCTAssertEqual(PastaFq(11) - PastaFq(8), PastaFq(3))
        XCTAssertEqual(PastaFq(9) * PastaFq(9), PastaFq(81))
        let inverse = try XCTUnwrap(PastaFq(13).inverted())
        XCTAssertEqual(PastaFq(13) * inverse, PastaFq.one)
    }

    func testPastaSquareRootAndVestaCompressedEncoding() throws {
        XCTAssertEqual(PastaFp(25).squareRoot()?.squared(), PastaFp(25))
        XCTAssertEqual(PastaFq(25).squareRoot()?.squared(), PastaFq(25))
        XCTAssertNil(PastaFp(5).squareRoot())

        let generator = VestaAffine.generator
        var expectedGeneratorX = PastaFqParameters.modulus
        expectedGeneratorX[0] -= 1
        XCTAssertEqual(generator.compressedBytes(), Self.bytes(fromLimbs: expectedGeneratorX))
        XCTAssertEqual(VestaAffine.fromCompressedBytes(generator.compressedBytes()), generator)
        XCTAssertEqual(VestaAffine.fromCompressedBytes(Data(repeating: 0, count: 32)), .identity)

        var invalidIdentity = Data(repeating: 0, count: 32)
        invalidIdentity[31] = 0x80
        XCTAssertNil(VestaAffine.fromCompressedBytes(invalidIdentity))
    }

    func testPastaUniformBytesAndVestaGroupArithmetic() throws {
        XCTAssertEqual(PastaFp.fromUniformBytes64(Data(repeating: 0, count: 64)), .zero)
        var threeWide = Data(repeating: 0, count: 64)
        threeWide[0] = 3
        XCTAssertEqual(PastaFp.fromUniformBytes64(threeWide), PastaFp(3))

        let g = VestaAffine.generator
        let inv16 = try XCTUnwrap(PastaFq(16).inverted())
        let inv64 = try XCTUnwrap(PastaFq(64).inverted())
        let expectedDouble = try XCTUnwrap(VestaAffine(
            x: PastaFq(41) * inv16,
            y: -PastaFq(299) * inv64
        ))
        XCTAssertEqual(g.doubled(), expectedDouble)
        XCTAssertEqual(g.multiplied(by: PastaFp(2)), expectedDouble)
        XCTAssertEqual(g.projective.mixedAdded(g).toAffine(), expectedDouble)
        XCTAssertEqual(g.multiplied(by: .zero), .identity)

        let threeG = g.multiplied(by: PastaFp(3))
        XCTAssertEqual(expectedDouble.added(g), threeG)
        XCTAssertEqual(VestaProjective.multiscalarMultiply(
            scalars: [PastaFp(2), PastaFp(3)],
            bases: [g, g]
        ).toAffine(), g.multiplied(by: PastaFp(5)))
    }

    func testHalo2Blake2bTranscriptRoundTripsProofMessages() throws {
        var writer = Halo2Blake2bWriteTranscript()
        try writer.writePoint(.generator)
        writer.writeScalar(PastaFp(42))
        let writerChallenge = writer.squeezeChallenge()

        var reader = Halo2Blake2bReadTranscript(proof: writer.proofBytes)
        XCTAssertEqual(try reader.readPoint(), .generator)
        XCTAssertEqual(try reader.readScalar(), PastaFp(42))
        let readerChallenge = reader.squeezeChallenge()

        XCTAssertEqual(reader.remainingBytes, 0)
        XCTAssertEqual(readerChallenge, writerChallenge)
        XCTAssertEqual(writerChallenge.encodedBytes.count, 32)
    }

    func testHalo2Blake2bTranscriptRejectsMalformedProofMessages() throws {
        var writer = Halo2Blake2bWriteTranscript()
        XCTAssertThrowsError(try writer.commonPoint(.identity)) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .pointAtInfinity)
        }

        var truncatedPoint = Halo2Blake2bReadTranscript(proof: Data(repeating: 0, count: 31))
        XCTAssertThrowsError(try truncatedPoint.readPoint()) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .truncatedProof)
        }

        var identityPoint = Halo2Blake2bReadTranscript(proof: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try identityPoint.readPoint()) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidPointEncoding)
        }

        var nonCanonicalScalar = Halo2Blake2bReadTranscript(proof: Self.bytes(fromLimbs: PastaFpParameters.modulus))
        XCTAssertThrowsError(try nonCanonicalScalar.readScalar()) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidScalarEncoding)
        }
    }

    func testIPAParametersReadAndCommitUseIrohaLayout() throws {
        let generator = VestaAffine.generator
        let g = (1...4).map { generator.multiplied(by: PastaFp(UInt64($0))) }
        let gLagrange = (5...8).map { generator.multiplied(by: PastaFp(UInt64($0))) }
        let params = try Halo2IPAParameters(
            k: 2,
            g: g,
            gLagrange: gLagrange,
            w: generator.multiplied(by: PastaFp(9)),
            u: generator.multiplied(by: PastaFp(10))
        )
        XCTAssertEqual(try Halo2IPAParameters.read(from: params.serialized()), params)

        let coefficients = [PastaFp(1), PastaFp(2), PastaFp(3), PastaFp(4)]
        let blind = PastaFp(5)
        var expectedScalars = coefficients
        var expectedBases = g
        expectedScalars.append(blind)
        expectedBases.append(params.w)
        XCTAssertEqual(
            try params.commit(coefficients: coefficients, blind: blind).toAffine(),
            VestaProjective.multiscalarMultiply(scalars: expectedScalars, bases: expectedBases).toAffine()
        )
        XCTAssertThrowsError(try params.commit(coefficients: [PastaFp(1)], blind: blind))
    }

    func testIPAParametersRejectMalformedEncodingsAndLengths() throws {
        let generator = VestaAffine.generator
        XCTAssertThrowsError(try Halo2IPAParameters.generated(k: 32)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidK(32))
        }
        XCTAssertThrowsError(try Halo2IPAParameters(
            k: 2,
            g: [generator, generator, generator],
            gLagrange: [generator, generator, generator, generator],
            w: generator,
            u: generator
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointCount(expected: 4, actual: 3))
        }

        let params = try Halo2IPAParameters.generated(k: 1)
        XCTAssertThrowsError(try params.commitLagrange(evaluations: [PastaFp.one], blind: .zero)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 2, actual: 1))
        }

        XCTAssertThrowsError(try Halo2IPAParameters.read(from: Data([0, 0, 0]))) { error in
            XCTAssertEqual(error as? Halo2IPAError, .truncatedParameters)
        }

        var invalidK = Data()
        Self.appendUInt32LE(32, to: &invalidK)
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: invalidK)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidK(32))
        }

        var identityPointParameters = Data()
        Self.appendUInt32LE(0, to: &identityPointParameters)
        identityPointParameters.append(Data(repeating: 0, count: 32 * 4))
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: identityPointParameters)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }

        var trailingByteParameters = params.serialized()
        trailingByteParameters.append(0)
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: trailingByteParameters)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }
    }

    func testZeroRoundIPAParametersRoundTripAndCommit() throws {
        let params = try Halo2IPAParameters.generated(k: 0)
        XCTAssertEqual(params.k, 0)
        XCTAssertEqual(params.n, 1)
        XCTAssertEqual(params.serialized().count, 132)
        XCTAssertEqual(params.gLagrange, params.g)
        XCTAssertEqual(try Halo2IPAParameters.read(from: params.serialized()), params)

        let coefficient = PastaFp(21)
        let evaluation = PastaFp(23)
        let blind = PastaFp(29)
        XCTAssertEqual(
            try params.commit(coefficients: [coefficient], blind: blind).toAffine(),
            VestaProjective.multiscalarMultiply(
                scalars: [coefficient, blind],
                bases: [params.g[0], params.w]
            ).toAffine()
        )
        XCTAssertEqual(
            try params.commitLagrange(evaluations: [evaluation], blind: blind).toAffine(),
            VestaProjective.multiscalarMultiply(
                scalars: [evaluation, blind],
                bases: [params.gLagrange[0], params.w]
            ).toAffine()
        )
        XCTAssertThrowsError(try params.commit(coefficients: [], blind: blind)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertThrowsError(try params.commit(coefficients: [coefficient, PastaFp(31)], blind: blind)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }
        XCTAssertThrowsError(try params.commitLagrange(evaluations: [], blind: blind)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertThrowsError(try params.commitLagrange(evaluations: [evaluation, PastaFp(37)], blind: blind)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }
    }

    func testZeroRoundIPAParametersRejectMalformedSerializedPayloads() throws {
        let params = try Halo2IPAParameters.generated(k: 0)

        var truncated = params.serialized()
        truncated.removeLast()
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: truncated)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .truncatedParameters)
        }

        var trailingByte = params.serialized()
        trailingByte.append(0)
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: trailingByte)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }

        var identityGenerator = params.serialized()
        identityGenerator.replaceSubrange(4..<36, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: identityGenerator)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }

        var identityLagrangeGenerator = params.serialized()
        identityLagrangeGenerator.replaceSubrange(36..<68, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: identityLagrangeGenerator)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }

        var identityW = params.serialized()
        identityW.replaceSubrange(68..<100, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: identityW)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }

        var identityU = params.serialized()
        identityU.replaceSubrange(100..<132, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try Halo2IPAParameters.read(from: identityU)) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointEncoding)
        }
    }

    func testZeroRoundIPAParametersRejectConstructorLengthMismatches() throws {
        let generator = VestaAffine.generator
        let point = generator.multiplied(by: PastaFp(3))

        XCTAssertThrowsError(try Halo2IPAParameters(
            k: 0,
            g: [],
            gLagrange: [point],
            w: point,
            u: point
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointCount(expected: 1, actual: 0))
        }

        XCTAssertThrowsError(try Halo2IPAParameters(
            k: 0,
            g: [point],
            gLagrange: [],
            w: point,
            u: point
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointCount(expected: 1, actual: 0))
        }

        XCTAssertThrowsError(try Halo2IPAParameters(
            k: 0,
            g: [point, generator.multiplied(by: PastaFp(5))],
            gLagrange: [point],
            w: point,
            u: point
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointCount(expected: 1, actual: 2))
        }

        XCTAssertThrowsError(try Halo2IPAParameters(
            k: 0,
            g: [point],
            gLagrange: [point, generator.multiplied(by: PastaFp(7))],
            w: point,
            u: point
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPointCount(expected: 1, actual: 2))
        }
    }

    func testGeneratedIPAParametersMatchRustHalo2() throws {
        let expected = try Self.hex("""
        0300000045065ed079bf389758f591131095ef419310e8c708a805852b9b77bed8c7ecbde0c0802686d3ed571f7f3399526b24460b16ace461ebda9dcfe6e5b7b298c18cd27962962ce9ed2b87ae4c95462914917a9da0295f593956c1a76adca2ebc394a74f2af0a526eb437ebb22c416a78fda0a275a2f2756115e51248f7c5a7acdaca3c70c4c2497a714ebe96b6192bf940a13a277a6a7c4152a3524988d86be181bc887e3240ff4de618fb8531a447b4f469f50a8ac35fe905da8b8e8732e527eb8b7e796477bd8c4fe4b9ef9806c39cb32ed5558ef9d8c582909779eec700e259a6787540156e67c55fba03e961d98971ccd6bf71c4edd0d80d4d7aec06f9cdb0a1d499756f70cf0deb7b52b1b0def41337e091269c9bcd46334f97a05842a0f0105c58c4a440e77a80e25f3dd162f2b74d6866003b664d1c3ddd6a185c3309314dd77de397f82818f5b36495c402dbb92096a5fc5f8b73210ac79f9b8af3e7ab9cd97784c9657aeefb68194c1546984ae653ba1e4abfc06e887990db451c7cd3227cdb3ef3e31a17568c8953cffba5a911f7c2020cdde94c8b96d35d5eeb7f6021ad59e266061b96ab8968ac8e547138a9f47b9186eddf13ad329d0921707de14fe16f4009d2fa0c76524113ba0796a5f512d5bf2acdd92721d841139fd5b242d3904ea829062811fcf825218a681c7881caa753008c9a8aecd1f8cc600b410907520d96f3e5cd41760367151608b54821883c10c4b9a4ff2beae227bef94bcab379dc4dcfdbf61ccc7d5a0bb9759acf611694f24d0c040f249bad30a83b1a897
        """)
        XCTAssertEqual(try Halo2IPAParameters.generated(k: 3).serialized(), expected)
    }

    func testIPAOpeningProofVerifiesNativeTranscript() throws {
        let generator = VestaAffine.generator
        let g = (1...8).map { generator.multiplied(by: PastaFp(UInt64($0))) }
        let params = try Halo2IPAParameters(
            k: 3,
            g: g,
            gLagrange: g,
            w: generator.multiplied(by: PastaFp(11)),
            u: generator.multiplied(by: PastaFp(12))
        )
        let polynomial = (1...8).map { PastaFp(UInt64($0 * 3 + 1)) }
        var rng = SeededGenerator(state: 0x0123_4567_89ab_cdef)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: polynomial,
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &rng
        )

        XCTAssertTrue(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: proof.proof
        ))
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value + PastaFp.one,
            proof: proof.proof
        ))

        var proofWithTrailingByte = proof.proof
        proofWithTrailingByte.append(0)
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: proofWithTrailingByte
        ))

        var tamperedProof = proof.proof
        tamperedProof[0] ^= 0x01
        XCTAssertFalse((try? Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: tamperedProof
        )) == true)

        var invalidLengthRNG = SeededGenerator(state: 0x1111_2222_3333_4444)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: Array(polynomial.dropLast()),
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &invalidLengthRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: params.n, actual: params.n - 1))
        }

        var emptyQueryTranscript = Halo2Blake2bWriteTranscript()
        var emptyQueryRNG = SeededGenerator(state: 0x2222_3333_4444_5555)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &emptyQueryTranscript,
            queries: [],
            rng: &emptyQueryRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: params.n, actual: 0))
        }

        var mismatchedPointTranscript = Halo2Blake2bWriteTranscript()
        var mismatchedPointRNG = SeededGenerator(state: 0x3333_4444_5555_6666)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &mismatchedPointTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: polynomial, blind: PastaFp(17)),
                Halo2IPAProverQuery(point: PastaFp(23), polynomial: polynomial, blind: PastaFp(29)),
            ],
            rng: &mismatchedPointRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .nonInvertibleChallenge)
        }
    }

    func testIPAOpeningProofHandlesZeroRoundParameters() throws {
        let params = try Self.zeroRoundIPAParameters()
        var rng = SeededGenerator(state: 0x4444_5555_6666_7777)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13)],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &rng
        )
        XCTAssertEqual(proof.proof.count, 96)

        XCTAssertTrue(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: proof.proof
        ))
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value + PastaFp.one,
            proof: proof.proof
        ))

        var transcript = Halo2Blake2bReadTranscript(proof: proof.proof)
        try transcript.commonPoint(proof.commitment)
        transcript.commonScalar(proof.value)
        transcript.commonScalar(proof.point)
        XCTAssertTrue(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &transcript
        ))
        XCTAssertEqual(transcript.remainingBytes, 0)

        var wrongTranscript = Halo2Blake2bReadTranscript(proof: proof.proof)
        try wrongTranscript.commonPoint(proof.commitment)
        wrongTranscript.commonScalar(proof.value + PastaFp.one)
        wrongTranscript.commonScalar(proof.point)
        XCTAssertFalse(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value + PastaFp.one,
            transcript: &wrongTranscript
        ))

        var proofWithTrailingByte = proof.proof
        proofWithTrailingByte.append(0)
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: proofWithTrailingByte
        ))

        var truncatedProof = proof.proof
        truncatedProof.removeLast()
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: truncatedProof
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .truncatedProof)
        }

        var invalidSCommitment = proof.proof
        invalidSCommitment.replaceSubrange(0..<32, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: invalidSCommitment
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidPointEncoding)
        }

        var nonCanonicalC = proof.proof
        nonCanonicalC.replaceSubrange(32..<64, with: Self.bytes(fromLimbs: PastaFpParameters.modulus))
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: nonCanonicalC
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidScalarEncoding)
        }

        var nonCanonicalF = proof.proof
        nonCanonicalF.replaceSubrange(64..<96, with: Self.bytes(fromLimbs: PastaFpParameters.modulus))
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: nonCanonicalF
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidScalarEncoding)
        }
    }

    func testZeroRoundGeneratedIPAOpeningProofUsesPublicCreateOverload() throws {
        let params = try Halo2IPAParameters.generated(k: 0)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(41)],
            blind: PastaFp(43),
            point: PastaFp(47)
        )

        XCTAssertEqual(proof.proof.count, 96)
        XCTAssertEqual(proof.value, PastaFp(41))
        XCTAssertTrue(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: proof.proof
        ))
    }

    func testZeroRoundIPAOpeningProofRejectsIdentityCommitmentTranscript() throws {
        let params = try Self.zeroRoundIPAParameters()
        XCTAssertEqual(
            try params.commit(coefficients: [.zero], blind: .zero).toAffine(),
            .identity
        )

        var identityCreateRNG = SeededGenerator(state: 0x4242_6464_8686_a8a8)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [.zero],
            blind: .zero,
            point: PastaFp(19),
            rng: &identityCreateRNG
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .pointAtInfinity)
        }

        var proofRNG = SeededGenerator(state: 0x5353_7575_9797_b9b9)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13)],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &proofRNG
        )
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: .identity,
            point: proof.point,
            value: proof.value,
            proof: proof.proof
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .pointAtInfinity)
        }
    }

    func testZeroRoundIPAOpeningProofRejectsCanonicalFinalScalarTampering() throws {
        let params = try Self.zeroRoundIPAParameters()
        var rng = SeededGenerator(state: 0x1212_3434_5656_7878)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13)],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &rng
        )

        let c = try XCTUnwrap(PastaFp.fromCanonicalBytes(Data(proof.proof[32..<64])))
        var tamperedC = proof.proof
        tamperedC.replaceSubrange(32..<64, with: (c + PastaFp.one).canonicalBytes())
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: tamperedC
        ))

        let f = try XCTUnwrap(PastaFp.fromCanonicalBytes(Data(proof.proof[64..<96])))
        var tamperedF = proof.proof
        tamperedF.replaceSubrange(64..<96, with: (f + PastaFp.one).canonicalBytes())
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: proof.point,
            value: proof.value,
            proof: tamperedF
        ))
    }

    func testZeroRoundIPAOpeningProofRejectsWrongCommitmentAndPoint() throws {
        let params = try Self.zeroRoundIPAParameters()
        var rng = SeededGenerator(state: 0x1313_3535_5757_7979)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13)],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &rng
        )
        let wrongCommitment = try params.commit(coefficients: [PastaFp(23)], blind: PastaFp(29)).toAffine()
        let wrongPoint = proof.point + PastaFp.one

        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: wrongCommitment,
            point: proof.point,
            value: proof.value,
            proof: proof.proof
        ))
        XCTAssertFalse(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: proof.commitment,
            point: wrongPoint,
            value: proof.value,
            proof: proof.proof
        ))

        var wrongCommitmentTranscript = Halo2Blake2bReadTranscript(proof: proof.proof)
        try wrongCommitmentTranscript.commonPoint(wrongCommitment)
        wrongCommitmentTranscript.commonScalar(proof.value)
        wrongCommitmentTranscript.commonScalar(proof.point)
        XCTAssertFalse(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: wrongCommitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &wrongCommitmentTranscript
        ))

        var wrongPointTranscript = Halo2Blake2bReadTranscript(proof: proof.proof)
        try wrongPointTranscript.commonPoint(proof.commitment)
        wrongPointTranscript.commonScalar(proof.value)
        wrongPointTranscript.commonScalar(wrongPoint)
        XCTAssertFalse(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: wrongPoint,
            value: proof.value,
            transcript: &wrongPointTranscript
        ))
    }

    func testZeroRoundIPAVerifyInTranscriptHandlesMalformedProofReads() throws {
        let params = try Self.zeroRoundIPAParameters()
        var rng = SeededGenerator(state: 0x2323_4545_6767_8989)
        let proof = try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13)],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &rng
        )

        var invalidSCommitmentProof = proof.proof
        invalidSCommitmentProof.replaceSubrange(0..<32, with: Data(repeating: 0, count: 32))
        var invalidSCommitmentTranscript = Halo2Blake2bReadTranscript(proof: invalidSCommitmentProof)
        try invalidSCommitmentTranscript.commonPoint(proof.commitment)
        invalidSCommitmentTranscript.commonScalar(proof.value)
        invalidSCommitmentTranscript.commonScalar(proof.point)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &invalidSCommitmentTranscript
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidPointEncoding)
        }

        var truncatedProof = proof.proof
        truncatedProof.removeLast()
        var truncatedTranscript = Halo2Blake2bReadTranscript(proof: truncatedProof)
        try truncatedTranscript.commonPoint(proof.commitment)
        truncatedTranscript.commonScalar(proof.value)
        truncatedTranscript.commonScalar(proof.point)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &truncatedTranscript
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .truncatedProof)
        }

        var nonCanonicalCProof = proof.proof
        nonCanonicalCProof.replaceSubrange(32..<64, with: Self.bytes(fromLimbs: PastaFpParameters.modulus))
        var nonCanonicalCTranscript = Halo2Blake2bReadTranscript(proof: nonCanonicalCProof)
        try nonCanonicalCTranscript.commonPoint(proof.commitment)
        nonCanonicalCTranscript.commonScalar(proof.value)
        nonCanonicalCTranscript.commonScalar(proof.point)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &nonCanonicalCTranscript
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidScalarEncoding)
        }

        var nonCanonicalFProof = proof.proof
        nonCanonicalFProof.replaceSubrange(64..<96, with: Self.bytes(fromLimbs: PastaFpParameters.modulus))
        var nonCanonicalFTranscript = Halo2Blake2bReadTranscript(proof: nonCanonicalFProof)
        try nonCanonicalFTranscript.commonPoint(proof.commitment)
        nonCanonicalFTranscript.commonScalar(proof.value)
        nonCanonicalFTranscript.commonScalar(proof.point)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &nonCanonicalFTranscript
        )) { error in
            XCTAssertEqual(error as? Halo2TranscriptError, .invalidScalarEncoding)
        }

        var proofWithTrailingByte = proof.proof
        proofWithTrailingByte.append(0)
        var trailingTranscript = Halo2Blake2bReadTranscript(proof: proofWithTrailingByte)
        try trailingTranscript.commonPoint(proof.commitment)
        trailingTranscript.commonScalar(proof.value)
        trailingTranscript.commonScalar(proof.point)
        XCTAssertTrue(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: proof.commitment.projective,
            point: proof.point,
            value: proof.value,
            transcript: &trailingTranscript
        ))
        XCTAssertEqual(trailingTranscript.remainingBytes, 1)
    }

    func testZeroRoundIPAAppendProofCanVerifyStandaloneTranscript() throws {
        let params = try Self.zeroRoundIPAParameters()
        let polynomial = [PastaFp(13)]
        let blind = PastaFp(17)
        let point = PastaFp(19)
        let commitment = try params.commit(coefficients: polynomial, blind: blind).toAffine()
        let value = Self.evaluate(polynomial, at: point)

        var writer = Halo2Blake2bWriteTranscript()
        try writer.commonPoint(commitment)
        writer.commonScalar(value)
        writer.commonScalar(point)
        var rng = SeededGenerator(state: 0x5555_aaaa_7777_cccc)
        try Halo2IPAOpeningProof.appendProof(
            params: params,
            transcript: &writer,
            polynomial: polynomial,
            blind: blind,
            point: point,
            rng: &rng
        )
        XCTAssertEqual(writer.proofBytes.count, 96)

        var reader = Halo2Blake2bReadTranscript(proof: writer.proofBytes)
        try reader.commonPoint(commitment)
        reader.commonScalar(value)
        reader.commonScalar(point)
        XCTAssertTrue(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: commitment.projective,
            point: point,
            value: value,
            transcript: &reader
        ))
        XCTAssertEqual(reader.remainingBytes, 0)
    }

    func testZeroRoundIPAAppendProofRejectsInvalidPolynomialLengths() throws {
        let params = try Self.zeroRoundIPAParameters()

        var shortCreateRNG = SeededGenerator(state: 0xaaaa_bbbb_cccc_dddd)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &shortCreateRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }

        var longCreateRNG = SeededGenerator(state: 0xbbbb_cccc_dddd_eeee)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.create(
            params: params,
            polynomial: [PastaFp(13), PastaFp(17)],
            blind: PastaFp(19),
            point: PastaFp(23),
            rng: &longCreateRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }

        var shortAppendTranscript = Halo2Blake2bWriteTranscript()
        var shortAppendRNG = SeededGenerator(state: 0xcccc_dddd_eeee_ffff)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendProof(
            params: params,
            transcript: &shortAppendTranscript,
            polynomial: [],
            blind: PastaFp(17),
            point: PastaFp(19),
            rng: &shortAppendRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertEqual(shortAppendTranscript.proofBytes.count, 0)

        var longAppendTranscript = Halo2Blake2bWriteTranscript()
        var longAppendRNG = SeededGenerator(state: 0xdddd_eeee_ffff_aaaa)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendProof(
            params: params,
            transcript: &longAppendTranscript,
            polynomial: [PastaFp(13), PastaFp(17)],
            blind: PastaFp(19),
            point: PastaFp(23),
            rng: &longAppendRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }
        XCTAssertEqual(longAppendTranscript.proofBytes.count, 0)
    }

    func testZeroRoundSamePointMultiOpeningProofVerifies() throws {
        let params = try Self.zeroRoundIPAParameters()
        let polynomial = [PastaFp(13)]
        let blind = PastaFp(17)
        let point = PastaFp(19)
        let commitment = try params.commit(coefficients: polynomial, blind: blind)
        let value = Self.evaluate(polynomial, at: point)

        var writer = Halo2Blake2bWriteTranscript()
        var rng = SeededGenerator(state: 0x5555_6666_7777_8888)
        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &writer,
            queries: [
                Halo2IPAProverQuery(point: point, polynomial: polynomial, blind: blind),
            ],
            rng: &rng
        )

        XCTAssertEqual(writer.proofBytes.count, 160)
        var reader = Halo2Blake2bReadTranscript(proof: writer.proofBytes)
        _ = reader.squeezeChallenge()
        _ = reader.squeezeChallenge()
        let qPrimeCommitment = try reader.readPoint()
        let x3 = reader.squeezeChallenge().scalar
        let qAtX3 = try reader.readScalar()
        let x4 = reader.squeezeChallenge().scalar
        let denominatorInv = try XCTUnwrap((x3 - point).inverted())
        let quotientEval = (qAtX3 - value) * denominatorInv
        let pCommitment = qPrimeCommitment.projective.multiplied(by: x4) + commitment
        let pValue = quotientEval * x4 + qAtX3

        XCTAssertTrue(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: pCommitment,
            point: x3,
            value: pValue,
            transcript: &reader
        ))
        XCTAssertEqual(reader.remainingBytes, 0)
    }

    func testZeroRoundSamePointMultiOpeningProofTranscriptShape() throws {
        let params = try Self.zeroRoundIPAParameters()
        let point = PastaFp(19)
        let queries = [
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(13)], blind: PastaFp(17)),
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(23)], blind: PastaFp(29)),
        ]

        var writer = Halo2Blake2bWriteTranscript()
        var rng = SeededGenerator(state: 0x2424_4646_6868_8a8a)
        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &writer,
            queries: queries,
            rng: &rng
        )
        XCTAssertEqual(writer.proofBytes.count, 160)

        var reader = Halo2Blake2bReadTranscript(proof: writer.proofBytes)
        _ = reader.squeezeChallenge()
        _ = reader.squeezeChallenge()
        XCTAssertEqual(reader.remainingBytes, 160)
        _ = try reader.readPoint()
        XCTAssertEqual(reader.remainingBytes, 128)
        _ = reader.squeezeChallenge()
        _ = try reader.readScalar()
        XCTAssertEqual(reader.remainingBytes, 96)
        _ = reader.squeezeChallenge()
        _ = try reader.readPoint()
        XCTAssertEqual(reader.remainingBytes, 64)
        _ = reader.squeezeChallenge()
        _ = reader.squeezeChallenge()
        _ = try reader.readScalar()
        _ = try reader.readScalar()
        XCTAssertEqual(reader.remainingBytes, 0)
    }

    func testZeroRoundSamePointMultiOpeningProofAggregatesMultipleQueries() throws {
        let params = try Self.zeroRoundIPAParameters()
        let point = PastaFp(19)
        let queries = [
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(13)], blind: PastaFp(17)),
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(23)], blind: PastaFp(29)),
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(31)], blind: PastaFp(37)),
        ]
        let commitments = try queries.map { query in
            try params.commit(coefficients: query.polynomial, blind: query.blind)
        }

        var writer = Halo2Blake2bWriteTranscript()
        var rng = SeededGenerator(state: 0x6666_7777_8888_9999)
        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &writer,
            queries: queries,
            rng: &rng
        )

        var reader = Halo2Blake2bReadTranscript(proof: writer.proofBytes)
        let x1 = reader.squeezeChallenge().scalar
        _ = reader.squeezeChallenge()

        var qCommitment = VestaProjective.identity
        var qEval = PastaFp.zero
        var power = PastaFp.one
        for idx in queries.indices.reversed() {
            qCommitment = qCommitment + commitments[idx].multiplied(by: power)
            qEval += Self.evaluate(queries[idx].polynomial, at: point) * power
            power *= x1
        }

        let qPrimeCommitment = try reader.readPoint()
        let x3 = reader.squeezeChallenge().scalar
        let qAtX3 = try reader.readScalar()
        let x4 = reader.squeezeChallenge().scalar
        let denominatorInv = try XCTUnwrap((x3 - point).inverted())
        let quotientEval = (qAtX3 - qEval) * denominatorInv
        let pCommitment = qPrimeCommitment.projective.multiplied(by: x4) + qCommitment
        let pValue = quotientEval * x4 + qAtX3

        XCTAssertTrue(try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: pCommitment,
            point: x3,
            value: pValue,
            transcript: &reader
        ))
        XCTAssertEqual(reader.remainingBytes, 0)
    }

    func testZeroRoundSamePointMultiOpeningProofRejectsTamperedProof() throws {
        let params = try Self.zeroRoundIPAParameters()
        let point = PastaFp(19)
        let queries = [
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(13)], blind: PastaFp(17)),
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(23)], blind: PastaFp(29)),
        ]
        let commitments = try queries.map { query in
            try params.commit(coefficients: query.polynomial, blind: query.blind)
        }

        var writer = Halo2Blake2bWriteTranscript()
        var rng = SeededGenerator(state: 0x1111_7777_2222_8888)
        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &writer,
            queries: queries,
            rng: &rng
        )

        let verified = try Self.verifyZeroRoundSamePointMultiOpeningProof(
            params: params,
            queries: queries,
            commitments: commitments,
            proof: writer.proofBytes
        )
        XCTAssertTrue(verified.ok)
        XCTAssertEqual(verified.remainingBytes, 0)

        var tamperedQAtX3 = writer.proofBytes
        tamperedQAtX3[32] ^= 0x01
        let tampered = try Self.verifyZeroRoundSamePointMultiOpeningProof(
            params: params,
            queries: queries,
            commitments: commitments,
            proof: tamperedQAtX3
        )
        XCTAssertFalse(tampered.ok)

        var proofWithTrailingByte = writer.proofBytes
        proofWithTrailingByte.append(0)
        let trailing = try Self.verifyZeroRoundSamePointMultiOpeningProof(
            params: params,
            queries: queries,
            commitments: commitments,
            proof: proofWithTrailingByte
        )
        XCTAssertTrue(trailing.ok)
        XCTAssertEqual(trailing.remainingBytes, 1)
    }

    func testZeroRoundSamePointMultiOpeningProofRejectsWrongCommitmentSet() throws {
        let params = try Self.zeroRoundIPAParameters()
        let point = PastaFp(19)
        let queries = [
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(13)], blind: PastaFp(17)),
            Halo2IPAProverQuery(point: point, polynomial: [PastaFp(23)], blind: PastaFp(29)),
        ]
        let commitments = try queries.map { query in
            try params.commit(coefficients: query.polynomial, blind: query.blind)
        }

        var writer = Halo2Blake2bWriteTranscript()
        var rng = SeededGenerator(state: 0x1414_3636_5858_7a7a)
        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &writer,
            queries: queries,
            rng: &rng
        )

        var wrongCommitments = commitments
        wrongCommitments[1] = try params.commit(coefficients: [PastaFp(31)], blind: PastaFp(37))
        let wrongCommitmentSet = try Self.verifyZeroRoundSamePointMultiOpeningProof(
            params: params,
            queries: queries,
            commitments: wrongCommitments,
            proof: writer.proofBytes
        )
        XCTAssertFalse(wrongCommitmentSet.ok)
        XCTAssertEqual(wrongCommitmentSet.remainingBytes, 0)
    }

    func testZeroRoundSamePointMultiOpeningProofRejectsInvalidQueries() throws {
        let params = try Self.zeroRoundIPAParameters()

        var emptyTranscript = Halo2Blake2bWriteTranscript()
        var emptyRNG = SeededGenerator(state: 0x7777_8888_9999_aaaa)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &emptyTranscript,
            queries: [],
            rng: &emptyRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertEqual(emptyTranscript.proofBytes.count, 0)

        var invalidFirstTranscript = Halo2Blake2bWriteTranscript()
        var invalidFirstRNG = SeededGenerator(state: 0x8888_9999_aaaa_bbbb)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &invalidFirstTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [], blind: PastaFp(17)),
            ],
            rng: &invalidFirstRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertEqual(invalidFirstTranscript.proofBytes.count, 0)

        var tooLongFirstTranscript = Halo2Blake2bWriteTranscript()
        var tooLongFirstRNG = SeededGenerator(state: 0xbbbb_cccc_dddd_eeee)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &tooLongFirstTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [PastaFp(13), PastaFp(17)], blind: PastaFp(19)),
            ],
            rng: &tooLongFirstRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }
        XCTAssertEqual(tooLongFirstTranscript.proofBytes.count, 0)

        var invalidLaterTranscript = Halo2Blake2bWriteTranscript()
        var invalidLaterRNG = SeededGenerator(state: 0x9999_aaaa_bbbb_cccc)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &invalidLaterTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [PastaFp(13)], blind: PastaFp(17)),
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [], blind: PastaFp(29)),
            ],
            rng: &invalidLaterRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 0))
        }
        XCTAssertEqual(invalidLaterTranscript.proofBytes.count, 0)

        var tooLongLaterTranscript = Halo2Blake2bWriteTranscript()
        var tooLongLaterRNG = SeededGenerator(state: 0xcccc_dddd_eeee_ffff)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &tooLongLaterTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [PastaFp(13)], blind: PastaFp(17)),
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [PastaFp(23), PastaFp(29)], blind: PastaFp(31)),
            ],
            rng: &tooLongLaterRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .invalidPolynomialLength(expected: 1, actual: 2))
        }
        XCTAssertEqual(tooLongLaterTranscript.proofBytes.count, 0)

        var mismatchedPointTranscript = Halo2Blake2bWriteTranscript()
        var mismatchedPointRNG = SeededGenerator(state: 0xaaaa_bbbb_cccc_dddd)
        XCTAssertThrowsError(try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &mismatchedPointTranscript,
            queries: [
                Halo2IPAProverQuery(point: PastaFp(19), polynomial: [PastaFp(13)], blind: PastaFp(17)),
                Halo2IPAProverQuery(point: PastaFp(23), polynomial: [PastaFp(29)], blind: PastaFp(31)),
            ],
            rng: &mismatchedPointRNG
        )) { error in
            XCTAssertEqual(error as? Halo2IPAError, .nonInvertibleChallenge)
        }
        XCTAssertEqual(mismatchedPointTranscript.proofBytes.count, 0)
    }

    func testIPAOpeningProofVerifiesRustHalo2Vector() throws {
        let params = try Halo2IPAParameters.read(from: try Self.hex("""
        0300000045065ed079bf389758f591131095ef419310e8c708a805852b9b77bed8c7ecbde0c0802686d3ed571f7f3399526b24460b16ace461ebda9dcfe6e5b7b298c18cd27962962ce9ed2b87ae4c95462914917a9da0295f593956c1a76adca2ebc394a74f2af0a526eb437ebb22c416a78fda0a275a2f2756115e51248f7c5a7acdaca3c70c4c2497a714ebe96b6192bf940a13a277a6a7c4152a3524988d86be181bc887e3240ff4de618fb8531a447b4f469f50a8ac35fe905da8b8e8732e527eb8b7e796477bd8c4fe4b9ef9806c39cb32ed5558ef9d8c582909779eec700e259a6787540156e67c55fba03e961d98971ccd6bf71c4edd0d80d4d7aec06f9cdb0a1d499756f70cf0deb7b52b1b0def41337e091269c9bcd46334f97a05842a0f0105c58c4a440e77a80e25f3dd162f2b74d6866003b664d1c3ddd6a185c3309314dd77de397f82818f5b36495c402dbb92096a5fc5f8b73210ac79f9b8af3e7ab9cd97784c9657aeefb68194c1546984ae653ba1e4abfc06e887990db451c7cd3227cdb3ef3e31a17568c8953cffba5a911f7c2020cdde94c8b96d35d5eeb7f6021ad59e266061b96ab8968ac8e547138a9f47b9186eddf13ad329d0921707de14fe16f4009d2fa0c76524113ba0796a5f512d5bf2acdd92721d841139fd5b242d3904ea829062811fcf825218a681c7881caa753008c9a8aecd1f8cc600b410907520d96f3e5cd41760367151608b54821883c10c4b9a4ff2beae227bef94bcab379dc4dcfdbf61ccc7d5a0bb9759acf611694f24d0c040f249bad30a83b1a897
        """))
        let commitment = try XCTUnwrap(VestaAffine.fromCompressedBytes(try Self.hex(
            "32dd3d6bbd12b8c91e6adef35ea9167cf421235a00d545b997b490adcf0ba899"
        )))
        let point = try XCTUnwrap(PastaFp.fromCanonicalBytes(try Self.hex(
            "1300000000000000000000000000000000000000000000000000000000000000"
        )))
        let value = try XCTUnwrap(PastaFp.fromCanonicalBytes(try Self.hex(
            "6418997405000000000000000000000000000000000000000000000000000000"
        )))
        let proof = try Self.hex("""
        cdcd08013f80738db8fff07f03cdeddd667afefc7fe8e56e45d549070d4b49ac7747197e31080209ef8cd47196430d885686f359fc132c18910d44a4c8811cacbbfa69e2de537cf1a5d432fa806e9fd03b16f950d8e9c70969e73e25b95398abc6f39d2a2db7d0c16e27a1bf21d24dd6d2047d541cf9306904bdaa659c190a3cb1b4312141fbf14902a740907fe43fab43cfe02c6d0810910416101571a8bea7c187b09fd8541d6b5e8316dd754086e33c0b2a2a3fb9c8174b0fe5e74f072e24a208f5af0153c56b6acfd1152fecb1cfdfd309c1094703bab93fc053c9ee508624d445bbde5b74318ee02b67ee43077e1a3d2019ec8f66198db83f7da7b8e50942b81a693ef4a0be087d6953ff5b6484ac9542489bfbe84f4e8fafa151b05917
        """)

        XCTAssertTrue(try Halo2IPAOpeningProof.verify(
            params: params,
            commitment: commitment,
            point: point,
            value: value,
            proof: proof
        ))
    }

    func testEvaluationDomainFftMatchesDirectEvaluation() throws {
        let domain = try Halo2EvaluationDomain(k: 3)
        let coefficients = (0..<domain.n).map { PastaFp(UInt64($0 + 1)) }
        let evaluations = try domain.coeffToLagrange(coefficients)

        var point = PastaFp.one
        for idx in 0..<domain.n {
            XCTAssertEqual(evaluations[idx], Self.evaluate(coefficients, at: point))
            point *= domain.omega
        }

        XCTAssertEqual(try domain.lagrangeToCoeff(evaluations), coefficients)
        XCTAssertEqual(domain.vanishingPolynomialEvaluation(at: domain.omega), .zero)
        XCTAssertEqual(domain.evaluateLagrangeBasisZero(at: .one), .one)
        XCTAssertEqual(domain.evaluateLagrangeBasisZero(at: domain.omega), .zero)
    }

    func testEvaluationDomainsRejectInvalidParametersAndLengths() throws {
        XCTAssertThrowsError(try Halo2EvaluationDomain(k: 33)) { error in
            XCTAssertEqual(error as? Halo2EvaluationDomainError, .invalidK(33))
        }

        let domain = try Halo2EvaluationDomain(k: 2)
        XCTAssertThrowsError(try domain.coeffToLagrange([PastaFp.one])) { error in
            XCTAssertEqual(error as? Halo2EvaluationDomainError, .invalidValueCount(expected: 4, actual: 1))
        }
        XCTAssertThrowsError(try domain.lagrangeToCoeff([PastaFp.one, PastaFp(2)])) { error in
            XCTAssertEqual(error as? Halo2EvaluationDomainError, .invalidValueCount(expected: 4, actual: 2))
        }

        XCTAssertThrowsError(try Halo2ExtendedEvaluationDomain(degree: 1, k: 2)) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidDegree(1))
        }
        XCTAssertThrowsError(try Halo2ExtendedEvaluationDomain(degree: 2, k: 33)) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidK(33))
        }

        let extended = try Halo2ExtendedEvaluationDomain(degree: 6, k: 3)
        XCTAssertEqual(extended.rotateOmega(PastaFp(7), by: 1), PastaFp(7) * extended.omega)
        XCTAssertEqual(extended.rotateOmega(PastaFp(7), by: -1), PastaFp(7) * extended.omegaInv)
        XCTAssertThrowsError(try extended.coeffToExtendedPart([PastaFp.one], factor: .one)) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 8, actual: 1))
        }
        XCTAssertThrowsError(try extended.extendedFromParts([[PastaFp.one]])) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 8, actual: 1))
        }
        XCTAssertThrowsError(try extended.extendedFromParts(Array(repeating: [PastaFp.one], count: 4))) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 8, actual: 4))
        }
        XCTAssertThrowsError(try extended.extendedFromParts(Array(repeating: [PastaFp.one], count: 8))) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 8, actual: 1))
        }
        XCTAssertThrowsError(try extended.divideByVanishingPolynomial([PastaFp.one])) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 64, actual: 1))
        }
        XCTAssertThrowsError(try extended.extendedToCoeff([PastaFp.one])) { error in
            XCTAssertEqual(error as? Halo2ExtendedEvaluationDomainError, .invalidValueCount(expected: 64, actual: 1))
        }
    }

    func testExtendedEvaluationDomainPartsRoundTripCoefficients() throws {
        let domain = try Halo2ExtendedEvaluationDomain(degree: 6, k: 3)
        let coefficients = (0..<domain.n).map { PastaFp(UInt64($0 * 7 + 3)) }
        let lagrange = try domain.coeffToLagrange(coefficients)
        var point = PastaFp.one
        for idx in 0..<domain.n {
            XCTAssertEqual(lagrange[idx], Self.evaluate(coefficients, at: point))
            point *= domain.omega
        }
        XCTAssertEqual(try domain.lagrangeToCoeff(lagrange), coefficients)

        var factor = PastaFp.one
        var parts: [[PastaFp]] = []
        for _ in 0..<(domain.extendedN / domain.n) {
            parts.append(try domain.coeffToExtendedPart(coefficients, factor: factor))
            factor *= domain.extendedOmega
        }

        let extended = try domain.extendedFromParts(parts)
        let roundTrip = try domain.extendedToCoeff(extended)
        XCTAssertEqual(Array(roundTrip.prefix(domain.n)), coefficients)
        XCTAssertEqual(Array(roundTrip.dropFirst(domain.n)), [PastaFp](repeating: .zero, count: domain.extendedN - domain.n))
    }

    func testExtendedEvaluationDomainDividesVanishingPolynomial() throws {
        let domain = try Halo2ExtendedEvaluationDomain(degree: 6, k: 3)
        let quotientLength = domain.n * domain.quotientPolynomialDegree
        let quotient = (0..<quotientLength).map { PastaFp(UInt64($0 * 5 + 11)) }
        var numerator = [PastaFp](repeating: .zero, count: domain.extendedN)
        for idx in quotient.indices {
            numerator[idx] -= quotient[idx]
            numerator[idx + domain.n] += quotient[idx]
        }

        var point = PastaFp.zeta
        var numeratorExtended: [PastaFp] = []
        for _ in 0..<domain.extendedN {
            numeratorExtended.append(Self.evaluate(numerator, at: point))
            point *= domain.extendedOmega
        }

        let quotientExtended = try domain.divideByVanishingPolynomial(numeratorExtended)
        let roundTrip = try domain.extendedToCoeff(quotientExtended)
        XCTAssertEqual(Array(roundTrip.prefix(quotientLength)), quotient)
        XCTAssertEqual(
            Array(roundTrip.dropFirst(quotientLength)),
            [PastaFp](repeating: .zero, count: domain.extendedN - quotientLength)
        )
    }

    func testFieldReprRejectsModulusAndWrapsAtModulusMinusOne() throws {
        let modulus = Self.bytes(fromLimbs: PastaFpParameters.modulus)
        XCTAssertNil(PastaFp.fromCanonicalBytes(Data(repeating: 0, count: 31)))
        XCTAssertNil(PastaFp.fromUniformBytes64(Data(repeating: 0, count: 63)))
        XCTAssertNil(PastaFp.fromCanonicalBytes(modulus))
        XCTAssertNil(PastaFp.zero.inverted())
        XCTAssertNil(PastaFp.sqrtRatio(numerator: .one, denominator: .zero))

        var pMinusOne = PastaFpParameters.modulus
        pMinusOne[0] -= 1
        let maxFp = try XCTUnwrap(PastaFp.fromCanonicalBytes(Self.bytes(fromLimbs: pMinusOne)))
        XCTAssertEqual(maxFp + PastaFp.one, PastaFp.zero)

        let squareRatio = try XCTUnwrap(PastaFp.sqrtRatio(numerator: PastaFp(25), denominator: .one))
        XCTAssertTrue(squareRatio.isSquare)
        XCTAssertEqual(squareRatio.root.squared(), PastaFp(25))

        let nonSquareRatio = try XCTUnwrap(PastaFp.sqrtRatio(numerator: PastaFp(5), denominator: .one))
        XCTAssertFalse(nonSquareRatio.isSquare)
        XCTAssertEqual(nonSquareRatio.root.squared(), PastaFp(5) * PastaFp.rootOfUnity)
    }

    func testVestaRejectsInvalidEncodingsAndHandlesIdentityBranches() throws {
        XCTAssertNil(VestaAffine.fromCompressedBytes(Data(repeating: 0, count: 31)))
        XCTAssertNil(VestaAffine.fromCompressedBytes(Self.bytes(fromLimbs: PastaFqParameters.modulus)))
        XCTAssertNil(VestaAffine(x: .zero, y: .zero))

        XCTAssertEqual(VestaAffine.identity.compressedBytes(), Data(repeating: 0, count: 32))
        XCTAssertEqual(VestaAffine.identity.negated(), .identity)
        XCTAssertEqual(VestaProjective.identity.toAffine(), .identity)
        XCTAssertEqual(VestaProjective.identity.negated(), .identity)
        XCTAssertEqual(VestaAffine.generator.projective.mixedAdded(.identity).toAffine(), .generator)
        XCTAssertEqual(VestaProjective.identity.mixedAdded(.generator).toAffine(), .generator)
        XCTAssertEqual((VestaAffine.generator.projective + VestaProjective.identity).toAffine(), .generator)
        XCTAssertEqual((VestaProjective.identity + VestaAffine.generator.projective).toAffine(), .generator)
    }

    func testOfflineNoteV2InstanceValuesMatchIrohaFixture() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        let auditValues = try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: audit)
        XCTAssertEqual(auditValues.publicValues, [
            10_903_869_479_213_959_155,
            964_545_862_331_868_624,
            62_073_266_647_774_258,
            10_177_727_713_799_530_712,
            2,
            1,
            2,
            52,
            52,
            7_774_543_532_909_120_318,
            13_773_009_191_653_973_469,
            9_169_562_048_745_222_370,
            16_854_595_775_279_027_198,
            13_453_331_281_027_170_942,
            323_649_634_537_089_673,
            0,
        ])
        XCTAssertEqual(auditValues.inputAmounts, [52, 0, 0, 0])
        XCTAssertEqual(auditValues.outputAmounts, [5, 47])
        XCTAssertEqual(auditValues.publicInstanceColumns()[0], [OfflineNoteV2InstanceValues.instanceScalarBytes(
            10_903_869_479_213_959_155
        )])

        let redeemValues = try OfflineNoteV2InstanceBuilder.redeemInstanceValues(for: redeem)
        XCTAssertEqual(redeemValues.publicValues, [
            10_581_903_064_317_986_161,
            14_339_039_858_142_495_484,
            15_374_235_055_253_948_207,
            4_155_438_619_149_796_958,
            1,
            1,
            1,
            5,
            5,
            12_627_440_414_421_573_188,
            0,
            13_363_141_946_654_618_346,
            9_816_966_047_697_816_686,
            298_900_535_230_543_113,
            0,
            0,
        ])
        XCTAssertEqual(redeemValues.inputAmounts, [5, 0, 0, 0])
        XCTAssertEqual(redeemValues.outputAmounts, [5, 0])
    }

    func testOfflineNoteV2NativeHalo2ProofEnvelopeFitsQrBudget() throws {
        let fixture = try Self.loadFixture()
        try Halo2OfflineNoteV2Prover.prewarm()
        let audit = try Self.audit(fixture)
        let auditValues = try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: audit)
        let zk1Payload = try Halo2OfflineNoteV2Prover.proveZK1Payload(instanceValues: auditValues)
        XCTAssertTrue(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            zk1Payload,
            publicValues: auditValues.publicValues
        ))
        var mismatchedPublicValues = auditValues.publicValues
        mismatchedPublicValues[15] ^= 1
        XCTAssertFalse(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            zk1Payload,
            publicValues: mismatchedPublicValues
        ))

        var tamperedPayload = zk1Payload
        tamperedPayload[12] ^= 0x01
        XCTAssertFalse((try? Halo2OfflineNoteV2Prover.verifyZK1Payload(
            tamperedPayload,
            publicValues: auditValues.publicValues
        )) == true)

        let proof = try Halo2OfflineNoteV2Prover.proveAudit(audit)
        if let proofOut = ProcessInfo.processInfo.environment["IROHA_SWIFT_OFFLINE_V2_PROOF_OUT"] {
            try proof.proof.bytes.write(to: URL(fileURLWithPath: proofOut))
        }
        if let externalPayload = ProcessInfo.processInfo.environment["IROHA_SWIFT_OFFLINE_V2_VERIFY_PAYLOAD_IN"] {
            let payload = try Data(contentsOf: URL(fileURLWithPath: externalPayload))
            XCTAssertTrue(try Halo2OfflineNoteV2Prover.verifyZK1Payload(payload, publicValues: auditValues.publicValues))
        }
        XCTAssertLessThan(proof.proof.bytes.count, Halo2OfflineNoteV2Prover.maxEnvelopeBytes)
        XCTAssertEqual(proof.publicInputsHash, Data(hexString: fixture.chainVectors.audit.publicInputsHash))
        try audit.replacingRecursiveProof(proof).validateProofBinding()
    }

    func testOfflineNoteV2NativeHalo2ProofPerformanceWhenRequested() throws {
        let env = ProcessInfo.processInfo.environment
        guard env["IROHA_SWIFT_OFFLINE_V2_BENCH"] == "1" else {
            throw XCTSkip("set IROHA_SWIFT_OFFLINE_V2_BENCH=1 to run the Offline V2 proof benchmark")
        }
        let iterations = env["IROHA_SWIFT_OFFLINE_V2_BENCH_ITERATIONS"].flatMap(Int.init) ?? 20
        XCTAssertGreaterThan(iterations, 0)
        let medianBudgetSeconds = benchmarkBudgetSeconds(
            env: env,
            key: "IROHA_SWIFT_OFFLINE_V2_BENCH_MEDIAN_BUDGET_MS",
            defaultMilliseconds: 850
        )
        let p95BudgetSeconds = benchmarkBudgetSeconds(
            env: env,
            key: "IROHA_SWIFT_OFFLINE_V2_BENCH_P95_BUDGET_MS",
            defaultMilliseconds: 1_200
        )

        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        try Halo2OfflineNoteV2Prover.prewarm()
        _ = try Halo2OfflineNoteV2Prover.proveAudit(audit)
        _ = try Halo2OfflineNoteV2Prover.proveRedeem(redeem)

        let auditSeconds = try benchmarkSeconds(iterations: iterations) {
            _ = try Halo2OfflineNoteV2Prover.proveAudit(audit)
        }
        let redeemSeconds = try benchmarkSeconds(iterations: iterations) {
            _ = try Halo2OfflineNoteV2Prover.proveRedeem(redeem)
        }
        let auditMetrics = benchmarkMetrics(auditSeconds)
        let redeemMetrics = benchmarkMetrics(redeemSeconds)
        print("offline_note_v2_swift_bench audit=\(auditMetrics.summary) redeem=\(redeemMetrics.summary)")
        XCTAssertLessThanOrEqual(
            auditMetrics.median,
            medianBudgetSeconds,
            "audit median exceeded \(medianBudgetSeconds)s budget: \(auditMetrics.summary)"
        )
        XCTAssertLessThanOrEqual(
            auditMetrics.p95,
            p95BudgetSeconds,
            "audit p95 exceeded \(p95BudgetSeconds)s budget: \(auditMetrics.summary)"
        )
        XCTAssertLessThanOrEqual(
            redeemMetrics.median,
            medianBudgetSeconds,
            "redeem median exceeded \(medianBudgetSeconds)s budget: \(redeemMetrics.summary)"
        )
        XCTAssertLessThanOrEqual(
            redeemMetrics.p95,
            p95BudgetSeconds,
            "redeem p95 exceeded \(p95BudgetSeconds)s budget: \(redeemMetrics.summary)"
        )
    }

    func testOfflineNoteV2ProofPayloadRejectsMalformedInputs() throws {
        let publicValues = [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.publicValueCount)
        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Data([0x5A, 0x4B, 0x31]),
            publicValues: publicValues
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidZK1Payload)
        }

        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Data([0x5A, 0x4B, 0x31, 0x00]),
            publicValues: Array(publicValues.dropLast())
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidInstanceValues)
        }
    }

    func testOfflineNoteV2ProofPayloadRejectsMalformedTLVs() throws {
        let publicValues = [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.publicValueCount)
        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Self.zk1Payload(proof: nil, publicValues: publicValues),
            publicValues: publicValues
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidZK1Payload)
        }
        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Self.zk1Payload(proof: Data(), publicValues: publicValues),
            publicValues: publicValues
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidZK1Payload)
        }
        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Self.zk1Payload(proof: Data([1]), publicValues: publicValues, rows: 2),
            publicValues: publicValues
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidZK1Payload)
        }
        XCTAssertThrowsError(try Halo2OfflineNoteV2Prover.verifyZK1Payload(
            Self.zk1Payload(proof: Data([1]), publicValues: publicValues) { instances in
                instances[16] = 1
            },
            publicValues: publicValues
        )) { error in
            XCTAssertEqual(error as? Halo2OfflineNoteV2ProverError, .invalidZK1Payload)
        }
    }

    func testOfflineNoteV2InstanceValuesRejectInvalidWitnessShapes() throws {
        XCTAssertThrowsError(try OfflineNoteV2InstanceValues(
            publicValues: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.publicValueCount - 1),
            inputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxInputAmounts),
            outputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxOutputAmounts)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .invalidPublicValueCount(15))
        }
        XCTAssertThrowsError(try OfflineNoteV2InstanceValues(
            publicValues: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.publicValueCount),
            inputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxInputAmounts - 1),
            outputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxOutputAmounts)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .invalidInputAmountCount(3))
        }
        XCTAssertThrowsError(try OfflineNoteV2InstanceValues(
            publicValues: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.publicValueCount),
            inputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxInputAmounts),
            outputAmounts: [UInt64](repeating: 0, count: OfflineNoteV2InstanceValues.maxOutputAmounts - 1)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .invalidOutputAmountCount(1))
        }
    }

    func testOfflineNoteV2InstanceBuilderRejectsCountAndAmountViolations() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let repeatedInputClaims = [OfflineNoteIssuedClaimV2](repeating: audit.inputClaims[0], count: 5)
        let repeatedInputNullifiers = [Data](repeating: audit.inputNullifiers[0], count: 5)
        let tooManyInputs = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: repeatedInputNullifiers,
            inputClaims: repeatedInputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: tooManyInputs)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .invalidCount(label: "audit input", count: 5, max: 4))
        }

        let tooManyOutputs = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: [Data](repeating: audit.outputCommitments[0], count: 3),
            outputClaims: [OfflineNoteAuditOutputClaimV2](repeating: audit.outputClaims[0], count: 3),
            recursiveProof: audit.recursiveProof
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: tooManyOutputs)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .invalidCount(label: "audit output", count: 3, max: 2))
        }

        var mismatchedOutputClaims = audit.outputClaims
        mismatchedOutputClaims[0] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: mismatchedOutputClaims[0].noteCommitment,
            keyCertificate: mismatchedOutputClaims[0].keyCertificate,
            assetId: mismatchedOutputClaims[0].assetId,
            amount: "6"
        )
        let mismatchedAmounts = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: mismatchedOutputClaims,
            recursiveProof: audit.recursiveProof
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: mismatchedAmounts)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .amountConservationMismatch(input: 52, output: 53))
        }

        let maxClaim = try OfflineNoteIssuedClaimV2(
            domain: audit.inputClaims[0].domain,
            noteCommitment: audit.inputClaims[0].noteCommitment,
            keyCertificatePayloadHash: audit.inputClaims[0].keyCertificatePayloadHash,
            assetId: audit.inputClaims[0].assetId,
            amount: String(UInt64.max)
        )
        let oneClaim = try OfflineNoteIssuedClaimV2(
            domain: audit.inputClaims[0].domain,
            noteCommitment: audit.inputClaims[0].noteCommitment,
            keyCertificatePayloadHash: audit.inputClaims[0].keyCertificatePayloadHash,
            assetId: audit.inputClaims[0].assetId,
            amount: "1"
        )
        let overflowingInputs = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: [audit.inputNullifiers[0], audit.inputNullifiers[0]],
            inputClaims: [maxClaim, oneClaim],
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: overflowingInputs)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .amountSumOverflow("input"))
        }
    }

    func testOfflineNoteV2InstanceBuilderNormalizesDecimalWitnessAmounts() throws {
        let audit = try Self.audit(Self.loadFixture())
        let decimalAudit = try Self.audit(
            audit,
            replacingInputAmounts: ["1.20"],
            outputAmounts: ["0.70", "0.5"]
        )

        let values = try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: decimalAudit)

        XCTAssertEqual(values.inputAmounts, [12, 0, 0, 0])
        XCTAssertEqual(values.outputAmounts, [7, 5])
        XCTAssertEqual(values.publicValues[7], 12)
        XCTAssertEqual(values.publicValues[8], 12)
    }

    func testOfflineNoteV2InstanceBuilderRejectsNegativeOversizedAndOutputOverflowAmounts() throws {
        let audit = try Self.audit(Self.loadFixture())

        let negative = try Self.audit(
            audit,
            replacingInputAmounts: ["1"],
            outputAmounts: ["-1", "2"]
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: negative)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .negativeAmount("-1"))
        }

        let oversized = try Self.audit(
            audit,
            replacingInputAmounts: ["18446744073709551616"],
            outputAmounts: ["0", "0"]
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: oversized)) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2InstanceError,
                .amountDoesNotFitUInt64("18446744073709551616")
            )
        }

        let overflowingOutput = try Self.audit(
            audit,
            replacingInputAmounts: ["0"],
            outputAmounts: [String(UInt64.max), "1"]
        )
        XCTAssertThrowsError(try OfflineNoteV2InstanceBuilder.auditInstanceValues(for: overflowingOutput)) { error in
            XCTAssertEqual(error as? OfflineNoteV2InstanceError, .amountSumOverflow("output"))
        }
    }

    private func benchmarkSeconds(iterations: Int, body: () throws -> Void) rethrows -> [Double] {
        var durations: [Double] = []
        durations.reserveCapacity(iterations)
        for _ in 0..<iterations {
            let start = Date()
            try body()
            durations.append(Date().timeIntervalSince(start))
        }
        return durations
    }

    private func benchmarkBudgetSeconds(
        env: [String: String],
        key: String,
        defaultMilliseconds: Double
    ) -> Double {
        (env[key].flatMap(Double.init) ?? defaultMilliseconds) / 1_000
    }

    private func benchmarkMetrics(_ values: [Double]) -> BenchmarkMetrics {
        let sorted = values.sorted()
        guard let maxValue = sorted.last else {
            return BenchmarkMetrics(median: 0, p95: 0, max: 0, count: 0)
        }
        let median: Double
        if sorted.count % 2 == 0 {
            median = (sorted[sorted.count / 2 - 1] + sorted[sorted.count / 2]) / 2
        } else {
            median = sorted[sorted.count / 2]
        }
        let p95Index = min(sorted.count - 1, max(0, Int(ceil(Double(sorted.count) * 0.95)) - 1))
        return BenchmarkMetrics(
            median: median,
            p95: sorted[p95Index],
            max: maxValue,
            count: sorted.count
        )
    }

    private struct BenchmarkMetrics {
        let median: Double
        let p95: Double
        let max: Double
        let count: Int

        var summary: String {
            guard count > 0 else {
                return "empty"
            }
            return String(
                format: "median=%.3fs p95=%.3fs max=%.3fs n=%d",
                median,
                p95,
                max,
                count
            )
        }
    }

    private static func scalarBytes(_ value: UInt64) -> Data {
        var bytes = Data(count: 32)
        var value = value
        for idx in 0..<8 {
            bytes[idx] = UInt8(value & 0xff)
            value >>= 8
        }
        return bytes
    }

    private static func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        var littleEndian = value.littleEndian
        data.append(contentsOf: withUnsafeBytes(of: &littleEndian, Array.init))
    }

    private static func bytes(fromLimbs limbs: [UInt64]) -> Data {
        var bytes = Data(capacity: 32)
        for limb in limbs {
            var limb = limb
            for _ in 0..<8 {
                bytes.append(UInt8(limb & 0xff))
                limb >>= 8
            }
        }
        return bytes
    }

    private static func evaluate(_ coefficients: [PastaFp], at point: PastaFp) -> PastaFp {
        coefficients.reversed().reduce(PastaFp.zero) { accumulator, coefficient in
            accumulator * point + coefficient
        }
    }

    private static func zeroRoundIPAParameters() throws -> Halo2IPAParameters {
        let generator = VestaAffine.generator
        return try Halo2IPAParameters(
            k: 0,
            g: [generator.multiplied(by: PastaFp(3))],
            gLagrange: [generator.multiplied(by: PastaFp(5))],
            w: generator.multiplied(by: PastaFp(7)),
            u: generator.multiplied(by: PastaFp(11))
        )
    }

    private static func verifyZeroRoundSamePointMultiOpeningProof(
        params: Halo2IPAParameters,
        queries: [Halo2IPAProverQuery],
        commitments: [VestaProjective],
        proof: Data
    ) throws -> (ok: Bool, remainingBytes: Int) {
        precondition(params.k == 0)
        precondition(queries.count == commitments.count)
        precondition(!queries.isEmpty)

        var reader = Halo2Blake2bReadTranscript(proof: proof)
        let x1 = reader.squeezeChallenge().scalar
        _ = reader.squeezeChallenge()

        var qCommitment = VestaProjective.identity
        var qEval = PastaFp.zero
        var power = PastaFp.one
        for idx in queries.indices.reversed() {
            qCommitment = qCommitment + commitments[idx].multiplied(by: power)
            qEval += Self.evaluate(queries[idx].polynomial, at: queries[idx].point) * power
            power *= x1
        }

        let qPrimeCommitment = try reader.readPoint()
        let x3 = reader.squeezeChallenge().scalar
        let qAtX3 = try reader.readScalar()
        let x4 = reader.squeezeChallenge().scalar
        let denominatorInv = try XCTUnwrap((x3 - queries[0].point).inverted())
        let quotientEval = (qAtX3 - qEval) * denominatorInv
        let pCommitment = qPrimeCommitment.projective.multiplied(by: x4) + qCommitment
        let pValue = quotientEval * x4 + qAtX3
        let ok = try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: pCommitment,
            point: x3,
            value: pValue,
            transcript: &reader
        )
        return (ok, reader.remainingBytes)
    }

    private static func zk1Payload(
        proof: Data?,
        publicValues: [UInt64],
        columns: UInt32 = 16,
        rows: UInt32 = 1,
        mutateInstances: ((inout Data) -> Void)? = nil
    ) -> Data {
        var payload = Data([0x5A, 0x4B, 0x31, 0x00])
        if let proof {
            appendTLV(tag: "PROF", value: proof, to: &payload)
        }
        var instances = Data()
        appendUInt32LE(columns, to: &instances)
        appendUInt32LE(rows, to: &instances)
        for value in publicValues {
            instances.append(OfflineNoteV2InstanceValues.instanceScalarBytes(value))
        }
        mutateInstances?(&instances)
        appendTLV(tag: "I10P", value: instances, to: &payload)
        return payload
    }

    private static func appendTLV(tag: String, value: Data, to data: inout Data) {
        data.append(Data(tag.utf8))
        appendUInt32LE(UInt32(value.count), to: &data)
        data.append(value)
    }

    private struct SeededGenerator: RandomNumberGenerator {
        var state: UInt64

        mutating func next() -> UInt64 {
            state = state &* 6364136223846793005 &+ 1442695040888963407
            var z = state
            z = (z ^ (z >> 30)) &* 0xbf58476d1ce4e5b9
            z = (z ^ (z >> 27)) &* 0x94d049bb133111eb
            return z ^ (z >> 31)
        }
    }

    private static func redeem(_ fixture: Halo2OfflineInteropFixture) throws -> OfflineNoteRedeemV2 {
        let vector = fixture.chainVectors.redeem
        return try OfflineNoteRedeemV2(
            sourceNoteCommitment: hex(vector.sourceNoteCommitment),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            senderKeyCertificate: certificate(fixture.paymentToken.recipientKeyCertificate),
            recipient: fixture.paymentToken.recipientAccountId,
            assetId: vector.assetId,
            amount: vector.amount,
            recursiveProof: OfflineNoteRecursiveProofV2(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
            )
        )
    }

    private static func audit(_ fixture: Halo2OfflineInteropFixture) throws -> OfflineNoteAuditBundleV2 {
        let vector = fixture.chainVectors.audit
        return try OfflineNoteAuditBundleV2(
            tokenId: hex(vector.tokenId),
            senderKeyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            inputClaims: try fixture.paymentToken.inputClaims.map(issuedClaim),
            outputCommitments: try vector.outputCommitments.map(hex),
            outputClaims: try fixture.paymentToken.outputClaims.map(auditOutputClaim),
            recursiveProof: OfflineNoteRecursiveProofV2(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-audit-proof".utf8)
            )
        )
    }

    private static func audit(
        _ audit: OfflineNoteAuditBundleV2,
        replacingInputAmounts inputAmounts: [String],
        outputAmounts: [String]
    ) throws -> OfflineNoteAuditBundleV2 {
        XCTAssertEqual(inputAmounts.count, audit.inputClaims.count)
        XCTAssertEqual(outputAmounts.count, audit.outputClaims.count)
        let inputClaims = try zip(audit.inputClaims, inputAmounts).map { claim, amount in
            try OfflineNoteIssuedClaimV2(
                domain: claim.domain,
                noteCommitment: claim.noteCommitment,
                keyCertificatePayloadHash: claim.keyCertificatePayloadHash,
                assetId: claim.assetId,
                amount: amount
            )
        }
        let outputClaims = try zip(audit.outputClaims, outputAmounts).map { claim, amount in
            try OfflineNoteAuditOutputClaimV2(
                noteCommitment: claim.noteCommitment,
                keyCertificate: claim.keyCertificate,
                assetId: claim.assetId,
                amount: amount
            )
        }
        return try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: audit.recursiveProof
        )
    }

    private static func certificate(_ json: Halo2OfflineCertificateJSON) throws -> OfflineNoteKeyCertificateV2 {
        try OfflineNoteKeyCertificateV2(
            version: json.version,
            platform: json.platform,
            keyId: json.keyId,
            deviceId: json.deviceId,
            accountId: json.accountId,
            publicKey: base64(json.publicKey),
            assertionScheme: json.assertionScheme,
            assertionKeyAlgorithm: json.assertionKeyAlgorithm,
            assertionPublicKey: base64(json.assertionPublicKey),
            assertionUsageCountLimit: json.assertionUsageCountLimit,
            oneUse: json.oneUse,
            issuerSignature: base64(json.issuerSignatureBase64)
        )
    }

    private static func issuedClaim(_ json: Halo2OfflineInputClaimJSON) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            domain: json.domain,
            noteCommitment: hex(json.noteCommitment),
            keyCertificatePayloadHash: hex(json.keyCertificatePayloadHash),
            assetId: json.assetId,
            amount: json.amount
        )
    }

    private static func auditOutputClaim(_ json: Halo2OfflineOutputClaimJSON) throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteAuditOutputClaimV2(
            noteCommitment: hex(json.noteCommitment),
            keyCertificate: certificate(json.keyCertificate),
            assetId: "\(json.assetDefinitionId)#\(json.accountId)",
            amount: json.amount
        )
    }

    private static func loadFixture() throws -> Halo2OfflineInteropFixture {
        let testFile = URL(fileURLWithPath: #filePath)
        let fixtureURL = testFile
            .deletingLastPathComponent()
            .appendingPathComponent("../../../fixtures/offline/interop_contract_v2.json")
            .standardizedFileURL
        let data = try Data(contentsOf: fixtureURL)
        return try JSONDecoder().decode(Halo2OfflineInteropFixture.self, from: data)
    }

    private static func hex(_ value: String) throws -> Data {
        let compact = value.filter { !$0.isWhitespace }
        guard let data = Data(hexString: compact) else {
            throw Halo2PastaFixtureError.invalidHex(value)
        }
        return data
    }

    private static func base64(_ value: String) throws -> Data {
        guard let data = Data(base64Encoded: value) else {
            throw Halo2PastaFixtureError.invalidBase64
        }
        return data
    }
}

private enum Halo2PastaFixtureError: Error {
    case invalidHex(String)
    case invalidBase64
}

private struct Halo2OfflineInteropFixture: Decodable {
    let chainVectors: Halo2OfflineChainVectors
    let paymentToken: Halo2OfflinePaymentTokenJSON

    private enum CodingKeys: String, CodingKey {
        case chainVectors = "chain_vectors"
        case paymentToken = "payment_token"
    }
}

private struct Halo2OfflineChainVectors: Decodable {
    let audit: Halo2OfflineAuditVector
    let redeem: Halo2OfflineRedeemVector
}

private struct Halo2OfflineAuditVector: Decodable {
    let tokenId: String
    let inputNullifiers: [String]
    let outputCommitments: [String]
    let publicInputsHash: String

    private enum CodingKeys: String, CodingKey {
        case tokenId = "token_id"
        case inputNullifiers = "input_nullifiers"
        case outputCommitments = "output_commitments"
        case publicInputsHash = "public_inputs_hash"
    }
}

private struct Halo2OfflineRedeemVector: Decodable {
    let sourceNoteCommitment: String
    let inputNullifiers: [String]
    let assetId: String
    let amount: String
    let publicInputsHash: String

    private enum CodingKeys: String, CodingKey {
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifiers = "input_nullifiers"
        case assetId = "asset_id"
        case amount
        case publicInputsHash = "public_inputs_hash"
    }
}

private struct Halo2OfflinePaymentTokenJSON: Decodable {
    let senderAccountId: String
    let recipientAccountId: String
    let senderKeyCertificate: Halo2OfflineCertificateJSON
    let recipientKeyCertificate: Halo2OfflineCertificateJSON
    let inputClaims: [Halo2OfflineInputClaimJSON]
    let outputClaims: [Halo2OfflineOutputClaimJSON]

    private enum CodingKeys: String, CodingKey {
        case senderAccountId = "sender_account_id"
        case recipientAccountId = "recipient_account_id"
        case senderKeyCertificate = "sender_key_certificate"
        case recipientKeyCertificate = "recipient_key_certificate"
        case inputClaims = "input_claims"
        case outputClaims = "output_claims"
    }
}

private struct Halo2OfflineCertificateJSON: Decodable {
    let version: UInt16
    let platform: String
    let keyId: String
    let deviceId: String
    let accountId: String
    let publicKey: String
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionPublicKey: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let issuerSignatureBase64: String

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case keyId = "key_id"
        case deviceId = "device_id"
        case accountId = "account_id"
        case publicKey = "public_key"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case assertionPublicKey = "assertion_public_key"
        case assertionUsageCountLimit = "assertion_usage_count_limit"
        case oneUse = "one_use"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

private struct Halo2OfflineInputClaimJSON: Decodable {
    let domain: String
    let noteCommitment: String
    let keyCertificatePayloadHash: String
    let assetId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case domain
        case noteCommitment = "note_commitment"
        case keyCertificatePayloadHash = "key_certificate_payload_hash"
        case assetId = "asset_id"
        case amount
    }
}

private struct Halo2OfflineOutputClaimJSON: Decodable {
    let noteCommitment: String
    let keyCertificate: Halo2OfflineCertificateJSON
    let accountId: String
    let assetDefinitionId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case noteCommitment = "note_commitment"
        case keyCertificate = "key_certificate"
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
    }
}
