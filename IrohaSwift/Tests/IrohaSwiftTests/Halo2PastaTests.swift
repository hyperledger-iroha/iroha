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

    private static func scalarBytes(_ value: UInt64) -> Data {
        var bytes = Data(count: 32)
        var value = value
        for index in 0..<8 {
            bytes[index] = UInt8(value & 0xff)
            value >>= 8
        }
        return bytes
    }

    private static func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        var littleEndian = value.littleEndian
        data.append(contentsOf: withUnsafeBytes(of: &littleEndian, Array.init))
    }

    private static func bytes(fromLimbs limbs: [UInt64]) -> Data {
        var bytes = Data(capacity: limbs.count * MemoryLayout<UInt64>.size)
        for limb in limbs {
            var remaining = limb
            for _ in 0..<MemoryLayout<UInt64>.size {
                bytes.append(UInt8(remaining & 0xff))
                remaining >>= 8
            }
        }
        return bytes
    }

    private static func evaluate(
        _ coefficients: [PastaFp],
        at point: PastaFp
    ) -> PastaFp {
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

        var aggregateCommitment = VestaProjective.identity
        var aggregateEvaluation = PastaFp.zero
        var power = PastaFp.one
        for index in queries.indices.reversed() {
            aggregateCommitment = aggregateCommitment
                + commitments[index].multiplied(by: power)
            aggregateEvaluation += Self.evaluate(
                queries[index].polynomial,
                at: queries[index].point
            ) * power
            power *= x1
        }

        let quotientCommitment = try reader.readPoint()
        let evaluationPoint = reader.squeezeChallenge().scalar
        let aggregateAtEvaluationPoint = try reader.readScalar()
        let quotientChallenge = reader.squeezeChallenge().scalar
        let denominatorInverse = try XCTUnwrap(
            (evaluationPoint - queries[0].point).inverted()
        )
        let quotientEvaluation =
            (aggregateAtEvaluationPoint - aggregateEvaluation) * denominatorInverse
        let commitment = quotientCommitment.projective.multiplied(by: quotientChallenge)
            + aggregateCommitment
        let value = quotientEvaluation * quotientChallenge + aggregateAtEvaluationPoint
        let verified = try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: commitment,
            point: evaluationPoint,
            value: value,
            transcript: &reader
        )
        return (verified, reader.remainingBytes)
    }

    private static func hex(_ value: String) throws -> Data {
        let compact = value.filter { !$0.isWhitespace }
        guard let data = Data(hexString: compact) else {
            throw NSError(
                domain: "Halo2PastaTests",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "Invalid hexadecimal test vector"]
            )
        }
        return data
    }

    private struct SeededGenerator: RandomNumberGenerator {
        var state: UInt64

        mutating func next() -> UInt64 {
            state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
            var value = state
            value = (value ^ (value >> 30)) &* 0xbf58_476d_1ce4_e5b9
            value = (value ^ (value >> 27)) &* 0x94d0_49bb_1331_11eb
            return value ^ (value >> 31)
        }
    }

}
