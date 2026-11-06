import Foundation

/// How far Native's background fold has come toward the current wallet head, read from its public
/// fold telemetry (`KagemushaWalletV1.foldMetricsJSON()`). Observation only: it confers no Send,
/// Unload or Request readiness, and the snapshot's folded head remains the only completion.
///
/// Native folds each released head through an ordered schedule of sub-proofs that ends in Ω. It
/// records every turn that selected one once the turn ends: the head's sequence, its operation, the
/// ordinal it worked on, which is the head's durable sub-proof count when the turn began, and, once
/// the turn read the head's schedule, the head's whole schedule with Ω (`stages`). A turn cancelled
/// by a payment therefore still says where its head stands and how far it has to go. A head Native
/// has not started, or whose first sub-proof is still running, has no record, so callers pass what
/// earlier readings of the same owner taught them for those heads: sub-proofs per head and the time
/// each sub-proof took by operation name (`schedules`, `costs`), and the operations of heads they
/// released.
public struct KagemushaWalletFoldProgressV1: Equatable, Sendable {
  /// The telemetry schema this reading accepts.
  public static let schema = "iroha.kagemusha.fold-metrics.v3"
  /// The running sub-proof never counts as more than this share of its cost before Native records it.
  public static let runningShareMaximum = 0.9
  /// Only the snapshot's folded head completes a wait.
  public static let fractionMaximum = 0.99
  private static let durableOutcomes: Set<String> = ["checkpoint", "folded", "adopted"]
  private static let outcomes = durableOutcomes.union(["cancelled", "discarded", "failed"])

  struct Turn: Equatable, Sendable {
    let startedNativeMicros: UInt64
    let endedNativeMicros: UInt64
    let sequence: UInt64
    let operation: String?
    let stage: String?
    let ordinal: UInt32
    let stages: UInt32?
    let outcome: String
    /// The turn left a durable sub-proof, proved now or adopted from an interrupted publication.
    var durable: Bool { KagemushaWalletFoldProgressV1.durableOutcomes.contains(outcome) }
    /// The durable sub-proof was the head's Ω.
    var folded: Bool { outcome == "folded" || durable && stage == "omega" }
    /// The turn proved its sub-proof on this phone; adopted bytes were proved before.
    var proved: Bool { outcome == "checkpoint" || outcome == "folded" }
  }

  /// Native's monotonic time of this reading, in microseconds from its process-local origin.
  public let sampledNativeMicros: UInt64
  /// The head Native is folding as of this reading: the last turn's, or the next one after its Ω; nil before a turn.
  public let head: UInt64?
  /// Sub-proofs per head by operation name: the largest schedule a retained turn of that operation reported.
  public let schedules: [String: Int]
  /// The wall time of each sub-proof on this phone, in microseconds by operation name and ordinal: the mean of the
  /// retained turns that proved it. Sub-proofs of one schedule differ in cost, so a caller that keeps these between
  /// readings can weight a wait by time before its turns are recorded.
  public let costs: [String: [UInt32: UInt64]]
  private let turns: [Turn]

  init(sampledNativeMicros: UInt64, turns: [Turn]) {
    self.sampledNativeMicros = sampledNativeMicros
    self.turns = turns
    head = turns.last.map { $0.folded ? $0.sequence &+ 1 : $0.sequence }
    var schedules: [String: Int] = [:]
    var sums: [String: [UInt32: (total: UInt64, count: UInt64)]] = [:]
    for turn in turns {
      guard let operation = turn.operation else { continue }
      if let stages = turn.stages { schedules[operation] = max(schedules[operation] ?? 0, Int(stages)) }
      guard turn.proved else { continue }
      let before = sums[operation]?[turn.ordinal] ?? (0, 0)
      sums[operation, default: [:]][turn.ordinal] =
        (before.total + (turn.endedNativeMicros - turn.startedNativeMicros), before.count + 1)
    }
    self.schedules = schedules
    costs = sums.compactMapValues { byOrdinal in
      let means = byOrdinal.compactMapValues { sum -> UInt64? in sum.total / sum.count > 0 ? sum.total / sum.count : nil }
      return means.isEmpty ? nil : means
    }
  }

  /// One estimate of a fold wait, weighted by what each sub-proof costs: microseconds at this phone's pace once it
  /// has proved one (`timed`), sub-proofs before that. `fraction` stays below one until the snapshot reports the
  /// folded head.
  public struct Estimate: Equatable, Sendable {
    /// Sub-proofs done toward the wait: the current head's durable ones and every sub-proof of a head folded since it began.
    public let proved: UInt64
    /// Sub-proofs still to prove, the running one included; a head of unknown size counts as the largest schedule.
    public let remaining: UInt64
    /// What the `proved` sub-proofs cost.
    public let provedWeight: Double
    /// What the `remaining` sub-proofs cost, the running one in full.
    public let remainingWeight: Double
    /// The running sub-proof's part already spent, at most `runningShareMaximum` of its cost.
    public let runningWeight: Double
    /// Whether the weights are microseconds at this phone's pace; before it proved a sub-proof they count sub-proofs.
    public let timed: Bool

    public var fraction: Double {
      min(max((provedWeight + runningWeight) / (provedWeight + remainingWeight), 0),
          KagemushaWalletFoldProgressV1.fractionMaximum)
    }

    /// Expected monotonic time until the last released head is folded at this phone's pace, or nil unless `timed`.
    public var remainingMicros: UInt64? {
      guard timed else { return nil }
      let value = max(remainingWeight - runningWeight, 0).rounded()
      return value < Double(UInt64.max) ? UInt64(value) : .max
    }
  }

  /// Estimate the wait until every released head is folded.
  ///
  /// `backlog` counts the released heads still needing folds, as the latest snapshot reports it. `running` says
  /// Native's worker is proving now. `firstHead` is where the caller's wait began: the `head` of its first reading,
  /// or before any turn the snapshot's oldest unfolded head (its sequence minus its backlog, plus one). Every head
  /// folded since then counts in full, from its own Ω. A reading with no turn of the current head knows none of its
  /// sub-proofs durable, so the head after the last Ω, or `firstHead` before any turn, counts from its first.
  ///
  /// A head counts at the schedule its own turns reported. Heads without one count by their operation, named by a
  /// turn or by the caller in `operations` (such as the heads its own Load, Send, Receive or Unload released), in
  /// `schedules` merged with this reading's, and at the largest schedule when no operation is known. Each sub-proof
  /// weighs what this reading's or the caller's `costs` say it took, else its operation's mean, else the mean of every
  /// known one. The running sub-proof is timed from the last turn's end or `since`, whichever is later: the caller's
  /// earliest reading at which it may have started, such as its first reading of the wait or its last that found
  /// the worker parked. No head is counted finished before its Ω. Nil when nothing is known yet: no backlog, or no
  /// size for a head still to fold.
  public func estimate(backlog: UInt64, running: Bool, firstHead: UInt64, schedules: [String: Int] = [:],
                       costs: [String: [UInt32: UInt64]] = [:], operations: [UInt64: String] = [:],
                       since: UInt64? = nil) -> Estimate? {
    guard backlog > 0 else { return nil }
    var sizes = schedules
    for (operation, count) in self.schedules where count > (sizes[operation] ?? 0) { sizes[operation] = count }
    var learned = costs.mapValues { $0.filter { $0.value > 0 } }
    for (operation, byOrdinal) in self.costs { learned[operation, default: [:]].merge(byOrdinal) { _, reading in reading } }
    let known = learned.values.flatMap(\.values)
    let pace = known.isEmpty ? nil : known.reduce(0.0) { $0 + Double($1) } / Double(known.count)
    let means = learned.compactMapValues { byOrdinal -> Double? in
      byOrdinal.isEmpty ? nil : byOrdinal.values.reduce(0.0) { $0 + Double($1) } / Double(byOrdinal.count)
    }
    // Sub-proofs `from` until `until` of a head of `operation`: known ordinals at their cost, the rest at a mean.
    func weight(_ operation: String?, from: UInt64, until: UInt64) -> Double {
      guard until > from else { return 0 }
      guard let pace else { return Double(until - from) }
      let byOrdinal = operation.flatMap { learned[$0] }?.filter { UInt64($0.key) >= from && UInt64($0.key) < until } ?? [:]
      let fallback = operation.flatMap { means[$0] } ?? pace
      return byOrdinal.values.reduce(0.0) { $0 + Double($1) } + Double(until - from - UInt64(byOrdinal.count)) * fallback
    }
    let largest = sizes.values.filter { $0 > 0 }.max().map(UInt64.init)
    let last = turns.last
    let head = self.head.flatMap { $0 >= firstHead ? $0 : nil } ?? firstHead
    var done: UInt64 = 0
    if let last, last.sequence == head { done = UInt64(last.ordinal) + (last.durable ? 1 : 0) }
    let ofHead = turns.filter { $0.sequence == head }
    // Native's own schedule for a head it has started; otherwise the head's operation, or the largest.
    let operation = ofHead.last { $0.operation != nil }?.operation ?? operations[head]
    let named = operation.flatMap { sizes[$0] }.flatMap { $0 > 0 ? UInt64($0) : nil }
    guard let schedule = ofHead.last(where: { $0.stages != nil })?.stages.map(UInt64.init) ?? named ?? largest else {
      return nil
    }
    let size = max(schedule, done + 1)
    var proved = done
    var provedWeight = weight(operation, from: 0, until: done)
    var omegas: [UInt64: (count: UInt64, operation: String?)] = [:]
    for turn in turns where turn.folded && turn.sequence >= firstHead && turn.sequence < head {
      let before = omegas[turn.sequence]
      omegas[turn.sequence] = (max(before?.count ?? 0, UInt64(turn.ordinal) + 1), before?.operation ?? turn.operation)
    }
    for omega in omegas.values {
      proved += omega.count
      provedWeight += weight(omega.operation, from: 0, until: omega.count)
    }
    var remaining = size - done
    var remainingWeight = weight(operation, from: done, until: size)
    if backlog > 1 {
      let (end, past) = head.addingReportingOverflow(backlog)
      let laterNamed = operations.filter { $0.key > head && (past || $0.key < end) }.map(\.value)
      for later in laterNamed {
        guard let each = sizes[later].flatMap({ $0 > 0 ? UInt64($0) : nil }) ?? largest else { return nil }
        remaining += each
        remainingWeight += weight(later, from: 0, until: each)
      }
      // The other later heads count at the largest schedule; the bound keeps the sum exact.
      let unnamed = backlog - 1 - UInt64(laterNamed.count)
      if unnamed > 0 {
        guard let largest else { return nil }
        remaining += min(unnamed, UInt64.max / 4 / largest) * largest
        remainingWeight += Double(unnamed) * weight(nil, from: 0, until: largest)
      }
    }
    var runningWeight = 0.0
    if running, pace != nil, let start = [last?.endedNativeMicros, since].compactMap({ $0 }).max() {
      runningWeight = min(Double(sampledNativeMicros - min(start, sampledNativeMicros)),
                          Self.runningShareMaximum * weight(operation, from: done, until: done + 1))
    }
    return Estimate(proved: proved, remaining: remaining, provedWeight: provedWeight, remainingWeight: remainingWeight,
                    runningWeight: runningWeight, timed: pace != nil)
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
    let stages: UInt32?
    let outcome: String

    private enum CodingKeys: String, CodingKey {
      case startedNativeMicros, endedNativeMicros, sequence, operation, stage, ordinal, stages, outcome
    }

    // Every field is required; only operation, stage and stages may be null.
    init(from decoder: Decoder) throws {
      let fields = try decoder.container(keyedBy: CodingKeys.self)
      startedNativeMicros = try fields.decode(UInt64.self, forKey: .startedNativeMicros)
      endedNativeMicros = try fields.decode(UInt64.self, forKey: .endedNativeMicros)
      sequence = try fields.decode(UInt64.self, forKey: .sequence)
      operation = try fields.decode(String?.self, forKey: .operation)
      stage = try fields.decode(String?.self, forKey: .stage)
      ordinal = try fields.decode(UInt32.self, forKey: .ordinal)
      stages = try fields.decode(UInt32?.self, forKey: .stages)
      outcome = try fields.decode(String.self, forKey: .outcome)
    }
  }

  /// Read `KagemushaWalletV1.foldMetricsJSON()`; malformed or foreign telemetry is `invalidNativeOutput`.
  public static func parse(_ json: String) throws -> KagemushaWalletFoldProgressV1 {
    guard let wire = try? JSONDecoder().decode(Wire.self, from: Data(json.utf8)), wire.schema == schema else {
      throw KagemushaWalletErrorV1.invalidNativeOutput
    }
    let turns = try wire.records.map { record -> Turn in
      // A schedule always ends in Ω, so a reported one has at least one sub-proof.
      guard record.startedNativeMicros <= record.endedNativeMicros,
            record.endedNativeMicros <= wire.sampledNativeMicros,
            record.stages != 0,
            outcomes.contains(record.outcome) else { throw KagemushaWalletErrorV1.invalidNativeOutput }
      return Turn(startedNativeMicros: record.startedNativeMicros, endedNativeMicros: record.endedNativeMicros,
                  sequence: record.sequence, operation: record.operation, stage: record.stage,
                  ordinal: record.ordinal, stages: record.stages, outcome: record.outcome)
    }
    return KagemushaWalletFoldProgressV1(sampledNativeMicros: wire.sampledNativeMicros, turns: turns)
  }
}
