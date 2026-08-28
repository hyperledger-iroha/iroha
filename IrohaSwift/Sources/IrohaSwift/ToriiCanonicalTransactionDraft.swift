import CryptoKit
import Foundation

/// Strictly decoded canonical unsigned transaction material shared by every V1 draft route.
struct ToriiCanonicalTransactionDraft {
  static let maximumTransactionPayloadBytes = 16 * 1024 * 1024
  private static let maximumMetadataEntries = 1_024
  private static let maximumMetadataJSONBytes = 1 * 1024 * 1024

  struct Payload {
    let networkId: NetworkId
    let authority: Data
    let creationTimeMs: UInt64
    let executable: Data
    let timeToLiveMs: UInt64?
    let nonce: UInt32?
    let feePayment: Data
    let metadataWire: Data
    let metadata: [String: ToriiJSONValue]
  }

  struct SignedTransactionV1 {
    let wire: Data
    let transactionPayload: Data
    let payload: Payload
    let signerPublicKey: Data
  }

  let transactionPayload: Data
  let signingMessage: Data
  let payload: Payload

  static func decode(
    transactionPayloadB64: String,
    signingMessageB64: String,
    expectedAdmissionIntent: TransactionAdmissionIntentV1,
    context: String
  ) throws -> Self {
    let transactionPayload = try canonicalBase64(
      transactionPayloadB64,
      maximumBytes: maximumTransactionPayloadBytes,
      exactBytes: nil,
      field: "\(context).transaction_payload_b64"
    )
    let signingMessage = try canonicalBase64(
      signingMessageB64,
      maximumBytes: 32,
      exactBytes: 32,
      field: "\(context).signing_message_b64"
    )
    guard signingMessage == IrohaHash.hash(transactionPayload) else {
      throw ToriiClientError.invalidPayload(
        "\(context).signing_message_b64 must be the exact Iroha prehash of transaction_payload_b64."
      )
    }
    return Self(
      transactionPayload: transactionPayload,
      signingMessage: signingMessage,
      payload: try parsePayload(
        transactionPayload,
        expectedAdmissionIntent: expectedAdmissionIntent,
        context: context
      )
    )
  }

  static func canonicalBase64(
    _ value: String,
    maximumBytes: Int,
    exactBytes: Int?,
    field: String
  ) throws -> Data {
    let encodedLengthBound = 4 * ((maximumBytes + 2) / 3)
    guard !value.isEmpty,
      value.utf8.count <= encodedLengthBound,
      value.utf8.count.isMultiple(of: 4),
      let decoded = Data(base64Encoded: value),
      !decoded.isEmpty,
      decoded.count <= maximumBytes,
      decoded.base64EncodedString() == value
    else {
      throw ToriiClientError.invalidPayload(
        "\(field) must be bounded canonical non-empty padded base64."
      )
    }
    if let exactBytes, decoded.count != exactBytes {
      throw ToriiClientError.invalidPayload(
        "\(field) must decode to exactly \(exactBytes) bytes."
      )
    }
    return decoded
  }

  static func requireAuthority(
    _ payload: Payload,
    equals accountId: String,
    context: String
  ) throws {
    let expected = try AccountAddress.parseEncoded(accountId)
      .compactNoritoAccountControllerPayload()
    guard payload.authority == expected else {
      throw ToriiClientError.invalidPayload(
        "\(context) changed the requested transaction authority."
      )
    }
  }

  static func finalize(
    transactionPayload: Data,
    publicKey: Data,
    signature: Data
  ) throws -> DetachedTransactionFinalizationResult {
    guard publicKey.count == 32,
      signature.count == 64,
      let key = try? Curve25519.Signing.PublicKey(rawRepresentation: publicKey),
      key.isValidSignature(signature, for: IrohaHash.hash(transactionPayload))
    else {
      throw ToriiClientError.invalidPayload(
        "detached signature is not a valid Ed25519 signature of the exact transaction payload."
      )
    }

    var signatureBytes = CompactNoritoWriter()
    signatureBytes.writeUInt64LE(UInt64(signature.count))
    signatureBytes.writeByteFields(signature)
    var signatureSet = CompactNoritoWriter()
    signatureSet.writeField(signatureBytes.data)
    var signed = CompactNoritoWriter()
    signed.writeField(signatureSet.data)
    signed.writeField(transactionPayload)
    signed.writeField(Data([0]))
    var versioned = Data([1])
    versioned.append(signed.data)

    var entrypoint = CompactNoritoWriter()
    entrypoint.writeUInt32LE(0)
    entrypoint.writeField(transactionPayload)
    let payloadSigningHash = IrohaHash.hash(transactionPayload)
    let transactionHash = IrohaHash.hash(entrypoint.data)
    return DetachedTransactionFinalizationResult(
      signedTransaction: versioned,
      finalization: DetachedTransactionFinalization(
        payloadSigningHash: payloadSigningHash,
        transactionHash: transactionHash,
        entrypointHash: transactionHash
      )
    )
  }

  static func transactionPayload(
    fromVersionedSignedTransaction bytes: Data,
    expectedAdmissionIntent: TransactionAdmissionIntentV1,
    context: String
  ) throws -> Data {
    try inspectVersionedSignedTransaction(
      bytes,
      expectedAdmissionIntent: expectedAdmissionIntent,
      context: context
    ).transactionPayload
  }

  static func validateTransactionPayload(
    _ bytes: Data,
    expectedAdmissionIntent: TransactionAdmissionIntentV1,
    context: String
  ) throws {
    guard !bytes.isEmpty, bytes.count <= maximumTransactionPayloadBytes else {
      throw ToriiClientError.invalidPayload(
        "\(context) must be a bounded non-empty canonical V1 TransactionPayload."
      )
    }
    _ = try parsePayload(
      bytes,
      expectedAdmissionIntent: expectedAdmissionIntent,
      context: context
    )
  }

  static func inspectVersionedSignedTransaction(
    _ bytes: Data,
    expectedAdmissionIntent: TransactionAdmissionIntentV1,
    context: String
  ) throws -> SignedTransactionV1 {
    guard !bytes.isEmpty,
      bytes.count <= maximumTransactionPayloadBytes + 16 * 1024,
      bytes.first == 1
    else {
      throw ToriiClientError.invalidPayload(
        "\(context) must use signed-transaction wire version 1.")
    }
    var reader = CanonicalNoritoReader(data: Data(bytes.dropFirst()))
    let signatureWrapper = try reader.readCompactField()
    let transactionPayload = try reader.readCompactField()
    let attachments = try reader.readCompactField()
    guard reader.remaining() == 0,
      attachments == Data([0]),
      !signatureWrapper.isEmpty,
      !transactionPayload.isEmpty,
      transactionPayload.count <= maximumTransactionPayloadBytes
    else {
      throw ToriiClientError.invalidPayload(
        "\(context) must contain exactly one signature, one payload, and no attachments."
      )
    }
    let signature = try decodeEd25519Signature(
      signatureWrapper,
      context: "\(context).signature"
    )
    let payload = try parsePayload(
      transactionPayload,
      expectedAdmissionIntent: expectedAdmissionIntent,
      context: context
    )
    let signerPublicKey = try decodeEd25519Authority(
      payload.authority,
      context: "\(context).authority"
    )
    guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: signerPublicKey),
      key.isValidSignature(signature, for: IrohaHash.hash(transactionPayload))
    else {
      throw ToriiClientError.invalidPayload(
        "\(context) contains an invalid inner Ed25519 transaction signature."
      )
    }

    var canonicalSigned = CompactNoritoWriter()
    canonicalSigned.writeField(signatureWrapper)
    canonicalSigned.writeField(transactionPayload)
    canonicalSigned.writeField(Data([0]))
    var canonicalWire = Data([1])
    canonicalWire.append(canonicalSigned.data)
    guard canonicalWire == bytes else {
      throw ToriiClientError.invalidPayload(
        "\(context) is not one canonical fixed-V1 signed transaction."
      )
    }
    return SignedTransactionV1(
      wire: bytes,
      transactionPayload: transactionPayload,
      payload: payload,
      signerPublicKey: signerPublicKey
    )
  }

  /// Requires the exact fixed-V1 `Executable::Instructions` layout and at least one instruction.
  ///
  /// Prepared account envelopes use `ProofRequired` for an otherwise empty onboarding plan. An
  /// authenticated `Prepared` envelope therefore cannot carry an empty instruction vector without
  /// bypassing the required fresh atomic account-and-alias proof.
  static func requireNonemptyInstructionExecutable(
    _ executable: Data,
    context: String
  ) throws {
    do {
      var executableReader = CanonicalNoritoReader(data: executable)
      guard try executableReader.readUInt32LE() == 0 else {
        throw ToriiClientError.invalidPayload(
          "\(context) must use the fixed-V1 Executable::Instructions variant."
        )
      }
      let encodedInstructions = try executableReader.readCompactField()
      guard executableReader.remaining() == 0 else {
        throw ToriiClientError.invalidPayload(
          "\(context) executable contains trailing bytes."
        )
      }

      var instructionsReader = CanonicalNoritoReader(data: encodedInstructions)
      let instructionCount = try instructionsReader.readUInt64LE()
      guard instructionCount > 0,
        instructionCount <= UInt64(instructionsReader.remaining())
      else {
        throw ToriiClientError.invalidPayload(
          "\(context) must contain at least one bounded instruction."
        )
      }

      var canonicalInstructions = CompactNoritoWriter()
      canonicalInstructions.writeUInt64LE(instructionCount)
      for _ in 0..<instructionCount {
        let instruction = try instructionsReader.readCompactField()
        guard !instruction.isEmpty else {
          throw ToriiClientError.invalidPayload(
            "\(context) contains an empty instruction."
          )
        }
        canonicalInstructions.writeField(instruction)
      }
      guard instructionsReader.remaining() == 0,
        canonicalInstructions.data == encodedInstructions
      else {
        throw ToriiClientError.invalidPayload(
          "\(context) instruction vector is not canonical fixed-V1 Norito."
        )
      }

      var canonicalExecutable = CompactNoritoWriter()
      canonicalExecutable.writeUInt32LE(0)
      canonicalExecutable.writeField(encodedInstructions)
      guard canonicalExecutable.data == executable else {
        throw ToriiClientError.invalidPayload(
          "\(context) executable is not canonical fixed-V1 Norito."
        )
      }
    } catch let error as ToriiClientError {
      throw error
    } catch {
      throw ToriiClientError.invalidPayload(
        "\(context) is not one canonical non-empty instruction executable."
      )
    }
  }

  private static func parsePayload(
    _ bytes: Data,
    expectedAdmissionIntent: TransactionAdmissionIntentV1,
    context: String
  ) throws -> Payload {
    var transaction = ToriiVerifyingKeyCompactReader(bytes)
    let domain = try transaction.takeField("\(context).domain")
    let authority = try transaction.takeField("\(context).authority")
    let creation = try transaction.takeField("\(context).creation_time_ms")
    let executable = try transaction.takeField("\(context).executable")
    let timeToLive = try transaction.takeField("\(context).time_to_live_ms")
    let nonce = try transaction.takeField("\(context).nonce")
    let feePayment = try transaction.takeField("\(context).fee_payment")
    let admissionIntent = try transaction.takeField("\(context).admission_intent")
    let metadata = try transaction.takeField("\(context).metadata")
    let attachments = try transaction.takeField("\(context).attachments")
    guard transaction.isFinished,
      !authority.isEmpty,
      !executable.isEmpty,
      creation.count == MemoryLayout<UInt64>.size
    else {
      throw ToriiClientError.invalidPayload(
        "\(context).transaction_payload_b64 must contain exactly one canonical ten-field TransactionPayload."
      )
    }
    let creationTimeMs = creation.withUnsafeBytes {
      UInt64(littleEndian: $0.loadUnaligned(as: UInt64.self))
    }
    guard creationTimeMs > 0 else {
      throw ToriiClientError.invalidPayload("\(context) creation_time_ms must be positive.")
    }
    do {
      try SccpSubmitValidation.requireCanonicalTransactionFeePayment(feePayment)
    } catch {
      throw ToriiClientError.invalidPayload("\(context) fee_payment is not canonical.")
    }
    guard admissionIntent == expectedAdmissionIntent.norito else {
      let expectedName =
        switch expectedAdmissionIntent {
        case .ordinary: "Ordinary"
        case .queuePlanSynced: "QueuePlanSynced"
        }
      throw ToriiClientError.invalidPayload(
        "\(context) admission_intent must be \(expectedName).")
    }
    guard attachments == Data([0]) else {
      throw ToriiClientError.invalidPayload(
        "\(context) attachments must use the exact None encoding.")
    }
    guard AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(authority) else {
      throw ToriiClientError.invalidPayload(
        "\(context) authority is not one canonical AccountId controller."
      )
    }
    let networkId = try decodeNetworkDomain(domain, context: context)
    let metadataValue = try decodeMetadata(metadata, context: context)
    var canonicalDomain = CompactNoritoWriter()
    canonicalDomain.writeUInt32LE(0)
    canonicalDomain.writeField(networkId.bytes)
    var canonicalPayload = CompactNoritoWriter()
    for field in [
      canonicalDomain.data,
      authority,
      creation,
      executable,
      timeToLive,
      nonce,
      feePayment,
      admissionIntent,
      metadata,
      attachments,
    ] {
      canonicalPayload.writeField(field)
    }
    guard canonicalPayload.data == bytes else {
      throw ToriiClientError.invalidPayload(
        "\(context) transaction payload does not use the canonical fixed-V1 field layout."
      )
    }
    return Payload(
      networkId: networkId,
      authority: authority,
      creationTimeMs: creationTimeMs,
      executable: executable,
      timeToLiveMs: try decodeNonzeroOption(
        timeToLive,
        as: UInt64.self,
        field: "\(context).time_to_live_ms"
      ),
      nonce: try decodeNonzeroOption(
        nonce,
        as: UInt32.self,
        field: "\(context).nonce"
      ),
      feePayment: feePayment,
      metadataWire: metadata,
      metadata: metadataValue
    )
  }

  private static func decodeEd25519Signature(
    _ bytes: Data,
    context: String
  ) throws -> Data {
    var wrapper = ToriiVerifyingKeyCompactReader(bytes)
    let encoded = try wrapper.takeField("\(context).payload")
    guard wrapper.isFinished else {
      throw ToriiClientError.invalidPayload("\(context) contains trailing fields.")
    }
    let signature = try decodeConstVec(
      encoded,
      exactCount: Ed25519SignatureAdmission.signatureLength,
      context: context
    )
    var canonicalWrapper = CompactNoritoWriter()
    canonicalWrapper.writeField(CompactNorito.encodeConstVec(signature))
    guard canonicalWrapper.data == bytes,
      Ed25519SignatureAdmission.isValidSignature(signature)
    else {
      throw ToriiClientError.invalidPayload(
        "\(context) is not one canonical Ed25519 signature."
      )
    }
    return signature
  }

  static func decodeEd25519Authority(
    _ bytes: Data,
    context: String
  ) throws -> Data {
    var authority = ToriiVerifyingKeyCompactReader(bytes)
    guard try authority.takeUInt32("\(context).kind") == 0 else {
      throw ToriiClientError.invalidPayload("\(context) must be one single-key authority.")
    }
    let encodedController = try authority.takeField("\(context).controller")
    guard authority.isFinished else {
      throw ToriiClientError.invalidPayload("\(context) contains trailing fields.")
    }
    let controller = try decodeConstVec(
      encodedController,
      exactCount: 1 + Ed25519PublicKeyAdmission.publicKeyLength,
      context: "\(context).controller"
    )
    guard controller.first == SigningAlgorithm.ed25519.noritoDiscriminant else {
      throw ToriiClientError.invalidPayload("\(context) must use Ed25519.")
    }
    let publicKey = Data(controller.dropFirst())
    guard Ed25519PublicKeyAdmission.isValidPublicKey(publicKey) else {
      throw ToriiClientError.invalidPayload("\(context) carries an invalid Ed25519 key.")
    }
    return publicKey
  }

  private static func decodeConstVec(
    _ bytes: Data,
    exactCount: Int,
    context: String
  ) throws -> Data {
    var value = ToriiVerifyingKeyCompactReader(bytes)
    let count = try value.takeUInt64("\(context).count")
    guard count == UInt64(exactCount) else {
      throw ToriiClientError.invalidPayload(
        "\(context) must contain exactly \(exactCount) bytes."
      )
    }
    var decoded = Data()
    decoded.reserveCapacity(exactCount)
    for index in 0..<exactCount {
      let byte = try value.takeField("\(context)[\(index)]")
      guard byte.count == 1, let element = byte.first else {
        throw ToriiClientError.invalidPayload(
          "\(context) byte fields must each contain exactly one byte."
        )
      }
      decoded.append(element)
    }
    guard value.isFinished, CompactNorito.encodeConstVec(decoded) == bytes else {
      throw ToriiClientError.invalidPayload("\(context) is not canonical.")
    }
    return decoded
  }

  static func compactMetadata(
    _ metadata: [String: ToriiJSONValue]
  ) throws -> Data {
    var writer = CompactNoritoWriter()
    let keys = metadata.keys.sorted {
      Data($0.utf8).lexicographicallyPrecedes(Data($1.utf8))
    }
    writer.writeUInt64LE(UInt64(keys.count))
    for key in keys {
      guard let value = metadata[key] else { continue }
      var entry = CompactNoritoWriter()
      entry.writeField(CompactNorito.encodeString(key))
      var json = CompactNoritoWriter()
      json.writeField(
        CompactNorito.encodeString(try CanonicalNorito.jsonString(from: value))
      )
      entry.writeField(json.data)
      writer.writeField(entry.data)
    }
    return writer.data
  }

  private static func decodeNetworkDomain(_ bytes: Data, context: String) throws -> NetworkId {
    var domain = ToriiVerifyingKeyCompactReader(bytes)
    guard try domain.takeUInt32("\(context).domain.kind") == 0 else {
      throw ToriiClientError.invalidPayload(
        "\(context) transaction domain must use TransactionDomain::Network."
      )
    }
    let networkIdBytes = try domain.takeField("\(context).domain.value")
    guard domain.isFinished else {
      throw ToriiClientError.invalidPayload(
        "\(context) transaction domain must contain exactly one NetworkId."
      )
    }
    do {
      return try NetworkId(bytes: networkIdBytes)
    } catch {
      throw ToriiClientError.invalidPayload(
        "\(context) transaction domain contains an invalid canonical NetworkId."
      )
    }
  }

  static func decodeString(_ bytes: Data, field: String) throws -> String {
    var value = ToriiVerifyingKeyCompactReader(bytes)
    let length = try value.takeLength("\(field).length")
    guard length <= UInt64(Int.max) else {
      throw ToriiClientError.invalidPayload("\(field) exceeds the runtime bound.")
    }
    let utf8 = try value.takeBytes(Int(length), field: field)
    guard value.isFinished,
      let decoded = String(data: utf8, encoding: .utf8),
      Data(decoded.utf8) == utf8
    else {
      throw ToriiClientError.invalidPayload("\(field) is not one canonical UTF-8 string.")
    }
    return decoded
  }

  private static func decodeNonzeroOption<T: FixedWidthInteger>(
    _ bytes: Data,
    as: T.Type,
    field: String
  ) throws -> T? {
    var option = ToriiVerifyingKeyCompactReader(bytes)
    switch try option.takeUInt8("\(field).tag") {
    case 0:
      guard option.isFinished else {
        throw ToriiClientError.invalidPayload("\(field) None contains trailing bytes.")
      }
      return nil
    case 1:
      let value = try option.takeField("\(field).value")
      guard option.isFinished, value.count == MemoryLayout<T>.size else {
        throw ToriiClientError.invalidPayload("\(field) has a noncanonical integer width.")
      }
      let decoded = value.withUnsafeBytes { T(littleEndian: $0.loadUnaligned(as: T.self)) }
      guard decoded > 0 else {
        throw ToriiClientError.invalidPayload("\(field) must be nonzero when present.")
      }
      return decoded
    default:
      throw ToriiClientError.invalidPayload("\(field) has an invalid option tag.")
    }
  }

  private static func decodeMetadata(
    _ bytes: Data,
    context: String
  ) throws -> [String: ToriiJSONValue] {
    var metadata = ToriiVerifyingKeyCompactReader(bytes)
    let count = try metadata.takeUInt64("\(context).metadata.count")
    guard count <= UInt64(maximumMetadataEntries) else {
      throw ToriiClientError.invalidPayload("\(context) metadata exceeds the entry bound.")
    }
    var result: [String: ToriiJSONValue] = [:]
    var previousKeyBytes: Data?
    for index in 0..<count {
      var entry = ToriiVerifyingKeyCompactReader(
        try metadata.takeField("\(context).metadata[\(index)]")
      )
      let key = try decodeString(
        try entry.takeField("\(context).metadata[\(index)].key"),
        field: "\(context).metadata[\(index)].key"
      )
      let keyBytes = Data(key.utf8)
      guard !key.isEmpty,
        keyBytes.count <= 256,
        key.precomposedStringWithCanonicalMapping == key,
        !key.unicodeScalars.contains(where: CharacterSet.whitespacesAndNewlines.contains),
        !key.contains("@"), !key.contains("#"), !key.contains("$"),
        previousKeyBytes.map({ $0.lexicographicallyPrecedes(keyBytes) }) ?? true
      else {
        throw ToriiClientError.invalidPayload("\(context) metadata keys are not canonical.")
      }
      previousKeyBytes = keyBytes
      var json = ToriiVerifyingKeyCompactReader(
        try entry.takeField("\(context).metadata[\(index)].json")
      )
      let jsonText = try decodeString(
        try json.takeField("\(context).metadata[\(index)].json.value"),
        field: "\(context).metadata[\(index)].json.value"
      )
      guard json.isFinished,
        entry.isFinished,
        jsonText.utf8.count <= maximumMetadataJSONBytes,
        let jsonData = jsonText.data(using: .utf8),
        let value = try? JSONDecoder().decode(ToriiJSONValue.self, from: jsonData),
        (try? CanonicalNorito.jsonString(from: value)) == jsonText
      else {
        throw ToriiClientError.invalidPayload("\(context) metadata JSON is not canonical.")
      }
      result[key] = value
    }
    guard metadata.isFinished else {
      throw ToriiClientError.invalidPayload("\(context) metadata contains trailing bytes.")
    }
    return result
  }
}
