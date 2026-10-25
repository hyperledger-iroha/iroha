import XCTest
@testable import IrohaSwift

final class KagemushaWalletFoldProgressV1Tests: XCTestCase {
  // Fold telemetry readings only; no owner, proof or readiness is involved.
  private struct Row {
    let started: UInt64, ended: UInt64, sequence: UInt64
    let operation: String?, stage: String?, ordinal: UInt64, outcome: String
  }
  private let second: UInt64 = 1_000_000

  private func row(_ started: UInt64, _ ended: UInt64, _ sequence: UInt64, _ operation: String?, _ stage: String?,
                   _ ordinal: UInt64, _ outcome: String) -> Row {
    Row(started: started * second, ended: ended * second, sequence: sequence, operation: operation, stage: stage,
        ordinal: ordinal, outcome: outcome)
  }

  private func name(_ value: String?) -> String { value.map { "\"\($0)\"" } ?? "null" }

  // The shape Native's FoldMetricsV1::to_json writes, including the fields a progress reading ignores.
  private func telemetry(_ sampled: UInt64, _ rows: [Row], schema: String = KagemushaWalletFoldProgressV1.schema) -> String {
    let records = rows.map {
      "{\"startedNativeMicros\":\($0.started),\"endedNativeMicros\":\($0.ended),\"sequence\":\($0.sequence),"
        + "\"operation\":\(name($0.operation)),\"stage\":\(name($0.stage)),\"ordinal\":\($0.ordinal),"
        + "\"outcome\":\"\($0.outcome)\",\"cause\":null,\"ingests\":0,\"peakRssBytes\":null,"
        + "\"micros\":{\"prove\":\(Int64($0.ended) - Int64($0.started))}}"
    }.joined(separator: ",")
    return "{\"schema\":\"\(schema)\",\"sampledNativeMicros\":\(sampled * second),\"counts\":{\"subProofs\":\(rows.count)},"
      + "\"processScratch\":{\"limitBytes\":1,\"inUseBytes\":0,\"peakBytes\":0},\"processPeakRssBytes\":null,"
      + "\"records\":[\(records)],\"nativeWorkCount\":0,\"nativeWork\":[]}"
  }

  private func progress(_ sampled: UInt64, _ rows: [Row]) throws -> KagemushaWalletFoldProgressV1 {
    try KagemushaWalletFoldProgressV1.parse(telemetry(sampled, rows))
  }

  private var loadThenReceive: [Row] {
    [row(0, 20, 1, "load", "q", 0, "checkpoint"),
     row(20, 40, 1, "load", "a", 1, "checkpoint"),
     row(40, 60, 1, "load", "omega", 2, "folded"),
     row(60, 90, 2, "receive", "q", 0, "checkpoint")]
  }

  func testLearnsTheHeadSchedulesAndPaceFromProvedTurns() throws {
    let reading = try progress(100, loadThenReceive)
    XCTAssertEqual(reading.sampledNativeMicros, 100 * second)
    XCTAssertEqual(reading.head, 2)
    XCTAssertEqual(reading.schedules, ["load": 3])
    XCTAssertEqual(reading.subProofMicros, 22_500_000)
    XCTAssertNil(try progress(10, []).head)
  }

  func testCountsTheCurrentHeadAndEveryHeadFoldedSinceTheWaitBegan() throws {
    let reading = try progress(100, loadThenReceive)
    let schedules = ["load": 3, "receive": 12]
    let whole = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 1, schedules: schedules))
    XCTAssertEqual(whole.proved, 4)
    XCTAssertEqual(whole.remaining, 11)
    XCTAssertEqual(whole.runningShare, 4.0 / 9.0, accuracy: 1e-9)
    XCTAssertEqual(whole.fraction, (4 + 4.0 / 9.0) / 15, accuracy: 1e-9)
    XCTAssertEqual(whole.remainingMicros, 237_500_000)
    // A wait that began after the Load folded counts the Receive head alone.
    let later = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 2, schedules: schedules))
    XCTAssertEqual(later.proved, 1)
    XCTAssertEqual(later.fraction, (1 + 4.0 / 9.0) / 12, accuracy: 1e-9)
    // A parked worker spends nothing on a sub-proof.
    let parked = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(parked.runningShare, 0)
    XCTAssertEqual(parked.fraction, 4.0 / 15, accuracy: 1e-9)
  }

  func testAWaitThatBeganMidHeadNeverDropsThatHeadsEarlierSubProofs() throws {
    let schedules = ["load": 3, "send": 5]
    let loading = [row(0, 20, 1, "load", "q", 0, "checkpoint"), row(20, 40, 1, "load", "a", 1, "checkpoint")]
    // The wait begins with two of the Load head's three sub-proofs done and a Send head behind it.
    let start = try XCTUnwrap(try progress(50, loading).estimate(backlog: 2, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(start.fraction, 0.25, accuracy: 1e-9)
    let sending = try progress(80, loading + [row(40, 60, 1, "load", "omega", 2, "folded"),
                                              row(60, 80, 2, "send", "q", 0, "checkpoint")])
    let after = try XCTUnwrap(sending.estimate(backlog: 1, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(after.proved, 4)
    XCTAssertEqual(after.fraction, 0.5, accuracy: 1e-9)
  }

  func testAFoldedHeadMovesToTheNextHeadAtTheLargestSchedule() throws {
    let reading = try progress(55, [
      row(0, 10, 5, "send", "q", 0, "checkpoint"),
      row(10, 20, 5, "send", "a", 1, "checkpoint"),
      row(20, 30, 5, "send", "w", 2, "checkpoint"),
      row(30, 40, 5, "send", "a", 3, "checkpoint"),
      row(40, 50, 5, "send", "omega", 4, "folded")])
    XCTAssertEqual(reading.schedules, ["send": 5])
    XCTAssertEqual(reading.head, 6)
    let estimate = try XCTUnwrap(reading.estimate(backlog: 2, running: false, firstHead: 5,
                                                  schedules: ["send": 5, "archive_sent": 20]))
    XCTAssertEqual(estimate.proved, 5)
    XCTAssertEqual(estimate.remaining, 40)
    XCTAssertEqual(estimate.fraction, 5.0 / 45, accuracy: 1e-9)
    XCTAssertEqual(estimate.remainingMicros, 400 * second)
  }

  func testACancelledTurnStillSaysWhereItsHeadStands() throws {
    let reading = try progress(80, [
      row(0, 20, 7, "receive", "q", 0, "checkpoint"),
      row(20, 40, 7, "receive", "q", 1, "checkpoint"),
      row(40, 60, 7, "receive", "a", 2, "checkpoint"),
      row(60, 70, 7, "receive", "w", 3, "cancelled"),
      // Cancelled before its released step was read: the head's earlier turns name it.
      row(70, 70, 7, nil, nil, 3, "cancelled")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 7, schedules: ["receive": 12]))
    XCTAssertEqual(estimate.proved, 3)
    XCTAssertEqual(estimate.remaining, 9)
    XCTAssertEqual(estimate.fraction, 0.25, accuracy: 1e-9)
  }

  func testAHeadPastItsKnownScheduleIsNeverCountedDone() throws {
    let reading = try progress(30, [row(0, 20, 9, "receive", "a", 12, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 9, schedules: ["receive": 12]))
    XCTAssertEqual(estimate.proved, 13)
    XCTAssertEqual(estimate.remaining, 1)
    XCTAssertEqual(estimate.fraction, 13.0 / 14, accuracy: 1e-9)
  }

  func testOnlyTheSnapshotCompletesAWait() throws {
    let reading = try progress(1000, [row(0, 10, 3, "x", "a", 998, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 3, schedules: ["x": 1000]))
    XCTAssertEqual(estimate.runningShare, KagemushaWalletFoldProgressV1.runningShareMaximum)
    XCTAssertEqual(estimate.fraction, KagemushaWalletFoldProgressV1.fractionMaximum)
  }

  func testAdoptedTurnsAreDurableButOnlyAnOmegaFoldsAHead() throws {
    let reading = try progress(50, [
      row(0, 1, 4, "load", nil, 0, "adopted"),
      row(1, 2, 4, "load", "omega", 2, "adopted"),
      row(2, 3, 5, "unload", nil, 1, "adopted")])
    // Adopted bytes were never proved here: they set no pace.
    XCTAssertNil(reading.subProofMicros)
    XCTAssertEqual(reading.schedules, ["load": 3])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 4,
                                                  schedules: ["load": 3, "unload": 3]))
    XCTAssertEqual(estimate.runningShare, 0)
    XCTAssertNil(estimate.remainingMicros)
    XCTAssertEqual(estimate.proved, 5) // load's three sub-proofs, from its Ω, and unload's first two
    XCTAssertEqual(estimate.remaining, 1)
  }

  func testNothingKnownIsNoEstimate() throws {
    XCTAssertNil(try progress(10, []).estimate(backlog: 1, running: true, firstHead: 1, schedules: ["load": 3]))
    let reading = try progress(30, [row(0, 20, 1, "load", "q", 0, "checkpoint")])
    XCTAssertNil(reading.estimate(backlog: 1, running: true, firstHead: 1, schedules: [:]))
    XCTAssertNil(reading.estimate(backlog: 1, running: true, firstHead: 1, schedules: ["load": 0]))
    XCTAssertNil(reading.estimate(backlog: 0, running: true, firstHead: 1, schedules: ["load": 3]))
  }

  func testManyLaterHeadsStayExact() throws {
    let reading = try progress(30, [row(0, 20, 1, "receive", "q", 0, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: .max, running: false, firstHead: 1, schedules: ["receive": 12]))
    XCTAssertGreaterThan(estimate.remaining, 0)
    XCTAssertTrue((0...KagemushaWalletFoldProgressV1.fractionMaximum).contains(estimate.fraction))
  }

  func testRefusesMalformedOrForeignTelemetry() {
    let good = row(0, 20, 1, "load", "q", 0, "checkpoint")
    let inputs = [
      "", "[]", "{}", telemetry(30, [good], schema: "iroha.kagemusha.fold-metrics.v1"),
      telemetry(10, [good]), // ended after the sample
      telemetry(30, [row(20, 10, 1, "load", "q", 0, "checkpoint")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 0, "finished")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 4_294_967_296, "checkpoint")]),
      telemetry(30, [good]).replacingOccurrences(of: "\"sequence\":1", with: "\"sequence\":\"1\""),
      telemetry(30, [good]).replacingOccurrences(of: "\"sequence\":1", with: "\"sequence\":-1"),
      telemetry(30, [good]).replacingOccurrences(of: "\"operation\":\"load\"", with: "\"operation\":7"),
      telemetry(30, [good]).replacingOccurrences(of: "\"records\":[", with: "\"records\":{\"x\":["),
      telemetry(30, [good]).replacingOccurrences(of: ",\"outcome\":\"checkpoint\"", with: ""),
    ]
    for input in inputs {
      XCTAssertThrowsError(try KagemushaWalletFoldProgressV1.parse(input), input) {
        XCTAssertEqual($0 as? KagemushaWalletErrorV1, .invalidNativeOutput)
      }
    }
  }
}
