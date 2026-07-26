import Foundation

/// Deterministic exponential back-off policy for Connect transports.
///
/// Mirrors `iroha_torii_shared::connect_retry::policy` so the Swift SDK shares
/// the same retry envelope as Rust, Android, and JavaScript implementations.
public struct ConnectRetryPolicy: Sendable {
    public static let defaultBaseDelayMillis: UInt64 = 5_000
    public static let defaultMaxDelayMillis: UInt64 = 60_000

    private static let gamma: UInt64 = 0x9E37_79B9_7F4A_7C15
    private static let attemptMix: UInt64 = 0xD1B5_4A32_D192_ED03
    private static let seedInit: UInt64 = 0xA076_1D64_78BD_642F

    public let baseDelayMillis: UInt64
    public let maxDelayMillis: UInt64

    public init(baseDelayMillis: UInt64 = ConnectRetryPolicy.defaultBaseDelayMillis,
                maxDelayMillis: UInt64 = ConnectRetryPolicy.defaultMaxDelayMillis) {
        self.baseDelayMillis = baseDelayMillis
        self.maxDelayMillis = maxDelayMillis
    }

    /// Maximum (capped) delay for the provided attempt.
    public func capMillis(forAttempt attempt: UInt32) -> UInt64 {
        if baseDelayMillis == 0 {
            return 0
        }
        var cap = baseDelayMillis
        var remaining = attempt
        while remaining > 0 && cap < maxDelayMillis {
            let doubled = cap &<< 1
            if doubled <= cap || doubled > maxDelayMillis {
                cap = maxDelayMillis
                break
            }
            cap = doubled
            remaining -= 1
        }
        return min(cap, maxDelayMillis)
    }

    /// Deterministic delay in milliseconds for `attempt` given `seed`.
    public func delayMillis(forAttempt attempt: UInt32, seed: Data) -> UInt64 {
        let cap = capMillis(forAttempt: attempt)
        guard cap > 0 else { return 0 }
        let sample = ConnectRetryPolicy.deterministicSample(seed: seed, attempt: attempt)
        if cap == UInt64.max {
            return sample
        }
        let span = cap &+ 1
        return sample % span
    }

    /// Deterministic delay represented as seconds.
    public func delay(forAttempt attempt: UInt32, seed: Data) -> TimeInterval {
        TimeInterval(Double(delayMillis(forAttempt: attempt, seed: seed)) / 1000.0)
    }

    private static func deterministicSample(seed: Data, attempt: UInt32) -> UInt64 {
        let bytes = [UInt8](seed)
        var state = seedInit
        var index = 0
        while index < bytes.count {
            let end = min(index + 8, bytes.count)
            let chunk = bytes[index..<end]
            state = state &+ gamma
            let loaded = loadLittleEndian(chunk: chunk)
            state ^= loaded &* gamma
            state = splitmix64(state)
            index = end
        }
        state = state &+ gamma
        state ^= UInt64(attempt) &* attemptMix
        return splitmix64(state)
    }

    private static func loadLittleEndian(chunk: ArraySlice<UInt8>) -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        for byte in chunk {
            value |= UInt64(byte) << shift
            shift &+= 8
        }
        return value
    }

    private static func splitmix64(_ x: UInt64) -> UInt64 {
        var z = x
        z ^= z >> 30
        z &*= 0xBF58_476D_1CE4_E5B9
        z ^= z >> 27
        z &*= 0x94D0_49BB_1331_11EB
        z ^= z >> 31
        return z
    }
}
