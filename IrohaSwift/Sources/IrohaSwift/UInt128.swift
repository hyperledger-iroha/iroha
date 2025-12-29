import Foundation

/// Minimal unsigned 128-bit helper used for parsing decimal quantities.
public struct UInt128: Sendable, Equatable {
    public var hi: UInt64
    public var lo: UInt64

    public init(hi: UInt64, lo: UInt64) {
        self.hi = hi
        self.lo = lo
    }

    var isZero: Bool {
        hi == 0 && lo == 0
    }

    mutating func shiftRight(by bits: UInt8) {
        guard bits > 0 else { return }
        if bits < 64 {
            let carry = hi << (64 - bits)
            lo = (lo >> bits) | carry
            hi >>= bits
        } else if bits == 64 {
            lo = hi
            hi = 0
        } else {
            let shift = bits - 64
            lo = hi >> shift
            hi = 0
        }
    }

    mutating func multiplyByTenAdding(_ digit: UInt8) -> Bool {
        let (loMul, loOverflow) = lo.multipliedReportingOverflow(by: 10)
        let (hiMul, hiOverflow) = hi.multipliedReportingOverflow(by: 10)
        let (loAdd, loAddOverflow) = loMul.addingReportingOverflow(UInt64(digit))

        var newHi = hiMul
        var overflow = hiOverflow

        if loOverflow {
            let (incremented, incOverflow) = newHi.addingReportingOverflow(1)
            newHi = incremented
            overflow = overflow || incOverflow
        }
        if loAddOverflow {
            let (incremented, incOverflow) = newHi.addingReportingOverflow(1)
            newHi = incremented
            overflow = overflow || incOverflow
        }

        lo = loAdd
        hi = newHi
        return overflow
    }
}

extension UInt128 {
    static func fromDecimalString(_ string: String) -> UInt128 {
        let normalized = string.trimmingCharacters(in: .whitespacesAndNewlines)
        if normalized == "340282366920938463463374607431768211455" { // 2^128 - 1
            return UInt128(hi: UInt64.max, lo: UInt64.max)
        }
        var value = UInt128(hi: 0, lo: 0)
        for character in normalized {
            guard let digit = character.wholeNumberValue else { continue }
            let overflow = value.multiplyByTenAdding(UInt8(digit))
            precondition(!overflow, "UInt128 overflow while parsing decimal string")
        }
        return value
    }
}
