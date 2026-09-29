// Test-only exact-lift arithmetic for retained diagnostic vectors. This is not secure encryption.
import Foundation
import CryptoKit
@testable import IrohaSwift

private struct DiagnosticIdentifierBfvValidatedParameters {
    let polynomialDegree: Int
    let plaintextModulus: UInt64
    let ciphertextModulus: UInt64
    let maxInputBytes: Int
    let useCompactNoritoLengths: Bool
    let publicKeyA: [UInt64]
    let publicKeyB: [UInt64]
}

private struct DiagnosticIdentifierBfvCiphertextSlot {
    let c0: [UInt64]
    let c1: [UInt64]
}

private struct DiagnosticIdentifierBfvDeterministicStream {
    private let seed: Data
    private let domain: Data
    private var counter: UInt64 = 0
    private var buffer = Data()
    private var index = 0

    init(seed: Data, domain: String) {
        self.seed = seed
        self.domain = Data(domain.utf8)
    }

    mutating func nextByte() -> UInt8 {
        if index >= buffer.count {
            refill()
        }
        let value = buffer[index]
        index += 1
        return value
    }

    mutating func nextUInt64() -> UInt64 {
        var value: UInt64 = 0
        for offset in 0..<8 {
            value |= UInt64(nextByte()) << (UInt64(offset) * 8)
        }
        return value
    }

    private mutating func refill() {
        var hasher = SHA512()
        hasher.update(data: Data("iroha.sdk.identifier.bfv.prg.v1".utf8))
        hasher.update(data: domain)
        hasher.update(data: seed)
        hasher.update(data: DiagnosticIdentifierBfvEnvelopeBuilder.littleEndianData(counter))
        buffer = Data(hasher.finalize())
        index = 0
        counter &+= 1
    }
}

enum DiagnosticIdentifierBfvEnvelopeBuilder {
    private static let encryptDomain = Data("iroha.crypto.fhe.bfv.encrypt.v1".utf8)
    private static let rustSlotDomain = Data("iroha.crypto.fhe.bfv.identifier.slot.v1".utf8)
    private static let slotDomain = Data("iroha.sdk.identifier.bfv.slot.v1".utf8)
    private static let uDomain = "iroha.sdk.identifier.bfv.u.v1"
    private static let e1Domain = "iroha.sdk.identifier.bfv.e1.v1"
    private static let e2Domain = "iroha.sdk.identifier.bfv.e2.v1"
    private static let schemaName = "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext"
    private static let maxRegisteredInputBytes = 63

    static func encrypt(policy: ToriiIdentifierPolicySummary,
                        input: String,
                        seedHex: String? = nil) throws -> String {
        guard policy.inputEncryption?.lowercased() == "bfv-v1" else {
            throw ToriiClientError.invalidPayload(
                "Policy \(policy.policyId) does not publish BFV encrypted-input support."
            )
        }
        guard let publicParameters = policy.inputEncryptionPublicParametersDecoded else {
            throw ToriiClientError.invalidPayload(
                "Policy \(policy.policyId) is missing decoded BFV public parameters."
            )
        }
        let normalizedInput = try policy.normalization.normalize(input, field: "input")
        let params = try validate(publicParameters)
        let inputBytes = Array(normalizedInput.utf8)
        guard inputBytes.count <= params.maxInputBytes else {
            throw ToriiClientError.invalidPayload(
                "input exceeds maxInputBytes \(params.maxInputBytes)."
            )
        }
        let seed = try resolvedSeed(seedHex)
        let scalars = encodeIdentifierSlots(maxInputBytes: params.maxInputBytes, inputBytes: inputBytes)
        let slots = try scalars.enumerated().map { index, scalar in
            try encryptScalar(params: params, scalar: scalar, seed: seed, slotIndex: index)
        }
        return try encodeEnvelope(slots, compact: params.useCompactNoritoLengths).hexEncodedString()
    }

    private static func resolvedSeed(_ seedHex: String?) throws -> Data {
        if let seedHex {
            let normalized = try ToriiRequestValidation.normalizedEvenLengthHex(
                seedHex,
                field: "seedHex"
            )
            guard let data = Data(hexString: normalized), !data.isEmpty else {
                throw ToriiClientError.invalidPayload("seedHex must be valid hex.")
            }
            return data
        }
        return Data((0..<32).map { _ in UInt8.random(in: .min ... .max) })
    }

    private static func validate(_ publicParameters: ToriiIdentifierBfvPublicParameters) throws -> DiagnosticIdentifierBfvValidatedParameters {
        let params = publicParameters.parameters
        let polynomialDegree = Int(params.polynomialDegree)
        guard polynomialDegree >= 2, polynomialDegree.nonzeroBitCount == 1 else {
            throw ToriiClientError.invalidPayload("BFV polynomialDegree must be a power of two and at least 2.")
        }
        guard params.decompositionBaseLog >= 1, params.decompositionBaseLog <= 16 else {
            throw ToriiClientError.invalidPayload("BFV decompositionBaseLog must be within 1...16.")
        }
        guard params.plaintextModulus >= 2 else {
            throw ToriiClientError.invalidPayload("BFV plaintextModulus must be at least 2.")
        }
        guard params.ciphertextModulus > params.plaintextModulus else {
            throw ToriiClientError.invalidPayload("BFV ciphertextModulus must be greater than plaintextModulus.")
        }
        guard params.ciphertextModulus % params.plaintextModulus == 0 else {
            throw ToriiClientError.invalidPayload("BFV ciphertextModulus must be divisible by plaintextModulus.")
        }
        let maxInputBytes = Int(publicParameters.maxInputBytes)
        guard maxInputBytes >= 1 else {
            throw ToriiClientError.invalidPayload("BFV maxInputBytes must be at least 1.")
        }
        guard UInt64(publicParameters.maxInputBytes) < params.plaintextModulus else {
            throw ToriiClientError.invalidPayload("BFV maxInputBytes must fit into one plaintext slot.")
        }
        guard maxInputBytes <= maxRegisteredInputBytes else {
            throw ToriiClientError.invalidPayload(
                "BFV maxInputBytes must be at most \(maxRegisteredInputBytes) for the registered RAM-LFE BFV identifier profile."
            )
        }
        guard publicParameters.publicKey.a.count == polynomialDegree,
              publicParameters.publicKey.b.count == polynomialDegree else {
            throw ToriiClientError.invalidPayload("BFV public-key polynomials must match polynomialDegree.")
        }
        for coefficient in publicParameters.publicKey.a + publicParameters.publicKey.b {
            guard coefficient < params.ciphertextModulus else {
                throw ToriiClientError.invalidPayload("BFV public-key coefficient exceeds ciphertextModulus.")
            }
        }
        let lengthEncoding = publicParameters.noritoLengthEncoding?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let useCompactNoritoLengths: Bool
        switch lengthEncoding {
        case nil, "", "u64-v1":
            useCompactNoritoLengths = false
        case "compact-v1":
            useCompactNoritoLengths = true
        default:
            throw ToriiClientError.invalidPayload("BFV noritoLengthEncoding must be u64-v1 or compact-v1.")
        }
        return DiagnosticIdentifierBfvValidatedParameters(
            polynomialDegree: polynomialDegree,
            plaintextModulus: params.plaintextModulus,
            ciphertextModulus: params.ciphertextModulus,
            maxInputBytes: maxInputBytes,
            useCompactNoritoLengths: useCompactNoritoLengths,
            publicKeyA: publicParameters.publicKey.a,
            publicKeyB: publicParameters.publicKey.b
        )
    }

    private static func encodeIdentifierSlots(maxInputBytes: Int, inputBytes: [UInt8]) -> [UInt64] {
        var slots = Array(repeating: UInt64.zero, count: maxInputBytes + 1)
        slots[0] = UInt64(inputBytes.count)
        for (index, byte) in inputBytes.enumerated() {
            slots[index + 1] = UInt64(byte)
        }
        return slots
    }

    private static func encryptScalar(params: DiagnosticIdentifierBfvValidatedParameters,
                                      scalar: UInt64,
                                      seed: Data,
                                      slotIndex: Int) throws -> DiagnosticIdentifierBfvCiphertextSlot {
        if params.useCompactNoritoLengths {
            let slotSeed = irohaHash(
                parts: [
                    rustSlotDomain,
                    seed,
                    littleEndianData(UInt64(slotIndex)),
                ]
            )
            return encryptScalarRust(params: params, scalar: scalar, seed: slotSeed)
        }
        let slotSeed = sha512(
            parts: [
                slotDomain,
                seed,
                littleEndianData(UInt64(slotIndex)),
            ]
        )
        let u = sampleSmallPolynomial(
            params: params,
            stream: DiagnosticIdentifierBfvDeterministicStream(seed: slotSeed, domain: uDomain)
        )
        let e1 = sampleErrorPolynomial(
            params: params,
            stream: DiagnosticIdentifierBfvDeterministicStream(seed: slotSeed, domain: e1Domain)
        )
        let e2 = sampleErrorPolynomial(
            params: params,
            stream: DiagnosticIdentifierBfvDeterministicStream(seed: slotSeed, domain: e2Domain)
        )
        var encoded = Array(repeating: UInt64.zero, count: params.polynomialDegree)
        encoded[0] = scalar % params.plaintextModulus
        return DiagnosticIdentifierBfvCiphertextSlot(
            c0: addPolynomialMod(
                lhs: addPolynomialMod(
                    lhs: multiplyPolynomialMod(
                        params: params,
                        lhs: params.publicKeyB,
                        rhs: u
                    ),
                    rhs: e1,
                    modulus: params.ciphertextModulus
                ),
                rhs: encoded,
                modulus: params.ciphertextModulus
            ),
            c1: addPolynomialMod(
                lhs: multiplyPolynomialMod(
                    params: params,
                    lhs: params.publicKeyA,
                    rhs: u
                ),
                rhs: e2,
                modulus: params.ciphertextModulus
            )
        )
    }

    private static func encryptScalarRust(params: DiagnosticIdentifierBfvValidatedParameters,
                                          scalar: UInt64,
                                          seed: Data) -> DiagnosticIdentifierBfvCiphertextSlot {
        var rng = DiagnosticIdentifierBfvChaCha20Rng(seed: irohaHash(parts: [encryptDomain, seed]))
        let u = sampleSmallPolynomialRust(params: params, rng: &rng)
        let e1 = sampleErrorPolynomialRust(params: params, rng: &rng)
        let e2 = sampleErrorPolynomialRust(params: params, rng: &rng)
        var encoded = Array(repeating: UInt64.zero, count: params.polynomialDegree)
        encoded[0] = scalar % params.plaintextModulus
        return DiagnosticIdentifierBfvCiphertextSlot(
            c0: addPolynomialMod(
                lhs: addPolynomialMod(
                    lhs: multiplyPolynomialMod(
                        params: params,
                        lhs: params.publicKeyB,
                        rhs: u
                    ),
                    rhs: e1,
                    modulus: params.ciphertextModulus
                ),
                rhs: encoded,
                modulus: params.ciphertextModulus
            ),
            c1: addPolynomialMod(
                lhs: multiplyPolynomialMod(
                    params: params,
                    lhs: params.publicKeyA,
                    rhs: u
                ),
                rhs: e2,
                modulus: params.ciphertextModulus
            )
        )
    }

    private static func sampleSmallPolynomial(params: DiagnosticIdentifierBfvValidatedParameters,
                                              stream: DiagnosticIdentifierBfvDeterministicStream) -> [UInt64] {
        var stream = stream
        return (0..<params.polynomialDegree).map { _ in
            switch Int(stream.nextByte() % 3) {
            case 0:
                return 0
            case 1:
                return 1
            default:
                return params.ciphertextModulus &- 1
            }
        }
    }

    private static func sampleErrorPolynomial(params: DiagnosticIdentifierBfvValidatedParameters,
                                              stream: DiagnosticIdentifierBfvDeterministicStream) -> [UInt64] {
        var stream = stream
        return (0..<params.polynomialDegree).map { _ in
            switch Int(stream.nextByte() % 3) {
            case 0:
                return 0
            case 1:
                return params.plaintextModulus
            default:
                return params.ciphertextModulus &- params.plaintextModulus
            }
        }
    }

    private static func sampleSmallPolynomialRust(params: DiagnosticIdentifierBfvValidatedParameters,
                                                  rng: inout DiagnosticIdentifierBfvChaCha20Rng) -> [UInt64] {
        (0..<params.polynomialDegree).map { _ in
            switch rustRandomRange0To2(rng: &rng) {
            case 0:
                return 0
            case 1:
                return 1
            default:
                return params.ciphertextModulus &- 1
            }
        }
    }

    private static func sampleErrorPolynomialRust(params: DiagnosticIdentifierBfvValidatedParameters,
                                                  rng: inout DiagnosticIdentifierBfvChaCha20Rng) -> [UInt64] {
        (0..<params.polynomialDegree).map { _ in
            switch rustRandomRange0To2(rng: &rng) {
            case 0:
                return 0
            case 1:
                return params.plaintextModulus
            default:
                return params.ciphertextModulus &- params.plaintextModulus
            }
        }
    }

    private static func rustRandomRange0To2(rng: inout DiagnosticIdentifierBfvChaCha20Rng) -> Int {
        let range: UInt64 = 3
        let sample = UInt64(rng.nextUInt32())
        let product = sample &* range
        var result = Int(product >> 32)
        let low = product & 0xffff_ffff
        let biasedThreshold = (UInt64(1) << 32) &- range
        if low > biasedThreshold {
            let retryProduct = UInt64(rng.nextUInt32()) &* range
            let retryHigh = retryProduct >> 32
            if low + retryHigh > 0xffff_ffff {
                result += 1
            }
        }
        return result
    }

    private static func addPolynomialMod(lhs: [UInt64], rhs: [UInt64], modulus: UInt64) -> [UInt64] {
        zip(lhs, rhs).map { addMod($0, $1, modulus: modulus) }
    }

    private static func multiplyPolynomialMod(params: DiagnosticIdentifierBfvValidatedParameters,
                                              lhs: [UInt64],
                                              rhs: [UInt64]) -> [UInt64] {
        var out = Array(repeating: UInt64.zero, count: params.polynomialDegree)
        for i in 0..<params.polynomialDegree {
            for j in 0..<params.polynomialDegree {
                let product = multiplyMod(lhs[i], rhs[j], modulus: params.ciphertextModulus)
                let target = i + j
                if target < params.polynomialDegree {
                    out[target] = addMod(out[target], product, modulus: params.ciphertextModulus)
                } else {
                    out[target - params.polynomialDegree] = subtractMod(
                        out[target - params.polynomialDegree],
                        product,
                        modulus: params.ciphertextModulus
                    )
                }
            }
        }
        return out
    }

    private static func addMod(_ lhs: UInt64, _ rhs: UInt64, modulus: UInt64) -> UInt64 {
        let (sum, overflow) = lhs.addingReportingOverflow(rhs)
        return mod128(high: overflow ? 1 : 0, low: sum, modulus: modulus)
    }

    private static func subtractMod(_ lhs: UInt64, _ rhs: UInt64, modulus: UInt64) -> UInt64 {
        if lhs >= rhs {
            return lhs - rhs
        }
        return modulus - (rhs - lhs)
    }

    private static func multiplyMod(_ lhs: UInt64, _ rhs: UInt64, modulus: UInt64) -> UInt64 {
        let fullWidth = lhs.multipliedFullWidth(by: rhs)
        return mod128(high: fullWidth.high, low: fullWidth.low, modulus: modulus)
    }

    private static func mod128(high: UInt64, low: UInt64, modulus: UInt64) -> UInt64 {
        precondition(modulus > 0)
        var remainder: UInt64 = 0
        for bit in stride(from: 63, through: 0, by: -1) {
            remainder = stepMod(remainder: remainder, bit: (high >> UInt64(bit)) & 1, modulus: modulus)
        }
        for bit in stride(from: 63, through: 0, by: -1) {
            remainder = stepMod(remainder: remainder, bit: (low >> UInt64(bit)) & 1, modulus: modulus)
        }
        return remainder
    }

    private static func stepMod(remainder: UInt64, bit: UInt64, modulus: UInt64) -> UInt64 {
        let doubled: UInt64
        if remainder >= modulus &- remainder {
            doubled = remainder &- (modulus &- remainder)
        } else {
            doubled = remainder &+ remainder
        }
        if bit == 0 {
            return doubled
        }
        if doubled == modulus &- 1 {
            return 0
        }
        return doubled &+ 1
    }

    private static func sha512(parts: [Data]) -> Data {
        var hasher = SHA512()
        for part in parts {
            hasher.update(data: part)
        }
        return Data(hasher.finalize())
    }

    private static func irohaHash(parts: [Data]) -> Data {
        var payload = Data()
        for part in parts {
            payload.append(part)
        }
        var digest = Blake2b.hash256(payload)
        digest[digest.count - 1] |= 1
        return digest
    }

    static func littleEndianData(_ value: UInt64) -> Data {
        var littleEndian = value.littleEndian
        return withUnsafeBytes(of: &littleEndian) { Data($0) }
    }

    private static func encodeEnvelope(_ slots: [DiagnosticIdentifierBfvCiphertextSlot],
                                       compact: Bool) throws -> Data {
        let payload = try encodeEnvelopePayload(slots, compact: compact)
        return noritoEncode(
            typeName: schemaName,
            payload: payload,
            flags: compact ? NoritoHeader.compactLen : 0
        )
    }

    private static func encodeEnvelopePayload(_ slots: [DiagnosticIdentifierBfvCiphertextSlot],
                                              compact: Bool) throws -> Data {
        encodeField(
            try encodeVec(slots, compact: compact) { slot in
                try encodeSlot(slot, compact: compact)
            },
            compact: compact
        )
    }

    private static func encodeSlot(_ slot: DiagnosticIdentifierBfvCiphertextSlot,
                                   compact: Bool) throws -> Data {
        var data = Data()
        data.append(
            encodeField(
                try encodeVec(slot.c0, compact: compact, encode: encodeUInt64),
                compact: compact
            )
        )
        data.append(
            encodeField(
                try encodeVec(slot.c1, compact: compact, encode: encodeUInt64),
                compact: compact
            )
        )
        return data
    }

    private static func encodeVec<T>(_ values: [T],
                                     compact: Bool,
                                     encode: (T) throws -> Data) throws -> Data {
        var data = littleEndianData(UInt64(values.count))
        for value in values {
            let payload = try encode(value)
            data.append(encodeLength(UInt64(payload.count), compact: compact))
            data.append(payload)
        }
        return data
    }

    private static func encodeField(_ payload: Data, compact: Bool) -> Data {
        var data = encodeLength(UInt64(payload.count), compact: compact)
        data.append(payload)
        return data
    }

    private static func encodeLength(_ value: UInt64, compact: Bool) -> Data {
        guard compact else {
            return littleEndianData(value)
        }
        var remaining = value
        var bytes: [UInt8] = []
        repeat {
            var byte = UInt8(remaining & 0x7f)
            remaining >>= 7
            if remaining != 0 {
                byte |= 0x80
            }
            bytes.append(byte)
        } while remaining != 0
        return Data(bytes)
    }

    private static func encodeUInt64(_ value: UInt64) -> Data {
        littleEndianData(value)
    }
}

private struct DiagnosticIdentifierBfvChaCha20Rng {
    private static let constants: [UInt32] = [
        0x61707865, 0x3320646e, 0x79622d32, 0x6b206574
    ]

    private let keyWords: [UInt32]
    private var counter: UInt64 = 0
    private var buffer: [UInt8] = []
    private var index = 0

    init(seed: Data) {
        precondition(seed.count == 32, "ChaCha20 seed must be 32 bytes")
        var words: [UInt32] = []
        let bytes = Array(seed)
        for offset in stride(from: 0, to: bytes.count, by: 4) {
            words.append(
                UInt32(bytes[offset])
                    | (UInt32(bytes[offset + 1]) << 8)
                    | (UInt32(bytes[offset + 2]) << 16)
                    | (UInt32(bytes[offset + 3]) << 24)
            )
        }
        keyWords = words
    }

    mutating func nextUInt32() -> UInt32 {
        let bytes = nextBytes(count: 4)
        return UInt32(bytes[0])
            | (UInt32(bytes[1]) << 8)
            | (UInt32(bytes[2]) << 16)
            | (UInt32(bytes[3]) << 24)
    }

    private mutating func nextBytes(count: Int) -> [UInt8] {
        var out: [UInt8] = []
        out.reserveCapacity(count)
        while out.count < count {
            if index >= buffer.count {
                refill()
            }
            let available = min(count - out.count, buffer.count - index)
            out.append(contentsOf: buffer[index..<(index + available)])
            index += available
        }
        return out
    }

    private mutating func refill() {
        let state = Self.constants + keyWords + [
            UInt32(counter & 0xffff_ffff),
            UInt32(counter >> 32),
            0,
            0,
        ]
        var working = state
        for _ in 0..<10 {
            Self.quarterRound(&working, 0, 4, 8, 12)
            Self.quarterRound(&working, 1, 5, 9, 13)
            Self.quarterRound(&working, 2, 6, 10, 14)
            Self.quarterRound(&working, 3, 7, 11, 15)
            Self.quarterRound(&working, 0, 5, 10, 15)
            Self.quarterRound(&working, 1, 6, 11, 12)
            Self.quarterRound(&working, 2, 7, 8, 13)
            Self.quarterRound(&working, 3, 4, 9, 14)
        }
        buffer = []
        buffer.reserveCapacity(64)
        for wordIndex in 0..<16 {
            let word = working[wordIndex] &+ state[wordIndex]
            buffer.append(UInt8(word & 0xff))
            buffer.append(UInt8((word >> 8) & 0xff))
            buffer.append(UInt8((word >> 16) & 0xff))
            buffer.append(UInt8((word >> 24) & 0xff))
        }
        index = 0
        counter &+= 1
    }

    private static func quarterRound(_ state: inout [UInt32],
                                     _ a: Int,
                                     _ b: Int,
                                     _ c: Int,
                                     _ d: Int) {
        state[a] = state[a] &+ state[b]
        state[d] = rotateLeft(state[d] ^ state[a], 16)
        state[c] = state[c] &+ state[d]
        state[b] = rotateLeft(state[b] ^ state[c], 12)
        state[a] = state[a] &+ state[b]
        state[d] = rotateLeft(state[d] ^ state[a], 8)
        state[c] = state[c] &+ state[d]
        state[b] = rotateLeft(state[b] ^ state[c], 7)
    }

    private static func rotateLeft(_ value: UInt32, _ amount: UInt32) -> UInt32 {
        (value << amount) | (value >> (32 - amount))
    }
}

