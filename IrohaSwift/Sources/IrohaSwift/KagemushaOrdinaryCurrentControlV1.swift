import Foundation

/// Detached historical correlation only. These hashes cannot recreate FI or monetary custody.
public struct KagemushaOrdinaryCurrentControlOriginalsV1: Sendable {
  public let requestID: String
  public let requestOriginalDigest: Data
  public let signedControlOriginalDigest: Data
  public let authorityOriginalDigest: Data
  init(requestID: String, request: Data, signed: Data, authority: Data) {
    self.requestID = requestID
    requestOriginalDigest = KagemushaOrdinaryOutgoingFrameV1.sha(request)
    signedControlOriginalDigest = KagemushaOrdinaryOutgoingFrameV1.sha(signed)
    authorityOriginalDigest = KagemushaOrdinaryOutgoingFrameV1.sha(authority)
  }
}

/// Actual retained Native account/current FI workflow. It returns no managed money readiness.
public actor KagemushaOrdinaryCurrentControlV1 {
  private let session: KagemushaOrdinaryNativeAccountSessionV1
  private let binding: KagemushaOrdinaryNativeBindingV1
  private let transport: any KagemushaOrdinaryCurrentControlOriginalTransportV1
  private var active = false
  private var frozen = false
  private final class Cycle {
    var started = false
    var request: Data?
    var carrier: KagemushaOrdinaryCurrentControlHttpOriginalV1?
    var lifetime: KagemushaOrdinaryHttpLifetimeV1?
    var response: Data?
    var intakeStarted = false
    var completed: KagemushaOrdinaryCurrentControlOriginalsV1?
  }
  private var cycle = Cycle()

  public init(session: KagemushaOrdinaryNativeAccountSessionV1,
    transport: any KagemushaOrdinaryCurrentControlOriginalTransportV1) throws {
    self.session = session; self.transport = transport
    binding = try session.originalCoordinator().ordinaryNativeTransportBinding()
  }
  init(session: KagemushaOrdinaryNativeAccountSessionV1, binding: KagemushaOrdinaryNativeBindingV1,
    transport: any KagemushaOrdinaryCurrentControlOriginalTransportV1) {
    self.session = session; self.binding = binding; self.transport = transport
  }

  /// HTTP uncertainty resumes the sole exact request/signature/UUID without any new Native nonce.
  public func beginOrResumeCurrentFinancialControl() async throws -> KagemushaOrdinaryCurrentControlOriginalsV1 {
    try await perform(freshCompletedCycle: false)
  }
  /// Fresh actual wallet/World/FI read only after the prior cycle completed.
  public func refreshCurrentFinancialControl() async throws {
    _ = try await perform(freshCompletedCycle: true)
  }
  private func perform(freshCompletedCycle: Bool) async throws -> KagemushaOrdinaryCurrentControlOriginalsV1 {
    guard !active else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("current FI refresh already active") }
    active = true; defer { active = false }
    do { try current() } catch { throw freeze(error) }
    if freshCompletedCycle {
      guard !cycle.started || cycle.completed != nil else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("unfinished FI original must be resumed unchanged")
      }
      if cycle.completed != nil { cycle = Cycle() }
    }
    let original = cycle
    if let completed = original.completed { return completed }
    if !original.started {
      original.started = true
      do {
        let reservation = try binding.startup(1, id: 0)
        _ = try binding.startup(6, id: reservation.id) // Native fetches all four signed wallet/World originals.
        try current()
        let reserved = try binding.current(1)
        original.request = Data(reserved[0])
        let signed = try binding.current(2)
        guard signed[0] == original.request else {
          throw KagemushaCoreCoordinatorErrorV1.invalidFrame("retained FI account request changed")
        }
        try current()
        let lifetime = KagemushaOrdinaryHttpLifetimeV1(session: session, binding: binding)
        original.lifetime = lifetime
        original.carrier = try KagemushaOrdinaryCurrentControlHttpOriginalV1(
          request: signed[0], signature: signed[1], requireOriginal: { try lifetime.requireCurrent() })
      } catch { throw freeze(error) }
    }
    if original.response == nil {
      guard let carrier = original.carrier else { throw freeze(KagemushaCoreCoordinatorErrorV1.unavailable) }
      let response: Data
      do { response = try await transport.exchange(carrier) }
      catch {
        do { try current() } catch { throw freeze(error) }
        throw error // Sole safe retry: HTTP only, same body/signature/request ID.
      }
      do {
        try current()
        guard (1...carrier.maximumResponseBytes).contains(response.count) else {
          throw KagemushaCoreCoordinatorErrorV1.invalidFrame("FI HTTP response exceeds bound")
        }
        original.response = Data(response)
      } catch { throw freeze(error) }
    }
    do {
      try current()
      guard !original.intakeStarted, let raw = original.response, let request = original.request,
        let carrier = original.carrier else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      let originals = try KagemushaOrdinaryFinancialHttpCodecV1.responseOriginals(.currentControl, raw: raw)
      let acknowledgment = KagemushaOrdinaryCurrentControlOriginalsV1(requestID: carrier.requestID,
        request: request, signed: originals[0], authority: originals[1])
      original.intakeStarted = true; original.lifetime?.retire() // Own before Native's possible fsync.
      _ = try binding.current(3, signed: originals[0], authority: originals[1])
      try current()
      original.completed = acknowledgment; original.response = nil
      return acknowledgment
    } catch { throw freeze(error) }
  }
  private func current() throws {
    guard !frozen else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    try session.requireCurrent(); try binding.requireOpen(); try session.requireCurrent()
  }
  private func freeze(_ failure: Error) -> Error {
    frozen = true; cycle.lifetime?.retire(); cycle.response = nil
    try? binding.revoke()
    return failure
  }
}
