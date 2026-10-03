import CryptoKit
import Foundation

/// Historical correlation only. Native retains all credit, State and monetary custody.
public struct KagemushaOrdinaryIncomingAcknowledgementV1: Sendable {
  public let reserveRequestOriginalDigest: Data
  public let commitRequestOriginalDigest: Data
  init(reserve: Data, commit: Data) {
    reserveRequestOriginalDigest = Data(reserve); commitRequestOriginalDigest = Data(commit)
  }
}

// Internal deterministic control seam. Only the public actual-session constructor below
// selects production implementations; scripted tests never supply Native signing capability.
protocol KagemushaIncomingApprovalStepV1: Sendable {
  func approve() async throws
}
protocol KagemushaIncomingWorkflowNativeV1: Sendable {
  func requireOpen() throws
  func revoke() throws
  func refreshAccount() throws
  func prepareMint(finalized: Data, credit: Data) throws -> any KagemushaIncomingApprovalStepV1
  func prepareReceive(request: Data, outgoing: Data, assertion: Data) throws -> any KagemushaIncomingApprovalStepV1
  func selectTerminal(reserve: Data) throws -> any KagemushaIncomingApprovalStepV1
  func invoke(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data]) throws -> [Data]
}

/// Full incoming Mint/Receive workflow on one actual Native account owner. W2 and W1 use
/// distinct durable Native selections, platform fences and original counters. Retry may
/// redispatch only the exact retained HTTP originals; unknown Native/device effects freeze.
public actor KagemushaOrdinaryIncomingCashV1 {
  private let native: any KagemushaIncomingWorkflowNativeV1
  private let hardwareTransport: any KagemushaOrdinaryIncomingOriginalTransportV1
  private let refreshFinancial: @Sendable (Bool) async throws -> Void
  private var active = false
  private var frozen = false
  private var financialRefreshPending = false
  private final class Cycle {
    let receive: Bool
    let originalDigests: [Data]
    var preparation: (any KagemushaIncomingApprovalStepV1)?
    var preparationApproved = false
    var candidateRetained = false
    var reserve: Data?
    var terminal: (any KagemushaIncomingApprovalStepV1)?
    var terminalApproved = false
    var commitRetained = false
    var commit: Data?
    var advanced = false
    var acknowledged: KagemushaOrdinaryIncomingAcknowledgementV1?
    var carrier: KagemushaOrdinaryIncomingHttpOriginalV1?
    var lifetime: KagemushaOrdinaryIncomingHttpLifetimeV1?
    var response: [Data]?
    var transportPhase: KagemushaOrdinaryIncomingPhaseV1?
    init(receive: Bool, inputs: [Data]) {
      self.receive = receive; originalDigests = inputs.map { Data(SHA256.hash(data: $0)) }
    }
  }
  private var cycle: Cycle?

  /// Compose from the retained actual session and a genuine App Attest provider. No raw handle,
  /// key/App ID selector, signing subject, financial verdict or Native callback is accepted.
  public init(session: KagemushaOrdinaryNativeAccountSessionV1,
    hardware: KagemushaAppAttestOrdinaryIncomingApprovalProviderV1,
    currentControlTransport: any KagemushaOrdinaryCurrentControlOriginalTransportV1,
    lineageTransport: any KagemushaOrdinaryIncomingOriginalTransportV1) throws {
    let binding = try session.originalCoordinator().ordinaryIncomingBinding(session: session)
    native = KagemushaActualIncomingWorkflowV1(binding: binding, hardware: hardware)
    hardwareTransport = lineageTransport
    let financial = try KagemushaOrdinaryCurrentControlV1(session: session, transport: currentControlTransport)
    refreshFinancial = { fresh in
      if fresh { try await financial.refreshCurrentFinancialControl() }
      else { _ = try await financial.beginOrResumeCurrentFinancialControl() }
    }
  }
  init(native: any KagemushaIncomingWorkflowNativeV1,
    transport: any KagemushaOrdinaryIncomingOriginalTransportV1,
    refreshFinancial: @escaping @Sendable (Bool) async throws -> Void) {
    self.native = native; hardwareTransport = transport; self.refreshFinancial = refreshFinancial
  }
  public func beginOrResumeFinalizedMint(finalizedTopupOriginal: Data, mintCreditOriginal: Data) async throws
    -> KagemushaOrdinaryIncomingAcknowledgementV1 {
    try await perform(receive: false, inputs: [finalizedTopupOriginal, mintCreditOriginal])
  }
  public func beginOrResumeReceive(receiverRequestID: Data, fullOutgoingOriginal: Data,
    fullReceivedAssertionOriginal: Data) async throws -> KagemushaOrdinaryIncomingAcknowledgementV1 {
    try await perform(receive: true, inputs: [receiverRequestID, fullOutgoingOriginal, fullReceivedAssertionOriginal])
  }
  /// Clear managed correlation only after both durable acknowledgements. This cannot reset a
  /// Native journal, key counter, source, failed invocation fence or incomplete cycle.
  public func releaseCompletedCycle() throws {
    guard !active else { throw invalid("incoming cycle is already active") }
    try current()
    guard cycle?.acknowledged != nil else { throw invalid("incoming cycle is not acknowledged") }
    cycle = nil
    try current()
  }
  private func perform(receive: Bool, inputs: [Data]) async throws -> KagemushaOrdinaryIncomingAcknowledgementV1 {
    try KagemushaOrdinaryIncomingFrameV1.requireOriginals(receive ? .prepareReceive : .prepareFinalizedMint,
      originals: inputs)
    guard !active else { throw invalid("incoming cycle is already active") }
    active = true; defer { active = false }
    do { try current() } catch { throw freeze(error) }
    let digests = inputs.map { Data(SHA256.hash(data: $0)) }
    let held: Cycle
    if let original = cycle {
      guard original.receive == receive, original.originalDigests == digests else {
        throw invalid("unfinished incoming originals cannot be replaced")
      }
      held = original
    } else { held = Cycle(receive: receive, inputs: inputs); cycle = held }
    if let acknowledged = held.acknowledged { return acknowledged }
    try await refreshDependencies()
    if held.preparation == nil {
      held.preparation = try effect {
        if receive { return try native.prepareReceive(request: inputs[0], outgoing: inputs[1], assertion: inputs[2]) }
        return try native.prepareMint(finalized: inputs[0], credit: inputs[1])
      }
    }
    if !held.preparationApproved {
      do { try current(); try await held.preparation!.approve(); try current(); held.preparationApproved = true }
      catch { throw freeze(error) }
    }
    if !held.candidateRetained { _ = try invoke(.proveCandidate); held.candidateRetained = true }
    if held.reserve == nil {
      try await refreshDependencies()
      held.reserve = try await exchange(held, phase: .reserveTransport)
    }
    if held.terminal == nil {
      try await refreshDependencies()
      held.terminal = try effect { try native.selectTerminal(reserve: held.reserve!) }
    }
    if !held.terminalApproved {
      do { try current(); try await held.terminal!.approve(); try current(); held.terminalApproved = true }
      catch { throw freeze(error) }
    }
    if !held.commitRetained { _ = try invoke(.proveCommit); held.commitRetained = true }
    if held.commit == nil {
      try await refreshDependencies()
      held.commit = try await exchange(held, phase: .commitTransport)
    }
    if !held.advanced {
      try await refreshDependencies()
      _ = try invoke(.advanceState, originals: [held.commit!]); held.advanced = true
    }
    _ = try invoke(.acknowledge, originals: [held.commit!])
    try current()
    let result = KagemushaOrdinaryIncomingAcknowledgementV1(reserve: held.reserve!, commit: held.commit!)
    held.acknowledged = result
    return result
  }
  private func refreshDependencies() async throws {
    try effect { try native.refreshAccount() }
    let fresh = !financialRefreshPending
    financialRefreshPending = true
    do {
      try await refreshFinancial(fresh); try current(); financialRefreshPending = false
    } catch {
      // The concrete FI workflow retains its exact HTTP original and freezes its Native
      // descriptor on any unknown effect. HTTP-only uncertainty may resume that same request.
      do { try current() } catch { throw freeze(error) }
      throw error
    }
  }
  private func exchange(_ held: Cycle, phase: KagemushaOrdinaryIncomingPhaseV1) async throws -> Data {
    if held.carrier == nil {
      let fields = try invoke(phase)
      if fields[0] == Data([2]) { return fields[4] }
      let lifetime = KagemushaOrdinaryIncomingHttpLifetimeV1(native: native)
      held.carrier = try effect { try KagemushaOrdinaryIncomingHttpOriginalV1(fields: fields,
        commit: phase == .commitTransport, lifetime: lifetime) }
      held.lifetime = lifetime; held.transportPhase = phase
    }
    guard held.transportPhase == phase, let carrier = held.carrier else { throw freeze(invalid("incoming retained transport phase changed")) }
    if held.response == nil {
      let raw: Data
      do { raw = try await hardwareTransport.exchange(carrier) }
      catch { do { try current() } catch { throw freeze(error) }; throw error }
      held.response = try effect { try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.lineage, raw: raw) }
    }
    let acknowledged = try invoke(.retainGlobalResult, originals: held.response!)[0]
    guard acknowledged == carrier.requestOriginalDigest else { throw freeze(invalid("Native acknowledged another incoming request")) }
    held.lifetime?.retire(); held.lifetime = nil; held.carrier = nil
    held.response = nil; held.transportPhase = nil
    return acknowledged
  }
  private func invoke(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data] = []) throws -> [Data] {
    try effect {
      let fields = try native.invoke(phase, originals: originals)
      try KagemushaOrdinaryIncomingFrameV1.requireResponse(phase, fields: fields)
      return fields
    }
  }
  private func current() throws {
    guard !frozen else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    try native.requireOpen()
  }
  private func effect<T>(_ body: () throws -> T) throws -> T {
    do { try current(); let result = try body(); try current(); return result }
    catch { throw freeze(error) }
  }
  private func freeze(_ failure: Error) -> Error {
    frozen = true; cycle?.lifetime?.retire(); cycle?.carrier = nil; cycle?.response = nil
    try? native.revoke(); return failure
  }
  private func invalid(_ reason: String) -> KagemushaCoreCoordinatorErrorV1 { .invalidFrame(reason) }
}

private final class KagemushaActualIncomingWorkflowV1: KagemushaIncomingWorkflowNativeV1, @unchecked Sendable {
  private let binding: KagemushaOrdinaryIncomingNativeBindingV1
  private let hardware: KagemushaAppAttestOrdinaryIncomingApprovalProviderV1
  init(binding: KagemushaOrdinaryIncomingNativeBindingV1, hardware: KagemushaAppAttestOrdinaryIncomingApprovalProviderV1) {
    self.binding = binding; self.hardware = hardware
  }
  func requireOpen() throws { try binding.requireOpen() }
  func revoke() throws { try binding.revoke() }
  func refreshAccount() throws { try binding.refreshAccount() }
  func prepareMint(finalized: Data, credit: Data) throws -> any KagemushaIncomingApprovalStepV1 {
    Step(holder: try binding.prepareMint(finalized: finalized, credit: credit), hardware: hardware)
  }
  func prepareReceive(request: Data, outgoing: Data, assertion: Data) throws -> any KagemushaIncomingApprovalStepV1 {
    Step(holder: try binding.prepareReceive(request: request, outgoing: outgoing, assertion: assertion), hardware: hardware)
  }
  func selectTerminal(reserve: Data) throws -> any KagemushaIncomingApprovalStepV1 {
    Step(holder: try binding.selectTerminal(reserve: reserve), hardware: hardware)
  }
  func invoke(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data]) throws -> [Data] {
    try binding.invoke(phase, originals: originals)
  }
  private struct Step: KagemushaIncomingApprovalStepV1 {
    let holder: KagemushaNativePreparedOrdinaryIncomingApprovalV1
    let hardware: KagemushaAppAttestOrdinaryIncomingApprovalProviderV1
    func approve() async throws { _ = try await hardware.approve(holder) }
  }
}
