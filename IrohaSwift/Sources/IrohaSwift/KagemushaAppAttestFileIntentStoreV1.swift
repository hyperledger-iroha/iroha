import CryptoKit
import Darwin
import Foundation

@_silgen_name("flock")
private func kagemushaFlock(_ descriptor: Int32, _ operation: Int32) -> Int32

/// A process-shared, crash-safe journal for raw App Attest assertion evidence.
///
/// The caller supplies a dedicated, existing, owner-only directory in app-private storage.
/// `bootstrapNew` is the only way to establish its first key and counter. Missing, malformed,
/// or interrupted journal files are never interpreted as an unused key. This journal is not a
/// hardware counter, an App Attest verifier, or monetary authority.
public final class KagemushaAppAttestFileIntentStoreV1:
  KagemushaAppAttestAssertionIntentStoringV1, @unchecked Sendable {
  private static let lockName = "intent.lock"
  private static let recordName = "intent.bin"
  private static let temporaryName = "intent.pending-write"
  private static let magic = Data("KAGEMUSHA-ATJ-V1".utf8)
  private static let maximumKeyIDBytes = 512
  private static let maximumAssertionBytes = 8_192
  private static let maximumRecordBytes = 9_000

  private let directoryPath: String

  /// Opens a previously bootstrapped private directory. It never creates or repairs a record.
  public init(directoryURL: URL) throws {
    guard directoryURL.isFileURL, directoryURL.path.hasPrefix("/") else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    directoryPath = directoryURL.path
    let directory = try Self.openDirectory(directoryPath)
    defer { _ = Darwin.close(directory) }
  }

  /// Establishes one new journal for a freshly attested, unused App Attest key.
  /// Enrollment verifies signCount zero; a caller cannot choose a later counter here.
  /// A failed or interrupted bootstrap leaves the directory unusable until explicit recovery.
  public static func bootstrapNew(directoryURL: URL, keyID: String) throws
    -> KagemushaAppAttestFileIntentStoreV1 {
    let key = try keyBytes(keyID)
    let store = try KagemushaAppAttestFileIntentStoreV1(directoryURL: directoryURL)
    let directory = try openDirectory(store.directoryPath)
    defer { _ = Darwin.close(directory) }
    let lock = lockName.withCString { name in
      Darwin.openat(directory, name, O_RDWR | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
    }
    guard lock >= 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    defer { _ = Darwin.close(lock) }
    try validateRegularFile(lock)
    try sync(lock)
    try sync(directory)
    try acquire(lock)
    defer { _ = kagemushaFlock(lock, LOCK_UN) }
    guard !exists(directory, name: recordName), !exists(directory, name: temporaryName) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    try writeRecord(directory, record: Record(key: key, intent: .ready(counter: 0)))
    return store
  }

  public func load(keyID: String) throws -> KagemushaAppAttestAssertionIntentV1 {
    let key = try Self.keyBytes(keyID)
    return try withLock { directory in
      let record = try Self.readRecord(directory)
      guard record.key == key else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
      return record.intent
    }
  }

  public func reserve(keyID: String, previousCounter: UInt32,
    selectionDigest: Data) throws {
    let key = try Self.keyBytes(keyID)
    try Self.validateDigest(selectionDigest)
    guard previousCounter < UInt32.max else {
      throw KagemushaAppAttestEvidenceErrorV1.assertionCounterMismatch
    }
    try withLock { directory in
      let current = try Self.readRecord(directory)
      guard current.key == key else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
      switch current.intent {
      case .ready(let counter) where counter == previousCounter:
        break
      default:
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      // Only advanceAfterCommitted can promote a completed assertion to ready;
      // caller-supplied counters cannot bypass the native Core acknowledgment.
      try Self.writeRecord(directory, record: Record(key: key,
        intent: .pending(previousCounter: previousCounter, selectionDigest: selectionDigest)))
    }
  }

  public func complete(keyID: String, counter: UInt32, selectionDigest: Data,
    rawAssertion: Data) throws {
    let key = try Self.keyBytes(keyID)
    try Self.validateDigest(selectionDigest)
    guard !rawAssertion.isEmpty, rawAssertion.count <= Self.maximumAssertionBytes else {
      throw KagemushaAppAttestEvidenceErrorV1.emptyRawObject
    }
    try withLock { directory in
      let current = try Self.readRecord(directory)
      guard current.key == key,
        case .pending(let previousCounter, let pendingDigest) = current.intent,
        previousCounter < UInt32.max,
        counter == previousCounter + 1,
        pendingDigest == selectionDigest else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      try Self.writeRecord(directory, record: Record(key: key,
        intent: .complete(counter: counter, selectionDigest: selectionDigest,
          rawAssertion: rawAssertion)))
    }
  }

  /// Atomically consume only the exact completed assertion acknowledged by native Core.
  /// The SDK creates the acknowledgment only after exact method-13 frame correlation.
  public func advanceAfterCommitted(keyID: String, counter: UInt32,
    selectionDigest: Data, rawAssertion: Data,
    acknowledgment: KagemushaAppAttestCoreCommitAcknowledgmentV1) throws {
    let key = try Self.keyBytes(keyID)
    try Self.validateDigest(selectionDigest)
    guard counter > 0,
      acknowledgment.committedCounter == counter,
      acknowledgment.keyIDDigest == Data(SHA256.hash(data: key)),
      acknowledgment.selectionDigest == selectionDigest,
      acknowledgment.rawAssertionDigest == Data(SHA256.hash(data: rawAssertion)) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    try withLock { directory in
      let current = try Self.readRecord(directory)
      guard current.key == key,
        current.intent == .complete(counter: counter, selectionDigest: selectionDigest,
          rawAssertion: rawAssertion) else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      try Self.writeRecord(directory, record: Record(key: key, intent: .ready(counter: counter)))
    }
  }

  private func withLock<T>(_ body: (Int32) throws -> T) throws -> T {
    let directory = try Self.openDirectory(directoryPath)
    defer { _ = Darwin.close(directory) }
    let lock = Self.lockName.withCString { name in
      Darwin.openat(directory, name, O_RDWR | O_NOFOLLOW | O_CLOEXEC)
    }
    guard lock >= 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    defer { _ = Darwin.close(lock) }
    try Self.validateRegularFile(lock)
    try Self.acquire(lock)
    defer { _ = kagemushaFlock(lock, LOCK_UN) }
    guard !Self.exists(directory, name: Self.temporaryName) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    return try body(directory)
  }

  private struct Record: Equatable {
    let key: Data
    let intent: KagemushaAppAttestAssertionIntentV1
  }

  private static func keyBytes(_ keyID: String) throws -> Data {
    let bytes = Data(keyID.utf8)
    guard !bytes.isEmpty, bytes.count <= maximumKeyIDBytes, !bytes.contains(0) else {
      throw KagemushaAppAttestEvidenceErrorV1.emptyKeyID
    }
    return bytes
  }

  private static func validateDigest(_ digest: Data) throws {
    guard digest.count == 32 else {
      throw KagemushaAppAttestEvidenceErrorV1.invalidDigestLength
    }
  }

  private static func openDirectory(_ path: String) throws -> Int32 {
    let fd = path.withCString { Darwin.open($0, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC) }
    guard fd >= 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    do {
      var metadata = stat()
      guard Darwin.fstat(fd, &metadata) == 0,
        metadata.st_mode & mode_t(S_IFMT) == mode_t(S_IFDIR),
        metadata.st_uid == geteuid(),
        metadata.st_mode & 0o777 == 0o700 else {
        throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
      }
      return fd
    } catch {
      _ = Darwin.close(fd)
      throw error
    }
  }

  private static func validateRegularFile(_ fd: Int32) throws {
    var metadata = stat()
    guard Darwin.fstat(fd, &metadata) == 0,
      metadata.st_mode & mode_t(S_IFMT) == mode_t(S_IFREG),
      metadata.st_uid == geteuid(), metadata.st_nlink == 1,
      metadata.st_mode & 0o777 == 0o600 else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
  }

  private static func acquire(_ fd: Int32) throws {
    while kagemushaFlock(fd, LOCK_EX) != 0 {
      guard errno == EINTR else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    }
  }

  private static func sync(_ fd: Int32) throws {
    while Darwin.fsync(fd) != 0 {
      guard errno == EINTR else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    }
  }

  private static func exists(_ directory: Int32, name: String) -> Bool {
    var metadata = stat()
    return name.withCString { Darwin.fstatat(directory, $0, &metadata, AT_SYMLINK_NOFOLLOW) == 0 }
      || errno != ENOENT
  }

  private static func readRecord(_ directory: Int32) throws -> Record {
    let fd = recordName.withCString { Darwin.openat(directory, $0, O_RDONLY | O_NOFOLLOW | O_CLOEXEC) }
    guard fd >= 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    defer { _ = Darwin.close(fd) }
    try validateRegularFile(fd)
    var metadata = stat()
    guard Darwin.fstat(fd, &metadata) == 0, metadata.st_size > 0,
      metadata.st_size <= maximumRecordBytes else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    var bytes = [UInt8](repeating: 0, count: Int(metadata.st_size))
    let total = bytes.count
    var offset = 0
    while offset < total {
      let count = bytes.withUnsafeMutableBytes { raw in
        Darwin.read(fd, raw.baseAddress!.advanced(by: offset), total - offset)
      }
      if count < 0 && errno == EINTR { continue }
      guard count > 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
      offset += count
    }
    var extra: UInt8 = 0
    guard Darwin.read(fd, &extra, 1) == 0 else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    return try decode(Data(bytes))
  }

  private static func writeRecord(_ directory: Int32, record: Record) throws {
    let bytes = encode(record)
    let fd = temporaryName.withCString { name in
      Darwin.openat(directory, name, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
    }
    guard fd >= 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    defer { _ = Darwin.close(fd) }
    try validateRegularFile(fd)
    var offset = 0
    try bytes.withUnsafeBytes { raw in
      while offset < raw.count {
        let count = Darwin.write(fd, raw.baseAddress!.advanced(by: offset), raw.count - offset)
        if count < 0 && errno == EINTR { continue }
        guard count > 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
        offset += count
      }
    }
    try sync(fd)
    let renamed = temporaryName.withCString { source in
      recordName.withCString { destination in
        Darwin.renameat(directory, source, directory, destination)
      }
    }
    guard renamed == 0 else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    try sync(directory)
    guard try readRecord(directory) == record else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
  }

  private static func encode(_ record: Record) -> Data {
    var body = magic
    let state: UInt8
    let counter: UInt32
    let digest: Data
    let assertion: Data
    switch record.intent {
    case .ready(let value):
      state = 0; counter = value; digest = Data(repeating: 0, count: 32); assertion = Data()
    case .pending(let value, let selection):
      state = 1; counter = value; digest = selection; assertion = Data()
    case .complete(let value, let selection, let raw):
      state = 2; counter = value; digest = selection; assertion = raw
    }
    body.append(state)
    appendLE(UInt16(record.key.count), to: &body)
    body.append(record.key)
    appendLE(counter, to: &body)
    body.append(digest)
    appendLE(UInt16(assertion.count), to: &body)
    body.append(assertion)
    body.append(contentsOf: SHA256.hash(data: body))
    return body
  }

  private static func appendLE<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
    for index in 0..<MemoryLayout<T>.size {
      data.append(UInt8(truncatingIfNeeded: value >> (index * 8)))
    }
  }

  private static func decode(_ data: Data) throws -> Record {
    let bytes = [UInt8](data)
    let minimum = magic.count + 1 + 2 + 1 + 4 + 32 + 2 + 32
    guard bytes.count >= minimum, bytes.count <= maximumRecordBytes,
      Array(bytes.prefix(magic.count)) == Array(magic),
      Data(SHA256.hash(data: Data(bytes.dropLast(32)))) == Data(bytes.suffix(32)) else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    var offset = magic.count
    let state = bytes[offset]; offset += 1
    let keyCount = Int(readLE(UInt16.self, bytes, at: &offset))
    guard (1...maximumKeyIDBytes).contains(keyCount),
      offset + keyCount + 4 + 32 + 2 + 32 <= bytes.count else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let key = Data(bytes[offset..<(offset + keyCount)]); offset += keyCount
    guard !key.contains(0) else { throw KagemushaAppAttestEvidenceErrorV1.journalMismatch }
    let counter = readLE(UInt32.self, bytes, at: &offset)
    let digest = Data(bytes[offset..<(offset + 32)]); offset += 32
    let assertionCount = Int(readLE(UInt16.self, bytes, at: &offset))
    guard assertionCount <= maximumAssertionBytes,
      offset + assertionCount + 32 == bytes.count else {
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    let assertion = Data(bytes[offset..<(offset + assertionCount)])
    let intent: KagemushaAppAttestAssertionIntentV1
    switch state {
    case 0 where digest == Data(repeating: 0, count: 32) && assertion.isEmpty:
      intent = .ready(counter: counter)
    case 1 where assertion.isEmpty && counter < UInt32.max:
      intent = .pending(previousCounter: counter, selectionDigest: digest)
    case 2 where !assertion.isEmpty && counter > 0:
      intent = .complete(counter: counter, selectionDigest: digest, rawAssertion: assertion)
    default:
      throw KagemushaAppAttestEvidenceErrorV1.journalMismatch
    }
    return Record(key: key, intent: intent)
  }

  private static func readLE<T: FixedWidthInteger>(_ type: T.Type, _ bytes: [UInt8],
    at offset: inout Int) -> T {
    var value: T = 0
    for index in 0..<MemoryLayout<T>.size {
      value |= T(bytes[offset + index]) << (index * 8)
    }
    offset += MemoryLayout<T>.size
    return value
  }
}
