import CryptoKit
import Foundation

/// Data projection of one Native-selected incoming W2 or W1; it is never a capability.
struct KagemushaOrdinaryIncomingSigningProjectionV1: Sendable {
  let fields: [Data]
  let approval: KagemushaOrdinaryCashApprovalProjectionV1
  let keyAlias: String
  let counterFloor: UInt32?
  var platform: UInt8 { fields[4][0] }
  var publicKey: Data { fields[6] }
  var keyID: Data { fields[7] }
  var appID: Data { fields[9] }
  var terminal: Bool { approval.purpose == .monetaryTransition }

  init(_ original: [Data]) throws {
    let f = original.map { Data([UInt8]($0)) }
    guard f.count == 11, f[1].count == 325, f[2].count == 460,
      [0, 7, 8, 9].allSatisfy({ f[$0].count == 32 && f[$0].contains(where: { $0 != 0 }) }),
      (1...KagemushaOrdinaryIncomingFrameV1.signedMaximum).contains(f[3].count),
      [Data([4]), Data([5])].contains(f[4]),
      (1...255).contains(f[5].count), !f[5].contains(0),
      let alias = String(data: f[5], encoding: .utf8), Data(alias.utf8) == f[5],
      f[6].count == 65, f[6].first == 4,
      (try? P256.Signing.PublicKey(x963Representation: f[6])) != nil,
      Data(SHA256.hash(data: f[6])) == f[7] else {
      throw Self.invalid()
    }
    let binding = try KagemushaOrdinaryCashApprovalOriginalBindingV1(operationID: f[0],
      accountBinding: Data(f[1][117..<149]), authorityPolicyDigest: Data(f[1][149..<181]),
      attestedKeyID: f[7], enrollmentDigest: f[8], normalizedGuardDigest: Data(f[1][277..<309]),
      originalSelection: f[2])
    let approval: KagemushaOrdinaryCashApprovalProjectionV1
    switch f[1][52] {
    case 1: approval = try .requireIncomingTerminal(nativeSigningBytes: f[1], nativeFinancialSubject: f[2], binding: binding)
    case 2: approval = try .requireIncomingPreparation(nativeSigningBytes: f[1], nativeFinancialSubject: f[2], binding: binding)
    default: throw Self.invalid()
    }
    let floor: UInt32?
    if f[4] == Data([4]) {
      guard f[10].count == 4, alias == f[7].base64EncodedString() else { throw Self.invalid() }
      floor = f[10].enumerated().reduce(UInt32(0)) { $0 | UInt32($1.element) << ($1.offset * 8) }
    } else {
      guard f[10].isEmpty else { throw Self.invalid() }
      floor = nil
    }
    fields = f; self.approval = approval; keyAlias = alias; counterFloor = floor
  }
  private static func invalid() -> KagemushaCoreCoordinatorErrorV1 {
    .invalidFrame("incoming original W/S/FI/key/App ID/counter differs")
  }
}

/// Only this retained session and actual bridge can construct incoming signing holders.
/// A decoded projection, raw handle, callback or caller-selected key cannot create one.
final class KagemushaOrdinaryIncomingNativeBindingV1: @unchecked Sendable {
  private let session: KagemushaOrdinaryNativeAccountSessionV1
  private let bridge: KagemushaCoreCoordinatorBridgeV1
  private let key: KagemushaNativeCompletedAppKeyOriginalV1
  private let lock = NSRecursiveLock()
  private var unusable = false
  private weak var active: KagemushaNativePreparedOrdinaryIncomingApprovalV1?

  init(session: KagemushaOrdinaryNativeAccountSessionV1, bridge: KagemushaCoreCoordinatorBridgeV1) throws {
    self.session = session; self.bridge = bridge
    key = try bridge.completedAppKeyOriginal()
    try session.requireCurrent(); try key.requireForCoordinator(bridge)
    guard try key.metadata().androidSecurityMask == 0 else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Swift incoming signing requires the original Apple key")
    }
  }
  func requireOpen() throws { try guarded { try bridge.requireOrdinaryDescriptorOpen() } }
  func revoke() throws {
    lock.lock(); defer { lock.unlock() }
    unusable = true; active = nil
    try session.close()
  }
  func refreshAccount() throws {
    try guarded { try session.refreshOriginalAccountClock(); try key.requireForCoordinator(bridge) }
  }
  func invoke(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data] = []) throws -> [Data] {
    guard ![.prepareFinalizedMint, .prepareReceive, .prepareTerminal, .originalPlatformSigning,
      .refreshAccountClock].contains(phase) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming phase requires its original typed entry")
    }
    return try guarded {
      // Long proofs retain historical Native custody, then must renew before a new effect.
      // The Native phase independently enforces that distinction; no managed clock is offered.
      if phase != .proveCandidate && phase != .proveCommit { try current() }
      let result = try bridge.invokeOrdinaryIncoming(phase, originals: originals)
      if phase != .proveCandidate && phase != .proveCommit { try current() }
      return result
    }
  }
  func prepareMint(finalized: Data, credit: Data) throws -> KagemushaNativePreparedOrdinaryIncomingApprovalV1 {
    try prepare(.prepareFinalizedMint, originals: [finalized, credit], terminal: false)
  }
  func prepareReceive(request: Data, outgoing: Data, assertion: Data) throws -> KagemushaNativePreparedOrdinaryIncomingApprovalV1 {
    try prepare(.prepareReceive, originals: [request, outgoing, assertion], terminal: false)
  }
  func selectTerminal(reserve: Data) throws -> KagemushaNativePreparedOrdinaryIncomingApprovalV1 {
    try prepare(.prepareTerminal, originals: [reserve], terminal: true)
  }
  private func prepare(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data], terminal: Bool) throws
    -> KagemushaNativePreparedOrdinaryIncomingApprovalV1 {
    try guarded {
      try current()
      let selected = try bridge.invokeOrdinaryIncoming(phase, originals: originals)
      let projection = try read(operation: selected[0], terminal: terminal)
      guard Array(projection.fields.prefix(4)) == selected else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming original changed after selection")
      }
      let result = KagemushaNativePreparedOrdinaryIncomingApprovalV1(binding: self, projection: projection)
      active = result
      _ = try recheck(result, original: projection)
      return result
    }
  }
  fileprivate func recheck(_ holder: KagemushaNativePreparedOrdinaryIncomingApprovalV1,
    original: KagemushaOrdinaryIncomingSigningProjectionV1) throws -> KagemushaOrdinaryIncomingSigningProjectionV1 {
    try guarded {
      guard active === holder else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
      let fresh = try read(operation: original.approval.operationID, terminal: original.terminal)
      guard fresh.fields == original.fields else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming signing owner original was replaced")
      }
      return fresh
    }
  }
  private func read(operation: Data, terminal: Bool) throws -> KagemushaOrdinaryIncomingSigningProjectionV1 {
    try current()
    let projection = try KagemushaOrdinaryIncomingSigningProjectionV1(bridge.invokeOrdinaryIncoming(
      .originalPlatformSigning, originals: [operation, Data([terminal ? 1 : 2])]))
    let metadata = try key.metadata()
    guard projection.approval.operationID == operation, projection.terminal == terminal,
      projection.fields[3] == metadata.financialCertificateOriginal,
      projection.keyAlias == metadata.keyReference, projection.publicKey == metadata.publicKey,
      projection.keyID == metadata.attestedKeyID, projection.fields[8] == metadata.credentialDigest,
      (projection.platform == 4) == (metadata.androidSecurityMask == 0) else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("incoming key/FI differs from completed same-owner originals")
    }
    try current()
    return projection
  }
  private func current() throws {
    try session.requireCurrent(); try key.requireForCoordinator(bridge); try session.requireCurrent()
  }
  private func guarded<T>(_ body: () throws -> T) throws -> T {
    lock.lock(); defer { lock.unlock() }
    guard !unusable else { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    do { try bridge.requireOrdinaryDescriptorOpen(); return try body() }
    catch { unusable = true; active = nil; try? session.close(); throw error }
  }
}

/// Opaque original incoming W2/W1 selected by the actual Native account and key owner.
/// Public data parsing cannot construct, replace or reset this holder or its invocation fence.
public final class KagemushaNativePreparedOrdinaryIncomingApprovalV1: @unchecked Sendable {
  private let binding: KagemushaOrdinaryIncomingNativeBindingV1
  private let original: KagemushaOrdinaryIncomingSigningProjectionV1
  fileprivate init(binding: KagemushaOrdinaryIncomingNativeBindingV1,
    projection: KagemushaOrdinaryIncomingSigningProjectionV1) {
    self.binding = binding; original = projection
  }
  func recheck() throws -> KagemushaOrdinaryIncomingSigningProjectionV1 {
    try binding.recheck(self, original: original)
  }
  func recover() throws -> (state: UInt8, raw: Data, wrapper: Data) {
    try recovery(original.terminal ? .recoverTerminal : .recoverPreparation)
  }
  func fence() throws -> (state: UInt8, raw: Data, wrapper: Data) {
    try recovery(original.terminal ? .fenceTerminal : .fencePreparation)
  }
  private func recovery(_ phase: KagemushaOrdinaryIncomingPhaseV1) throws -> (state: UInt8, raw: Data, wrapper: Data) {
    _ = try recheck()
    let fields = try binding.invoke(phase)
    _ = try recheck()
    return (fields[0][0], fields[1], fields[2])
  }
  func retainOriginal(_ raw: Data) throws {
    _ = try recheck()
    let result = try binding.invoke(original.terminal ? .retainTerminalAssertion : .retainPreparationAssertion,
      originals: [raw])
    guard result == [Data(SHA256.hash(data: raw))] else {
      try? binding.revoke()
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Native incoming retained a different raw assertion")
    }
    _ = try recheck()
  }
  func consumedReceipt(_ evidence: KagemushaIncomingAppAttestOriginalV1) throws
    -> KagemushaNativeOrdinaryIncomingApprovalReceiptV1 {
    let p = try recheck(), held = try recover()
    guard p.platform == 4, held.state == 2, held.raw == evidence.rawAssertion,
      evidence.clientDataHash == p.approval.clientDataHash, !held.wrapper.isEmpty else {
      throw KagemushaCoreCoordinatorErrorV1.invalidFrame("Native has not consumed this exact incoming Apple assertion")
    }
    _ = try recheck()
    return KagemushaNativeOrdinaryIncomingApprovalReceiptV1(projection: p, evidence: evidence, wrapper: held.wrapper)
  }
}

/// Exact Native-consumed incoming approval identity; it supplies no committed monetary authority.
public struct KagemushaNativeOrdinaryIncomingApprovalReceiptV1: Sendable {
  public let operationID: Data
  public let purpose: KagemushaOrdinaryCashApprovalPurposeV1
  public let keyID: Data
  public let keyAlias: String
  public let signingDigest: Data
  public let rawAssertionDigest: Data
  public let observedCounter: UInt32
  public let canonicalApprovalOriginal: Data
  fileprivate init(projection p: KagemushaOrdinaryIncomingSigningProjectionV1,
    evidence: KagemushaIncomingAppAttestOriginalV1, wrapper: Data) {
    operationID = p.approval.operationID; purpose = p.approval.purpose
    keyID = p.keyID; keyAlias = p.keyAlias; signingDigest = evidence.clientDataHash
    rawAssertionDigest = Data(SHA256.hash(data: evidence.rawAssertion))
    observedCounter = evidence.observedCounter; canonicalApprovalOriginal = Data(wrapper)
  }
}

struct KagemushaIncomingAppAttestOriginalV1: Sendable {
  let rawAssertion: Data
  let observedCounter: UInt32
  let clientDataHash: Data
  init(raw: Data, projection p: KagemushaOrdinaryIncomingSigningProjectionV1,
    expectedRelease: KagemushaAppAttestExpectedReleaseV1) throws {
    guard p.platform == 4, let floor = p.counterFloor, floor < UInt32.max,
      (1...4096).contains(raw.count) else { throw KagemushaAppAttestEvidenceErrorV1.invalidCanonicalSelection }
    let evidence = try KagemushaAppAttestAssertionEvidenceV1(rawAssertion: raw,
      clientDataHash: p.approval.clientDataHash, expectedAppIDHash: p.appID,
      expectedRelease: expectedRelease, enrolledAssertionPublicKeyX963: p.publicKey)
    guard (37...206).contains(evidence.authenticatorData.count),
      [UInt8(0x40), 0xc0].contains(evidence.authenticatorData[32]), evidence.signCount > floor else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionCounterMismatch
    }
    rawAssertion = Data(evidence.rawAssertion); observedCounter = evidence.signCount
    clientDataHash = p.approval.clientDataHash
  }
}

/// A store must durably archive this distinct actual incoming Native receipt.
public protocol KagemushaAppAttestOrdinaryIncomingIntentStoringV1: KagemushaAppAttestAssertionIntentStoringV1 {
  func advanceAfterNativeIncomingApproval(keyID: String, counter: UInt32,
    signingDigest: Data, rawAssertion: Data, receipt: KagemushaNativeOrdinaryIncomingApprovalReceiptV1) throws
}
