import Foundation

public enum Halo2OfflineNoteProverError: Error, Equatable {
    case invalidInstanceValues
    case invalidProofSize(Int)
    case invalidQuotientEvaluation
    case nonVanishingQuotientNumerator(row: Int)
    case nonVanishingGateConstraint(index: Int)
    case invalidZK1Payload
}

public enum Halo2OfflineNoteProver {
    public static let circuitID = OfflineNoteConstants.recursiveVerifierName
    public static let backendTag: UInt32 = 0
    public static let ipaK: UInt32 = 7
    public static let maxEnvelopeBytes = 20 * 1024

    public static let canonicalVKHash = Data(hexString: "3493ea067302cab2180cef8f5dc60e0e6751ab9bb0c850286e2aaace2f863c25")!
    private static let canonicalTranscriptRepresentation = Data(hexString: "7db2235914292d4e825d6d51e1a880da77f107eb2c7853e3ec9c9d0dccc59813")!
    private static let proofDegree = 6
    private static let blindingFactors = 5

    private struct ProverContext: Sendable {
        let params: Halo2IPAParameters
        let domain: Halo2ExtendedEvaluationDomain
        let vkRepr: PastaFp
        let fixedPolys: [[PastaFp]]
    }

    private static let contextResult: Result<ProverContext, Error> = Result(catching: {
        let params = try Halo2IPAParameters.generated(k: ipaK)
        let domain = try Halo2ExtendedEvaluationDomain(degree: proofDegree, k: ipaK)
        guard let vkRepr = PastaFp.fromCanonicalBytes(canonicalTranscriptRepresentation) else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }
        var selectorLagrange = [PastaFp](repeating: .zero, count: domain.n)
        selectorLagrange[0] = .one
        return try ProverContext(
            params: params,
            domain: domain,
            vkRepr: vkRepr,
            fixedPolys: [domain.lagrangeToCoeff(selectorLagrange)]
        )
    })

    private static func context() throws -> ProverContext {
        try contextResult.get()
    }

    public static func prove(instanceValues: OfflineNoteInstanceValues) throws -> OfflineNoteRecursiveProof {
        let proofPayload = try proveZK1Payload(instanceValues: instanceValues)
        let envelope = try openVerifyEnvelope(proofPayload: proofPayload)
        guard envelope.count <= maxEnvelopeBytes else {
            throw Halo2OfflineNoteProverError.invalidProofSize(envelope.count)
        }
        return try OfflineNoteRecursiveProof(
            publicInputsHash: instanceValues.publicInputsHash(),
            proofBytes: envelope
        )
    }

    public static func proveOpenVerifyEnvelope(instanceValues: OfflineNoteInstanceValues) throws -> Data {
        try prove(instanceValues: instanceValues).proof.bytes
    }

    public static func proveRedeem(_ redemption: OfflineNoteRedeem) throws -> OfflineNoteRecursiveProof {
        try prove(instanceValues: OfflineNoteInstanceBuilder.redeemInstanceValues(for: redemption))
    }

    public static func proveAudit(_ audit: OfflineNoteAuditBundle) throws -> OfflineNoteRecursiveProof {
        try prove(instanceValues: OfflineNoteInstanceBuilder.auditInstanceValues(for: audit))
    }

    public static func prewarm() throws {
        _ = try context()
    }

    public static func proveZK1Payload(instanceValues: OfflineNoteInstanceValues) throws -> Data {
        let context = try context()
        let params = context.params
        let domain = context.domain
        let publicScalars = instanceValues.publicValues.map(PastaFp.init)
        let inputScalars = instanceValues.inputAmounts.map(PastaFp.init)
        let outputScalars = instanceValues.outputAmounts.map(PastaFp.init)
        guard publicScalars.count == 16, inputScalars.count == 4, outputScalars.count == 2 else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }

        var rng = SystemRandomNumberGenerator()
        var transcript = Halo2Blake2bWriteTranscript()
        transcript.commonScalar(context.vkRepr)

        let instanceLagrange = publicScalars.map { scalar -> [PastaFp] in
            var column = [PastaFp](repeating: .zero, count: domain.n)
            column[0] = scalar
            return column
        }
        let instancePolys = try instanceLagrange.map { try domain.lagrangeToCoeff($0) }
        for scalar in publicScalars {
            try transcript.commonPoint(
                params.commitLagrangeSparse(entries: [(index: 0, scalar: scalar)], blind: .one).toAffine()
            )
        }

        var adviceLagrange = [[PastaFp]]()
        adviceLagrange.reserveCapacity(22)
        for scalar in publicScalars + inputScalars + outputScalars {
            var column = [PastaFp](repeating: .zero, count: domain.n)
            column[0] = scalar
            adviceLagrange.append(column)
        }
        let unusableRowsStart = domain.n - (blindingFactors + 1)
        for columnIndex in adviceLagrange.indices {
            for row in unusableRowsStart..<domain.n {
                adviceLagrange[columnIndex][row] = randomScalar(rng: &rng)
            }
        }
        let adviceBlinds = adviceLagrange.map { _ in randomScalar(rng: &rng) }
        for (column, blind) in zip(adviceLagrange, adviceBlinds) {
            try transcript.writePoint(params.commitLagrangeSparse(entries: sparseEntries(column), blind: blind).toAffine())
        }
        let advicePolys = try adviceLagrange.map { try domain.lagrangeToCoeff($0) }

        let theta = transcript.squeezeChallenge().scalar
        _ = theta
        let beta = transcript.squeezeChallenge().scalar
        let gamma = transcript.squeezeChallenge().scalar
        _ = beta
        _ = gamma

        let randomPoly = (0..<domain.n).map { _ in randomScalar(rng: &rng) }
        let randomBlind = randomScalar(rng: &rng)
        try transcript.writePoint(params.commit(coefficients: randomPoly, blind: randomBlind).toAffine())
        let y = transcript.squeezeChallenge().scalar

        let fixedPolys = context.fixedPolys

        let hCoefficients = try evaluateQuotientCoefficients(
            domain: domain,
            y: y,
            fixedPolys: fixedPolys,
            advicePolys: advicePolys,
            instancePolys: instancePolys
        )
        var hPieces: [[PastaFp]] = []
        hPieces.reserveCapacity(domain.quotientPolynomialDegree)
        var hBlinds: [PastaFp] = []
        for offset in stride(from: 0, to: hCoefficients.count, by: domain.n) {
            hPieces.append(Array(hCoefficients[offset..<(offset + domain.n)]))
            hBlinds.append(randomScalar(rng: &rng))
        }
        for (piece, blind) in zip(hPieces, hBlinds) {
            try transcript.writePoint(params.commit(coefficients: piece, blind: blind).toAffine())
        }

        let x = transcript.squeezeChallenge().scalar
        let xN = x.powVartime([UInt64(domain.n), 0, 0, 0])

        let instanceEvals = instancePolys.map { evaluatePolynomial($0, at: x) }
        let adviceEvals = advicePolys.map { evaluatePolynomial($0, at: x) }
        let fixedEvals = fixedPolys.map { evaluatePolynomial($0, at: x) }
        let randomEval = evaluatePolynomial(randomPoly, at: x)

        for eval in instanceEvals {
            transcript.writeScalar(eval)
        }
        for eval in adviceEvals {
            transcript.writeScalar(eval)
        }
        for eval in fixedEvals {
            transcript.writeScalar(eval)
        }
        transcript.writeScalar(randomEval)

        let hPoly = hPieces.reversed().reduce([PastaFp](repeating: .zero, count: domain.n)) { acc, piece in
            add(scale(acc, by: xN), piece)
        }
        let hBlind = hBlinds.reversed().reduce(PastaFp.zero) { acc, blind in
            acc * xN + blind
        }
        let hEval = evaluatePolynomial(hPoly, at: x)
        guard let vanishingInv = (xN - .one).inverted(),
              hEval == evaluateGate(
                selector: fixedEvals[0],
                advice: adviceEvals.map { [$0] },
                instance: instanceEvals.map { [$0] },
                row: 0,
                y: y
              ) * vanishingInv else {
            throw Halo2OfflineNoteProverError.invalidQuotientEvaluation
        }

        var queries: [Halo2IPAProverQuery] = []
        queries.reserveCapacity(instancePolys.count + advicePolys.count + fixedPolys.count + 2)
        for poly in instancePolys {
            queries.append(Halo2IPAProverQuery(point: x, polynomial: poly, blind: .one))
        }
        for (poly, blind) in zip(advicePolys, adviceBlinds) {
            queries.append(Halo2IPAProverQuery(point: x, polynomial: poly, blind: blind))
        }
        for poly in fixedPolys {
            queries.append(Halo2IPAProverQuery(point: x, polynomial: poly, blind: .one))
        }
        queries.append(Halo2IPAProverQuery(point: x, polynomial: hPoly, blind: hBlind))
        queries.append(Halo2IPAProverQuery(point: x, polynomial: randomPoly, blind: randomBlind))

        try Halo2IPAOpeningProof.appendSamePointMultiOpeningProof(
            params: params,
            transcript: &transcript,
            queries: queries,
            rng: &rng
        )

        return zk1ProofPayload(proofTranscript: transcript.proofBytes, instanceValues: instanceValues)
    }

    public static func verifyZK1Payload(_ payload: Data, publicValues: [UInt64]) throws -> Bool {
        guard publicValues.count == 16 else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }
        let (proofTranscript, encodedPublicValues) = try decodeZK1ProofPayload(payload)
        guard encodedPublicValues == publicValues else {
            return false
        }

        let context = try context()
        let params = context.params
        let domain = context.domain
        let publicScalars = publicValues.map(PastaFp.init)
        var transcript = Halo2Blake2bReadTranscript(proof: proofTranscript)
        transcript.commonScalar(context.vkRepr)

        let instanceLagrange = publicScalars.map { scalar -> [PastaFp] in
            var column = [PastaFp](repeating: .zero, count: domain.n)
            column[0] = scalar
            return column
        }
        let instanceCommitments = try instanceLagrange.map {
            try params.commitLagrange(evaluations: $0, blind: .one).toAffine()
        }
        for commitment in instanceCommitments {
            try transcript.commonPoint(commitment)
        }

        var adviceCommitments: [VestaAffine] = []
        adviceCommitments.reserveCapacity(22)
        for _ in 0..<22 {
            adviceCommitments.append(try transcript.readPoint())
        }
        _ = transcript.squeezeChallenge()
        _ = transcript.squeezeChallenge()
        _ = transcript.squeezeChallenge()

        let randomCommitment = try transcript.readPoint()
        let y = transcript.squeezeChallenge().scalar

        let fixedCommitment = try params.commitLagrangeSparse(
            entries: [(index: 0, scalar: PastaFp.one)],
            blind: .one
        ).toAffine()

        var hCommitments: [VestaAffine] = []
        hCommitments.reserveCapacity(domain.quotientPolynomialDegree)
        for _ in 0..<domain.quotientPolynomialDegree {
            hCommitments.append(try transcript.readPoint())
        }

        let x = transcript.squeezeChallenge().scalar
        let xN = x.powVartime([UInt64(domain.n), 0, 0, 0])
        let instanceEvals = try readScalars(count: 16, transcript: &transcript)
        let adviceEvals = try readScalars(count: 22, transcript: &transcript)
        let fixedEvals = try readScalars(count: 1, transcript: &transcript)
        let randomEval = try transcript.readScalar()
        let expectedHEval = evaluateGate(
            selector: fixedEvals[0],
            advice: adviceEvals.map { [$0] },
            instance: instanceEvals.map { [$0] },
            row: 0,
            y: y
        )
        guard let vanishingInv = (xN - .one).inverted() else {
            return false
        }

        var queries: [Halo2VerifierQuery] = []
        queries.reserveCapacity(41)
        for (commitment, eval) in zip(instanceCommitments, instanceEvals) {
            queries.append(Halo2VerifierQuery(commitment: commitment.projective, eval: eval))
        }
        for (commitment, eval) in zip(adviceCommitments, adviceEvals) {
            queries.append(Halo2VerifierQuery(commitment: commitment.projective, eval: eval))
        }
        queries.append(Halo2VerifierQuery(commitment: fixedCommitment.projective, eval: fixedEvals[0]))

        var hCommitment = VestaProjective.identity
        for commitment in hCommitments.reversed() {
            hCommitment = hCommitment.multiplied(by: xN) + commitment.projective
        }
        queries.append(Halo2VerifierQuery(commitment: hCommitment, eval: expectedHEval * vanishingInv))
        queries.append(Halo2VerifierQuery(commitment: randomCommitment.projective, eval: randomEval))

        let x1 = transcript.squeezeChallenge().scalar
        _ = transcript.squeezeChallenge()
        var qCommitment = VestaProjective.identity
        var qEval = PastaFp.zero
        var power = PastaFp.one
        for query in queries.reversed() {
            qCommitment += query.commitment.multiplied(by: power)
            qEval += query.eval * power
            power *= x1
        }

        let qPrimeCommitment = try transcript.readPoint()
        let x3 = transcript.squeezeChallenge().scalar
        let qAtX3 = try transcript.readScalar()
        let x4 = transcript.squeezeChallenge().scalar
        guard let denominatorInv = (x3 - x).inverted() else {
            return false
        }
        let quotientEval = (qAtX3 - qEval) * denominatorInv
        let pCommitment = qPrimeCommitment.projective.multiplied(by: x4) + qCommitment
        let pValue = quotientEval * x4 + qAtX3
        let ipaOK = try Halo2IPAOpeningProof.verifyInTranscript(
            params: params,
            commitment: pCommitment,
            point: x3,
            value: pValue,
            transcript: &transcript
        )
        return ipaOK && transcript.remainingBytes == 0
    }

    public static func verifyOpenVerifyEnvelope(_ envelope: Data, publicValues: [UInt64]) throws -> Bool {
        try verifyZK1Payload(
            proofPayload(fromOpenVerifyEnvelope: envelope),
            publicValues: publicValues
        )
    }

    public static func verifyOpenVerifyEnvelope(_ envelope: Data, publicInputsHashHex: String) throws -> Bool {
        let normalized = publicInputsHashHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard normalized.count == 64, let expectedPublicInputsHash = Data(hexString: normalized) else {
            return false
        }
        let proofPayload = try proofPayload(fromOpenVerifyEnvelope: envelope)
        let (_, publicValues) = try decodeZK1ProofPayload(proofPayload)
        guard try publicInputsHash(fromPublicValues: publicValues) == expectedPublicInputsHash else {
            return false
        }
        return try verifyZK1Payload(proofPayload, publicValues: publicValues)
    }

    public static func publicValues(fromOpenVerifyEnvelope envelope: Data) throws -> [UInt64] {
        let proofPayload = try proofPayload(fromOpenVerifyEnvelope: envelope)
        let (_, publicValues) = try decodeZK1ProofPayload(proofPayload)
        return publicValues
    }

    public static func verifyAudit(_ audit: OfflineNoteAuditBundle) throws -> Bool {
        try audit.validateProofBinding()
        let instanceValues = try OfflineNoteInstanceBuilder.auditInstanceValues(for: audit)
        guard audit.recursiveProof.publicInputsHash == (try instanceValues.publicInputsHash()) else {
            return false
        }
        let proofPayload = try proofPayload(fromOpenVerifyEnvelope: audit.recursiveProof.proof.bytes)
        return try verifyZK1Payload(proofPayload, publicValues: instanceValues.publicValues)
    }

    public static func verifyRedeem(_ redemption: OfflineNoteRedeem) throws -> Bool {
        try redemption.validateProofBinding()
        let instanceValues = try OfflineNoteInstanceBuilder.redeemInstanceValues(for: redemption)
        guard redemption.recursiveProof.publicInputsHash == (try instanceValues.publicInputsHash()) else {
            return false
        }
        let proofPayload = try proofPayload(fromOpenVerifyEnvelope: redemption.recursiveProof.proof.bytes)
        return try verifyZK1Payload(proofPayload, publicValues: instanceValues.publicValues)
    }

    public static func openVerifyEnvelope(proofPayload: Data) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(backendTag))
        writer.writeField(OfflineCompactNorito.encodeString(circuitID))
        writer.writeField(canonicalVKHash)
        writer.writeField(OfflineNorito.encodeBytesVec(Data(OfflineNoteConstants.recursivePublicInputsSchema.utf8)))
        writer.writeField(OfflineNorito.encodeBytesVec(proofPayload))
        writer.writeField(OfflineNorito.encodeBytesVec(Data()))
        return noritoEncode(
            typeName: "iroha_data_model::zk::OpenVerifyEnvelope",
            payload: writer.data,
            flags: 2
        )
    }

    public static func proofPayload(fromOpenVerifyEnvelope envelope: Data) throws -> Data {
        guard let frame = noritoDecodeFrame(envelope),
              frame.header.schema == noritoSchemaHash(forTypeName: "iroha_data_model::zk::OpenVerifyEnvelope"),
              frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let tag = try field(&reader) { try $0.readUInt32LE() }
        guard tag == backendTag else {
            return Data()
        }
        let circuit = try field(&reader, readCompactString)
        guard circuit == circuitID else {
            return Data()
        }
        let vkHash = try field(&reader) { try $0.readBytes(32) }
        guard vkHash == canonicalVKHash else {
            return Data()
        }
        _ = try field(&reader, readU64BytesVec)
        let proofPayload = try field(&reader, readU64BytesVec)
        _ = try field(&reader, readU64BytesVec)
        guard reader.remaining() == 0, !proofPayload.isEmpty else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        return proofPayload
    }

    private static func field<T>(
        _ reader: inout OfflineNoritoReader,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        var child = OfflineNoritoReader(data: try reader.readCompactField())
        let value = try decode(&child)
        guard child.remaining() == 0 else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        return value
    }

    private static func readCompactString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max),
              let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8)
        else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        return value
    }

    private static func readU64BytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        return try reader.readBytes(Int(length))
    }

    private static func evaluateQuotientCoefficients(
        domain: Halo2ExtendedEvaluationDomain,
        y: PastaFp,
        fixedPolys: [[PastaFp]],
        advicePolys: [[PastaFp]],
        instancePolys: [[PastaFp]]
    ) throws -> [PastaFp] {
        let selector = fixedPolys[0]
        let publicValues = Array(advicePolys[0..<16])
        let inputs = Array(advicePolys[16..<20])
        let outputs = Array(advicePolys[20..<22])
        let mode = publicValues[4]
        let inputCount = publicValues[5]
        let outputCount = publicValues[6]
        let inputSumPublic = publicValues[7]
        let outputSumPublic = publicValues[8]
        let one = [PastaFp.one]
        let two = [PastaFp(2)]
        let three = [PastaFp(3)]
        let four = [PastaFp(4)]

        var constraints: [[PastaFp]] = []
        constraints.reserveCapacity(32)
        for index in 0..<16 {
            constraints.append(polyMul(selector, polySub(publicValues[index], instancePolys[index])))
        }
        constraints.append(polyMul(selector, polyMul(polySub(mode, one), polySub(mode, two))))
        constraints.append(polyMul(
            selector,
            polyMul(
                polyMul(polySub(inputCount, one), polySub(inputCount, two)),
                polyMul(polySub(inputCount, three), polySub(inputCount, four))
            )
        ))
        constraints.append(polyMul(selector, polyMul(polySub(outputCount, one), polySub(outputCount, two))))
        constraints.append(polyMul(selector, polyMul(polySub(mode, two), polySub(outputCount, one))))
        constraints.append(polyMul(selector, polySub(polySum(inputs), inputSumPublic)))
        constraints.append(polyMul(selector, polySub(polySum(outputs), outputSumPublic)))
        constraints.append(polyMul(selector, polySub(inputSumPublic, outputSumPublic)))
        constraints.append(polyMul(
            selector,
            polyMul(
                inputs[1],
                polyMul(polySub(inputCount, two), polyMul(polySub(inputCount, three), polySub(inputCount, four)))
            )
        ))
        constraints.append(polyMul(
            selector,
            polyMul(inputs[2], polyMul(polySub(inputCount, three), polySub(inputCount, four)))
        ))
        constraints.append(polyMul(selector, polyMul(inputs[3], polySub(inputCount, four))))
        constraints.append(polyMul(selector, polyMul(outputs[1], polySub(outputCount, two))))

        for (index, constraint) in constraints.enumerated() {
            guard evaluatePolynomial(constraint, at: .one) == .zero else {
                throw Halo2OfflineNoteProverError.nonVanishingGateConstraint(index: index)
            }
        }
        let numerator = constraints.reduce([PastaFp]()) { accumulator, constraint in
            polyAdd(polyScale(accumulator, by: y), constraint)
        }
        return try divideByVanishingPolynomial(
            numerator,
            domainSize: domain.n,
            quotientLength: domain.n * domain.quotientPolynomialDegree
        )
    }

    private static func evaluateGate(
        selector: PastaFp,
        advice: [[PastaFp]],
        instance: [[PastaFp]],
        row: Int,
        y: PastaFp
    ) -> PastaFp {
        let publicValues = (0..<16).map { advice[$0][row] }
        let inputs = (0..<4).map { advice[16 + $0][row] }
        let outputs = (0..<2).map { advice[20 + $0][row] }

        let mode = publicValues[4]
        let inputCount = publicValues[5]
        let outputCount = publicValues[6]
        let inputSumPublic = publicValues[7]
        let outputSumPublic = publicValues[8]
        let one = PastaFp.one
        let two = PastaFp(2)
        let three = PastaFp(3)
        let four = PastaFp(4)

        var constraints: [PastaFp] = []
        constraints.reserveCapacity(32)
        for index in 0..<16 {
            constraints.append(selector * (publicValues[index] - instance[index][row]))
        }
        constraints.append(selector * (mode - one) * (mode - two))
        constraints.append(selector * (inputCount - one) * (inputCount - two) * (inputCount - three) * (inputCount - four))
        constraints.append(selector * (outputCount - one) * (outputCount - two))
        constraints.append(selector * (mode - two) * (outputCount - one))
        constraints.append(selector * (inputs.reduce(.zero, +) - inputSumPublic))
        constraints.append(selector * (outputs.reduce(.zero, +) - outputSumPublic))
        constraints.append(selector * (inputSumPublic - outputSumPublic))
        constraints.append(selector * inputs[1] * (inputCount - two) * (inputCount - three) * (inputCount - four))
        constraints.append(selector * inputs[2] * (inputCount - three) * (inputCount - four))
        constraints.append(selector * inputs[3] * (inputCount - four))
        constraints.append(selector * outputs[1] * (outputCount - two))

        return constraints.reduce(PastaFp.zero) { accumulator, constraint in
            accumulator * y + constraint
        }
    }

    private static func zk1ProofPayload(proofTranscript: Data, instanceValues: OfflineNoteInstanceValues) -> Data {
        var out = Data([0x5A, 0x4B, 0x31, 0x00])
        appendTLV(tag: "PROF", value: proofTranscript, to: &out)

        var instances = Data()
        appendUInt32LE(16, to: &instances)
        appendUInt32LE(1, to: &instances)
        for value in instanceValues.publicValues {
            instances.append(OfflineNoteInstanceValues.instanceScalarBytes(value))
        }
        appendTLV(tag: "I10P", value: instances, to: &out)
        return out
    }

    private static func decodeZK1ProofPayload(_ payload: Data) throws -> (Data, [UInt64]) {
        let bytes = [UInt8](payload)
        guard bytes.count >= 4, bytes[0] == 0x5A, bytes[1] == 0x4B, bytes[2] == 0x31, bytes[3] == 0x00 else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        var cursor = 4
        var proof: Data?
        var publicValues: [UInt64]?
        while cursor < bytes.count {
            guard cursor + 8 <= bytes.count else {
                throw Halo2OfflineNoteProverError.invalidZK1Payload
            }
            let tag = String(bytes: bytes[cursor..<(cursor + 4)], encoding: .utf8)
            cursor += 4
            let length = Int(readUInt32LE(bytes, at: cursor))
            cursor += 4
            guard cursor + length <= bytes.count else {
                throw Halo2OfflineNoteProverError.invalidZK1Payload
            }
            let value = Data(bytes[cursor..<(cursor + length)])
            cursor += length
            if tag == "PROF" {
                proof = value
            } else if tag == "I10P" {
                publicValues = try decodeI10PPublicValues(value)
            }
        }
        guard let proof, !proof.isEmpty, let publicValues else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        return (proof, publicValues)
    }

    private static func decodeI10PPublicValues(_ payload: Data) throws -> [UInt64] {
        let bytes = [UInt8](payload)
        guard bytes.count == 8 + 16 * 32,
              readUInt32LE(bytes, at: 0) == 16,
              readUInt32LE(bytes, at: 4) == 1 else {
            throw Halo2OfflineNoteProverError.invalidZK1Payload
        }
        var values: [UInt64] = []
        values.reserveCapacity(16)
        var cursor = 8
        for _ in 0..<16 {
            let scalarBytes = Data(bytes[cursor..<(cursor + 32)])
            guard PastaFp.fromCanonicalBytes(scalarBytes) != nil else {
                throw Halo2OfflineNoteProverError.invalidZK1Payload
            }
            var value: UInt64 = 0
            for idx in 0..<8 {
                value |= UInt64(bytes[cursor + idx]) << UInt64(idx * 8)
            }
            guard bytes[(cursor + 8)..<(cursor + 32)].allSatisfy({ $0 == 0 }) else {
                throw Halo2OfflineNoteProverError.invalidZK1Payload
            }
            values.append(value)
            cursor += 32
        }
        return values
    }

    private static func readScalars(count: Int, transcript: inout Halo2Blake2bReadTranscript) throws -> [PastaFp] {
        try (0..<count).map { _ in try transcript.readScalar() }
    }

    private static func readUInt32LE(_ bytes: [UInt8], at offset: Int) -> UInt32 {
        UInt32(bytes[offset])
            | (UInt32(bytes[offset + 1]) << 8)
            | (UInt32(bytes[offset + 2]) << 16)
            | (UInt32(bytes[offset + 3]) << 24)
    }

    private static func appendTLV(tag: String, value: Data, to out: inout Data) {
        out.append(Data(tag.utf8))
        appendUInt32LE(UInt32(value.count), to: &out)
        out.append(value)
    }

    private static func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        var le = value.littleEndian
        out.append(contentsOf: withUnsafeBytes(of: &le, Array.init))
    }

    private static func randomScalar<R: RandomNumberGenerator>(rng: inout R) -> PastaFp {
        var bytes = Data(capacity: 64)
        for _ in 0..<8 {
            var word = rng.next().littleEndian
            withUnsafeBytes(of: &word) { bytes.append(contentsOf: $0) }
        }
        return PastaFp.fromUniformBytes64(bytes)!
    }

    private static func sparseEntries(_ values: [PastaFp]) -> [(index: Int, scalar: PastaFp)] {
        var entries: [(index: Int, scalar: PastaFp)] = []
        entries.reserveCapacity(8)
        for (index, value) in values.enumerated() where value != .zero {
            entries.append((index: index, scalar: value))
        }
        return entries
    }

    private static func evaluatePolynomial(_ polynomial: [PastaFp], at point: PastaFp) -> PastaFp {
        polynomial.reversed().reduce(PastaFp.zero) { accumulator, coefficient in
            accumulator * point + coefficient
        }
    }

    private static func add(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> [PastaFp] {
        precondition(lhs.count == rhs.count)
        return zip(lhs, rhs).map { $0 + $1 }
    }

    private static func scale(_ values: [PastaFp], by scalar: PastaFp) -> [PastaFp] {
        values.map { $0 * scalar }
    }

    private static func polySum(_ values: [[PastaFp]]) -> [PastaFp] {
        values.reduce([PastaFp](), polyAdd)
    }

    private static func polyAdd(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> [PastaFp] {
        let count = max(lhs.count, rhs.count)
        var result = [PastaFp](repeating: .zero, count: count)
        for idx in lhs.indices {
            result[idx] += lhs[idx]
        }
        for idx in rhs.indices {
            result[idx] += rhs[idx]
        }
        return trim(result)
    }

    private static func polySub(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> [PastaFp] {
        let count = max(lhs.count, rhs.count)
        var result = [PastaFp](repeating: .zero, count: count)
        for idx in lhs.indices {
            result[idx] += lhs[idx]
        }
        for idx in rhs.indices {
            result[idx] -= rhs[idx]
        }
        return trim(result)
    }

    private static func polyMul(_ lhs: [PastaFp], _ rhs: [PastaFp]) -> [PastaFp] {
        guard !lhs.isEmpty, !rhs.isEmpty else {
            return []
        }
        var result = [PastaFp](repeating: .zero, count: lhs.count + rhs.count - 1)
        for lhsIndex in lhs.indices where lhs[lhsIndex] != .zero {
            for rhsIndex in rhs.indices where rhs[rhsIndex] != .zero {
                result[lhsIndex + rhsIndex] += lhs[lhsIndex] * rhs[rhsIndex]
            }
        }
        return trim(result)
    }

    private static func polyScale(_ values: [PastaFp], by scalar: PastaFp) -> [PastaFp] {
        guard scalar != .zero else {
            return []
        }
        return trim(values.map { $0 * scalar })
    }

    private static func divideByVanishingPolynomial(
        _ numerator: [PastaFp],
        domainSize: Int,
        quotientLength: Int
    ) throws -> [PastaFp] {
        var remainder = trim(numerator)
        var quotient = [PastaFp](repeating: .zero, count: max(0, remainder.count - domainSize))

        while let degree = remainder.indices.last, degree >= domainSize {
            let coefficient = remainder[degree]
            let quotientIndex = degree - domainSize
            if quotientIndex >= quotient.count {
                quotient.append(contentsOf: repeatElement(.zero, count: quotientIndex - quotient.count + 1))
            }
            quotient[quotientIndex] += coefficient
            remainder[degree] -= coefficient
            remainder[quotientIndex] += coefficient
            remainder = trim(remainder)
        }

        guard remainder.allSatisfy({ $0 == .zero }) else {
            throw Halo2OfflineNoteProverError.invalidQuotientEvaluation
        }
        let nonZeroOverflow = quotient.dropFirst(quotientLength).contains { $0 != .zero }
        guard !nonZeroOverflow else {
            throw Halo2OfflineNoteProverError.invalidQuotientEvaluation
        }
        if quotient.count < quotientLength {
            quotient.append(contentsOf: repeatElement(.zero, count: quotientLength - quotient.count))
        }
        return Array(quotient.prefix(quotientLength))
    }

    private static func trim(_ values: [PastaFp]) -> [PastaFp] {
        var values = values
        while values.last == .some(.zero) {
            values.removeLast()
        }
        return values
    }

    private static func publicInputsHash(fromPublicValues publicValues: [UInt64]) throws -> Data {
        guard publicValues.count >= 4 else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }
        var hash = Data()
        for value in publicValues.prefix(4) {
            var word = value
            for _ in 0..<8 {
                hash.append(UInt8(word & 0xff))
                word >>= 8
            }
        }
        guard hash.count == 32 else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }
        return hash
    }
}

private struct Halo2VerifierQuery {
    let commitment: VestaProjective
    let eval: PastaFp
}

private extension OfflineNoteInstanceValues {
    func publicInputsHash() throws -> Data {
        var out = Data()
        for value in publicValues.prefix(4) {
            var word = value
            for _ in 0..<8 {
                out.append(UInt8(word & 0xff))
                word >>= 8
            }
        }
        guard out.count == 32 else {
            throw Halo2OfflineNoteProverError.invalidInstanceValues
        }
        return out
    }
}
