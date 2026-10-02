import Foundation

/// Exact acknowledged outgoing bytes. Detached DATA cannot recreate State/proof/money custody.
public struct KagemushaOrdinaryCommittedOutgoingOriginalV1: Sendable {
  public let commitRequestOriginalSha256: Data
  public let completeOutgoingOriginal: Data
  init(key: Data, original: Data) {
    commitRequestOriginalSha256 = Data(key); completeOutgoingOriginal = Data(original)
  }
}

/// One real retained ordinary Cash operation. Software financial WAL is not hardware sealing.
/// Actual Native/current FI/hardware/proof admission remains mandatory at every effect.
public actor KagemushaOrdinaryOutgoingCashV1 {
  private let session: KagemushaOrdinaryNativeAccountSessionV1
  private let coordinator: KagemushaNativeCoreCoordinatorAdapterV1
  private let binding: KagemushaOrdinaryNativeBindingV1
  private let preparationHardware: KagemushaAppAttestAppApprovalProviderV1
  private let terminalHardware: KagemushaAppAttestOrdinaryTerminalApprovalProviderV1
  private let currentControl: KagemushaOrdinaryCurrentControlV1
  private let transport: any KagemushaOrdinaryLineageOriginalTransportV1
  private var active = false
  private var frozen = false
  private final class Dispatch {
    let fields: [Data]
    var lifetime: KagemushaOrdinaryHttpLifetimeV1?
    var carrier: KagemushaOrdinaryLineageHttpOriginalV1?
    var response: Data?
    var intakeStarted = false
    var completed = false
    init(_ fields: [Data]) { self.fields = fields.map { Data($0) } }
  }
  private final class Cycle {
    let kind: UInt8
    let business: Data
    var prepareStarted = false
    var preparation: KagemushaNativePreparedAppApprovalV1?
    var w2Captured = false
    var reservationProved = false
    var reserve: Dispatch?
    var terminal: KagemushaNativePreparedOrdinaryTerminalApprovalV1?
    var w1Captured = false
    var commitProved = false
    var commit: Dispatch?
    var stateAdvanced = false
    var acknowledged = false
    var completed: KagemushaOrdinaryCommittedOutgoingOriginalV1?
    init(kind: UInt8, business: Data) { self.kind = kind; self.business = Data(business) }
  }
  private var cycle: Cycle?

  /// Compose only one retained actual account session. No public handle, W, S, signer or ready DTO.
  public init(session: KagemushaOrdinaryNativeAccountSessionV1,
    preparationHardware: KagemushaAppAttestAppApprovalProviderV1,
    terminalHardware: KagemushaAppAttestOrdinaryTerminalApprovalProviderV1,
    currentControlTransport: any KagemushaOrdinaryCurrentControlOriginalTransportV1,
    lineageTransport: any KagemushaOrdinaryLineageOriginalTransportV1) throws {
    self.session = session
    coordinator = try session.originalCoordinator()
    binding = try coordinator.ordinaryNativeTransportBinding()
    self.preparationHardware = preparationHardware; self.terminalHardware = terminalHardware
    currentControl = KagemushaOrdinaryCurrentControlV1(session: session, binding: binding,
      transport: currentControlTransport)
    transport = lineageTransport
  }

  /// Exact request of the genuine enrolled receiver; Native derives its State, reservation and W.
  public func beginOrResumeSend(originalReceiverRequest: Data) async throws -> KagemushaOrdinaryCommittedOutgoingOriginalV1 {
    guard (1...4096).contains(originalReceiverRequest.count) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary receiver request exceeds bound")
    }
    return try await perform(kind: 2, business: Data(originalReceiverRequest))
  }
  /// Positive exact u128, without floating point or an alternate arbitrary-precision wire format.
  public func beginOrResumeRedemption(amountLittleEndian: Data) async throws -> KagemushaOrdinaryCommittedOutgoingOriginalV1 {
    guard amountLittleEndian.count == 16, amountLittleEndian.contains(where: { $0 != 0 }) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary redemption is not a positive LE128")
    }
    return try await perform(kind: 4, business: Data(amountLittleEndian))
  }
  private func perform(kind: UInt8, business: Data) async throws -> KagemushaOrdinaryCommittedOutgoingOriginalV1 {
    guard !active else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("ordinary outgoing operation already active") }
    active = true; defer { active = false }
    do { try current() } catch { throw freeze(error) }
    let original = cycle ?? Cycle(kind: kind, business: business)
    if cycle == nil { cycle = original }
    guard original.kind == kind, original.business == business else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("unfinished outgoing business original cannot be replaced")
    }
    if let completed = original.completed { return completed }
    if !original.prepareStarted {
      // Only this exact Native FI workflow may acquire Cash from the published Bootstrap.
      _ = try await currentControl.beginOrResumeCurrentFinancialControl()
      original.prepareStarted = true // Own before Native may persist its original W2.
      original.preparation = try effect {
        if kind == 2 { return try coordinator.prepareOrdinarySendApproval(originalReceiverRequest: original.business) }
        return try coordinator.prepareOrdinaryRedemptionApproval(amountLittleEndian: original.business)
      }
    }
    if !original.w2Captured {
      do {
        try current()
        guard let preparation = original.preparation else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
        _ = try await preparationHardware.approve(preparation)
        try current(); original.w2Captured = true
      } catch { throw freeze(error) }
    }
    if !original.reservationProved { _ = try effect { try invoke(5) }; original.reservationProved = true }
    if original.reserve == nil { original.reserve = try effect { Dispatch(try invoke(6)) } }
    guard let reserve = original.reserve else { throw freeze(KagemushaCoreCoordinatorErrorV1.unavailable) }
    try await dispatch(reserve)
    let reserveKey = reserve.fields[4]
    if original.terminal == nil {
      _ = try effect { try invoke(16) } // Genuine four-node clock; no managed timestamp.
      try await currentControl.refreshCurrentFinancialControl()
      original.terminal = try effect { try KagemushaNativePreparedOrdinaryTerminalApprovalV1.selected(binding: binding, reserveKey: reserveKey) }
    }
    if !original.w1Captured {
      do {
        try current()
        guard let terminal = original.terminal else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
        _ = try await terminalHardware.approve(terminal)
        try current(); original.w1Captured = true
      } catch { throw freeze(error) }
    }
    if !original.commitProved { _ = try effect { try invoke(12) }; original.commitProved = true }
    if original.commit == nil { original.commit = try effect { Dispatch(try invoke(13)) } }
    guard let commit = original.commit else { throw freeze(KagemushaCoreCoordinatorErrorV1.unavailable) }
    try await dispatch(commit)
    let commitKey = commit.fields[4]
    if !original.stateAdvanced {
      _ = try effect { try invoke(14, fields: [commitKey]) }; original.stateAdvanced = true
    }
    if !original.acknowledged {
      // Distinct actual post-State-fsync FI capture/Ack; stale pre-State custody is never reused.
      _ = try effect { try invoke(16) }
      try await currentControl.refreshCurrentFinancialControl()
      _ = try effect { try invoke(15, fields: [commitKey]) }; original.acknowledged = true
    }
    let delivery = try effect { try invoke(17, fields: [commitKey])[0] }
    let completed = KagemushaOrdinaryCommittedOutgoingOriginalV1(key: commitKey, original: delivery)
    try current(); original.completed = completed
    return completed
  }

  private func dispatch(_ original: Dispatch) async throws {
    if original.completed { return }
    if original.fields[0] == Data([2]) { original.completed = true; return }
    if original.carrier == nil {
      original.carrier = try effect {
        let lifetime = KagemushaOrdinaryHttpLifetimeV1(session: session, binding: binding)
        original.lifetime = lifetime
        return try KagemushaOrdinaryLineageHttpOriginalV1(fields: original.fields,
          requireOriginal: { try lifetime.requireCurrent() })
      }
    }
    guard let carrier = original.carrier else { throw freeze(KagemushaCoreCoordinatorErrorV1.unavailable) }
    if original.response == nil {
      let response: Data
      do { response = try await transport.exchange(carrier) }
      catch {
        do { try current() } catch { throw freeze(error) }
        throw error // HTTP uncertainty retains exactly this request/signature/service proof/UUID.
      }
      original.response = try effect {
        guard (1...carrier.maximumResponseBytes).contains(response.count) else {
          throw KagemushaCoreCoordinatorErrorV1.invalidFrame("lineage HTTP original exceeds bound")
        }
        return Data(response)
      }
    }
    try effect {
      guard !original.intakeStarted, let raw = original.response else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      let originals = try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.lineage, raw: raw)
      original.intakeStarted = true; original.lifetime?.retire() // Own before global Ack may persist.
      guard try invoke(7, fields: originals)[0] == original.fields[4] else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("acknowledged lineage request differs")
      }
      original.completed = true; original.response = nil
    }
  }
  private func invoke(_ phase: UInt8, fields: [Data] = []) throws -> [Data] {
    try current(); let result = try binding.outgoing(phase, fields: fields); try current(); return result
  }
  private func current() throws {
    guard !frozen else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    try session.requireCurrent(); try binding.requireOpen(); try session.requireCurrent()
  }
  private func effect<T>(_ body: () throws -> T) throws -> T {
    do { try current(); let result = try body(); try current(); return result }
    catch { throw freeze(error) }
  }
  private func freeze(_ failure: Error) -> Error {
    frozen = true; cycle?.reserve?.lifetime?.retire(); cycle?.commit?.lifetime?.retire()
    cycle?.reserve?.response = nil; cycle?.commit?.response = nil
    try? binding.revoke(); return failure
  }
}
