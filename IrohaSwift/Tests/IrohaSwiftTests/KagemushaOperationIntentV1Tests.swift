import Foundation
import XCTest
@testable import IrohaSwift

final class TestOperationIntentStore: KagemushaOperationIntentStoringV1 {
  let exclusiveLock = NSRecursiveLock()
  let applicationScope = Data("account/runtime-test-scope".utf8)
  var records: [String: KagemushaOperationIntentV1] = [:]
  var failAfterSave = false
  var corruptReadback = false
  func key(_ operation: UInt8, _ id: Data) -> String { "\(operation)/\(id.base64EncodedString())" }
  func load(operation: UInt8, operationID: Data) throws -> KagemushaOperationIntentV1? {
    if corruptReadback { throw NSError(domain: "corrupt", code: 1) }
    return records[key(operation, operationID)]
  }
  func pending(operation: UInt8, purpose: String, qualificationScope: Data?) throws -> [KagemushaOperationIntentV1] {
    records.values.filter { $0.operation == operation && $0.purpose == purpose && !$0.acknowledged
      && (qualificationScope == nil || $0.qualificationScope == qualificationScope) }
  }
  func save(_ intent: KagemushaOperationIntentV1) throws {
    if let prior = records[key(intent.operation, intent.operationID)] { try intent.validateSuccessor(of: prior) }
    records[key(intent.operation, intent.operationID)] = intent
    if failAfterSave { throw NSError(domain: "lost-response-after-save", code: 1) }
  }
}

func testOperationIntentOwner() -> KagemushaOperationIntentOwnerV1 {
  KagemushaOperationIntentOwnerV1(store: TestOperationIntentStore())
}

final class KagemushaOperationIntentV1Tests: XCTestCase {
  private let scope = Data("credential/epoch".utf8)
  private func bootstrap(_ id: Data) throws -> Data {
    try KagemushaDeviceOperationCodecV1.encodeControlCommand(.bootstrapAggregateState(operationID: id))
  }
  func testLostSaveResponseAndOwnerRestartResumeExactInternalIDAndCommand() throws {
    let store = TestOperationIntentStore()
    let first = KagemushaOperationIntentOwnerV1(store: store, generateID: { Data(repeating: 1, count: 32) })
    store.failAfterSave = true
    XCTAssertThrowsError(try first.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: scope, command: bootstrap))
    store.failAfterSave = false
    let restarted = KagemushaOperationIntentOwnerV1(store: store, generateID: { XCTFail("retry generated new ID"); return Data() })
    let recovered = try restarted.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: scope, command: bootstrap)
    XCTAssertEqual(recovered.operationID, Data(repeating: 1, count: 32))
    XCTAssertEqual(recovered.canonicalCommand, try bootstrap(recovered.operationID))
    XCTAssertEqual(recovered.publicBinding, recovered.canonicalCommand)
  }
  func testReadOperationsCannotEnterDurableIntentStore() throws {
    let store = TestOperationIntentStore(), owner = KagemushaOperationIntentOwnerV1(store: TestOperationIntentStore())
    for operation: UInt8 in [1, 13, 18, 21] {
      XCTAssertThrowsError(try owner.begin(operation: operation, purpose: "read", arguments: Data(),
        qualificationScope: scope) { _ in Data([1]) })
      XCTAssertThrowsError(try KagemushaOperationIntentV1(applicationScope: store.applicationScope,
        qualificationScope: scope, operation: operation, operationID: Data(repeating: 1, count: 32),
        purpose: "read", arguments: Data(), publicBinding: Data([1])))
    }
    XCTAssertTrue(store.records.isEmpty)
  }
  func testConflictingIDAndCommandCannotOverwriteSavedIntent() throws {
    let store = TestOperationIntentStore(); let owner = KagemushaOperationIntentOwnerV1(store: store)
    let id = Data(repeating: 2, count: 32)
    try owner.reserve(operation: 5, operationID: id, binding: Data([1]), qualificationScope: scope)
    XCTAssertThrowsError(try owner.reserve(operation: 5, operationID: id, binding: Data([2]), qualificationScope: scope))
    try owner.willDispatch(operation: 5, operationID: id, command: Data([3]), qualificationScope: scope)
    XCTAssertThrowsError(try owner.willDispatch(operation: 5, operationID: id, command: Data([4]), qualificationScope: scope))
    XCTAssertThrowsError(try owner.willDispatch(operation: 5, operationID: id, command: Data([3]), qualificationScope: Data([9])))
    XCTAssertEqual(try store.load(operation: 5, operationID: id)?.canonicalCommand, Data([3]))
  }
  func testAcknowledgementRequiresAcceptedReplyAndPreservesHistory() throws {
    let store = TestOperationIntentStore(); let owner = KagemushaOperationIntentOwnerV1(store: store)
    let value = try owner.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: scope, command: bootstrap)
    XCTAssertThrowsError(try owner.acknowledge(operation: 20, operationID: value.operationID, canonicalReply: Data([4])))
    try owner.accepted(operation: 20, operationID: value.operationID, command: value.canonicalCommand!,
      reply: Data([4]), authenticator: Data(repeating: 1, count: 64), qualificationScope: scope)
    XCTAssertThrowsError(try owner.acknowledge(operation: 20, operationID: value.operationID, canonicalReply: Data([5])))
    XCTAssertThrowsError(try owner.acknowledge(operation: 20, operationID: value.operationID, canonicalReply: Data([4])))
    try owner.completedResult(operation: 20, operationID: value.operationID, canonicalResult: Data([8]))
    XCTAssertThrowsError(try owner.acknowledgeDurableResult(operationID: value.operationID, canonicalResult: Data([9])))
    try owner.acknowledgeDurableResult(operationID: value.operationID, canonicalResult: Data([8]))
    XCTAssertEqual(store.records.count, 1)
    XCTAssertTrue(try XCTUnwrap(store.load(operation: 20, operationID: value.operationID)).acknowledged)
    XCTAssertTrue(try owner.pendingInternal(operation: 20).isEmpty)
  }
  func testZeroIDAndCorruptStoreStopBeforeReservation() throws {
    let store = TestOperationIntentStore()
    let owner = KagemushaOperationIntentOwnerV1(store: store, generateID: { Data(repeating: 0, count: 32) })
    XCTAssertThrowsError(try owner.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: scope, command: bootstrap))
    XCTAssertTrue(store.records.isEmpty)
    store.corruptReadback = true
    XCTAssertThrowsError(try owner.reserve(operation: 5, operationID: Data(repeating: 2, count: 32),
      binding: Data([1]), qualificationScope: scope))
    XCTAssertTrue(store.records.isEmpty)
  }

  func testRecreatedOwnerRequiresExactAcceptedReplyAuthenticatorAndQualification() throws {
    let originalStore = TestOperationIntentStore()
    let original = KagemushaOperationIntentOwnerV1(store: originalStore)
    let value = try original.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: scope, command: bootstrap)
    let command = try XCTUnwrap(value.canonicalCommand)
    let reply = Data([4]), authenticator = Data(repeating: 1, count: 64)
    try original.accepted(operation: 20, operationID: value.operationID, command: command,
      reply: reply, authenticator: authenticator, qualificationScope: scope)

    // Recreate both store contents and owner from serialized records, with no retained client state.
    let archive = try JSONEncoder().encode(originalStore.records)
    let reopenedStore = TestOperationIntentStore()
    reopenedStore.records = try JSONDecoder().decode([String: KagemushaOperationIntentV1].self, from: archive)
    let reopened = KagemushaOperationIntentOwnerV1(store: reopenedStore,
      generateID: { XCTFail("retry generated a replacement identity"); return Data() })
    let saved = reopenedStore.records
    let retry = try reopened.begin(operation: 20, purpose: "internal-20", arguments: Data(),
      qualificationScope: Data("current successor qualification".utf8), command: bootstrap)
    XCTAssertEqual(retry.qualificationScope, scope, "An unresolved mutation retains its original qualification")
    for (changedReply, changedAuthenticator, changedQualification) in [
      (Data([5]), authenticator, scope),
      (reply, Data(repeating: 2, count: 64), scope),
      (reply, authenticator, Data("current successor qualification".utf8)),
    ] {
      XCTAssertThrowsError(try reopened.accepted(operation: 20, operationID: value.operationID,
        command: command, reply: changedReply, authenticator: changedAuthenticator,
        qualificationScope: changedQualification))
      XCTAssertEqual(reopenedStore.records, saved, "Conflicting accepted evidence cannot replace the original")
    }
    reopenedStore.failAfterSave = true
    try reopened.accepted(operation: 20, operationID: value.operationID, command: command,
      reply: reply, authenticator: authenticator, qualificationScope: scope)
    XCTAssertEqual(reopenedStore.records, saved, "An exact retry neither rewrites nor acknowledges evidence")
    XCTAssertFalse(try XCTUnwrap(reopenedStore.load(operation: 20, operationID: value.operationID)).acknowledged)
  }
}
