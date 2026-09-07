import Foundation

/// Complete untrusted projection of Core's `DurabilityAnchorStatementV1`.
///
/// Every checkpoint field participates in the account signing message. Retaining or
/// decoding this statement proves neither its hardware seal nor its current selection.
public struct KagemushaDurabilityAnchorStatementProjectionV1: Equatable, Sendable {
  public let metadataRevision: KagemushaUInt128V1
  public let version: UInt16
  public let lane: KagemushaDeviceLaneIDV1
  public let stateCommitment: Data
  public let hardwareEpoch: KagemushaDeviceHardwareEpochV1
  public let devicePolicyBinding: KagemushaDevicePolicyBindingV1
  public let stateNonceCommitment: Data
  public let logicalSequence: KagemushaUInt128V1
  public let journalRevision: KagemushaUInt128V1
  public let inboxRevision: KagemushaUInt128V1
  public let snapshotCommitment: Data

  public init(
    metadataRevision: KagemushaUInt128V1, version: UInt16 = 1, lane: KagemushaDeviceLaneIDV1,
    stateCommitment: Data, hardwareEpoch: KagemushaDeviceHardwareEpochV1,
    devicePolicyBinding: KagemushaDevicePolicyBindingV1, stateNonceCommitment: Data,
    logicalSequence: KagemushaUInt128V1, journalRevision: KagemushaUInt128V1,
    inboxRevision: KagemushaUInt128V1, snapshotCommitment: Data
  ) throws {
    guard version == 1, lane.networkID.last.map({ $0 & 1 == 1 }) == true else {
      throw kagemushaInvalid("enrolledOpen.checkpoint.versionOrNetwork")
    }
    self.metadataRevision = try openCopy128(metadataRevision)
    self.version = version
    self.lane = try .init(networkID: Data(Array(lane.networkID)),
      deviceLaneID: Data(Array(lane.deviceLaneID)), asset: lane.asset, scale: lane.scale)
    self.stateCommitment = try openDigest(stateCommitment)
    self.hardwareEpoch = try .init(generation: openCopy128(hardwareEpoch.generation),
      epochID: openDigest(hardwareEpoch.epochID))
    self.devicePolicyBinding = try .init(
      deviceKeyReference: openDigest(devicePolicyBinding.deviceKeyReference),
      hardwarePolicyID: openDigest(devicePolicyBinding.hardwarePolicyID))
    self.stateNonceCommitment = try openDigest(stateNonceCommitment)
    self.logicalSequence = try openCopy128(logicalSequence)
    self.journalRevision = try openCopy128(journalRevision)
    self.inboxRevision = try openCopy128(inboxRevision)
    self.snapshotCommitment = try openDigest(snapshotCommitment)
  }
}

/// Untrusted projection of the native challenge's selected certificate or full checkpoint.
/// These cases do not construct authenticated issuer or recovery capabilities.
public enum KagemushaEnrolledOpenAuthoritySourceProjectionV1: Equatable, Sendable {
  case initialCertificate(certificateDigest: Data)
  case recoveryCheckpoint(
    statement: KagemushaDurabilityAnchorStatementProjectionV1, terminalCertificateDigest: Data)

  fileprivate func validatedCopy() throws -> Self {
    switch self {
    case .initialCertificate(let digest):
      return .initialCertificate(certificateDigest: try openDigest(digest))
    case .recoveryCheckpoint(let statement, let digest):
      return .recoveryCheckpoint(statement: statement, terminalCertificateDigest: try openDigest(digest))
    }
  }
}

/// Exact untrusted account signing projection emitted by the native enrolled-open lifecycle.
///
/// Parsing and correlation do not prove MiBank approval, hardware possession, elapsed time,
/// checkpoint freshness or monetary authority. Native pending state owns the one-use deadline.
public struct KagemushaEnrolledOpenAccountChallengeV1: Equatable, Sendable {
  public static let maximumCanonicalBytes = 16 * 1024
  public static let accountDomain = "iroha:kagemusha:v1:enrolled-open-account-possession"
  public static let lifetimeMS: UInt64 = 120_000
  public let version: UInt16
  public let domain: String
  public let enrollmentID: Data
  public let owner: KagemushaRetailEnrollmentOwnerProjectionV1
  public let nonce: Data
  public let authoritySource: KagemushaEnrolledOpenAuthoritySourceProjectionV1
  public let releaseID: Data
  public let hardwarePolicyDigest: Data
  public let coreAuthorizationKeyReference: Data
  public let lifetimeMilliseconds: UInt64

  public init(
    version: UInt16 = 1, domain: String = Self.accountDomain,
    enrollmentID: Data, owner: KagemushaRetailEnrollmentOwnerProjectionV1,
    nonce: Data, authoritySource: KagemushaEnrolledOpenAuthoritySourceProjectionV1,
    releaseID: Data, hardwarePolicyDigest: Data, coreAuthorizationKeyReference: Data,
    lifetimeMilliseconds: UInt64 = Self.lifetimeMS
  ) throws {
    guard version == 1, domain.utf8.elementsEqual(Self.accountDomain.utf8),
      lifetimeMilliseconds == Self.lifetimeMS
    else { throw kagemushaInvalid("enrolledOpen.versionDomainLifetime") }
    _ = try KagemushaEnrolledOpenSelectorV1(version: 1, owner: owner, enrollmentID: enrollmentID)
    let source = try authoritySource.validatedCopy()
    if case .recoveryCheckpoint(let statement, _) = source {
      guard statement.lane.networkID == owner.runtime.networkID,
        statement.lane.deviceLaneID == owner.laneID,
        statement.lane.asset == owner.runtime.asset, statement.lane.scale == owner.runtime.scale
      else { throw kagemushaInvalid("enrolledOpen.checkpointOwner") }
    }
    self.version = version
    self.domain = domain
    self.enrollmentID = Data(Array(enrollmentID))
    self.owner = owner
    self.nonce = try openDigest(nonce)
    self.authoritySource = source
    self.releaseID = try openDigest(releaseID)
    self.hardwarePolicyDigest = try openDigest(hardwarePolicyDigest)
    self.coreAuthorizationKeyReference = try openDigest(coreAuthorizationKeyReference)
    self.lifetimeMilliseconds = lifetimeMilliseconds
  }

  /// Check every independently retained request pin before presenting this signing message.
  /// Matching is correlation only; these arguments must not be populated from this challenge.
  public func validateCorrelation(
    selector: KagemushaEnrolledOpenSelectorV1, nonce: Data, releaseID: Data,
    hardwarePolicyDigest: Data, coreAuthorizationKeyReference: Data
  ) throws {
    guard owner == selector.owner, enrollmentID == selector.enrollmentID,
      self.nonce == nonce, self.releaseID == releaseID,
      self.hardwarePolicyDigest == hardwarePolicyDigest,
      self.coreAuthorizationKeyReference == coreAuthorizationKeyReference
    else { throw kagemushaInvalid("enrolledOpen.correlation") }
  }

  public func canonicalBytes() throws -> Data {
    let payload = OpenChallengeCodec.payload(self)
    let bytes = noritoEncode(typeName: OpenChallengeCodec.schema, payload: payload,
      flags: NoritoHeader.compactLen, payloadAlignment: 16)
    guard bytes.count <= Self.maximumCanonicalBytes else {
      throw kagemushaInvalid("enrolledOpen.size")
    }
    return bytes
  }

  public static func decodeCanonicalExact(_ bytes: Data) throws -> Self {
    guard !bytes.isEmpty, bytes.count <= maximumCanonicalBytes else {
      throw kagemushaInvalid("enrolledOpen.size")
    }
    let canonical = Data(Array(bytes))
    guard let frame = noritoDecodeFrame(canonical),
      frame.header.flags == NoritoHeader.compactLen,
      frame.header.schema == noritoSchemaHash(forTypeName: OpenChallengeCodec.schema),
      frame.paddingLength == noritoHeaderPaddingLength(payloadAlignment: 16)
    else { throw kagemushaInvalid("enrolledOpen.frame") }
    let value = try OpenChallengeCodec.decode(frame.payload)
    guard try value.canonicalBytes() == canonical else {
      throw kagemushaInvalid("enrolledOpen.noncanonical")
    }
    return value
  }

  /// Exact Rust `HashOf<EnrolledOpenAccountChallengeV1>` message for plain Ed25519 signing.
  /// HashOf hashes the bare fixed-layout payload once using Blake2b-256 and sets the
  /// last-byte LSB. The NRT0 frame and a second prehash are not part of that message.
  public func accountSigningMessage() -> Data {
    IrohaHash.hash(OpenChallengeCodec.payload(self))
  }
}

private enum OpenChallengeCodec {
  static let schema = "iroha.kagemusha.v1.enrolled-open-account-challenge"

  static func payload(_ value: KagemushaEnrolledOpenAccountChallengeV1) -> Data {
    openFields([
      CompactNorito.encodeUInt16(value.version), CompactNorito.encodeString(value.domain),
      value.enrollmentID, KagemushaNoritoV1.retailEnrollmentOwner(value.owner), value.nonce,
      authority(value.authoritySource), value.releaseID, value.hardwarePolicyDigest,
      value.coreAuthorizationKeyReference, CompactNorito.encodeUInt64(value.lifetimeMilliseconds),
    ])
  }

  static func authority(_ value: KagemushaEnrolledOpenAuthoritySourceProjectionV1) -> Data {
    var writer = CompactNoritoWriter()
    switch value {
    case .initialCertificate(let digest):
      writer.writeUInt32LE(0)
      writer.writeField(digest)
    case .recoveryCheckpoint(let statement, let digest):
      writer.writeUInt32LE(1)
      writer.writeField(checkpoint(statement))
      writer.writeField(digest)
    }
    return writer.data
  }

  static func checkpoint(_ value: KagemushaDurabilityAnchorStatementProjectionV1) -> Data {
    openFields([
      value.metadataRevision.littleEndianBytes, CompactNorito.encodeUInt16(value.version),
      openFields([value.lane.networkID, openAliasDigest(value.lane.deviceLaneID),
        value.lane.asset.canonicalPayload, CompactNorito.encodeUInt32(value.lane.scale)]),
      openAliasDigest(value.stateCommitment),
      openFields([value.hardwareEpoch.generation.littleEndianBytes, openAliasDigest(value.hardwareEpoch.epochID)]),
      openFields([openAliasDigest(value.devicePolicyBinding.deviceKeyReference),
        openAliasDigest(value.devicePolicyBinding.hardwarePolicyID)]),
      openAliasDigest(value.stateNonceCommitment), value.logicalSequence.littleEndianBytes,
      value.journalRevision.littleEndianBytes, value.inboxRevision.littleEndianBytes,
      openAliasDigest(value.snapshotCommitment),
    ])
  }

  static func decode(_ payload: Data) throws -> KagemushaEnrolledOpenAccountChallengeV1 {
    var r = OpenChallengeReader(payload)
    let version = try r.u16()
    var domain = OpenChallengeReader(try r.field())
    let textBytes = try domain.field()
    try domain.finish()
    guard let text = String(data: textBytes, encoding: .utf8) else {
      throw kagemushaInvalid("enrolledOpen.domain")
    }
    let value = try KagemushaEnrolledOpenAccountChallengeV1(
      version: version, domain: text, enrollmentID: r.exact(32),
      owner: KagemushaNoritoV1.decodeRetailEnrollmentOwner(r.field()), nonce: r.exact(32),
      authoritySource: decodeAuthority(r.field()), releaseID: r.exact(32),
      hardwarePolicyDigest: r.exact(32), coreAuthorizationKeyReference: r.exact(32),
      lifetimeMilliseconds: r.u64())
    try r.finish()
    return value
  }

  static func decodeAuthority(_ payload: Data) throws -> KagemushaEnrolledOpenAuthoritySourceProjectionV1 {
    var outer = CanonicalNoritoReader(data: payload)
    let tag = try outer.readUInt32LE()
    // A named enum variant writes its fields directly after the u32 tag.
    // There is no enclosing tuple field around the variant body.
    var r = OpenChallengeReader(try outer.readBytes(outer.remaining()))
    let value: KagemushaEnrolledOpenAuthoritySourceProjectionV1
    switch tag {
    case 0: value = .initialCertificate(certificateDigest: try r.exact(32))
    case 1: value = .recoveryCheckpoint(statement: try decodeCheckpoint(r.field()),
      terminalCertificateDigest: try r.exact(32))
    default: throw kagemushaInvalid("enrolledOpen.source.tag")
    }
    try r.finish()
    return value
  }

  static func decodeCheckpoint(_ payload: Data) throws -> KagemushaDurabilityAnchorStatementProjectionV1 {
    var r = OpenChallengeReader(payload)
    let metadata = try r.u128(), version = try r.u16()
    var laneReader = OpenChallengeReader(try r.field())
    let lane = try KagemushaDeviceLaneIDV1(networkID: laneReader.exact(32),
      deviceLaneID: laneReader.aliasDigest(), asset: .init(canonicalPayload: laneReader.field()),
      scale: laneReader.u32())
    try laneReader.finish()
    let stateCommitment = try r.aliasDigest()
    var epochReader = OpenChallengeReader(try r.field())
    let epoch = try KagemushaDeviceHardwareEpochV1(generation: epochReader.u128(), epochID: epochReader.aliasDigest())
    try epochReader.finish()
    var policyReader = OpenChallengeReader(try r.field())
    let policy = try KagemushaDevicePolicyBindingV1(
      deviceKeyReference: policyReader.aliasDigest(), hardwarePolicyID: policyReader.aliasDigest())
    try policyReader.finish()
    let value = try KagemushaDurabilityAnchorStatementProjectionV1(
      metadataRevision: metadata, version: version, lane: lane, stateCommitment: stateCommitment,
      hardwareEpoch: epoch, devicePolicyBinding: policy, stateNonceCommitment: r.aliasDigest(),
      logicalSequence: r.u128(), journalRevision: r.u128(), inboxRevision: r.u128(), snapshotCommitment: r.aliasDigest())
    try r.finish()
    return value
  }
}

private func openCopy128(_ value: KagemushaUInt128V1) throws -> KagemushaUInt128V1 {
  try .init(littleEndianBytes: Data(Array(value.littleEndianBytes)))
}

private func openDigest(_ bytes: Data) throws -> Data {
  Data(Array(try kagemushaDigest(bytes, "enrolledOpen.digest")))
}

private func openFields(_ values: [Data]) -> Data {
  var writer = CompactNoritoWriter()
  for value in values { writer.writeField(value) }
  return writer.data
}

private func openAliasDigest(_ bytes: Data) -> Data {
  openFields(bytes.map { Data([$0]) })
}

private struct OpenChallengeReader {
  private var reader: CanonicalNoritoReader
  init(_ bytes: Data) { reader = .init(data: bytes) }
  mutating func field() throws -> Data { try reader.readCompactField() }
  mutating func exact(_ count: Int) throws -> Data {
    let bytes = try field()
    guard bytes.count == count else { throw kagemushaInvalid("enrolledOpen.fieldWidth") }
    return bytes
  }
  mutating func u16() throws -> UInt16 {
    try exact(2).withUnsafeBytes { UInt16(littleEndian: $0.loadUnaligned(as: UInt16.self)) }
  }
  mutating func u32() throws -> UInt32 {
    try exact(4).withUnsafeBytes { UInt32(littleEndian: $0.loadUnaligned(as: UInt32.self)) }
  }
  mutating func u64() throws -> UInt64 {
    try exact(8).withUnsafeBytes { UInt64(littleEndian: $0.loadUnaligned(as: UInt64.self)) }
  }
  mutating func u128() throws -> KagemushaUInt128V1 { try .init(littleEndianBytes: exact(16)) }
  mutating func aliasDigest() throws -> Data {
    var nested = OpenChallengeReader(try exact(64))
    var value = Data()
    for _ in 0..<32 { value.append(try nested.exact(1)) }
    try nested.finish()
    return try openDigest(value)
  }
  func finish() throws {
    guard reader.remaining() == 0 else { throw kagemushaInvalid("enrolledOpen.tail") }
  }
}
