import XCTest
@testable import IrohaSwift

final class ConnectRetryPolicyTests: XCTestCase {
    func testCapSaturates() {
        let policy = ConnectRetryPolicy()
        XCTAssertEqual(policy.capMillis(forAttempt: 0), 5_000)
        XCTAssertEqual(policy.capMillis(forAttempt: 1), 10_000)
        XCTAssertEqual(policy.capMillis(forAttempt: 2), 20_000)
        XCTAssertEqual(policy.capMillis(forAttempt: 3), 40_000)
        XCTAssertEqual(policy.capMillis(forAttempt: 4), 60_000)
        XCTAssertEqual(policy.capMillis(forAttempt: 12), 60_000)
    }

    func testDelayHandlesMaxCap() {
        let policy = ConnectRetryPolicy(baseDelayMillis: 1, maxDelayMillis: UInt64.max)
        let cap = policy.capMillis(forAttempt: 64)
        XCTAssertEqual(cap, UInt64.max)

        let delay = policy.delayMillis(forAttempt: 64, seed: Data(repeating: 0, count: 32))
        XCTAssertLessThanOrEqual(delay, cap)
    }

    func testDeterministicSeriesZeroSeed() {
        let policy = ConnectRetryPolicy()
        let seed = Data(repeating: 0, count: 32)
        let expected: [UInt64] = [2_236, 4_203, 9_051, 15_827, 44_159, 3_907]
        for (attempt, exp) in expected.enumerated() {
            let value = policy.delayMillis(forAttempt: UInt32(attempt), seed: seed)
            XCTAssertEqual(value, exp, "attempt \(attempt)")
        }
    }

    func testDeterministicSeriesSequentialSeed() {
        let policy = ConnectRetryPolicy()
        let seed = Data((0..<32).map { UInt8($0) })
        let expected: [UInt64] = [4_133, 1_579, 16_071, 30_438, 7_169, 20_790]
        for (attempt, exp) in expected.enumerated() {
            let value = policy.delayMillis(forAttempt: UInt32(attempt), seed: seed)
            XCTAssertEqual(value, exp, "attempt \(attempt)")
        }
    }

    func testDelayNeverExceedsCap() {
        let policy = ConnectRetryPolicy()
        let seeds = [Data(repeating: 0, count: 32), Data((0..<32).map { UInt8($0) })]
        for attempt in 0..<12 {
            let cap = policy.capMillis(forAttempt: UInt32(attempt))
            for seed in seeds {
                let delay = policy.delayMillis(forAttempt: UInt32(attempt), seed: seed)
                XCTAssertLessThanOrEqual(delay, cap, "attempt \(attempt)")
            }
        }
    }
}
