import Foundation
import XCTest

@testable import IrohaSwift

private final class JournalRaceResults: @unchecked Sendable {
  private let lock = NSLock()
  private var successes = 0

  func succeeded() {
    lock.lock()
    successes += 1
    lock.unlock()
  }

  var count: Int {
    lock.lock()
    defer { lock.unlock() }
    return successes
  }
}

final class KagemushaAppAttestFileIntentStoreV1Tests: XCTestCase {
  private func privateDirectory() throws -> URL {
    let directory = FileManager.default.temporaryDirectory
      .appendingPathComponent("kagemusha-app-attest-journal-\(UUID().uuidString)", isDirectory: true)
    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false,
      attributes: [.posixPermissions: NSNumber(value: 0o700)])
    addTeardownBlock { try? FileManager.default.removeItem(at: directory) }
    return directory
  }

  func testBootstrapReserveCompleteAndReopen() throws {
    let directory = try privateDirectory()
    let selection = Data(repeating: 0x21, count: 32)
    let rawAssertion = Data([0xa2, 0x01, 0x02])
    let first = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "enrolled-key", initialCounter: 3)
    XCTAssertEqual(try first.load(keyID: "enrolled-key"), .ready(counter: 3))
    try first.reserve(keyID: "enrolled-key", previousCounter: 3,
      selectionDigest: selection)
    let reopened = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    XCTAssertEqual(try reopened.load(keyID: "enrolled-key"),
      .pending(previousCounter: 3, selectionDigest: selection))
    try reopened.complete(keyID: "enrolled-key", counter: 4,
      selectionDigest: selection, rawAssertion: rawAssertion)
    let afterCrash = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    XCTAssertEqual(try afterCrash.load(keyID: "enrolled-key"),
      .complete(counter: 4, selectionDigest: selection, rawAssertion: rawAssertion))
    XCTAssertThrowsError(try afterCrash.reserve(keyID: "enrolled-key", previousCounter: 4,
      selectionDigest: Data(repeating: 0x22, count: 32)))
    XCTAssertEqual(try afterCrash.load(keyID: "enrolled-key"),
      .complete(counter: 4, selectionDigest: selection, rawAssertion: rawAssertion))
    XCTAssertThrowsError(try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "another-key"))
  }

  func testPendingNeverClearsOnRecreationOrConflict() throws {
    let directory = try privateDirectory()
    let selection = Data(repeating: 0x31, count: 32)
    let other = Data(repeating: 0x32, count: 32)
    let first = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "key")
    try first.reserve(keyID: "key", previousCounter: 0, selectionDigest: selection)
    let restarted = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    XCTAssertThrowsError(try restarted.reserve(keyID: "key", previousCounter: 0,
      selectionDigest: selection))
    XCTAssertThrowsError(try restarted.reserve(keyID: "key", previousCounter: 0,
      selectionDigest: other))
    XCTAssertThrowsError(try restarted.complete(keyID: "key", counter: 1,
      selectionDigest: other, rawAssertion: Data([1])))
    XCTAssertThrowsError(try restarted.complete(keyID: "another-key", counter: 1,
      selectionDigest: selection, rawAssertion: Data([1])))
    XCTAssertEqual(try restarted.load(keyID: "key"),
      .pending(previousCounter: 0, selectionDigest: selection))
  }

  func testTornWriteAndMissingRecordFreezeLane() throws {
    let directory = try privateDirectory()
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "key")
    let temporary = directory.appendingPathComponent("intent.pending-write")
    try Data([0x01]).write(to: temporary)
    XCTAssertThrowsError(try store.load(keyID: "key"))
    XCTAssertThrowsError(try store.reserve(keyID: "key", previousCounter: 0,
      selectionDigest: Data(repeating: 1, count: 32)))
    XCTAssertTrue(FileManager.default.fileExists(atPath: temporary.path))
    try FileManager.default.removeItem(at: temporary)
    try FileManager.default.removeItem(at: directory.appendingPathComponent("intent.bin"))
    XCTAssertThrowsError(try KagemushaAppAttestFileIntentStoreV1(
      directoryURL: directory).load(keyID: "key"))
  }

  func testCorruptionAndSymlinksAreRejected() throws {
    let directory = try privateDirectory()
    let store = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "key")
    let record = directory.appendingPathComponent("intent.bin")
    var bytes = try Data(contentsOf: record)
    bytes[10] ^= 1
    try bytes.write(to: record)
    XCTAssertThrowsError(try store.load(keyID: "key"))
    try FileManager.default.removeItem(at: record)
    try FileManager.default.createSymbolicLink(at: record, withDestinationURL: directory
      .appendingPathComponent("intent.lock"))
    XCTAssertThrowsError(try store.load(keyID: "key"))
    let link = directory.deletingLastPathComponent()
      .appendingPathComponent("kagemusha-symlink-\(UUID().uuidString)")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: directory)
    defer { try? FileManager.default.removeItem(at: link) }
    XCTAssertThrowsError(try KagemushaAppAttestFileIntentStoreV1(directoryURL: link))
  }

  func testTwoInstancesCannotReserveSameCounter() throws {
    let directory = try privateDirectory()
    let first = try KagemushaAppAttestFileIntentStoreV1.bootstrapNew(
      directoryURL: directory, keyID: "key")
    let second = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directory)
    let results = JournalRaceResults()
    DispatchQueue.concurrentPerform(iterations: 2) { index in
      let store = index == 0 ? first : second
      do {
        try store.reserve(keyID: "key", previousCounter: 0,
          selectionDigest: Data(repeating: UInt8(index + 1), count: 32))
        results.succeeded()
      } catch {}
    }
    XCTAssertEqual(results.count, 1)
    let state = try first.load(keyID: "key")
    switch state {
    case .pending(let counter, let digest):
      XCTAssertEqual(counter, 0)
      XCTAssertTrue(digest == Data(repeating: 1, count: 32)
        || digest == Data(repeating: 2, count: 32))
    default:
      XCTFail("Exactly one durable reservation must remain")
    }
  }
}
