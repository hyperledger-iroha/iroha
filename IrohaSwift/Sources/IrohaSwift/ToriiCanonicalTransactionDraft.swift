import CryptoKit
import Foundation

/// Strictly decoded canonical unsigned transaction material shared by every V1 draft route.
struct ToriiCanonicalTransactionDraft {
  static let maximumTransactionPayloadBytes = 16 * 1024 * 1024
  private static let maximumMetadataEntries = 1_024
  private static let maximumMetadataJSONBytes = 1 * 1024 * 1024

  struct Payload {
    let chainId: String
    let authority: Data
    let creationTimeMs: UInt64
    let executable: Data
    let timeToLiveMs: UInt64?
    let nonce: UInt32?
    let feePayment: Data
    let metadata: [String: ToriiJSONValue]
  }

  let transactionPayload: Data
  let signingMessage: Data
  let payload: Payload

  static func decode(
    transactionPayloadB64: String,
    signingMessageB64: String,
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
      payload: try parsePayload(transactionPayload, context: context)
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
    for byte in signature {
      signatureBytes.writeField(Data([byte]))
    }
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
    context: String
  ) throws -> Data {
    guard bytes.first == 1 else {
      throw ToriiClientError.invalidPayload(
        "\(context) must use signed-transaction wire version 1.")
    }
    var reader = CanonicalNoritoReader(data: Data(bytes.dropFirst()))
    let signature = try reader.readCompactField()
    let transactionPayload = try reader.readCompactField()
    let attachments = try reader.readCompactField()
    guard reader.remaining() == 0,
      attachments == Data([0]),
      !signature.isEmpty
    else {
      throw ToriiClientError.invalidPayload(
        "\(context) must contain exactly one signature, one payload, and no attachments."
      )
    }
    return transactionPayload
  }

  private static func parsePayload(_ bytes: Data, context: String) throws -> Payload {
    var transaction = ToriiVerifyingKeyCompactReader(bytes)
    let chain = try transaction.takeField("\(context).chain")
    let authority = try transaction.takeField("\(context).authority")
    let creation = try transaction.takeField("\(context).creation_time_ms")
    let executable = try transaction.takeField("\(context).executable")
    let timeToLive = try transaction.takeField("\(context).time_to_live_ms")
    let nonce = try transaction.takeField("\(context).nonce")
    let feePayment = try transaction.takeField("\(context).fee_payment")
    let metadata = try transaction.takeField("\(context).metadata")
    let attachments = try transaction.takeField("\(context).attachments")
    guard transaction.isFinished,
      !authority.isEmpty,
      !executable.isEmpty,
      creation.count == MemoryLayout<UInt64>.size
    else {
      throw ToriiClientError.invalidPayload(
        "\(context).transaction_payload_b64 must contain exactly one canonical nine-field TransactionPayload."
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
    guard attachments == Data([0]) else {
      throw ToriiClientError.invalidPayload(
        "\(context) attachments must use the exact None encoding.")
    }
    return Payload(
      chainId: try decodeChain(chain, context: context),
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
      metadata: try decodeMetadata(metadata, context: context)
    )
  }

  private static func decodeChain(_ bytes: Data, context: String) throws -> String {
    var archive = ToriiVerifyingKeyCompactReader(bytes)
    let value = try decodeString(
      try archive.takeField("\(context).chain.value"),
      field: "\(context).chain.value"
    )
    guard archive.isFinished,
      !value.isEmpty,
      value.utf8.count <= 128,
      let first = value.utf8.first,
      let last = value.utf8.last,
      isASCIIAlphanumeric(first),
      isASCIIAlphanumeric(last),
      value.utf8.allSatisfy({
        isASCIIAlphanumeric($0) || $0 == 46 || $0 == 95 || $0 == 58 || $0 == 45
      })
    else {
      throw ToriiClientError.invalidPayload("\(context) chain id is not canonical.")
    }
    return value
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

  private static func isASCIIAlphanumeric(_ byte: UInt8) -> Bool {
    (48...57).contains(byte) || (65...90).contains(byte) || (97...122).contains(byte)
  }
}
