import Foundation

/// Rateless fountain code over GF(2) (port of `crates/iroha_petal/src/fountain.rs`).
///
/// A payload is cut into `k` source atoms of 16 bytes (the last one
/// zero-padded). Encoded atom `id` is source atom `id` for `id < k`
/// (systematic) and otherwise the XOR of a pseudo-random half of the source
/// atoms chosen by ``maskWords(sourceAtoms:crc:id:)``. A receiver holding any
/// `k + 2` or so independent atoms, in any order, recovers the payload by
/// Gaussian elimination; lost frames cost nothing but time.
public enum PetalFountain {
    /// Splits a payload into zero-padded 16-byte source atoms.
    public static func splitPayload(_ payload: [UInt8]) -> [[UInt8]] {
        stride(from: 0, to: payload.count, by: PetalLane.atomLength).map { start in
            var atom = [UInt8](repeating: 0, count: PetalLane.atomLength)
            let end = min(start + PetalLane.atomLength, payload.count)
            atom.replaceSubrange(0..<(end - start), with: payload[start..<end])
            return atom
        }
    }

    /// Number of 32-bit words needed for a mask over `k` source atoms.
    public static func maskLength(sourceAtoms k: Int) -> Int {
        (max(0, k) + 31) / 32
    }

    /// The 32-bit finalizer of MurmurHash3 (`fmix32`).
    ///
    /// Masks must not come from a GF(2)-linear generator such as xorshift:
    /// every mask would then lie in a subspace of dimension at most 32. The
    /// multiplications make this mixer nonlinear over GF(2).
    public static func mix32(_ value: UInt32) -> UInt32 {
        var x = value
        x ^= x &>> 16
        x = x &* 0x85EB_CA6B
        x ^= x &>> 13
        x = x &* 0xC2B2_AE35
        x ^= x &>> 16
        return x
    }

    /// The combination mask of encoded atom `id`, as little-endian bit words.
    ///
    /// `crc` is the payload CRC-32C and only diversifies masks between
    /// streams. Atoms with `id < k` are systematic (a unit vector); every
    /// other atom combines a pseudo-random half of the sources:
    ///
    /// ```text
    /// seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
    /// word[w] = mix32(seed + (w + 1) * 0x9E3779B9)      (all arithmetic mod 2^32)
    /// ```
    ///
    /// Bits at or above `k` are cleared, and an all-zero mask is replaced by
    /// the single bit `id mod k`. Returns an empty mask when `k` is not
    /// positive.
    public static func maskWords(sourceAtoms k: Int, crc: UInt32, id: UInt32) -> [UInt32] {
        guard k > 0 else { return [] }
        var mask = [UInt32](repeating: 0, count: maskLength(sourceAtoms: k))
        fillMask(&mask, sourceAtoms: k, crc: crc, id: id)
        return mask
    }

    /// Writes the mask of atom `id` into `mask` (``maskLength(sourceAtoms:)``
    /// words, `k > 0`).
    @inline(__always)
    static func fillMask(_ mask: inout [UInt32], sourceAtoms k: Int, crc: UInt32, id: UInt32) {
        if UInt64(id) < UInt64(k) {
            for index in mask.indices { mask[index] = 0 }
            mask[Int(id / 32)] = 1 &<< (id % 32)
            return
        }
        let seed = mix32((id &* 0x9E37_79B1) ^ crc ^ 0xA5A5_A5A5)
        for w in mask.indices {
            let step = (UInt32(truncatingIfNeeded: w) &+ 1) &* 0x9E37_79B9
            mask[w] = mix32(seed &+ step)
        }
        let tail = k % 32
        if tail != 0 {
            mask[mask.count - 1] &= (UInt32(1) &<< UInt32(tail)) &- 1
        }
        if mask.allSatisfy({ $0 == 0 }) {
            let bit = Int(id) % k
            mask[bit / 32] |= 1 &<< UInt32(bit % 32)
        }
    }

    /// Encodes atom `id` from 16-byte source atoms.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` when `source` is empty or an
    ///   atom is not 16 bytes.
    public static func encodeAtom(source: [[UInt8]], crc: UInt32, id: UInt32) throws -> [UInt8] {
        guard !source.isEmpty,
              source.allSatisfy({ $0.count == PetalLane.atomLength }) else {
            throw PetalCodecError.invalidShape
        }
        let flat = source.flatMap { $0 }
        var out = [UInt8](repeating: 0, count: PetalLane.atomLength)
        var mask = [UInt32](repeating: 0, count: maskLength(sourceAtoms: source.count))
        encodeAtom(flatSource: flat, sourceAtoms: source.count, crc: crc, id: id, mask: &mask, into: &out, at: 0)
        return out
    }

    /// Writes encoded atom `id` of the flat, zero-padded `flatSource` into
    /// `out[offset..<offset + 16]`.
    static func encodeAtom(
        flatSource: [UInt8],
        sourceAtoms k: Int,
        crc: UInt32,
        id: UInt32,
        mask: inout [UInt32],
        into out: inout [UInt8],
        at offset: Int
    ) {
        fillMask(&mask, sourceAtoms: k, crc: crc, id: id)
        let atomLength = PetalLane.atomLength
        mask.withUnsafeBufferPointer { maskWords in
            flatSource.withUnsafeBufferPointer { source in
                out.withUnsafeMutableBufferPointer { output in
                    for index in 0..<k where (maskWords[index / 32] >> UInt32(index % 32)) & 1 == 1 {
                        let base = index * atomLength
                        for byte in 0..<atomLength {
                            output[offset + byte] ^= source[base + byte]
                        }
                    }
                }
            }
        }
    }
}

/// Incremental Gaussian-elimination decoder of the Petal fountain code.
public struct PetalFountainDecoder: Sendable {
    /// Number of source atoms.
    public let sourceAtoms: Int
    private let maskLength: Int
    /// Row holding the pivot of each column, or `-1`.
    private var pivot: [Int]
    /// Row masks, `maskLength` words per row.
    private var rowMasks: [UInt32] = []
    /// Row data, 16 bytes per row.
    private var rowData: [UInt8] = []
    /// Number of linearly independent atoms received so far.
    public private(set) var rank = 0

    /// Creates a decoder for `sourceAtoms` source atoms.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` when `sourceAtoms` is not
    ///   positive.
    public init(sourceAtoms: Int) throws {
        guard sourceAtoms > 0 else { throw PetalCodecError.invalidShape }
        self.init(validatedSourceAtoms: sourceAtoms)
    }

    init(validatedSourceAtoms k: Int) {
        sourceAtoms = k
        maskLength = PetalFountain.maskLength(sourceAtoms: k)
        pivot = [Int](repeating: -1, count: k)
    }

    /// Whether enough independent atoms arrived to recover the payload.
    public var isComplete: Bool { rank == sourceAtoms }

    /// Adds encoded atom `id`; returns whether it increased the rank.
    @discardableResult
    public mutating func addEncoded(crc: UInt32, id: UInt32, atom: [UInt8]) -> Bool {
        var mask = [UInt32](repeating: 0, count: maskLength)
        PetalFountain.fillMask(&mask, sourceAtoms: sourceAtoms, crc: crc, id: id)
        return add(mask: mask, atom: atom)
    }

    /// Adds a received combination; returns whether it increased the rank.
    ///
    /// Masks of the wrong length and atoms that are not 16 bytes are ignored.
    @discardableResult
    public mutating func add(mask: [UInt32], atom: [UInt8]) -> Bool {
        let atomLength = PetalLane.atomLength
        guard mask.count == maskLength, atom.count == atomLength else { return false }
        var mask = mask
        var data = atom
        var word = 0
        while true {
            while word < mask.count && mask[word] == 0 { word += 1 }
            if word == mask.count { return false }
            let column = word * 32 + mask[word].trailingZeroBitCount
            if column >= sourceAtoms { return false }
            let row = pivot[column]
            if row >= 0 {
                let maskBase = row * maskLength
                rowMasks.withUnsafeBufferPointer { masks in
                    for index in word..<maskLength { mask[index] ^= masks[maskBase + index] }
                }
                let dataBase = row * atomLength
                rowData.withUnsafeBufferPointer { rows in
                    for index in 0..<atomLength { data[index] ^= rows[dataBase + index] }
                }
            } else {
                pivot[column] = rank
                rowMasks.append(contentsOf: mask)
                rowData.append(contentsOf: data)
                rank += 1
                return true
            }
        }
    }

    /// Returns the source atoms once the decoder is complete.
    public func solve() -> [[UInt8]]? {
        guard isComplete else { return nil }
        let atomLength = PetalLane.atomLength
        var solution = [UInt8](repeating: 0, count: sourceAtoms * atomLength)
        for column in stride(from: sourceAtoms - 1, through: 0, by: -1) {
            let row = pivot[column]
            guard row >= 0 else { return nil }
            var value = Array(rowData[(row * atomLength)..<((row + 1) * atomLength)])
            let firstWord = column / 32
            for word in firstWord..<maskLength {
                var bits = rowMasks[row * maskLength + word]
                if word == firstWord {
                    // keep only columns strictly above the pivot
                    let shift = column % 32 + 1
                    bits = shift >= 32 ? 0 : (bits >> UInt32(shift)) << UInt32(shift)
                }
                while bits != 0 {
                    let bit = bits.trailingZeroBitCount
                    bits &= bits &- 1
                    let other = (word * 32 + bit) * atomLength
                    for index in 0..<atomLength { value[index] ^= solution[other + index] }
                }
            }
            solution.replaceSubrange((column * atomLength)..<((column + 1) * atomLength), with: value)
        }
        return (0..<sourceAtoms).map {
            Array(solution[($0 * atomLength)..<(($0 + 1) * atomLength)])
        }
    }

    /// Solves and concatenates the source atoms, truncated to `length` bytes.
    func solvedPayload(length: Int) -> [UInt8]? {
        guard let atoms = solve() else { return nil }
        var payload = atoms.flatMap { $0 }
        if payload.count > length { payload.removeSubrange(length...) }
        return payload
    }
}
