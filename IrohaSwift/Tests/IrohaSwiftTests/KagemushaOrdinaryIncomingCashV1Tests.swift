import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Deterministic workflow/HTTP controls. Scripted owners never qualify Native monetary custody.
final class KagemushaOrdinaryIncomingCashV1Tests: XCTestCase {
  func testMintAndReceiveCompleteDistinctW2W1ProofReserveCommitStateAckOrder() async throws {
    for receive in [false, true] {
      let native = ScriptedNative(), transport = Transport()
      let workflow = make(native, transport)
      let first = try await run(workflow, receive: receive)
      XCTAssertEqual(first.reserveRequestOriginalDigest, native.digest(.reserveTransport))
      XCTAssertEqual(first.commitRequestOriginalDigest, native.digest(.commitTransport))
      XCTAssertEqual(native.events(), ["refresh", "fi", receive ? "prepare-receive" : "prepare-mint",
        "approve-w2", "phase:5", "refresh", "fi", "phase:6", "phase:7", "refresh", "fi",
        "select-terminal", "approve-w1", "phase:12", "refresh", "fi", "phase:13", "phase:7",
        "refresh", "fi", "phase:14", "phase:15"])
      let before = native.events()
      let replay = try await run(workflow, receive: receive)
      XCTAssertEqual(replay.commitRequestOriginalDigest, first.commitRequestOriginalDigest)
      XCTAssertEqual(native.events(), before)
      XCTAssertEqual(transport.snapshots().count, 2)
      for carrier in transport.carriers() { XCTAssertThrowsError(try carrier.requireCurrent()) }
      try await workflow.releaseCompletedCycle()
      _ = try await run(workflow, receive: receive)
      XCTAssertEqual(transport.snapshots().count, 4)
    }
  }

  func testHttpUncertaintyRetriesIdenticalCarrierWithoutRepeatingApprovalOrProof() async throws {
    let native = ScriptedNative(), transport = Transport(failures: 1), workflow = make(native, transport)
    do { _ = try await run(workflow); XCTFail("HTTP interruption must propagate") } catch Failure.http {}
    XCTAssertEqual(transport.snapshots().count, 1)
    do {
      _ = try await workflow.beginOrResumeFinalizedMint(finalizedTopupOriginal: Data([99]), mintCreditOriginal: Data([2]))
      XCTFail("unfinished originals must not change")
    } catch {}
    XCTAssertEqual(native.closeCount(), 0)
    _ = try await run(workflow)
    let observations = transport.snapshots()
    XCTAssertEqual(observations.count, 3)
    XCTAssertEqual(observations[0].0, observations[1].0)
    XCTAssertEqual(observations[0].1, observations[1].1)
    let held = transport.carriers(); XCTAssertTrue(held[0] === held[1])
    XCTAssertEqual(native.events().filter { $0 == "approve-w2" }.count, 1)
    XCTAssertEqual(native.events().filter { $0 == "phase:5" }.count, 1)
    XCTAssertEqual(native.events().filter { $0 == "phase:6" }.count, 1)
    XCTAssertThrowsError(try held[0].requireCurrent())
  }

  func testUnknownNativeOrHardwareEffectsFreezePermanentlyWithoutAnotherInvocation() async throws {
    for failure in ["approve-w2", "phase:7", "phase:14"] {
      let native = ScriptedNative(failAt: failure), transport = Transport(), workflow = make(native, transport)
      do { _ = try await run(workflow); XCTFail("unknown effect must propagate") } catch Failure.effect {}
      let before = native.events()
      do { _ = try await run(workflow); XCTFail("frozen owner must not retry") } catch {}
      XCTAssertEqual(native.events(), before)
      XCTAssertEqual(native.closeCount(), 1)
      for carrier in transport.carriers() { XCTAssertThrowsError(try carrier.requireCurrent()) }
      do { try await workflow.releaseCompletedCycle(); XCTFail("failed cycle cannot release") } catch {}
    }
  }

  func testNativeAcknowledgedTransportsSkipHttpAndPreserveBothOriginalDigests() async throws {
    let native = ScriptedNative(acknowledgedTransport: true), transport = Transport()
    let result = try await run(make(native, transport))
    XCTAssertEqual(result.reserveRequestOriginalDigest, native.digest(.reserveTransport))
    XCTAssertEqual(result.commitRequestOriginalDigest, native.digest(.commitTransport))
    XCTAssertTrue(transport.snapshots().isEmpty)
    XCTAssertFalse(native.events().contains("phase:7"))
    XCTAssertEqual(native.events().suffix(2), ["phase:14", "phase:15"])
  }

  func testCarrierStreamsCanonicalBase64AcrossChunkBoundariesAndRefusesRetiredLifetime() throws {
    let native = ScriptedNative(), lifetime = KagemushaOrdinaryIncomingHttpLifetimeV1(native: native)
    let request = Data((0..<196_608).map { UInt8(truncatingIfNeeded: $0) })
    let proof = Data((0..<147_461).map { UInt8(truncatingIfNeeded: $0 / 7) })
    let signature = Data(repeating: 5, count: 64)
    let fields = [Data([0]), request, signature, proof, Data(SHA256.hash(data: request))]
    let carrier = try KagemushaOrdinaryIncomingHttpOriginalV1(fields: fields, commit: true, lifetime: lifetime)
    let output = OutputStream.toMemory(); output.open(); defer { output.close() }
    try carrier.writeBody(to: output)
    let bytes = try XCTUnwrap(output.property(forKey: .dataWrittenToMemoryStreamKey) as? Data)
    let object = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: String])
    XCTAssertEqual(Set(object.keys), Set(["schema", "canonical_request_base64", "account_signature_base64", "proof_bundle_original_base64"]))
    XCTAssertEqual(object["canonical_request_base64"], request.base64EncodedString())
    XCTAssertEqual(object["account_signature_base64"], signature.base64EncodedString())
    XCTAssertEqual(object["proof_bundle_original_base64"], proof.base64EncodedString())
    XCTAssertEqual(carrier.requestOriginalDigest, fields[4])
    let second = try KagemushaOrdinaryIncomingHttpOriginalV1(fields: fields, commit: true, lifetime: lifetime)
    XCTAssertEqual(carrier.requestID, second.requestID)
    lifetime.retire()
    XCTAssertThrowsError(try carrier.writeBody(to: output))
    XCTAssertThrowsError(try second.requireCurrent())
    var changed = fields; changed[4][0] ^= 1
    XCTAssertThrowsError(try KagemushaOrdinaryIncomingHttpOriginalV1(fields: changed, commit: true, lifetime: lifetime))
  }

  private func make(_ native: ScriptedNative, _ transport: Transport) -> KagemushaOrdinaryIncomingCashV1 {
    KagemushaOrdinaryIncomingCashV1(native: native, transport: transport, refreshFinancial: { _ in try native.record("fi") })
  }
  private func run(_ workflow: KagemushaOrdinaryIncomingCashV1, receive: Bool = false) async throws -> KagemushaOrdinaryIncomingAcknowledgementV1 {
    if receive { return try await workflow.beginOrResumeReceive(receiverRequestID: Data(repeating: 1, count: 32),
      fullOutgoingOriginal: Data([2]), fullReceivedAssertionOriginal: Data([3])) }
    return try await workflow.beginOrResumeFinalizedMint(finalizedTopupOriginal: Data([1]), mintCreditOriginal: Data([2]))
  }
  private enum Failure: Error { case http, effect }
  private final class ScriptedNative: KagemushaIncomingWorkflowNativeV1, @unchecked Sendable {
    private let lock = NSRecursiveLock()
    private var log: [String] = [], closed = 0, pending = Data()
    private let failure: String?
    private let acknowledged: Bool
    init(failAt: String? = nil, acknowledgedTransport: Bool = false) { failure = failAt; acknowledged = acknowledgedTransport }
    func events() -> [String] { lock.lock(); defer { lock.unlock() }; return log }
    func closeCount() -> Int { lock.lock(); defer { lock.unlock() }; return closed }
    func requireOpen() throws { lock.lock(); defer { lock.unlock() }; guard closed == 0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable } }
    func revoke() throws { lock.lock(); defer { lock.unlock() }; if closed == 0 { closed = 1 } }
    func record(_ event: String) throws { lock.lock(); defer { lock.unlock() }; try requireOpen(); log.append(event); if failure == event { throw Failure.effect } }
    func refreshAccount() throws { try record("refresh") }
    func prepareMint(finalized: Data, credit: Data) throws -> any KagemushaIncomingApprovalStepV1 {
      XCTAssertEqual(finalized, Data([1])); XCTAssertEqual(credit, Data([2])); try record("prepare-mint"); return Step(owner: self, terminal: false)
    }
    func prepareReceive(request: Data, outgoing: Data, assertion: Data) throws -> any KagemushaIncomingApprovalStepV1 {
      XCTAssertEqual(request, Data(repeating: 1, count: 32)); XCTAssertEqual(outgoing, Data([2])); XCTAssertEqual(assertion, Data([3]))
      try record("prepare-receive"); return Step(owner: self, terminal: false)
    }
    func selectTerminal(reserve: Data) throws -> any KagemushaIncomingApprovalStepV1 {
      XCTAssertEqual(reserve, digest(.reserveTransport)); try record("select-terminal"); return Step(owner: self, terminal: true)
    }
    func digest(_ phase: KagemushaOrdinaryIncomingPhaseV1) -> Data { Data(SHA256.hash(data: Data([phase.rawValue]))) }
    func invoke(_ phase: KagemushaOrdinaryIncomingPhaseV1, originals: [Data]) throws -> [Data] {
      lock.lock(); defer { lock.unlock() }; try record("phase:\(phase.rawValue)")
      switch phase {
      case .proveCandidate, .proveCommit: XCTAssertTrue(originals.isEmpty); return [digest(phase)]
      case .reserveTransport, .commitTransport:
        pending = digest(phase)
        return [Data([acknowledged ? 2 : 0]), Data([phase.rawValue]), Data(repeating: 1, count: 64), Data([8]), pending]
      case .retainGlobalResult:
        XCTAssertEqual(originals, [Data([1]), Data([2]), Data([3])]); return [pending]
      case .advanceState, .acknowledge:
        XCTAssertEqual(originals, [digest(.commitTransport)]); return []
      default: XCTFail("unexpected Native phase \(phase)"); throw Failure.effect
      }
    }
    private struct Step: KagemushaIncomingApprovalStepV1 {
      let owner: ScriptedNative; let terminal: Bool
      func approve() async throws { try owner.record(terminal ? "approve-w1" : "approve-w2") }
    }
  }
  private final class Transport: KagemushaOrdinaryIncomingOriginalTransportV1, @unchecked Sendable {
    private let lock = NSLock()
    private var held: [KagemushaOrdinaryIncomingHttpOriginalV1] = [], observations: [(String, Data)] = []
    private var failures: Int
    init(failures: Int = 0) { self.failures = failures }
    func snapshots() -> [(String, Data)] { lock.lock(); defer { lock.unlock() }; return observations }
    func carriers() -> [KagemushaOrdinaryIncomingHttpOriginalV1] { lock.lock(); defer { lock.unlock() }; return held }
    private func retain(_ original: KagemushaOrdinaryIncomingHttpOriginalV1, _ body: Data) throws {
      lock.lock(); defer { lock.unlock() }; held.append(original); observations.append((original.requestID, body))
      if failures > 0 { failures -= 1; throw Failure.http }
    }
    func exchange(_ original: KagemushaOrdinaryIncomingHttpOriginalV1) async throws -> Data {
      let output = OutputStream.toMemory(); output.open(); defer { output.close() }
      try original.writeBody(to: output)
      try retain(original, XCTUnwrap(output.property(forKey: .dataWrittenToMemoryStreamKey) as? Data))
      return Data("{\"signed_result_original_base64\":\"AQ==\",\"data_record_original_base64\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}".utf8)
    }
  }
}
