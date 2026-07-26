import Foundation

/// An exact positive offline-cash amount expressed in the asset's atomic units.
///
/// Kagemusha proofs use an unsigned 128-bit integer amount, while public asset
/// balances use Iroha `Quantity` values. Keeping the asset scale beside the
/// atomic value prevents callers from accidentally charging or minting the
/// atomic integer as a scale-zero public amount.
public struct KagemushaScaledAmount: Equatable, Hashable, Sendable {
    public static let maximumScale: UInt32 = 28
    public static let maximumAtomicUnits = "340282366920938463463374607431768211455"

    /// Canonical positive `u128` decimal without leading zeroes.
    public let atomicUnits: String
    /// Authoritative scale from the on-chain asset definition.
    public let scale: UInt32

    /// Builds an amount from canonical atomic units and an asset scale.
    public init(atomicUnits: String, scale: UInt32) throws {
        guard scale <= Self.maximumScale else {
            throw KagemushaScaledAmountError.scaleTooLarge
        }
        guard Self.isCanonicalPositiveInteger(atomicUnits) else {
            throw KagemushaScaledAmountError.invalidAtomicUnits
        }
        guard Self.fitsU128(atomicUnits) else {
            throw KagemushaScaledAmountError.atomicUnitsOverflow
        }
        self.atomicUnits = atomicUnits
        self.scale = scale
    }

    /// Converts a canonical decimal amount to atomic units exactly.
    ///
    /// The conversion never rounds. A fractional component wider than the
    /// asset scale is rejected, including extra trailing zeroes.
    public init(decimal: String, scale: UInt32) throws {
        guard scale <= Self.maximumScale else {
            throw KagemushaScaledAmountError.scaleTooLarge
        }
        let components = decimal.split(separator: ".", omittingEmptySubsequences: false)
        guard components.count <= 2,
              let integerPart = components.first,
              !integerPart.isEmpty,
              integerPart.allSatisfy(\.isASCIIWholeNumber),
              integerPart == "0" || integerPart.first != "0"
        else {
            throw KagemushaScaledAmountError.invalidDecimal
        }
        let fractionalPart = components.count == 2 ? components[1] : Substring()
        guard components.count == 1 || !fractionalPart.isEmpty,
              fractionalPart.allSatisfy(\.isASCIIWholeNumber)
        else {
            throw KagemushaScaledAmountError.invalidDecimal
        }
        guard fractionalPart.count <= Int(scale) else {
            throw KagemushaScaledAmountError.excessPrecision
        }

        let paddedFraction = String(fractionalPart)
            + String(repeating: "0", count: Int(scale) - fractionalPart.count)
        let combined = Self.strippingLeadingZeroes(String(integerPart) + paddedFraction)
        try self.init(atomicUnits: combined, scale: scale)
    }

    /// Exact fixed-scale decimal at the authoritative asset scale.
    ///
    /// For example, `atomicUnits=10750000000, scale=9` is
    /// `10.750000000`, never the scale-zero value `10750000000`.
    ///
    /// This fixed-scale projection is proof-side evidence; use
    /// ``displayDecimal`` for the canonical public `Quantity` spelling.
    public var fixedScaleDecimal: String {
        guard scale > 0 else { return atomicUnits }
        var digits = atomicUnits
        let requiredDigits = Int(scale) + 1
        if digits.count < requiredDigits {
            digits = String(repeating: "0", count: requiredDigits - digits.count) + digits
        }
        let splitIndex = digits.index(digits.endIndex, offsetBy: -Int(scale))
        return String(digits[..<splitIndex]) + "." + String(digits[splitIndex...])
    }

    /// Canonical public `Quantity` spelling without insignificant zeroes.
    public var displayDecimal: String {
        guard scale > 0 else { return atomicUnits }
        var value = fixedScaleDecimal
        while value.last == "0" { value.removeLast() }
        if value.last == "." { value.removeLast() }
        return value
    }

    /// Returns the exact sum of two amounts at the same authoritative asset scale.
    ///
    /// This operation never rescales or rounds. A scale mismatch and a sum that
    /// exceeds the Kagemusha `u128` amount domain are rejected.
    public func adding(_ other: Self) throws -> Self {
        guard scale == other.scale else {
            throw KagemushaScaledAmountError.scaleMismatch(
                expected: scale,
                actual: other.scale
            )
        }
        return try Self(
            atomicUnits: Self.addAtomicUnits(atomicUnits, other.atomicUnits),
            scale: scale
        )
    }

    /// Returns the exact sum of a non-empty sequence of same-scale amounts.
    ///
    /// The first amount establishes the authoritative scale. Empty sequences,
    /// mixed scales, and `u128` overflow are rejected.
    public static func sum<S: Sequence>(_ amounts: S) throws -> Self
    where S.Element == Self {
        var iterator = amounts.makeIterator()
        guard var total = iterator.next() else {
            throw KagemushaScaledAmountError.emptyAmountSequence
        }
        while let amount = iterator.next() {
            total = try total.adding(amount)
        }
        return total
    }

    private static func isCanonicalPositiveInteger(_ value: String) -> Bool {
        !value.isEmpty
            && value.allSatisfy(\.isASCIIWholeNumber)
            && value != "0"
            && (value.count == 1 || value.first != "0")
    }

    private static func fitsU128(_ value: String) -> Bool {
        value.count < maximumAtomicUnits.count
            || (value.count == maximumAtomicUnits.count && value <= maximumAtomicUnits)
    }

    private static func addAtomicUnits(_ lhs: String, _ rhs: String) -> String {
        let left = Array(lhs.utf8.reversed())
        let right = Array(rhs.utf8.reversed())
        var carry = 0
        var result: [UInt8] = []
        result.reserveCapacity(max(left.count, right.count) + 1)
        for index in 0..<max(left.count, right.count) {
            let leftDigit = index < left.count ? Int(left[index] - 48) : 0
            let rightDigit = index < right.count ? Int(right[index] - 48) : 0
            let sum = leftDigit + rightDigit + carry
            result.append(UInt8(sum % 10) + 48)
            carry = sum / 10
        }
        if carry > 0 {
            result.append(UInt8(carry) + 48)
        }
        return String(decoding: result.reversed(), as: UTF8.self)
    }

    static func compareAtomicUnits(_ lhs: String, _ rhs: String) -> ComparisonResult {
        if lhs.count != rhs.count {
            return lhs.count < rhs.count ? .orderedAscending : .orderedDescending
        }
        if lhs == rhs { return .orderedSame }
        return lhs < rhs ? .orderedAscending : .orderedDescending
    }

    static func subtractAtomicUnits(_ subtrahend: String, from minuend: String) -> String? {
        guard compareAtomicUnits(minuend, subtrahend) == .orderedDescending else {
            return nil
        }
        let lhs = Array(minuend.utf8.reversed())
        let rhs = Array(subtrahend.utf8.reversed())
        var borrow = 0
        var result: [UInt8] = []
        result.reserveCapacity(lhs.count)
        for index in lhs.indices {
            var digit = Int(lhs[index] - 48) - borrow
            if index < rhs.count {
                digit -= Int(rhs[index] - 48)
            }
            if digit < 0 {
                digit += 10
                borrow = 1
            } else {
                borrow = 0
            }
            result.append(UInt8(digit) + 48)
        }
        guard borrow == 0 else { return nil }
        while result.count > 1, result.last == 48 {
            result.removeLast()
        }
        let difference = String(decoding: result.reversed(), as: UTF8.self)
        return difference == "0" ? nil : difference
    }

    private static func strippingLeadingZeroes(_ value: String) -> String {
        let stripped = value.drop(while: { $0 == "0" })
        return stripped.isEmpty ? "0" : String(stripped)
    }
}

public enum KagemushaScaledAmountError: Error, Equatable, LocalizedError {
    case invalidDecimal
    case excessPrecision
    case invalidAtomicUnits
    case atomicUnitsOverflow
    case scaleTooLarge
    case scaleMismatch(expected: UInt32, actual: UInt32)
    case emptyAmountSequence

    public var errorDescription: String? {
        switch self {
        case .invalidDecimal:
            return "Kagemusha amount must be a canonical positive decimal."
        case .excessPrecision:
            return "Kagemusha amount has more fractional digits than the asset supports."
        case .invalidAtomicUnits:
            return "Kagemusha atomic amount must be a canonical positive integer."
        case .atomicUnitsOverflow:
            return "Kagemusha atomic amount does not fit in u128."
        case .scaleTooLarge:
            return "Kagemusha asset scale exceeds Iroha Quantity's supported range."
        case let .scaleMismatch(expected, actual):
            return "Kagemusha amount scales must match; expected \(expected), got \(actual)."
        case .emptyAmountSequence:
            return "Kagemusha amount summation requires at least one amount."
        }
    }
}

private extension Character {
    var isASCIIWholeNumber: Bool {
        self >= "0" && self <= "9"
    }
}
