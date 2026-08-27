import CryptoKit
import Foundation

enum ToriiPreparedAccountProtocolV1 {
  static let bindingSchema = "iroha.taira.public-reset.mutation-binding.v1"
  static let preparedTransactionSchema = "iroha.taira.prepared-transaction.v1"
  static let onboardingPrepareSchema = "iroha.accounts.onboard.prepare.v1"
  static let onboardingProofRequiredSchema =
    "iroha.accounts.onboard.prepare-proof-required.v1"
  static let faucetPrepareSchema = "iroha.accounts.faucet.prepare.v1"
  static let submitResponseSchema = "iroha.taira.prepared-transaction-submit.v1"
  static let signatureTranscriptSchema = "iroha.taira.prepared-signature-transcript.v1"
  static let signatureDomain = Data("iroha:taira:prepared-transaction:v1\0".utf8)
  static let faucetClaimHashDomain = Data("iroha:accounts:faucet:claim:v1\0".utf8)
  static let preparedBindingMetadataKey = "taira_public_reset_binding"
  static let preparedOperationMetadataKey = "taira_prepared_operation"
  static let preparedSemanticHashMetadataKey = "taira_prepared_semantic_hash"

  static func rejectUnknownFields<K: CodingKey & CaseIterable>(
    from decoder: Decoder,
    keys: K.Type,
    name: String
  ) throws {
    let allowed = Set(K.allCases.map(\.stringValue))
    let container = try decoder.container(keyedBy: AnyCodingKey.self)
    guard container.allKeys.allSatisfy({ allowed.contains($0.stringValue) }) else {
      throw DecodingError.dataCorrupted(
        .init(
          codingPath: decoder.codingPath,
          debugDescription: "\(name) contains an unknown or retired field"
        )
      )
    }
  }

  static func exactLowerHex(_ value: String, bytes: Int, field: String) throws -> String {
    guard value.utf8.count == bytes * 2,
      value == value.lowercased(),
      value.utf8.allSatisfy({
        (UInt8(ascii: "0")...UInt8(ascii: "9")).contains($0)
          || (UInt8(ascii: "a")...UInt8(ascii: "f")).contains($0)
      }),
      Data(hexString: value) != nil
    else {
      throw ToriiClientError.invalidPayload(
        "\(field) must be exactly \(bytes * 2) lowercase hexadecimal characters."
      )
    }
    return value
  }

  static func exactLowerHexBytes(_ value: String, field: String) throws -> Data {
    guard !value.isEmpty,
      value.utf8.count.isMultiple(of: 2),
      value.utf8.count
        <= (ToriiCanonicalTransactionDraft.maximumTransactionPayloadBytes + 16 * 1024) * 2,
      value == value.lowercased(),
      value.utf8.allSatisfy({
        (UInt8(ascii: "0")...UInt8(ascii: "9")).contains($0)
          || (UInt8(ascii: "a")...UInt8(ascii: "f")).contains($0)
      }),
      let bytes = Data(hexString: value)
    else {
      throw ToriiClientError.invalidPayload(
        "\(field) must be non-empty canonical lowercase hexadecimal."
      )
    }
    return bytes
  }

  static func exactSignature(_ value: String) throws -> String {
    guard value.utf8.count == Ed25519SignatureAdmission.signatureLength * 2,
      value == value.uppercased(),
      value.utf8.allSatisfy({
        (UInt8(ascii: "0")...UInt8(ascii: "9")).contains($0)
          || (UInt8(ascii: "A")...UInt8(ascii: "F")).contains($0)
      }),
      let bytes = Data(hexString: value),
      Ed25519SignatureAdmission.isValidSignature(bytes)
    else {
      throw ToriiClientError.invalidPayload(
        "server_signature must be exactly one canonical uppercase Ed25519 signature."
      )
    }
    return value
  }

  static func canonicalAccountId(_ value: String, field: String) throws -> String {
    guard !value.contains("@"),
      value == value.trimmingCharacters(in: .whitespacesAndNewlines)
    else {
      throw ToriiClientError.invalidPayload(
        "\(field) must be a canonical domainless I105 account id."
      )
    }
    do {
      let prefix = try AccountAddress.inspectI105NetworkPrefix(value).chainDiscriminant
      let address = try AccountAddress.parseEncoded(value, expectedPrefix: prefix)
      guard try address.toI105(chainDiscriminant: prefix) == value else {
        throw ToriiClientError.invalidPayload(
          "\(field) must use its exact canonical I105 spelling."
        )
      }
    } catch let error as ToriiClientError {
      throw error
    } catch {
      throw ToriiClientError.invalidPayload(
        "\(field) must be a canonical domainless I105 account id."
      )
    }
    return value
  }

  static func canonicalAlias(_ value: String) throws -> String {
    let canonical = try AccountAliasName(parsing: value).canonicalText
    guard canonical == value else {
      throw ToriiClientError.invalidPayload("alias must use its exact canonical spelling.")
    }
    return value
  }

  static func exactIdentifier(_ value: String, field: String) throws -> String {
    guard !value.isEmpty,
      value == value.trimmingCharacters(in: .whitespacesAndNewlines),
      !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
    else {
      throw ToriiClientError.invalidPayload("\(field) must be exact non-blank text.")
    }
    return value
  }

  static func canonicalQuantity(_ value: String) throws -> String {
    let quantity: KotodamaQuantity
    do {
      quantity = try KotodamaQuantity(value)
    } catch {
      throw ToriiClientError.invalidPayload("amount must be a canonical Quantity string.")
    }
    guard quantity.canonicalString == value else {
      throw ToriiClientError.invalidPayload("amount must use its canonical Quantity spelling.")
    }
    return value
  }

  static func wireIdentity(
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSHA256: String
  ) throws -> (String, String, String) {
    let transactionHash = try exactLowerHex(
      transactionHashHex,
      bytes: 32,
      field: "transaction_hash_hex"
    )
    let wire = try exactLowerHexBytes(
      signedTransactionWireHex,
      field: "signed_transaction_wire_hex"
    )
    let wireSHA256 = try exactLowerHex(
      signedTransactionWireSHA256,
      bytes: 32,
      field: "signed_transaction_wire_sha256"
    )
    guard Data(SHA256.hash(data: wire)).hexEncodedString() == wireSHA256 else {
      throw ToriiClientError.invalidPayload(
        "signed_transaction_wire_sha256 does not match the exact prepared wire."
      )
    }
    let inspected: ToriiCanonicalTransactionDraft.SignedTransactionV1
    do {
      inspected = try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
        wire,
        context: "prepared transaction"
      )
    } catch {
      throw ToriiClientError.invalidPayload(
        "signed_transaction_wire_hex is not one canonical fixed-V1 signed transaction."
      )
    }
    try ToriiCanonicalTransactionDraft.requireNonemptyInstructionExecutable(
      inspected.payload.executable,
      context: "prepared transaction"
    )
    var entrypoint = CompactNoritoWriter()
    entrypoint.writeUInt32LE(0)
    entrypoint.writeField(inspected.transactionPayload)
    guard IrohaHash.hash(entrypoint.data).hexEncodedString() == transactionHash else {
      throw ToriiClientError.invalidPayload(
        "transaction_hash_hex does not match the exact prepared transaction payload."
      )
    }
    return (transactionHash, signedTransactionWireHex, wireSHA256)
  }

  static func inspectWire(
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSHA256: String
  ) throws -> ToriiCanonicalTransactionDraft.SignedTransactionV1 {
    _ = try wireIdentity(
      transactionHashHex: transactionHashHex,
      signedTransactionWireHex: signedTransactionWireHex,
      signedTransactionWireSHA256: signedTransactionWireSHA256
    )
    guard let wire = Data(hexString: signedTransactionWireHex) else {
      throw ToriiClientError.invalidPayload(
        "signed_transaction_wire_hex is not canonical lowercase hexadecimal."
      )
    }
    return try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
      wire,
      context: "prepared transaction"
    )
  }

  static func baseSignatureTranscript(
    envelopeSchema: String,
    operation: ToriiPreparedAccountOperationV1,
    binding: ToriiTairaPublicResetMutationBindingV1
  ) -> Data {
    var transcript = Data()
    appendFrame(signatureDomain, to: &transcript)
    appendField("transcript_schema", signatureTranscriptSchema, to: &transcript)
    appendField("envelope_schema", envelopeSchema, to: &transcript)
    appendField("operation", operation.rawValue, to: &transcript)
    appendField("binding.schema", binding.schema, to: &transcript)
    appendField(
      "binding.authorization_sha256",
      binding.authorizationSHA256,
      to: &transcript
    )
    appendField(
      "binding.authorization_nonce",
      binding.authorizationNonce,
      to: &transcript
    )
    appendField("binding.kind", binding.kind.rawValue, to: &transcript)
    appendField("binding.phase", binding.phase, to: &transcript)
    appendField("binding.idempotency_key", binding.idempotencyKey, to: &transcript)
    appendField(
      "binding.execution_expires_at_unix_ms",
      String(binding.executionExpiresAtUnixMs),
      to: &transcript
    )
    return transcript
  }

  static func appendField(_ label: String, _ value: String, to transcript: inout Data) {
    appendField(label, Data(value.utf8), to: &transcript)
  }

  static func appendField(_ label: String, _ value: Data, to transcript: inout Data) {
    appendFrame(Data(label.utf8), to: &transcript)
    appendFrame(value, to: &transcript)
  }

  private static func appendFrame(_ value: Data, to transcript: inout Data) {
    var length = UInt64(value.count).bigEndian
    transcript.append(contentsOf: withUnsafeBytes(of: &length, Array.init))
    transcript.append(value)
  }

  static func verifyServerSignature(
    transcript: Data,
    serverSignature: String,
    expectedAuthority: String
  ) throws {
    let exactAuthority = try canonicalAccountId(expectedAuthority, field: "expectedAuthority")
    let address = try AccountAddress.parseEncoded(exactAuthority)
    guard let controller = address.singleControllerInfo(),
      controller.algorithm == .ed25519,
      Ed25519PublicKeyAdmission.isValidPublicKey(controller.publicKey),
      let signature = Data(hexString: try exactSignature(serverSignature)),
      let key = try? Curve25519.Signing.PublicKey(rawRepresentation: controller.publicKey),
      key.isValidSignature(signature, for: IrohaHash.hash(transcript))
    else {
      throw ToriiClientError.invalidPayload(
        "server_signature does not authenticate the exact prepared V1 transcript."
      )
    }
  }

  static func validatePreparedTransaction(
    _ transaction: ToriiCanonicalTransactionDraft.SignedTransactionV1,
    feePayment: FeePaymentIntent,
    binding: ToriiTairaPublicResetMutationBindingV1,
    operation: ToriiPreparedAccountOperationV1,
    semanticHashHex: String,
    expectedAuthority: String,
    expectedNetworkId: NetworkId
  ) throws {
    let exactAuthority = try canonicalAccountId(expectedAuthority, field: "expectedAuthority")
    let address = try AccountAddress.parseEncoded(exactAuthority)
    let expectedAuthorityWire = try address.compactNoritoAccountControllerPayload()
    let expectedFeePayment = try feePayment.compactNorito()
    guard let controller = address.singleControllerInfo(),
      controller.algorithm == .ed25519,
      controller.publicKey == transaction.signerPublicKey,
      transaction.payload.authority == expectedAuthorityWire,
      transaction.payload.networkId == expectedNetworkId,
      transaction.payload.feePayment == expectedFeePayment
    else {
      throw ToriiClientError.invalidPayload(
        "prepared transaction changed the pinned network, authority, signature, or fee payment."
      )
    }
    let bindingJSON = ToriiJSONValue.object([
      "schema": .string(binding.schema),
      "authorization_sha256": .string(binding.authorizationSHA256),
      "authorization_nonce": .string(binding.authorizationNonce),
      "kind": .string(binding.kind.rawValue),
      "phase": .string(binding.phase),
      "idempotency_key": .string(binding.idempotencyKey),
      "execution_expires_at_unix_ms": .number(Double(binding.executionExpiresAtUnixMs)),
    ])
    let expectedMetadata: [String: ToriiJSONValue] = [
      preparedBindingMetadataKey: bindingJSON,
      preparedOperationMetadataKey: .string(operation.rawValue),
      preparedSemanticHashMetadataKey: .string(semanticHashHex),
    ]
    guard transaction.payload.metadata == expectedMetadata,
      transaction.payload.metadataWire
        == (try ToriiCanonicalTransactionDraft.compactMetadata(expectedMetadata))
    else {
      throw ToriiClientError.invalidPayload(
        "prepared transaction metadata differs from its exact V1 reset binding."
      )
    }
  }

  static func faucetSemanticHash(_ claim: ToriiAccountFaucetClaimV1) throws -> String {
    var encoded = CompactNoritoWriter()
    encoded.writeField(CompactNorito.encodeString(claim.accountId))
    encoded.writeField(CompactNorito.encodeUInt64(claim.powAnchorHeight))
    encoded.writeField(CompactNorito.encodeString(claim.powNonceHex))
    return IrohaHash.hash(faucetClaimHashDomain + encoded.data).hexEncodedString()
  }

  static func dispositionTransitionAllowed(
    planned: AliasPlanDispositionV1,
    prepared: AliasPlanDispositionV1
  ) -> Bool {
    switch (planned, prepared) {
    case (.create, .create), (.create, .repair), (.create, .noOp),
      (.repair, .repair), (.repair, .noOp), (.noOp, .noOp):
      return true
    default:
      return false
    }
  }

  private struct AnyCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
      self.stringValue = stringValue
      intValue = nil
    }

    init?(intValue: Int) {
      stringValue = String(intValue)
      self.intValue = intValue
    }
  }
}

/// The only account mutation kinds accepted by the prepared-transaction protocol.
public enum ToriiPreparedAccountOperationV1: String, Codable, Equatable, Sendable {
  case onboarding
  case faucet
}

/// Exact authorization and idempotency identity committed by a prepared transaction.
public struct ToriiTairaPublicResetMutationBindingV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.bindingSchema

  public let schema: String
  public let authorizationSHA256: String
  public let authorizationNonce: String
  public let kind: ToriiPreparedAccountOperationV1
  public let phase: String
  public let idempotencyKey: String
  public let executionExpiresAtUnixMs: UInt64

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, kind, phase
    case authorizationSHA256 = "authorization_sha256"
    case authorizationNonce = "authorization_nonce"
    case idempotencyKey = "idempotency_key"
    case executionExpiresAtUnixMs = "execution_expires_at_unix_ms"
  }

  public init(
    authorizationSHA256: String,
    authorizationNonce: String,
    kind: ToriiPreparedAccountOperationV1,
    phase: String,
    idempotencyKey: String,
    executionExpiresAtUnixMs: UInt64
  ) throws {
    guard executionExpiresAtUnixMs > 0 else {
      throw ToriiClientError.invalidPayload(
        "execution_expires_at_unix_ms must be a positive u64 timestamp."
      )
    }
    guard authorizationNonce.utf8.count == 32,
      authorizationNonce.utf8.allSatisfy({
        (97...122).contains($0) || (48...57).contains($0) || $0 == 45 || $0 == 95
      })
    else {
      throw ToriiClientError.invalidPayload(
        "authorization_nonce must be exactly 32 lowercase URL-safe characters."
      )
    }
    guard !phase.isEmpty,
      phase.utf8.count <= 128,
      phase.utf8.allSatisfy({
        (97...122).contains($0) || (48...57).contains($0) || $0 == 45 || $0 == 95
      })
    else {
      throw ToriiClientError.invalidPayload(
        "phase must be a canonical lowercase reset phase label."
      )
    }
    schema = Self.schemaV1
    self.authorizationSHA256 = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      authorizationSHA256,
      bytes: 32,
      field: "authorization_sha256"
    )
    self.authorizationNonce = authorizationNonce
    self.kind = kind
    self.phase = phase
    self.idempotencyKey = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      idempotencyKey,
      bytes: 32,
      field: "idempotency_key"
    )
    self.executionExpiresAtUnixMs = executionExpiresAtUnixMs
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "public-reset mutation binding"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1 else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported public-reset mutation binding schema"
      )
    }
    try self.init(
      authorizationSHA256: container.decode(String.self, forKey: .authorizationSHA256),
      authorizationNonce: container.decode(String.self, forKey: .authorizationNonce),
      kind: container.decode(ToriiPreparedAccountOperationV1.self, forKey: .kind),
      phase: container.decode(String.self, forKey: .phase),
      idempotencyKey: container.decode(String.self, forKey: .idempotencyKey),
      executionExpiresAtUnixMs: container.decode(UInt64.self, forKey: .executionExpiresAtUnixMs)
    )
  }

  func validate(
    expectedOperation: ToriiPreparedAccountOperationV1,
    activeAtUnixMs: UInt64?
  ) throws {
    guard kind == expectedOperation else {
      throw ToriiClientError.invalidPayload(
        "prepared mutation binding kind does not match \(expectedOperation.rawValue)."
      )
    }
    if let activeAtUnixMs, executionExpiresAtUnixMs <= activeAtUnixMs {
      throw ToriiClientError.invalidPayload(
        "prepared mutation binding is expired for a new forward prepare."
      )
    }
  }
}

/// Exact non-mutating onboarding prepare request.
public struct ToriiAccountOnboardingPrepareRequestV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.onboardingPrepareSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let receipt: ToriiAccountOnboardingPlanReceipt
  public let feePayment: FeePaymentIntent

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, receipt
    case feePayment = "fee_payment"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    receipt: ToriiAccountOnboardingPlanReceipt,
    feePayment: FeePaymentIntent
  ) throws {
    try binding.validate(expectedOperation: .onboarding, activeAtUnixMs: nil)
    _ = try feePayment.canonicalJSONData()
    schema = Self.schemaV1
    self.binding = binding
    self.receipt = receipt
    self.feePayment = feePayment
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "account onboarding prepare request"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1 else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported account onboarding prepare schema"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      receipt: container.decode(ToriiAccountOnboardingPlanReceipt.self, forKey: .receipt),
      feePayment: container.decode(FeePaymentIntent.self, forKey: .feePayment)
    )
  }
}

/// Authenticated exact sponsored-onboarding transaction returned by prepare.
public struct ToriiAccountOnboardingPreparedTransactionV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.preparedTransactionSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let operation: ToriiPreparedAccountOperationV1
  public let receipt: ToriiAccountOnboardingPlanReceipt
  public let semanticHashHex: String
  public let accountId: String
  public let alias: String
  public let disposition: AliasPlanDispositionV1
  public let transactionHashHex: String
  public let signedTransactionWireHex: String
  public let signedTransactionWireSHA256: String
  public let feePayment: FeePaymentIntent
  public let serverSignature: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, operation, receipt, alias, disposition
    case semanticHashHex = "semantic_hash_hex"
    case accountId = "account_id"
    case transactionHashHex = "transaction_hash_hex"
    case signedTransactionWireHex = "signed_transaction_wire_hex"
    case signedTransactionWireSHA256 = "signed_transaction_wire_sha256"
    case feePayment = "fee_payment"
    case serverSignature = "server_signature"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    receipt: ToriiAccountOnboardingPlanReceipt,
    semanticHashHex: String,
    accountId: String,
    alias: String,
    disposition: AliasPlanDispositionV1,
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSHA256: String,
    feePayment: FeePaymentIntent,
    serverSignature: String
  ) throws {
    try binding.validate(expectedOperation: .onboarding, activeAtUnixMs: nil)
    let wire = try ToriiPreparedAccountProtocolV1.wireIdentity(
      transactionHashHex: transactionHashHex,
      signedTransactionWireHex: signedTransactionWireHex,
      signedTransactionWireSHA256: signedTransactionWireSHA256
    )
    let canonicalAccountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "account_id"
    )
    let canonicalAlias = try ToriiPreparedAccountProtocolV1.canonicalAlias(alias)
    guard
      try toriiAccountIdsHaveSameIdentity(
        receipt.body.request.accountId,
        canonicalAccountId
      ),
      receipt.body.request.alias == canonicalAlias,
      ToriiPreparedAccountProtocolV1.dispositionTransitionAllowed(
        planned: receipt.body.resource.disposition,
        prepared: disposition
      )
    else {
      throw ToriiClientError.invalidPayload(
        "prepared onboarding identity or disposition differs from its receipt."
      )
    }
    schema = Self.schemaV1
    self.binding = binding
    operation = .onboarding
    self.receipt = receipt
    self.semanticHashHex = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      semanticHashHex,
      bytes: 32,
      field: "semantic_hash_hex"
    )
    self.accountId = canonicalAccountId
    self.alias = canonicalAlias
    self.disposition = disposition
    self.transactionHashHex = wire.0
    self.signedTransactionWireHex = wire.1
    self.signedTransactionWireSHA256 = wire.2
    self.feePayment = feePayment
    self.serverSignature = try ToriiPreparedAccountProtocolV1.exactSignature(serverSignature)
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "prepared onboarding transaction"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1,
      try container.decode(ToriiPreparedAccountOperationV1.self, forKey: .operation)
        == .onboarding
    else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported prepared onboarding transaction schema or operation"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      receipt: container.decode(ToriiAccountOnboardingPlanReceipt.self, forKey: .receipt),
      semanticHashHex: container.decode(String.self, forKey: .semanticHashHex),
      accountId: container.decode(String.self, forKey: .accountId),
      alias: container.decode(String.self, forKey: .alias),
      disposition: container.decode(AliasPlanDispositionV1.self, forKey: .disposition),
      transactionHashHex: container.decode(String.self, forKey: .transactionHashHex),
      signedTransactionWireHex: container.decode(String.self, forKey: .signedTransactionWireHex),
      signedTransactionWireSHA256: container.decode(
        String.self, forKey: .signedTransactionWireSHA256),
      feePayment: container.decode(FeePaymentIntent.self, forKey: .feePayment),
      serverSignature: container.decode(String.self, forKey: .serverSignature)
    )
  }

  func validate(
    receipt expectedReceipt: ToriiAccountOnboardingPlanReceipt,
    binding expectedBinding: ToriiTairaPublicResetMutationBindingV1,
    semanticHashHex expectedSemanticHashHex: String,
    expectedFeePayment: FeePaymentIntent,
    expectedAuthority: String,
    expectedNetworkId: NetworkId
  ) throws {
    guard receipt == expectedReceipt,
      binding == expectedBinding,
      semanticHashHex == expectedSemanticHashHex,
      feePayment.hasSamePayerAndGasBound(as: expectedFeePayment)
    else {
      throw ToriiClientError.invalidResponse
    }
    let transaction = try ToriiPreparedAccountProtocolV1.inspectWire(
      transactionHashHex: transactionHashHex,
      signedTransactionWireHex: signedTransactionWireHex,
      signedTransactionWireSHA256: signedTransactionWireSHA256
    )
    try ToriiPreparedAccountProtocolV1.validatePreparedTransaction(
      transaction,
      feePayment: feePayment,
      binding: binding,
      operation: operation,
      semanticHashHex: semanticHashHex,
      expectedAuthority: expectedAuthority,
      expectedNetworkId: expectedNetworkId
    )
    try ToriiPreparedAccountProtocolV1.verifyServerSignature(
      transcript: try signatureTranscript(),
      serverSignature: serverSignature,
      expectedAuthority: expectedAuthority
    )
  }

  func signatureTranscript() throws -> Data {
    var transcript = ToriiPreparedAccountProtocolV1.baseSignatureTranscript(
      envelopeSchema: schema,
      operation: operation,
      binding: binding
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "semantic_hash_hex",
      semanticHashHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField("account_id", accountId, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField("alias", alias, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField(
      "disposition",
      disposition.rawValue,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "transaction_hash_hex",
      transactionHashHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "signed_transaction_wire_sha256",
      signedTransactionWireSHA256,
      to: &transcript
    )
    guard let wire = Data(hexString: signedTransactionWireHex) else {
      throw ToriiClientError.invalidPayload(
        "signed_transaction_wire_hex is not canonical lowercase hexadecimal."
      )
    }
    ToriiPreparedAccountProtocolV1.appendField(
      "signed_transaction_wire",
      wire,
      to: &transcript
    )
    return transcript
  }
}

/// Authenticated nonterminal result that still requires one fresh atomic state observation.
public struct ToriiAccountOnboardingProofRequiredPrepareResponseV1:
  Codable, Equatable, Sendable
{
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.onboardingProofRequiredSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let operation: ToriiPreparedAccountOperationV1
  public let outcome: String
  public let proofKind: String
  public let semanticHashHex: String
  public let accountId: String
  public let alias: String
  public let disposition: AliasPlanDispositionV1
  public let serverSignature: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, operation, outcome, alias, disposition
    case proofKind = "proof_kind"
    case semanticHashHex = "semantic_hash_hex"
    case accountId = "account_id"
    case serverSignature = "server_signature"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    semanticHashHex: String,
    accountId: String,
    alias: String,
    disposition: AliasPlanDispositionV1,
    serverSignature: String
  ) throws {
    try binding.validate(expectedOperation: .onboarding, activeAtUnixMs: nil)
    guard disposition == .noOp else {
      throw ToriiClientError.invalidPayload(
        "a proof-required onboarding result must report the no_op disposition."
      )
    }
    schema = Self.schemaV1
    self.binding = binding
    operation = .onboarding
    outcome = "ProofRequired"
    proofKind = "account_alias_current_state"
    self.semanticHashHex = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      semanticHashHex,
      bytes: 32,
      field: "semantic_hash_hex"
    )
    self.accountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "account_id"
    )
    self.alias = try ToriiPreparedAccountProtocolV1.canonicalAlias(alias)
    self.disposition = disposition
    self.serverSignature = try ToriiPreparedAccountProtocolV1.exactSignature(serverSignature)
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "proof-required onboarding prepare response"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1,
      try container.decode(ToriiPreparedAccountOperationV1.self, forKey: .operation)
        == .onboarding,
      try container.decode(String.self, forKey: .outcome) == "ProofRequired",
      try container.decode(String.self, forKey: .proofKind) == "account_alias_current_state"
    else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported proof-required onboarding result"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      semanticHashHex: container.decode(String.self, forKey: .semanticHashHex),
      accountId: container.decode(String.self, forKey: .accountId),
      alias: container.decode(String.self, forKey: .alias),
      disposition: container.decode(AliasPlanDispositionV1.self, forKey: .disposition),
      serverSignature: container.decode(String.self, forKey: .serverSignature)
    )
  }

  func validate(
    receipt: ToriiAccountOnboardingPlanReceipt,
    binding expectedBinding: ToriiTairaPublicResetMutationBindingV1,
    semanticHashHex expectedSemanticHashHex: String,
    expectedAuthority: String
  ) throws {
    guard binding == expectedBinding,
      semanticHashHex == expectedSemanticHashHex,
      accountId == receipt.body.request.accountId,
      alias == receipt.body.request.alias
    else {
      throw ToriiClientError.invalidResponse
    }
    try ToriiPreparedAccountProtocolV1.verifyServerSignature(
      transcript: signatureTranscript(),
      serverSignature: serverSignature,
      expectedAuthority: expectedAuthority
    )
  }

  func signatureTranscript() -> Data {
    var transcript = ToriiPreparedAccountProtocolV1.baseSignatureTranscript(
      envelopeSchema: schema,
      operation: operation,
      binding: binding
    )
    ToriiPreparedAccountProtocolV1.appendField("outcome", outcome, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField("proof_kind", proofKind, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField(
      "semantic_hash_hex",
      semanticHashHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField("account_id", accountId, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField("alias", alias, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField(
      "disposition",
      disposition.rawValue,
      to: &transcript
    )
    return transcript
  }
}

/// Closed onboarding prepare result: either one exact transaction or a nonterminal proof request.
public enum ToriiAccountOnboardingPrepareResponseV1: Codable, Equatable, Sendable {
  case prepared(ToriiAccountOnboardingPreparedTransactionV1)
  case proofRequired(ToriiAccountOnboardingProofRequiredPrepareResponseV1)

  private enum ProbeKeys: String, CodingKey {
    case schema
  }

  public init(from decoder: Decoder) throws {
    let probe = try decoder.container(keyedBy: ProbeKeys.self)
    switch try probe.decode(String.self, forKey: .schema) {
    case ToriiAccountOnboardingPreparedTransactionV1.schemaV1:
      self = .prepared(try ToriiAccountOnboardingPreparedTransactionV1(from: decoder))
    case ToriiAccountOnboardingProofRequiredPrepareResponseV1.schemaV1:
      self = .proofRequired(
        try ToriiAccountOnboardingProofRequiredPrepareResponseV1(from: decoder)
      )
    default:
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: probe,
        debugDescription: "unsupported account onboarding prepare response schema"
      )
    }
  }

  public func encode(to encoder: Encoder) throws {
    switch self {
    case .prepared(let value):
      try value.encode(to: encoder)
    case .proofRequired(let value):
      try value.encode(to: encoder)
    }
  }
}

/// Exact closed request for one atomic account-onboarding state observation.
public struct ToriiAccountOnboardingCurrentStateRequestV1: Codable, Equatable, Sendable {
  public static let version: UInt8 = 1

  public let version: UInt8
  public let accountId: String
  public let alias: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version, alias
    case accountId = "account_id"
  }

  public init(accountId: String, alias: String) throws {
    version = Self.version
    self.accountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "account_id"
    )
    self.alias = try ToriiPreparedAccountProtocolV1.canonicalAlias(alias)
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "account onboarding current-state request"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(UInt8.self, forKey: .version) == Self.version else {
      throw DecodingError.dataCorruptedError(
        forKey: .version,
        in: container,
        debugDescription: "unsupported account onboarding current-state request version"
      )
    }
    try self.init(
      accountId: container.decode(String.self, forKey: .accountId),
      alias: container.decode(String.self, forKey: .alias)
    )
  }
}

/// Typed canonical committed-block hash carried by an atomic onboarding observation.
public struct ToriiAccountOnboardingBlockHashV1: Codable, Equatable, Hashable, Sendable {
  public let literal: String

  public init(literal: String) throws {
    do {
      _ = try NetworkId(literal: literal)
    } catch {
      throw ToriiClientError.invalidPayload(
        "observed_block_hash must be one exact canonical checksummed Iroha hash."
      )
    }
    self.literal = literal
  }

  public init(from decoder: Decoder) throws {
    let container = try decoder.singleValueContainer()
    do {
      try self.init(literal: container.decode(String.self))
    } catch {
      throw DecodingError.dataCorruptedError(
        in: container,
        debugDescription: "observed_block_hash must be a canonical Iroha hash"
      )
    }
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.singleValueContainer()
    try container.encode(literal)
  }
}

/// One internally consistent atomic account-onboarding state observation.
public struct ToriiAccountOnboardingCurrentStateResponseV1: Codable, Equatable, Sendable {
  public static let version: UInt8 = 1

  public let version: UInt8
  public let networkId: NetworkId
  public let accountId: String
  public let alias: String
  public let accountExists: Bool
  public let aliasTargetAccountId: String?
  public let observedBlockHeight: UInt64
  public let observedBlockHash: ToriiAccountOnboardingBlockHashV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version, alias
    case networkId = "network_id"
    case accountId = "account_id"
    case accountExists = "account_exists"
    case aliasTargetAccountId = "alias_target_account_id"
    case observedBlockHeight = "observed_block_height"
    case observedBlockHash = "observed_block_hash"
  }

  public init(
    networkId: NetworkId,
    accountId: String,
    alias: String,
    accountExists: Bool,
    aliasTargetAccountId: String?,
    observedBlockHeight: UInt64,
    observedBlockHash: ToriiAccountOnboardingBlockHashV1
  ) throws {
    guard observedBlockHeight > 0 else {
      throw ToriiClientError.invalidPayload(
        "observed_block_height must be a nonzero committed height."
      )
    }
    version = Self.version
    self.networkId = networkId
    self.accountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "account_id"
    )
    self.alias = try ToriiPreparedAccountProtocolV1.canonicalAlias(alias)
    self.accountExists = accountExists
    if let aliasTargetAccountId {
      self.aliasTargetAccountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
        aliasTargetAccountId,
        field: "alias_target_account_id"
      )
      let targetMatchesAccount = try toriiAccountIdsHaveSameIdentity(
        self.accountId,
        aliasTargetAccountId
      )
      guard accountExists || !targetMatchesAccount else {
        throw ToriiClientError.invalidPayload(
          "an alias target cannot equal an account reported absent in the same snapshot."
        )
      }
    } else {
      self.aliasTargetAccountId = nil
    }
    self.observedBlockHeight = observedBlockHeight
    self.observedBlockHash = observedBlockHash
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "account onboarding current-state response"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard CodingKeys.allCases.allSatisfy({ container.contains($0) }),
      try container.decode(UInt8.self, forKey: .version) == Self.version
    else {
      throw DecodingError.dataCorruptedError(
        forKey: .version,
        in: container,
        debugDescription: "incomplete or unsupported account onboarding current-state response"
      )
    }
    try self.init(
      networkId: container.decode(NetworkId.self, forKey: .networkId),
      accountId: container.decode(String.self, forKey: .accountId),
      alias: container.decode(String.self, forKey: .alias),
      accountExists: container.decode(Bool.self, forKey: .accountExists),
      aliasTargetAccountId: container.decodeIfPresent(
        String.self,
        forKey: .aliasTargetAccountId
      ),
      observedBlockHeight: container.decode(UInt64.self, forKey: .observedBlockHeight),
      observedBlockHash: container.decode(
        ToriiAccountOnboardingBlockHashV1.self,
        forKey: .observedBlockHash
      )
    )
  }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)
    try container.encode(version, forKey: .version)
    try container.encode(networkId, forKey: .networkId)
    try container.encode(accountId, forKey: .accountId)
    try container.encode(alias, forKey: .alias)
    try container.encode(accountExists, forKey: .accountExists)
    if let aliasTargetAccountId {
      try container.encode(aliasTargetAccountId, forKey: .aliasTargetAccountId)
    } else {
      try container.encodeNil(forKey: .aliasTargetAccountId)
    }
    try container.encode(observedBlockHeight, forKey: .observedBlockHeight)
    try container.encode(observedBlockHash, forKey: .observedBlockHash)
  }

  func validate(
    request: ToriiAccountOnboardingCurrentStateRequestV1,
    expectedNetworkId: NetworkId
  ) throws {
    guard networkId == expectedNetworkId,
      accountId == request.accountId,
      alias == request.alias
    else {
      throw ToriiClientError.invalidPayload(
        "account onboarding current-state response substituted the exact request or network."
      )
    }
  }
}

/// Closed classification derived from one committed state snapshot.
public enum ToriiAccountOnboardingCurrentStateVerificationV1: Equatable, Sendable {
  case applied(
    blockHeight: UInt64,
    blockHash: ToriiAccountOnboardingBlockHashV1
  )
  case aliasAbsent(
    blockHeight: UInt64,
    blockHash: ToriiAccountOnboardingBlockHashV1
  )
  case aliasConflict(
    blockHeight: UInt64,
    blockHash: ToriiAccountOnboardingBlockHashV1
  )
}

/// Solved account faucet claim consumed by non-mutating prepare.
public struct ToriiAccountFaucetClaimV1: Codable, Equatable, Sendable {
  public let accountId: String
  public let powAnchorHeight: UInt64
  public let powNonceHex: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case accountId = "account_id"
    case powAnchorHeight = "pow_anchor_height"
    case powNonceHex = "pow_nonce_hex"
  }

  public init(
    accountId: String,
    powAnchorHeight: UInt64,
    powNonceHex: String
  ) throws {
    self.accountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "claim.account_id"
    )
    guard powAnchorHeight > 0 else {
      throw ToriiClientError.invalidPayload("claim.pow_anchor_height must be positive.")
    }
    self.powAnchorHeight = powAnchorHeight
    let nonce = try ToriiPreparedAccountProtocolV1.exactLowerHexBytes(
      powNonceHex,
      field: "claim.pow_nonce_hex"
    )
    guard nonce.count <= 32 else {
      throw ToriiClientError.invalidPayload(
        "claim.pow_nonce_hex must not exceed 32 bytes."
      )
    }
    self.powNonceHex = powNonceHex
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "account faucet claim"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    try self.init(
      accountId: container.decode(String.self, forKey: .accountId),
      powAnchorHeight: container.decode(UInt64.self, forKey: .powAnchorHeight),
      powNonceHex: container.decode(String.self, forKey: .powNonceHex)
    )
  }
}

/// Independently trusted first-release faucet authority and issuance policy.
///
/// Construct this value only from trusted deployment configuration. Prepared responses cannot
/// select or replace the faucet authority, asset definition, or exact issuance amount.
public struct ToriiAccountFaucetPolicyV1: Equatable, Sendable {
  public let faucetAuthority: String
  public let assetDefinitionId: String
  public let amount: KotodamaQuantity

  public init(
    faucetAuthority: String,
    assetDefinitionId: String,
    amount: KotodamaQuantity
  ) throws {
    let exactAuthority = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      faucetAuthority,
      field: "faucetPolicy.faucetAuthority"
    )
    let address = try AccountAddress.parseEncoded(exactAuthority)
    guard let controller = address.singleControllerInfo(),
      controller.algorithm == .ed25519,
      Ed25519PublicKeyAdmission.isValidPublicKey(controller.publicKey)
    else {
      throw ToriiClientError.invalidPayload(
        "faucetPolicy.faucetAuthority must be one canonical Ed25519 account."
      )
    }
    guard AssetDefinitionAddressCodec.canonicalDefinitionLiteral(assetDefinitionId)
      == assetDefinitionId
    else {
      throw ToriiClientError.invalidPayload(
        "faucetPolicy.assetDefinitionId must be one canonical asset definition id."
      )
    }
    guard amount.canonicalString != "0" else {
      throw ToriiClientError.invalidPayload("faucetPolicy.amount must be positive.")
    }
    self.faucetAuthority = exactAuthority
    self.assetDefinitionId = assetDefinitionId
    self.amount = amount
  }
}

/// Exact non-mutating faucet prepare request.
public struct ToriiAccountFaucetPrepareRequestV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.faucetPrepareSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let claim: ToriiAccountFaucetClaimV1
  public let feePayment: FeePaymentIntent

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, claim
    case feePayment = "fee_payment"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    claim: ToriiAccountFaucetClaimV1,
    feePayment: FeePaymentIntent
  ) throws {
    try binding.validate(expectedOperation: .faucet, activeAtUnixMs: nil)
    _ = try feePayment.canonicalJSONData()
    schema = Self.schemaV1
    self.binding = binding
    self.claim = claim
    self.feePayment = feePayment
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "account faucet prepare request"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1 else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported account faucet prepare schema"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      claim: container.decode(ToriiAccountFaucetClaimV1.self, forKey: .claim),
      feePayment: container.decode(FeePaymentIntent.self, forKey: .feePayment)
    )
  }
}

/// Authenticated exact faucet transaction returned by prepare.
public struct ToriiAccountFaucetPreparedTransactionV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.preparedTransactionSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let operation: ToriiPreparedAccountOperationV1
  public let claim: ToriiAccountFaucetClaimV1
  public let semanticHashHex: String
  public let accountId: String
  public let assetDefinitionId: String
  public let assetId: String
  public let amount: String
  public let transactionHashHex: String
  public let signedTransactionWireHex: String
  public let signedTransactionWireSHA256: String
  public let feePayment: FeePaymentIntent
  public let serverSignature: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, operation, claim, amount
    case semanticHashHex = "semantic_hash_hex"
    case accountId = "account_id"
    case assetDefinitionId = "asset_definition_id"
    case assetId = "asset_id"
    case transactionHashHex = "transaction_hash_hex"
    case signedTransactionWireHex = "signed_transaction_wire_hex"
    case signedTransactionWireSHA256 = "signed_transaction_wire_sha256"
    case feePayment = "fee_payment"
    case serverSignature = "server_signature"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    claim: ToriiAccountFaucetClaimV1,
    semanticHashHex: String,
    accountId: String,
    assetDefinitionId: String,
    assetId: String,
    amount: String,
    transactionHashHex: String,
    signedTransactionWireHex: String,
    signedTransactionWireSHA256: String,
    feePayment: FeePaymentIntent,
    serverSignature: String
  ) throws {
    try binding.validate(expectedOperation: .faucet, activeAtUnixMs: nil)
    let wire = try ToriiPreparedAccountProtocolV1.wireIdentity(
      transactionHashHex: transactionHashHex,
      signedTransactionWireHex: signedTransactionWireHex,
      signedTransactionWireSHA256: signedTransactionWireSHA256
    )
    let canonicalAccountId = try ToriiPreparedAccountProtocolV1.canonicalAccountId(
      accountId,
      field: "account_id"
    )
    guard try toriiAccountIdsHaveSameIdentity(canonicalAccountId, claim.accountId),
      AssetDefinitionAddress.decode(assetDefinitionId) != nil,
      assetId == "\(assetDefinitionId)#\(canonicalAccountId)"
    else {
      throw ToriiClientError.invalidPayload(
        "prepared faucet account, asset definition, or destination asset is noncanonical."
      )
    }
    schema = Self.schemaV1
    self.binding = binding
    operation = .faucet
    self.claim = claim
    self.semanticHashHex = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      semanticHashHex,
      bytes: 32,
      field: "semantic_hash_hex"
    )
    self.accountId = canonicalAccountId
    self.assetDefinitionId = try ToriiPreparedAccountProtocolV1.exactIdentifier(
      assetDefinitionId,
      field: "asset_definition_id"
    )
    self.assetId = try ToriiPreparedAccountProtocolV1.exactIdentifier(
      assetId,
      field: "asset_id"
    )
    self.amount = try ToriiPreparedAccountProtocolV1.canonicalQuantity(amount)
    self.transactionHashHex = wire.0
    self.signedTransactionWireHex = wire.1
    self.signedTransactionWireSHA256 = wire.2
    self.feePayment = feePayment
    self.serverSignature = try ToriiPreparedAccountProtocolV1.exactSignature(serverSignature)
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "prepared faucet transaction"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1,
      try container.decode(ToriiPreparedAccountOperationV1.self, forKey: .operation)
        == .faucet
    else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported prepared faucet transaction schema or operation"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      claim: container.decode(ToriiAccountFaucetClaimV1.self, forKey: .claim),
      semanticHashHex: container.decode(String.self, forKey: .semanticHashHex),
      accountId: container.decode(String.self, forKey: .accountId),
      assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
      assetId: container.decode(String.self, forKey: .assetId),
      amount: container.decode(String.self, forKey: .amount),
      transactionHashHex: container.decode(String.self, forKey: .transactionHashHex),
      signedTransactionWireHex: container.decode(String.self, forKey: .signedTransactionWireHex),
      signedTransactionWireSHA256: container.decode(
        String.self, forKey: .signedTransactionWireSHA256),
      feePayment: container.decode(FeePaymentIntent.self, forKey: .feePayment),
      serverSignature: container.decode(String.self, forKey: .serverSignature)
    )
  }

  func validate(
    claim expectedClaim: ToriiAccountFaucetClaimV1,
    binding expectedBinding: ToriiTairaPublicResetMutationBindingV1,
    expectedFeePayment: FeePaymentIntent,
    policy: ToriiAccountFaucetPolicyV1,
    expectedNetworkId: NetworkId
  ) throws {
    guard claim == expectedClaim,
      binding == expectedBinding,
      semanticHashHex == (try ToriiPreparedAccountProtocolV1.faucetSemanticHash(expectedClaim)),
      assetDefinitionId == policy.assetDefinitionId,
      amount == policy.amount.canonicalString,
      feePayment.hasSamePayerAndGasBound(as: expectedFeePayment)
    else {
      throw ToriiClientError.invalidResponse
    }
    let transaction = try ToriiPreparedAccountProtocolV1.inspectWire(
      transactionHashHex: transactionHashHex,
      signedTransactionWireHex: signedTransactionWireHex,
      signedTransactionWireSHA256: signedTransactionWireSHA256
    )
    try ToriiPreparedAccountProtocolV1.validatePreparedTransaction(
      transaction,
      feePayment: feePayment,
      binding: binding,
      operation: operation,
      semanticHashHex: semanticHashHex,
      expectedAuthority: policy.faucetAuthority,
      expectedNetworkId: expectedNetworkId
    )
    try ToriiPreparedAccountProtocolV1.verifyServerSignature(
      transcript: try signatureTranscript(),
      serverSignature: serverSignature,
      expectedAuthority: policy.faucetAuthority
    )
  }

  func signatureTranscript() throws -> Data {
    var transcript = ToriiPreparedAccountProtocolV1.baseSignatureTranscript(
      envelopeSchema: schema,
      operation: operation,
      binding: binding
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "claim.account_id",
      claim.accountId,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "claim.pow_anchor_height",
      String(claim.powAnchorHeight),
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "claim.pow_nonce_hex",
      claim.powNonceHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "semantic_hash_hex",
      semanticHashHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField("account_id", accountId, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField(
      "asset_definition_id",
      assetDefinitionId,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField("asset_id", assetId, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField("amount", amount, to: &transcript)
    ToriiPreparedAccountProtocolV1.appendField(
      "transaction_hash_hex",
      transactionHashHex,
      to: &transcript
    )
    ToriiPreparedAccountProtocolV1.appendField(
      "signed_transaction_wire_sha256",
      signedTransactionWireSHA256,
      to: &transcript
    )
    guard let wire = Data(hexString: signedTransactionWireHex) else {
      throw ToriiClientError.invalidPayload(
        "signed_transaction_wire_hex is not canonical lowercase hexadecimal."
      )
    }
    ToriiPreparedAccountProtocolV1.appendField(
      "signed_transaction_wire",
      wire,
      to: &transcript
    )
    return transcript
  }
}

/// Canonical reconciliation outcome for one exact prepared transaction hash.
public enum ToriiPreparedTransactionOutcomeV1: String, Codable, Equatable, Sendable {
  case applied = "Applied"
  case pending = "Pending"
  case rejected = "Rejected"
}

/// Exact submit/reconciliation response for a prepared onboarding or faucet transaction.
public struct ToriiPreparedTransactionSubmitResponseV1: Codable, Equatable, Sendable {
  public static let schemaV1 = ToriiPreparedAccountProtocolV1.submitResponseSchema

  public let schema: String
  public let binding: ToriiTairaPublicResetMutationBindingV1
  public let operation: ToriiPreparedAccountOperationV1
  public let transactionHashHex: String
  public let outcome: ToriiPreparedTransactionOutcomeV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case schema, binding, operation, outcome
    case transactionHashHex = "transaction_hash_hex"
  }

  public init(
    binding: ToriiTairaPublicResetMutationBindingV1,
    operation: ToriiPreparedAccountOperationV1,
    transactionHashHex: String,
    outcome: ToriiPreparedTransactionOutcomeV1
  ) throws {
    try binding.validate(expectedOperation: operation, activeAtUnixMs: nil)
    schema = Self.schemaV1
    self.binding = binding
    self.operation = operation
    self.transactionHashHex = try ToriiPreparedAccountProtocolV1.exactLowerHex(
      transactionHashHex,
      bytes: 32,
      field: "transaction_hash_hex"
    )
    self.outcome = outcome
  }

  public init(from decoder: Decoder) throws {
    try ToriiPreparedAccountProtocolV1.rejectUnknownFields(
      from: decoder,
      keys: CodingKeys.self,
      name: "prepared transaction submit response"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    guard try container.decode(String.self, forKey: .schema) == Self.schemaV1 else {
      throw DecodingError.dataCorruptedError(
        forKey: .schema,
        in: container,
        debugDescription: "unsupported prepared transaction submit response schema"
      )
    }
    try self.init(
      binding: container.decode(ToriiTairaPublicResetMutationBindingV1.self, forKey: .binding),
      operation: container.decode(ToriiPreparedAccountOperationV1.self, forKey: .operation),
      transactionHashHex: container.decode(String.self, forKey: .transactionHashHex),
      outcome: container.decode(ToriiPreparedTransactionOutcomeV1.self, forKey: .outcome)
    )
  }

  func validate(
    binding expectedBinding: ToriiTairaPublicResetMutationBindingV1,
    operation expectedOperation: ToriiPreparedAccountOperationV1,
    transactionHashHex expectedTransactionHashHex: String,
    httpStatus: Int
  ) throws {
    guard binding == expectedBinding,
      operation == expectedOperation,
      transactionHashHex == expectedTransactionHashHex,
      httpStatus != 202 || outcome == .pending
    else {
      throw ToriiClientError.invalidResponse
    }
  }
}
