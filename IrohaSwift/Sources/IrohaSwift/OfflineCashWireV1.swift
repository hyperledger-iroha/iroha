import Foundation

/// Failures from the bounded Offline Cash V1 text envelope.
public enum OfflineCashWireEnvelopeErrorV1: Error, Equatable {
  case emptyPayload
  case invalidPrefix
  case invalidText
  case nonCanonicalBase64URL
  case invalidField(String)
  case sizeExceeded(actual: Int, maximum: Int)
}

/// The canonical Offline Cash V1 value whose opaque Norito bytes are transported.
public enum OfflineCashWirePayloadKindV1: CaseIterable, Sendable {
  case paymentRequest
  case acceptanceIntent
  case acceptanceIntentAuthorization
  case acceptanceTicket
  case payment
  case acknowledgement
  case mintAuthorization
  case mintCredit
  case redemptionVoucher

  /// Maximum canonical Norito bytes for this value.
  public var maximumRawBytes: Int {
    switch self {
    case .paymentRequest:
      OfflineCashWireV1.maximumPaymentRequestBytes
    case .acceptanceIntent:
      OfflineCashWireV1.maximumAcceptanceIntentBytes
    case .acceptanceIntentAuthorization:
      OfflineCashWireV1.maximumAcceptanceIntentAuthorizationBytes
    case .acceptanceTicket:
      OfflineCashWireV1.maximumAcceptanceTicketBytes
    case .payment:
      OfflineCashWireV1.maximumPaymentBytes
    case .acknowledgement:
      OfflineCashWireV1.maximumAcknowledgementBytes
    case .mintAuthorization:
      OfflineCashWireV1.maximumMintAuthorizationBytes
    case .mintCredit:
      OfflineCashWireV1.maximumMintCreditBytes
    case .redemptionVoucher:
      OfflineCashWireV1.maximumRedemptionVoucherBytes
    }
  }

  /// Maximum complete `oc1:` text bytes for this value.
  public var maximumTextBytes: Int {
    switch self {
    case .paymentRequest:
      OfflineCashWireV1.maximumPaymentRequestTextBytes
    case .acceptanceIntent:
      OfflineCashWireV1.maximumAcceptanceIntentTextBytes
    case .acceptanceIntentAuthorization:
      OfflineCashWireV1.maximumAcceptanceIntentAuthorizationTextBytes
    case .acceptanceTicket:
      OfflineCashWireV1.maximumAcceptanceTicketTextBytes
    case .payment:
      OfflineCashWireV1.maximumPaymentTextBytes
    case .acknowledgement:
      OfflineCashWireV1.maximumAcknowledgementTextBytes
    case .mintAuthorization:
      OfflineCashWireV1.maximumMintAuthorizationTextBytes
    case .mintCredit:
      OfflineCashWireV1.maximumMintCreditTextBytes
    case .redemptionVoucher:
      OfflineCashWireV1.maximumRedemptionVoucherTextBytes
    }
  }
}

/// Exact size contract and opaque text envelope for Offline Cash V1.
///
/// This codec does not interpret or validate Norito. Callers must pass bytes produced by the
/// canonical typed encoder and must run the typed decoder and cryptographic verifier after text
/// decoding. No authority follows from successful text decoding alone.
public enum OfflineCashWireV1 {
  public static let wireVersion: UInt16 = 1
  public static let deviceLifecycleVersion: UInt16 = 1
  public static let handoffCapability = "cash_handoff_v1"
  public static let textPrefix = "oc1:"
  public static let maximumAssetScale: UInt32 = 28
  public static let requestMaximumTTLMS: UInt64 = 5 * 60 * 1_000

  public static let maximumAggregateStateBytes = 768
  public static let maximumPaymentRequestBytes = 1_024
  public static let maximumAcceptanceIntentBytes = 256
  public static let maximumAcceptanceIntentAuthorizationBytes = 7_936
  public static let maximumNoCommitClosureBytes = 16_384
  public static let maximumAcceptanceTicketBytes = 1_024
  public static let maximumPaymentBytes = 7_936
  public static let maximumAcknowledgementBytes = 512
  public static let maximumMintAuthorizationBytes = 7_936
  public static let maximumMintCreditBytes = 7_936
  public static let maximumRedemptionVoucherBytes = 7_936
  public static let maximumPaymentRequestTextBytes = 1_370
  public static let maximumAcceptanceIntentTextBytes = 346
  public static let maximumAcceptanceIntentAuthorizationTextBytes = 10_586
  public static let maximumAcceptanceTicketTextBytes = 1_370
  public static let maximumPaymentTextBytes = 10_586
  public static let maximumAcknowledgementTextBytes = 687
  public static let maximumMintAuthorizationTextBytes = 10_586
  public static let maximumMintCreditTextBytes = 10_586
  public static let maximumRedemptionVoucherTextBytes = 10_586
  public static let maximumSessionRawBytes = 9_211
  public static let maximumSessionTextBytes = 12_288
  public static let preTicketExchangeTargetBytes = 8_960
  public static let maximumPreTicketExchangeBytes = 9_984
  public static let maximumPreTicketTextExchangeBytes = 13_326
  public static let completeExchangeTargetBytes = 16_384
  public static let maximumCompleteExchangeBytes = 18_171
  public static let maximumCompleteTextExchangeBytes = 24_244

  public static let maximumPairedProofBytes = 6_528
  public static let maximumCurrentProofsBytes = 4_990
  public static let maximumParityProofBytes = 2_495
  public static let historyAccumulatorBytes = 544
  public static let maximumEncryptedCreditBytes = 384
  public static let maximumCreditOpeningBytes = 256
  public static let x25519PublicKeyBytes = 32
  public static let xchachaNonceBytes = 24
  public static let xchachaTagBytes = 16
  public static let encryptedCreditKDFSaltLabel = Data(
    "iroha:offline-cash:v1:credit-envelope-salt\0".utf8)
  public static let encryptedCreditKDFInfoLabel = Data(
    "iroha:offline-cash:v1:credit-envelope-key\0".utf8)
  public static let maximumHardwareProfileBytes = 512
  public static let maximumHardwareCredentialBytes = 768
  public static let maximumCommitCertificateBytes = 1_024
  public static let minimumReservedInboxBytes: UInt32 = 8_960
  public static let minimumPaymentOutboxBytes: UInt32 = 26_112
  public static let minimumRedemptionOutboxBytes: UInt32 = 26_112
  public static let requiredHardwareCapabilityMask: UInt16 = 0xffff

  /// Exact minimum recoverable outbox reservation for a terminal operation.
  public static func minimumOutboxBytes(for operation: OfflineCashOperationKindV1) -> UInt32 {
    switch operation {
    case .sendSplit: minimumPaymentOutboxBytes
    case .redeemSplit: minimumRedemptionOutboxBytes
    default: 1
    }
  }

  /// Encode bounded canonical bytes as exact unpadded base64url with the `oc1:` discriminator.
  public static func encodeText(
    _ canonicalPayload: Data,
    kind: OfflineCashWirePayloadKindV1
  ) throws -> String {
    guard !canonicalPayload.isEmpty else {
      throw OfflineCashWireEnvelopeErrorV1.emptyPayload
    }
    guard canonicalPayload.count <= kind.maximumRawBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: canonicalPayload.count,
        maximum: kind.maximumRawBytes
      )
    }
    let body = canonicalPayload.base64EncodedString()
      .replacingOccurrences(of: "+", with: "-")
      .replacingOccurrences(of: "/", with: "_")
      .replacingOccurrences(of: "=", with: "")
    let text = textPrefix + body
    guard text.utf8.count <= kind.maximumTextBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: text.utf8.count,
        maximum: kind.maximumTextBytes
      )
    }
    return text
  }

  /// Decode one strict `oc1:` envelope into opaque canonical bytes.
  public static func decodeText(
    _ text: String,
    kind: OfflineCashWirePayloadKindV1
  ) throws -> Data {
    let textBytes = text.utf8
    guard textBytes.count <= kind.maximumTextBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: textBytes.count,
        maximum: kind.maximumTextBytes
      )
    }
    guard text.hasPrefix(textPrefix) else {
      throw OfflineCashWireEnvelopeErrorV1.invalidPrefix
    }
    let body = String(text.dropFirst(textPrefix.count))
    guard !body.isEmpty else {
      throw OfflineCashWireEnvelopeErrorV1.emptyPayload
    }
    guard body.utf8.allSatisfy(isBase64URLByte) else {
      throw OfflineCashWireEnvelopeErrorV1.invalidText
    }
    guard body.utf8.count % 4 != 1 else {
      throw OfflineCashWireEnvelopeErrorV1.nonCanonicalBase64URL
    }

    var padded =
      body
      .replacingOccurrences(of: "-", with: "+")
      .replacingOccurrences(of: "_", with: "/")
    padded.append(String(repeating: "=", count: (4 - padded.utf8.count % 4) % 4))
    guard let raw = Data(base64Encoded: padded, options: []) else {
      throw OfflineCashWireEnvelopeErrorV1.nonCanonicalBase64URL
    }
    guard raw.count <= kind.maximumRawBytes else {
      throw OfflineCashWireEnvelopeErrorV1.sizeExceeded(
        actual: raw.count,
        maximum: kind.maximumRawBytes
      )
    }
    guard try encodeText(raw, kind: kind) == text else {
      throw OfflineCashWireEnvelopeErrorV1.nonCanonicalBase64URL
    }
    return raw
  }

  private static func isBase64URLByte(_ byte: UInt8) -> Bool {
    (byte >= 0x41 && byte <= 0x5a)
      || (byte >= 0x61 && byte <= 0x7a)
      || (byte >= 0x30 && byte <= 0x39)
      || byte == 0x2d
      || byte == 0x5f
  }
}
