import Foundation

// Petal Stream lane codec: xorshift32 whitening, Reed–Solomon over GF(2^8)
// and the cell layout of the five lanes. This file is a line-by-line port of
// `crates/iroha_petal/src/{prng,rs,lanes}.rs`; the shared golden vectors in
// `fixtures/petal/petal_stream_v2.json` pin every byte.

/// Failures of the Petal lane codec.
public enum PetalCodecError: Error, Equatable, LocalizedError, Sendable {
    /// A codeword length, parity count, atom size or erasure list is invalid.
    case invalidShape
    /// More errata than the Reed–Solomon code can correct.
    case uncorrectable

    public var errorDescription: String? {
        switch self {
        case .invalidShape: return "Invalid Petal Reed-Solomon codeword shape."
        case .uncorrectable: return "Petal Reed-Solomon word is uncorrectable."
        }
    }
}

/// Marsaglia xorshift32 with shifts 13, 17 and 5.
///
/// The generator is part of the wire format: whitening sequences are derived
/// from it, so every implementation must match bit for bit.
public struct PetalXorshift32: Equatable, Sendable {
    private var state: UInt32

    /// Creates a generator; a zero seed is replaced by `0xDEADBEEF` because
    /// xorshift cannot leave the all-zero state.
    public init(seed: UInt32) {
        state = seed == 0 ? 0xDEAD_BEEF : seed
    }

    /// Advances the generator and returns the next 32-bit word.
    public mutating func nextUInt32() -> UInt32 {
        var x = state
        x ^= x &<< 13
        x ^= x &>> 17
        x ^= x &<< 5
        state = x
        return x
    }

    /// Returns the top byte of the next word.
    public mutating func nextByte() -> UInt8 {
        UInt8(truncatingIfNeeded: nextUInt32() &>> 24)
    }
}

/// GF(2^8) arithmetic with the QR Code field polynomial `0x11D` and `α = 2`.
enum PetalGF256 {
    static let exponents: [UInt8] = tables.exponents
    static let logarithms: [UInt8] = tables.logarithms

    private static let tables: (exponents: [UInt8], logarithms: [UInt8]) = {
        var exponents = [UInt8](repeating: 0, count: 512)
        var logarithms = [UInt8](repeating: 0, count: 256)
        var x: UInt16 = 1
        for i in 0..<255 {
            exponents[i] = UInt8(truncatingIfNeeded: x)
            logarithms[Int(x)] = UInt8(i)
            x <<= 1
            if x & 0x100 != 0 { x ^= 0x11D }
        }
        for j in 255..<512 { exponents[j] = exponents[j - 255] }
        return (exponents, logarithms)
    }()

    @inline(__always)
    static func mul(_ a: UInt8, _ b: UInt8) -> UInt8 {
        if a == 0 || b == 0 { return 0 }
        return exponents[Int(logarithms[Int(a)]) + Int(logarithms[Int(b)])]
    }

    /// Divides `a` by a non-zero `b`.
    @inline(__always)
    static func div(_ a: UInt8, _ b: UInt8) -> UInt8 {
        if a == 0 { return 0 }
        return exponents[Int(logarithms[Int(a)]) + 255 - Int(logarithms[Int(b)])]
    }

    /// Returns `α^exponent` for a non-negative exponent.
    @inline(__always)
    static func exp(_ exponent: Int) -> UInt8 {
        exponents[exponent % 255]
    }

    /// Multiplicative inverse of a non-zero element.
    @inline(__always)
    static func inv(_ a: UInt8) -> UInt8 {
        exponents[255 - Int(logarithms[Int(a)])]
    }

    /// Multiplies two polynomials stored lowest-degree first.
    static func polyMul(_ a: [UInt8], _ b: [UInt8]) -> [UInt8] {
        guard !a.isEmpty, !b.isEmpty else { return [] }
        var out = [UInt8](repeating: 0, count: a.count + b.count - 1)
        for (i, x) in a.enumerated() where x != 0 {
            for (j, y) in b.enumerated() {
                out[i + j] ^= mul(x, y)
            }
        }
        return out
    }

    /// Evaluates a lowest-degree-first polynomial at `x` (Horner).
    static func polyEval(_ poly: [UInt8], _ x: UInt8) -> UInt8 {
        var acc: UInt8 = 0
        for coefficient in poly.reversed() {
            acc = mul(acc, x) ^ coefficient
        }
        return acc
    }
}

/// A Reed–Solomon code over GF(2^8) with a fixed number of parity bytes.
///
/// The field uses `x^8 + x^4 + x^3 + x^2 + 1` (`0x11D`) with `α = 2`. A
/// codeword is `data || parity`, systematic, and the generator is
/// `(x - α^0)(x - α^1)…(x - α^(nsym-1))`. The first byte of a codeword is the
/// highest-degree coefficient. Low-confidence bytes may be passed to
/// ``decode(_:erasures:)`` as erasures, which cost one parity byte each instead
/// of two.
public struct PetalReedSolomon: Equatable, Sendable {
    /// Number of parity bytes.
    public let parityLength: Int
    /// Highest-degree-first monic generator polynomial.
    let generator: [UInt8]

    /// Creates a code with `parityLength` parity bytes (1–254).
    public init(parityLength: Int) throws {
        guard (1...254).contains(parityLength) else { throw PetalCodecError.invalidShape }
        self.init(validatedParityLength: parityLength)
    }

    init(validatedParityLength nsym: Int) {
        var generator: [UInt8] = [1]
        for i in 0..<nsym {
            let root = PetalGF256.exp(i)
            var next = [UInt8](repeating: 0, count: generator.count + 1)
            for (k, coefficient) in generator.enumerated() {
                next[k] ^= coefficient
                next[k + 1] ^= PetalGF256.mul(coefficient, root)
            }
            generator = next
        }
        self.parityLength = nsym
        self.generator = generator
    }

    /// Encodes `data`, returning `data || parity`.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` when the codeword would
    ///   exceed 255 bytes.
    public func encode(_ data: [UInt8]) throws -> [UInt8] {
        guard data.count + parityLength <= 255 else { throw PetalCodecError.invalidShape }
        return encodeValidated(data)
    }

    func encodeValidated(_ data: [UInt8]) -> [UInt8] {
        let nsym = parityLength
        var remainder = [UInt8](repeating: 0, count: nsym)
        // LFSR division by the generator; `exp[log a + log b]` is `a * b` for
        // non-zero operands.
        PetalGF256.exponents.withUnsafeBufferPointer { exp in
            PetalGF256.logarithms.withUnsafeBufferPointer { log in
                generator.withUnsafeBufferPointer { generator in
                    remainder.withUnsafeMutableBufferPointer { remainder in
                        for byte in data {
                            let feedback = byte ^ remainder[0]
                            let feedbackLog = Int(log[Int(feedback)])
                            for j in 0..<nsym {
                                let next: UInt8 = j + 1 < nsym ? remainder[j + 1] : 0
                                let coefficient = generator[j + 1]
                                let product: UInt8 = feedback == 0 || coefficient == 0
                                    ? 0
                                    : exp[feedbackLog + Int(log[Int(coefficient)])]
                                remainder[j] = next ^ product
                            }
                        }
                    }
                }
            }
        }
        return data + remainder
    }

    func syndromes(_ word: [UInt8]) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: parityLength)
        // S_j = word(α^j) by Horner; log α^j = j for j < 255.
        PetalGF256.exponents.withUnsafeBufferPointer { exp in
            PetalGF256.logarithms.withUnsafeBufferPointer { log in
                word.withUnsafeBufferPointer { word in
                    for j in 0..<parityLength {
                        let rootLog = j % 255
                        var acc: UInt8 = 0
                        for byte in word {
                            acc = (acc == 0 ? 0 : exp[Int(log[Int(acc)]) + rootLog]) ^ byte
                        }
                        out[j] = acc
                    }
                }
            }
        }
        return out
    }

    /// Corrects `word` in place, treating `erasures` as known-bad positions.
    ///
    /// Succeeds when `2 * errors + erasures <= parityLength`. The corrected
    /// word is re-checked against zero syndromes before it is written back,
    /// so a success always yields a valid codeword.
    ///
    /// - Returns: The number of corrected positions.
    /// - Throws: ``PetalCodecError/invalidShape`` for malformed arguments and
    ///   ``PetalCodecError/uncorrectable`` when the word cannot be decoded.
    @discardableResult
    public func decode(_ word: inout [UInt8], erasures: [Int] = []) throws -> Int {
        let nsym = parityLength
        let n = word.count
        guard n > nsym, n <= 255, erasures.count <= nsym else {
            throw PetalCodecError.invalidShape
        }
        var seen = [Bool](repeating: false, count: 255)
        for position in erasures {
            guard position >= 0, position < n, !seen[position] else {
                throw PetalCodecError.invalidShape
            }
            seen[position] = true
        }
        let syndromes = syndromes(word)
        if syndromes.allSatisfy({ $0 == 0 }) { return 0 }
        let f = erasures.count
        // Erasure locator Γ(x) = Π (1 + X_e x), lowest degree first.
        var gamma: [UInt8] = [1]
        for position in erasures {
            let x = PetalGF256.exp(n - 1 - position)
            gamma = PetalGF256.polyMul(gamma, [1, x])
        }
        // Forney syndromes: the coefficients of S(x)Γ(x) from index f upward
        // are the syndromes of the error-only word.
        let forney = Array(PetalGF256.polyMul(syndromes, gamma).prefix(nsym))
        let lambda = Self.berlekampMassey(Array(forney[f...]))
        let errorCount = lambda.count - 1
        guard 2 * errorCount + f <= nsym else { throw PetalCodecError.uncorrectable }
        let psi = PetalGF256.polyMul(lambda, gamma)
        let degree = psi.count - 1
        // Chien search over all positions.
        var positions: [Int] = []
        positions.reserveCapacity(degree)
        for i in 0..<n {
            let xInverse = PetalGF256.exp(255 - ((n - 1 - i) % 255))
            if PetalGF256.polyEval(psi, xInverse) == 0 { positions.append(i) }
        }
        guard positions.count == degree else { throw PetalCodecError.uncorrectable }
        // Ω(x) = S(x)Ψ(x) mod x^nsym.
        let omega = Array(PetalGF256.polyMul(syndromes, psi).prefix(nsym))
        // The formal derivative of Ψ in characteristic 2 keeps odd-degree terms.
        var derivative: [UInt8] = []
        derivative.reserveCapacity(max(0, psi.count - 1))
        for k in 1..<max(1, psi.count) {
            derivative.append(k % 2 == 1 ? psi[k] : 0)
        }
        var corrected = word
        for i in positions {
            let x = PetalGF256.exp(n - 1 - i)
            let xInverse = PetalGF256.inv(x)
            let numerator = PetalGF256.polyEval(omega, xInverse)
            let denominator = PetalGF256.polyEval(derivative, xInverse)
            guard denominator != 0 else { throw PetalCodecError.uncorrectable }
            corrected[i] ^= PetalGF256.mul(x, PetalGF256.div(numerator, denominator))
        }
        guard self.syndromes(corrected).allSatisfy({ $0 == 0 }) else {
            throw PetalCodecError.uncorrectable
        }
        word = corrected
        return positions.count
    }

    /// Berlekamp–Massey over GF(256); returns the lowest-degree-first locator.
    static func berlekampMassey(_ syndromes: [UInt8]) -> [UInt8] {
        let n = syndromes.count
        var c = [UInt8](repeating: 0, count: n + 1)
        var b = [UInt8](repeating: 0, count: n + 1)
        c[0] = 1
        b[0] = 1
        var l = 0
        var m = 1
        var previousDiscrepancy: UInt8 = 1
        for i in 0..<n {
            var d = syndromes[i]
            if l >= 1 {
                for j in 1...l {
                    d ^= PetalGF256.mul(c[j], syndromes[i - j])
                }
            }
            if d == 0 {
                m += 1
                continue
            }
            let scale = PetalGF256.div(d, previousDiscrepancy)
            let span = max(0, n + 1 - m)
            if 2 * l <= i {
                let snapshot = c
                for j in 0..<span {
                    c[j + m] ^= PetalGF256.mul(scale, b[j])
                }
                l = i + 1 - l
                b = snapshot
                previousDiscrepancy = d
                m = 1
            } else {
                for j in 0..<span {
                    c[j + m] ^= PetalGF256.mul(scale, b[j])
                }
                m += 1
            }
        }
        return Array(c.prefix(l + 1))
    }
}

/// One of the five data lanes of a Petal frame.
///
/// Each lane is exactly one Reed–Solomon codeword, XOR-whitened with a fixed
/// xorshift32 sequence so the picture is statistically balanced whatever the
/// payload is:
///
/// | lane | cells | codeword | data | parity |
/// |------|-------|----------|------|--------|
/// | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
/// | `F0`–`F3` petals | 840 field cells of one sector × 1 bit | 105 B | 83 B | 22 B |
///
/// Bit order is most-significant-bit first.
public enum PetalLane: Hashable, Sendable {
    /// Dots on the three rings.
    case d
    /// The petals of field sector `0..<4`.
    case field(Int)

    /// All lanes in decode order (`D`, then the field sectors `0`–`3`).
    public static let decodeOrder: [PetalLane] = [.d, .field(0), .field(1), .field(2), .field(3)]
    /// The four field lanes, sector 0 first.
    public static let fields: [PetalLane] = [.field(0), .field(1), .field(2), .field(3)]
    /// Length of a fountain atom in bytes.
    public static let atomLength = 16
    /// Bytes of the per-lane header (`tag`, `frame` high, `frame` low).
    public static let headerLength = 3
    /// Most atoms one frame can carry (the four field lanes and lane `D`).
    public static let atomsPerFrame = 21
    /// Field cells of a sector that carry no bit (they stay dark).
    public static let spareSectorCells = PetalLayout.sectorCellCount - 105 * 8

    /// Codeword length in bytes.
    public var wordLength: Int {
        switch self {
        case .d: return PetalLayout.dataBits / 8
        case .field: return 105
        }
    }

    /// Parity bytes.
    public var parityLength: Int {
        switch self {
        case .d: return 11
        case .field: return 22
        }
    }

    /// Data bytes (header plus atoms or beacon).
    public var dataLength: Int { wordLength - parityLength }

    /// Atoms the lane carries; lane `D` carries one only on non-beacon frames.
    public var atomCount: Int {
        switch self {
        case .d: return 1
        case .field: return 5
        }
    }

    /// The field sector of a field lane, `nil` for lane `D`.
    public var sector: Int? {
        if case .field(let sector) = self { return sector }
        return nil
    }

    /// The letter that names the lane in scan reports: `D`, or the sector
    /// digit `0`–`3`.
    public var letter: Character {
        switch self {
        case .d: return "D"
        case .field(let sector): return Character(Unicode.Scalar(UInt8(truncatingIfNeeded: 48 + sector)))
        }
    }

    /// The fixed whitening sequence of the lane.
    public var whitening: [UInt8] {
        switch self {
        case .d: return Self.whiteningD
        case .field(let sector):
            return (0..<Self.whiteningFields.count).contains(sector) ? Self.whiteningFields[sector] : makeWhitening()
        }
    }

    var code: PetalReedSolomon {
        switch self {
        case .d: return Self.codeD
        case .field: return Self.codeField
        }
    }

    private var whiteningSeed: UInt32 {
        switch self {
        case .d: return 0x444F_5453 // "DOTS"
        case .field(let sector): return 0x5045_5430 &+ UInt32(truncatingIfNeeded: sector) // "PET0" … "PET3"
        }
    }

    private static let whiteningD = PetalLane.d.makeWhitening()
    private static let whiteningFields = fields.map { $0.makeWhitening() }
    private static let codeD = PetalReedSolomon(validatedParityLength: PetalLane.d.parityLength)
    private static let codeField = PetalReedSolomon(validatedParityLength: PetalLane.field(0).parityLength)

    private func makeWhitening() -> [UInt8] {
        var rng = PetalXorshift32(seed: whiteningSeed)
        return (0..<wordLength).map { _ in rng.nextByte() }
    }

    /// Encodes lane data into the transmitted (whitened) codeword.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` unless `data` is exactly
    ///   ``dataLength`` bytes.
    public func encode(_ data: [UInt8]) throws -> [UInt8] {
        guard data.count == dataLength else { throw PetalCodecError.invalidShape }
        return encodeValidated(data)
    }

    func encodeValidated(_ data: [UInt8]) -> [UInt8] {
        var word = code.encodeValidated(data)
        let mask = whitening
        for index in word.indices { word[index] ^= mask[index] }
        return word
    }

    /// Decodes a transmitted codeword, returning the lane data bytes.
    ///
    /// `erasures` lists byte positions the caller distrusts.
    ///
    /// - Throws: ``PetalCodecError`` when the word has the wrong length or is
    ///   uncorrectable.
    public func decode(_ transmitted: [UInt8], erasures: [Int] = []) throws -> [UInt8] {
        try decodeCounted(transmitted, erasures: erasures).data
    }

    /// Like ``decode(_:erasures:)``, also returning how many byte positions
    /// the Reed–Solomon decoder rewrote (erased bytes plus unflagged errors).
    ///
    /// - Throws: ``PetalCodecError`` when the word has the wrong length or is
    ///   uncorrectable.
    public func decodeCounted(
        _ transmitted: [UInt8],
        erasures: [Int] = []
    ) throws -> (data: [UInt8], corrected: Int) {
        guard transmitted.count == wordLength else { throw PetalCodecError.invalidShape }
        let mask = whitening
        var word = transmitted
        for index in word.indices { word[index] ^= mask[index] }
        let corrected = try code.decode(&word, erasures: erasures)
        return (Array(word.prefix(dataLength)), corrected)
    }
}

/// Every cell of one frame: what a renderer draws and a decoder samples.
public struct PetalFrameCells: Equatable, Sendable {
    /// Lit state of all 296 ring slots, gate dots included.
    public let dots: [Bool]
    /// Lit state of all 3,376 field cells (spare cells are dark).
    public let petals: [Bool]

    /// Builds the cells from the transmitted codewords of lane `D` and the
    /// four field lanes (sector 0 first).
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` when a codeword has the
    ///   wrong length or there are not four field codewords.
    public init(d: [UInt8], fields: [[UInt8]]) throws {
        guard d.count == PetalLane.d.wordLength,
              fields.count == PetalLayout.sectors,
              fields.allSatisfy({ $0.count == PetalLane.field(0).wordLength }) else {
            throw PetalCodecError.invalidShape
        }
        self.init(validatedD: d, fields: fields)
    }

    init(validatedD d: [UInt8], fields: [[UInt8]]) {
        var dots = [Bool](repeating: false, count: PetalLayout.totalSlots)
        for (slot, role) in PetalLayout.slotRoles.enumerated() {
            switch role {
            case .gate:
                dots[slot] = true
            case .data(let bit):
                let bit = Int(bit)
                dots[slot] = (d[bit / 8] >> (7 - bit % 8)) & 1 == 1
            case .guard, .spare:
                dots[slot] = false
            }
        }
        var petals = [Bool](repeating: false, count: PetalLayout.fieldCells)
        for (sector, word) in fields.enumerated() {
            let cells = PetalLayout.sectorTable[sector]
            for bit in 0..<(word.count * 8) {
                petals[cells[bit]] = (word[bit / 8] >> (7 - bit % 8)) & 1 == 1
            }
        }
        self.dots = dots
        self.petals = petals
    }

    /// Packs the data dots into a lane `D` codeword.
    public var dWord: [UInt8] {
        var word = [UInt8](repeating: 0, count: PetalLane.d.wordLength)
        for (bit, slot) in PetalLayout.dataSlots.enumerated() where dots[slot] {
            word[bit / 8] |= 1 << (7 - bit % 8)
        }
        return word
    }

    /// Packs the petals of sector `sector` (`0..<4`) into its field codeword.
    public func fieldWord(sector: Int) -> [UInt8] {
        let length = PetalLane.field(sector).wordLength
        var word = [UInt8](repeating: 0, count: length)
        let cells = PetalLayout.sectorTable[sector]
        for bit in 0..<(length * 8) where petals[cells[bit]] {
            word[bit / 8] |= 1 << (7 - bit % 8)
        }
        return word
    }
}

/// Float helpers that reproduce the Rust reference semantics exactly.
enum PetalNumeric {
    /// Key whose signed integer order is the IEEE 754 `totalOrder` of
    /// doubles (Rust `f64::total_cmp`).
    @inline(__always)
    static func totalOrderKey(_ value: Double) -> Int64 {
        var bits = Int64(bitPattern: value.bitPattern)
        bits ^= Int64(bitPattern: UInt64(bitPattern: bits >> 63) >> 1)
        return bits
    }

    /// Rust `f64::clamp`: NaN stays NaN.
    @inline(__always)
    static func clamp(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
        var value = value
        if value < lower { value = lower }
        if value > upper { value = upper }
        return value
    }

    /// Sine and cosine of one angle, computed as the reference computes such a
    /// pair: its optimiser fuses a sine and a cosine of one value into a single
    /// `__sincos_stret` call on Apple platforms, which differs from separate
    /// `sin` and `cos` calls in the last bit for a few angles (enough to move
    /// two field cell centres and four ring slot centres by an ulp). Elsewhere
    /// `sincos` agrees with `sin` and `cos`.
    @inline(__always)
    static func sinCos(_ angle: Double) -> (sin: Double, cos: Double) {
        #if canImport(Darwin)
        let pair = __sincos_stret(angle)
        return (pair.__sinval, pair.__cosval)
        #else
        return (sin(angle), cos(angle))
        #endif
    }

    /// `sinCos` in single precision (`__sincosf_stret`).
    @inline(__always)
    static func sinCos(_ angle: Float) -> (sin: Float, cos: Float) {
        #if canImport(Darwin)
        let pair = __sincosf_stret(angle)
        return (pair.__sinval, pair.__cosval)
        #else
        return (sin(angle), cos(angle))
        #endif
    }

    /// Rust `f64 as isize`: saturating, NaN maps to zero.
    @inline(__always)
    static func saturatingInt(_ value: Double) -> Int {
        if value.isNaN { return 0 }
        if value >= 9.223372036854775807e18 { return Int.max }
        if value <= -9.223372036854775808e18 { return Int.min }
        return Int(value)
    }
}
