import XCTest
@testable import IrohaSwift

final class KagemushaWalletFoldProgressV1Tests: XCTestCase {
  // Fold telemetry readings only; no owner, proof or readiness is involved.
  private struct Row {
    let started: UInt64, ended: UInt64, sequence: UInt64
    let operation: String?, stage: String?, ordinal: UInt64, stages: UInt64?, outcome: String
  }
  private let second: UInt64 = 1_000_000

  private func row(_ started: UInt64, _ ended: UInt64, _ sequence: UInt64, _ operation: String?, _ stage: String?,
                   _ ordinal: UInt64, _ stages: UInt64?, _ outcome: String) -> Row {
    Row(started: started * second, ended: ended * second, sequence: sequence, operation: operation, stage: stage,
        ordinal: ordinal, stages: stages, outcome: outcome)
  }

  private func name(_ value: String?) -> String { value.map { "\"\($0)\"" } ?? "null" }

  // The shape Native's FoldMetricsV1::to_json writes, including the fields a progress reading ignores.
  private func telemetry(_ sampled: UInt64, _ rows: [Row], schema: String = KagemushaWalletFoldProgressV1.schema) -> String {
    let records = rows.map {
      "{\"startedNativeMicros\":\($0.started),\"endedNativeMicros\":\($0.ended),\"sequence\":\($0.sequence),"
        + "\"operation\":\(name($0.operation)),\"stage\":\(name($0.stage)),\"ordinal\":\($0.ordinal),"
        + "\"stages\":\($0.stages.map(String.init) ?? "null"),\"outcome\":\"\($0.outcome)\",\"cause\":null,\"ingests\":0,"
        + "\"peakRssBytes\":null,\"micros\":{\"prove\":\(Int64($0.ended) - Int64($0.started))}}"
    }.joined(separator: ",")
    return "{\"schema\":\"\(schema)\",\"sampledNativeMicros\":\(sampled * second),\"counts\":{\"subProofs\":\(rows.count)},"
      + "\"processScratch\":{\"limitBytes\":1,\"inUseBytes\":0,\"peakBytes\":0},\"processPeakRssBytes\":null,"
      + "\"records\":[\(records)],\"nativeWorkCount\":0,\"nativeWork\":[]}"
  }

  private func progress(_ sampled: UInt64, _ rows: [Row]) throws -> KagemushaWalletFoldProgressV1 {
    try KagemushaWalletFoldProgressV1.parse(telemetry(sampled, rows))
  }

  private var loadThenReceive: [Row] {
    [row(0, 20, 1, "load", "q", 0, 3, "checkpoint"),
     row(20, 40, 1, "load", "a", 1, 3, "checkpoint"),
     row(40, 60, 1, "load", "omega", 2, 3, "folded"),
     row(60, 90, 2, "receive", "q", 0, 12, "checkpoint")]
  }

  private func micros(_ seconds: Double) -> Double { seconds * Double(second) }

  func testLearnsTheHeadSchedulesAndCostsFromItsTurns() throws {
    let reading = try progress(100, loadThenReceive)
    XCTAssertEqual(reading.sampledNativeMicros, 100 * second)
    XCTAssertEqual(reading.head, 2)
    XCTAssertEqual(reading.schedules, ["load": 3, "receive": 12])
    XCTAssertEqual(reading.costs, ["load": [0: 20 * second, 1: 20 * second, 2: 20 * second], "receive": [0: 30 * second]])
    XCTAssertNil(try progress(10, []).head)
    XCTAssertEqual(try progress(10, []).costs, [:])
    // An ordinal proved twice costs its mean; adopted, cancelled and instant turns proved nothing here.
    let sends = try progress(100, [
      row(0, 10, 1, "send", "q", 0, 5, "checkpoint"),
      row(10, 30, 2, "send", "q", 0, 5, "checkpoint"),
      row(30, 31, 3, "send", "q", 0, 5, "adopted"),
      row(31, 40, 3, "send", "a", 1, 5, "cancelled"),
      row(40, 40, 3, "send", "a", 1, 5, "checkpoint")])
    XCTAssertEqual(sends.costs, ["send": [0: 15 * second]])
  }

  func testWeighsTheCurrentHeadAndEveryHeadFoldedSinceTheWaitBeganByTime() throws {
    let reading = try progress(100, loadThenReceive)
    // The Receive head's own turn says it needs twelve; no schedule passed in names receive.
    let schedules = ["load": 3]
    let whole = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 1, schedules: schedules))
    XCTAssertEqual(whole.proved, 4)
    XCTAssertEqual(whole.remaining, 11)
    XCTAssertTrue(whole.timed)
    // The Load's three at 20 s and the Receive's first at 30 s are done; its other eleven at its 30 s mean remain.
    XCTAssertEqual(whole.provedWeight, micros(90), accuracy: 1e-3)
    XCTAssertEqual(whole.remainingWeight, micros(330), accuracy: 1e-3)
    // The running sub-proof began when the last turn ended, 10 s before this reading.
    XCTAssertEqual(whole.runningWeight, micros(10), accuracy: 1e-3)
    XCTAssertEqual(whole.fraction, 100.0 / 420, accuracy: 1e-9)
    XCTAssertEqual(whole.remainingMicros, 320 * second)
    // A wait that began after the Load folded counts the Receive head alone.
    let later = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 2, schedules: schedules))
    XCTAssertEqual(later.proved, 1)
    XCTAssertEqual(later.fraction, 40.0 / 360, accuracy: 1e-9)
    // A parked worker spends nothing on a sub-proof.
    let parked = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(parked.runningWeight, 0)
    XCTAssertEqual(parked.fraction, 90.0 / 420, accuracy: 1e-9)
  }

  func testAWaitThatBeganMidHeadNeverDropsThatHeadsEarlierSubProofs() throws {
    let schedules = ["load": 3, "send": 5]
    let loading = [row(0, 20, 1, "load", "q", 0, 3, "checkpoint"), row(20, 40, 1, "load", "a", 1, 3, "checkpoint")]
    // The wait begins with two of the Load head's three sub-proofs done and a Send head behind it.
    let start = try XCTUnwrap(try progress(50, loading).estimate(backlog: 2, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(start.fraction, 0.25, accuracy: 1e-9)
    let sending = try progress(80, loading + [row(40, 60, 1, "load", "omega", 2, 3, "folded"),
                                              row(60, 80, 2, "send", "q", 0, 5, "checkpoint")])
    let after = try XCTUnwrap(sending.estimate(backlog: 1, running: false, firstHead: 1, schedules: schedules))
    XCTAssertEqual(after.proved, 4)
    XCTAssertEqual(after.fraction, 0.5, accuracy: 1e-9)
  }

  func testAFoldedHeadMovesToTheNextHeadAtTheLargestSchedule() throws {
    let reading = try progress(55, [
      row(0, 10, 5, "send", "q", 0, 5, "checkpoint"),
      row(10, 20, 5, "send", "a", 1, 5, "checkpoint"),
      row(20, 30, 5, "send", "w", 2, 5, "checkpoint"),
      row(30, 40, 5, "send", "a", 3, 5, "checkpoint"),
      row(40, 50, 5, "send", "omega", 4, 5, "folded")])
    XCTAssertEqual(reading.schedules, ["send": 5])
    XCTAssertEqual(reading.head, 6)
    let estimate = try XCTUnwrap(reading.estimate(backlog: 2, running: false, firstHead: 5,
                                                  schedules: ["send": 5, "archive_sent": 20]))
    XCTAssertEqual(estimate.proved, 5)
    XCTAssertEqual(estimate.remaining, 40)
    XCTAssertEqual(estimate.fraction, 5.0 / 45, accuracy: 1e-9)
    XCTAssertEqual(estimate.remainingMicros, 400 * second)
  }

  func testAnOperationTheCallerNamesSizesAndWeighsAHeadBeforeItsFirstTurn() throws {
    // This phone's earlier folds: a Load is three sub-proofs of 24, 48 and 49 s, a Receive twelve.
    let schedules = ["load": 3, "receive": 12]
    let costs: [String: [UInt32: UInt64]] = ["load": [0: 24 * second, 1: 48 * second, 2: 49 * second],
                                             "receive": [0: 30 * second]]
    // A new process: the app's Load released head 3 and its first reading at 5 s finds no turn yet.
    let operations: [UInt64: String] = [3: "load"]
    let first = try XCTUnwrap(try progress(5, []).estimate(backlog: 1, running: true, firstHead: 3, schedules: schedules,
                                                           costs: costs, operations: operations, since: 5 * second))
    XCTAssertEqual(first.proved, 0)
    XCTAssertEqual(first.remaining, 3)
    XCTAssertEqual(first.fraction, 0)
    XCTAssertEqual(first.remainingMicros, 121 * second)
    // Twelve seconds into the first sub-proof the share moves by time.
    let running = try XCTUnwrap(try progress(17, []).estimate(backlog: 1, running: true, firstHead: 3, schedules: schedules,
                                                              costs: costs, operations: operations, since: 5 * second))
    XCTAssertEqual(running.fraction, 12.0 / 121, accuracy: 1e-9)
    XCTAssertEqual(running.remainingMicros, 109 * second)
    // Native records the first sub-proof: the share goes on from it.
    let recorded = try XCTUnwrap(try progress(30, [row(5, 29, 3, "load", "q", 0, 3, "checkpoint")]).estimate(
      backlog: 1, running: true, firstHead: 3, schedules: schedules, costs: costs, operations: operations, since: 5 * second))
    XCTAssertEqual(recorded.proved, 1)
    XCTAssertEqual(recorded.fraction, 25.0 / 121, accuracy: 1e-9)
    // Unnamed, the head counts at the largest schedule and the mean cost.
    let unnamed = try XCTUnwrap(try progress(5, []).estimate(backlog: 1, running: true, firstHead: 3, schedules: schedules,
                                                             costs: costs, since: 5 * second))
    XCTAssertEqual(unnamed.remaining, 12)
    XCTAssertEqual(unnamed.remainingWeight, 12 * micros(37.75), accuracy: 1e-3)
    // Before this phone proved anything the weights count sub-proofs, with no time.
    let untimed = try XCTUnwrap(try progress(5, []).estimate(backlog: 1, running: true, firstHead: 3, schedules: schedules,
                                                             operations: operations))
    XCTAssertFalse(untimed.timed)
    XCTAssertEqual(untimed.remainingWeight, 3)
    XCTAssertNil(untimed.remainingMicros)
  }

  func testTheRunningSubProofIsTimedFromTheLaterOfTheLastTurnAndSince() throws {
    // The last fold ended at 60 s; the caller's wait for a new Load head began with its reading at 990 s.
    let reading = try progress(1000, [
      row(0, 20, 1, "load", "q", 0, 3, "checkpoint"),
      row(20, 40, 1, "load", "a", 1, 3, "checkpoint"),
      row(40, 60, 1, "load", "omega", 2, 3, "folded")])
    let operations: [UInt64: String] = [2: "load"]
    let timed = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 2, operations: operations,
                                               since: 990 * second))
    XCTAssertEqual(timed.runningWeight, micros(10), accuracy: 1e-3)
    XCTAssertEqual(timed.fraction, 10.0 / 60, accuracy: 1e-9)
    // Without it, the running sub-proof would seem to have run since 60 s, capped at nine tenths of its cost.
    let unbounded = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 2, operations: operations))
    XCTAssertEqual(unbounded.runningWeight, micros(18), accuracy: 1e-3)
  }

  func testAReadingsOwnCostsWinOverTheCallersAndTheCallersFillTheRest() throws {
    let costs: [String: [UInt32: UInt64]] = ["load": [0: 24 * second, 1: 48 * second, 2: 49 * second]]
    let reading = try progress(50, [row(0, 30, 1, "load", "q", 0, 3, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 1, costs: costs))
    XCTAssertEqual(estimate.provedWeight, micros(30), accuracy: 1e-3)
    XCTAssertEqual(estimate.remainingWeight, micros(97), accuracy: 1e-3)
    XCTAssertEqual(estimate.runningWeight, micros(20), accuracy: 1e-3)
    XCTAssertEqual(estimate.fraction, 50.0 / 127, accuracy: 1e-9)
    XCTAssertEqual(estimate.remainingMicros, 77 * second)
  }

  func testLaterHeadsCountByTheOperationTheCallerNamed() throws {
    let reading = try progress(30, [row(0, 20, 1, "load", "q", 0, 3, "checkpoint")])
    let schedules = ["load": 3, "send": 5, "receive": 12]
    // Head 2 is a Send the app made; head 3 is unknown and counts at the largest schedule.
    let estimate = try XCTUnwrap(reading.estimate(backlog: 3, running: false, firstHead: 1, schedules: schedules,
                                                  operations: [2: "send"]))
    XCTAssertEqual(estimate.remaining, 2 + 5 + 12)
    // Heads at or below the current one, or past the backlog, are not later heads.
    let ignored = try XCTUnwrap(reading.estimate(backlog: 2, running: false, firstHead: 1, schedules: schedules,
                                                 operations: [1: "receive", 3: "send"]))
    XCTAssertEqual(ignored.remaining, 2 + 12)
  }

  func testAHeadsOwnScheduleIsExactWhenItsOperationRanLongerBefore() throws {
    // ArchiveSent folds in 17 or 20 sub-proofs; the head's own turn says which.
    let reading = try progress(100, [row(0, 10, 8, "archive_sent", "a", 8, 17, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 2, running: false, firstHead: 8, schedules: ["archive_sent": 20]))
    XCTAssertEqual(estimate.proved, 9)
    XCTAssertEqual(estimate.remaining, 8 + 20) // this head's other eight, the next head at the largest
    XCTAssertEqual(estimate.fraction, 9.0 / 37, accuracy: 1e-9)
  }

  func testACancelledTurnStillSaysWhereItsHeadStands() throws {
    let reading = try progress(80, [
      row(0, 20, 7, "receive", "q", 0, 12, "checkpoint"),
      row(20, 40, 7, "receive", "q", 1, 12, "checkpoint"),
      row(40, 60, 7, "receive", "a", 2, 12, "checkpoint"),
      row(60, 70, 7, "receive", "w", 3, 12, "cancelled"),
      // Cancelled before its released step was read: the head's earlier turns name it and its schedule.
      row(70, 70, 7, nil, nil, 3, nil, "cancelled")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 7, schedules: [:]))
    XCTAssertEqual(estimate.proved, 3)
    XCTAssertEqual(estimate.remaining, 9)
    XCTAssertEqual(estimate.fraction, 0.25, accuracy: 1e-9)
  }

  func testAHeadPastItsKnownScheduleIsNeverCountedDone() throws {
    let reading = try progress(30, [row(0, 20, 9, "receive", "a", 12, nil, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 9, schedules: ["receive": 12]))
    XCTAssertEqual(estimate.proved, 13)
    XCTAssertEqual(estimate.remaining, 1)
    XCTAssertEqual(estimate.fraction, 13.0 / 14, accuracy: 1e-9)
  }

  func testOnlyTheSnapshotCompletesAWait() throws {
    let reading = try progress(1000, [row(0, 10, 3, "x", "a", 998, 1000, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 3, schedules: [:]))
    XCTAssertEqual(estimate.runningWeight, KagemushaWalletFoldProgressV1.runningShareMaximum * micros(10), accuracy: 1e-3)
    XCTAssertEqual(estimate.fraction, KagemushaWalletFoldProgressV1.fractionMaximum)
  }

  func testAdoptedTurnsAreDurableButOnlyAnOmegaFoldsAHead() throws {
    let reading = try progress(50, [
      row(0, 1, 4, "load", nil, 0, 3, "adopted"),
      row(1, 2, 4, "load", "omega", 2, 3, "adopted"),
      row(2, 3, 5, "unload", nil, 1, 3, "adopted")])
    // Adopted bytes were never proved here: they cost nothing known.
    XCTAssertEqual(reading.costs, [:])
    XCTAssertEqual(reading.schedules, ["load": 3, "unload": 3])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: true, firstHead: 4, schedules: [:]))
    XCTAssertFalse(estimate.timed)
    XCTAssertEqual(estimate.runningWeight, 0)
    XCTAssertNil(estimate.remainingMicros)
    XCTAssertEqual(estimate.proved, 5) // load's three sub-proofs, from its Ω, and unload's first two
    XCTAssertEqual(estimate.remaining, 1)
  }

  func testNothingKnownIsNoEstimate() throws {
    XCTAssertNil(try progress(10, []).estimate(backlog: 1, running: true, firstHead: 1, schedules: [:]))
    XCTAssertNil(try progress(10, []).estimate(backlog: 0, running: true, firstHead: 1, schedules: ["load": 3]))
    // Cancelled after its step was read but before its schedule: no size for the head.
    let unsized = try progress(30, [row(0, 20, 1, "load", nil, 0, nil, "cancelled")])
    XCTAssertNil(unsized.estimate(backlog: 1, running: true, firstHead: 1, schedules: [:]))
    XCTAssertNil(unsized.estimate(backlog: 1, running: true, firstHead: 1, schedules: ["load": 0]))
    // No turn yet: the wait's first head counts from its first sub-proof at the schedule the caller knows.
    let unstarted = try XCTUnwrap(try progress(10, []).estimate(backlog: 1, running: true, firstHead: 1, schedules: ["load": 3]))
    XCTAssertEqual(unstarted.proved, 0)
    XCTAssertEqual(unstarted.remaining, 3)
    XCTAssertEqual(unstarted.fraction, 0)
    // A later head counts at the largest schedule this reading or the caller knows.
    let sized = try progress(30, [row(0, 20, 1, "load", "q", 0, 3, "checkpoint")])
    XCTAssertEqual(try XCTUnwrap(sized.estimate(backlog: 2, running: true, firstHead: 1, schedules: [:])).remaining, 2 + 3)
    XCTAssertNil(sized.estimate(backlog: 0, running: true, firstHead: 1, schedules: ["load": 3]))
  }

  func testManyLaterHeadsStayExact() throws {
    let reading = try progress(30, [row(0, 20, 1, "receive", "q", 0, 12, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: .max, running: false, firstHead: 1, schedules: ["receive": 12]))
    XCTAssertGreaterThan(estimate.remaining, 0)
    XCTAssertTrue((0...KagemushaWalletFoldProgressV1.fractionMaximum).contains(estimate.fraction))
    XCTAssertEqual(estimate.remainingMicros, .max)
  }

  func testRefusesMalformedOrForeignTelemetry() {
    let good = row(0, 20, 1, "load", "q", 0, 3, "checkpoint")
    let inputs = [
      "", "[]", "{}",
      // A Native without schedules in its records.
      telemetry(30, [good], schema: "iroha.kagemusha.fold-metrics.v2").replacingOccurrences(of: ",\"stages\":3", with: ""),
      telemetry(30, [good], schema: "iroha.kagemusha.fold-metrics.v2"),
      telemetry(10, [good]), // ended after the sample
      telemetry(30, [row(20, 10, 1, "load", "q", 0, 3, "checkpoint")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 0, 3, "finished")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 4_294_967_296, 3, "checkpoint")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 0, 0, "checkpoint")]),
      telemetry(30, [row(0, 20, 1, "load", "q", 0, 4_294_967_296, "checkpoint")]),
      telemetry(30, [good]).replacingOccurrences(of: ",\"stages\":3", with: ""),
      telemetry(30, [good]).replacingOccurrences(of: "\"stages\":3", with: "\"stages\":\"3\""),
      telemetry(30, [good]).replacingOccurrences(of: "\"stages\":3", with: "\"stages\":-3"),
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

  func testTheLargestScheduleNativeCanReportStillCounts() throws {
    let reading = try progress(30, [row(0, 20, 1, "load", "q", 0, 4_294_967_295, "checkpoint")])
    let estimate = try XCTUnwrap(reading.estimate(backlog: 1, running: false, firstHead: 1, schedules: [:]))
    XCTAssertEqual(estimate.proved, 1)
    XCTAssertEqual(estimate.remaining, 4_294_967_294)
  }
}
