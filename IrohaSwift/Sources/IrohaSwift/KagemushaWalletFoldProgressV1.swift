import Foundation

/// How far Native's background fold has come toward the current wallet head, read from its public
/// fold telemetry (`KagemushaWalletV1.foldMetricsJSON()`). Observation only: it confers no Send,
/// Unload or Request readiness, and the snapshot's folded head remains the only completion.
///
/// Native folds each released head through an ordered schedule of sub-proofs that ends in Ω. It
/// records every turn that selected one with the head's sequence, its operation and the ordinal it
/// worked on, which is the head's durable sub-proof count when the turn began; a turn cancelled by
/// a payment therefore still says where the head stands. Native does not report a schedule's
/// length, so callers pass sub-proofs per head by operation name, such as the `schedules` that
/// earlier readings of the same installed producer catalog showed.
public struct KagemushaWalletFoldProgressV1: Equatable, Sendable {
  /// The telemetry schema this reading accepts.
  public static let schema = "iroha.kagemusha.fold-metrics.v2"
  /// The running sub-proof never counts as more than this share of one before Native records it.
  public static let runningShareMaximum = 0.9
  /// Only the snapshot's folded head completes a wait.
  public static let fractionMaximum = 0.99
  private static let paceTurns = 8
  private static let durableOutcomes: Set<String> = ["checkpoint", "folded", "adopted"]
  private static let outcomes = durableOutcomes.union(["cancelled", "discarded", "failed"])

  struct Turn: Equatable, Sendable {
    let startedNativeMicros: UInt64
    let endedNativeMicros: UInt64
    let sequence: UInt64
    let operation: String?
    let stage: String?
    let ordinal: UInt32
    let outcome: String
    /// The turn left a durable sub-proof, proved now or adopted from an interrupted publication.
    var durable: Bool { KagemushaWalletFoldProgressV1.durableOutcomes.contains(outcome) }
    /// The durable sub-proof was the head's Ω.
    var folded: Bool { outcome == "folded" || durable && stage == "omega" }
  }

  /// Native's monotonic time of this reading, in microseconds from its process-local origin.
  public let sampledNativeMicros: UInt64
  /// The head Native is folding as of this reading: the last turn's, or the next one after its Ω; nil before a turn.
  public let head: UInt64?
  /// Sub-proofs per head by operation name: each retained Ω's ordinal plus one, the largest per operation.
  public let schedules: [String: Int]
  /// Mean wall time of the last eight proved sub-proofs on this phone, or nil before one.
  public let subProofMicros: UInt64?
  private let turns: [Turn]

  init(sampledNativeMicros: UInt64, turns: [Turn]) {
    self.sampledNativeMicros = sampledNativeMicros
    self.turns = turns
    head = turns.last.map { $0.folded ? $0.sequence &+ 1 : $0.sequence }
    var schedules: [String: Int] = [:]
    for turn in turns where turn.folded {
      guard let operation = turn.operation else { continue }
      schedules[operation] = max(schedules[operation] ?? 0, Int(turn.ordinal) + 1)
    }
    self.schedules = schedules
    let proved = turns.filter { $0.outcome == "checkpoint" || $0.outcome == "folded" }.suffix(Self.paceTurns)
    subProofMicros = proved.isEmpty ? nil
      : proved.reduce(0) { $0 + ($1.endedNativeMicros - $1.startedNativeMicros) } / UInt64(proved.count)
  }

  /// One estimate of a fold wait. `fraction` stays below one until the snapshot reports the folded head.
  public struct Estimate: Equatable, Sendable {
    /// Sub-proofs done toward the wait: the current head's durable ones and every sub-proof of a head folded since it began.
    public let proved: UInt64
    /// Sub-proofs still to prove, the running one included; a head of unknown size counts as the largest schedule.
    public let remaining: UInt64
    /// Part of the running sub-proof already spent at this phone's pace, from zero up to `runningShareMaximum`.
    public let runningShare: Double
    /// Expected monotonic time until the last released head is folded at this phone's pace, or nil before a proved sub-proof.
    public let remainingMicros: UInt64?

    public var fraction: Double {
      min(max((Double(proved) + runningShare) / Double(proved + remaining), 0), KagemushaWalletFoldProgressV1.fractionMaximum)
    }
  }

  /// Estimate the wait until every released head is folded.
  ///
  /// `backlog` counts the released heads still needing folds, as the latest snapshot reports it.
  /// `running` says Native's worker is proving now. `firstHead` is the `head` of the reading that
  /// began the caller's wait; every head folded since then counts in full, from its own Ω.
  /// `schedules` gives sub-proofs per head by operation name. A head whose size is unknown counts
  /// as the largest known schedule, so an estimate can only shrink once the head is identified,
  /// and no head is counted finished before its Ω. Nil when nothing is known yet: no turn, no
  /// schedule or no backlog.
  public func estimate(backlog: UInt64, running: Bool, firstHead: UInt64, schedules: [String: Int]) -> Estimate? {
    guard let last = turns.last, let head, let largest = schedules.values.filter({ $0 > 0 }).max().map(UInt64.init),
          backlog > 0 else { return nil }
    let done: UInt64 = last.folded ? 0 : UInt64(last.ordinal) + (last.durable ? 1 : 0)
    let operation = turns.last { $0.sequence == head && $0.operation != nil }?.operation
    let known = operation.flatMap { schedules[$0] }.flatMap { $0 > 0 ? UInt64($0) : nil }
    let size = max(known ?? largest, done + 1)
    var omegas: [UInt64: UInt64] = [:]
    for turn in turns where turn.folded && turn.sequence >= firstHead && turn.sequence < head {
      omegas[turn.sequence] = max(omegas[turn.sequence] ?? 0, UInt64(turn.ordinal) + 1)
    }
    let earlier = omegas.values.reduce(0, +)
    // Later heads are counted at the largest schedule; the bound keeps the sum exact.
    let later = min(backlog - 1, UInt64.max / 4 / largest) * largest
    let remaining = size - done + later
    let pace = subProofMicros.flatMap { $0 > 0 ? $0 : nil }
    var share = 0.0
    if running, let pace {
      let spent = Double(sampledNativeMicros - min(last.endedNativeMicros, sampledNativeMicros)) / Double(pace)
      share = min(max(spent, 0), Self.runningShareMaximum)
    }
    let micros = pace.map { ((Double(remaining) - share) * Double($0)).rounded() }
    return Estimate(proved: earlier + done, remaining: remaining, runningShare: share,
                    remainingMicros: micros.map { $0 < Double(UInt64.max) ? UInt64($0) : UInt64.max })
  }

  private struct Wire: Decodable {
    let schema: String
    let sampledNativeMicros: UInt64
    let records: [Record]
  }

  private struct Record: Decodable {
    let startedNativeMicros: UInt64
    let endedNativeMicros: UInt64
    let sequence: UInt64
    let operation: String?
    let stage: String?
    let ordinal: UInt32
    let outcome: String

    private enum CodingKeys: String, CodingKey {
      case startedNativeMicros, endedNativeMicros, sequence, operation, stage, ordinal, outcome
    }

    // Every field is required; only operation and stage may be null.
    init(from decoder: Decoder) throws {
      let fields = try decoder.container(keyedBy: CodingKeys.self)
      startedNativeMicros = try fields.decode(UInt64.self, forKey: .startedNativeMicros)
      endedNativeMicros = try fields.decode(UInt64.self, forKey: .endedNativeMicros)
      sequence = try fields.decode(UInt64.self, forKey: .sequence)
      operation = try fields.decode(String?.self, forKey: .operation)
      stage = try fields.decode(String?.self, forKey: .stage)
      ordinal = try fields.decode(UInt32.self, forKey: .ordinal)
      outcome = try fields.decode(String.self, forKey: .outcome)
    }
  }

  /// Read `KagemushaWalletV1.foldMetricsJSON()`; malformed or foreign telemetry is `invalidNativeOutput`.
  public static func parse(_ json: String) throws -> KagemushaWalletFoldProgressV1 {
    guard let wire = try? JSONDecoder().decode(Wire.self, from: Data(json.utf8)), wire.schema == schema else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    let turns = try wire.records.map { record -> Turn in
      guard record.startedNativeMicros <= record.endedNativeMicros,
            record.endedNativeMicros <= wire.sampledNativeMicros,
            outcomes.contains(record.outcome) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return Turn(startedNativeMicros: record.startedNativeMicros, endedNativeMicros: record.endedNativeMicros,
                  sequence: record.sequence, operation: record.operation, stage: record.stage,
                  ordinal: record.ordinal, outcome: record.outcome)
    }
    return KagemushaWalletFoldProgressV1(sampledNativeMicros: wire.sampledNativeMicros, turns: turns)
  }
}
