import CryptoKit
import Foundation

// KAGEMUSHA wallet V1 wire consumer (specs/kagemusha_single_design_proposal.md §8).
//
// The canonical objects, transcripts and vectors are owned by the Rust module
// `iroha_data_model::kagemusha::kagemusha_wallet_v1`; this file consumes
// `fixtures/kagemusha/wallet_v1_vectors.json` and mirrors only the parts an SDK needs before it
// hands bytes to the typed decoder: domain digests, the raw low-S P-256 signature rule, the
// envelope frame header and per-kind bounds, and the strict `kgm1:` text form.
//
// TODO(G4): typed Swift decoding and `validate()` of the wallet message bodies (or the shared
// Rust core over the native bridge) when the Swift wallet wire migrates; structural envelope
// validation here carries no monetary or delivery authority on its own.

/// Failure of a KAGEMUSHA wallet V1 wire helper.
///
/// Field labels mirror the Rust `KagemushaWalletValidationErrorV1` labels (for example
/// `text.prefix`, `envelope.version`, `envelope.scheme_id`).
public enum KagemushaWalletWireErrorV1: Error, Equatable, Sendable {
  /// A complete text or canonical frame exceeds its bound.
  case encodedSizeExceeded(actual: Int, maximum: Int)
  /// A version field differs from ``KagemushaWalletWireV1/version``.
  case unsupportedVersion(field: String, version: UInt16)
  /// The envelope names a scheme other than the expected one.
  case schemeMismatch(field: String)
  /// A text, frame header or structural field is malformed.
  case invalidField(String)
}

/// Exact role label of one domain-separated KAGEMUSHA wallet V1 digest `H(role, body)`.
///
/// The cases and their order mirror the Rust `KagemushaWalletDigestRoleV1::ALL`. A `*-body`
/// role names the signed transcript of an object; the matching role without the suffix names
/// that signed object's digest `H(role, e || signature)`.
public enum KagemushaWalletDigestRoleV1: String, CaseIterable, Sendable {
  /// Scheme identity.
  case scheme = "scheme"
  /// Frozen relation identity.
  case relation = "relation"
  /// Advance contract and receipt format identity.
  case providerContract = "provider-contract"
  /// Asset incarnation and scale.
  case assetScope = "asset-scope"
  /// Canonical domainless `AccountId` frame.
  case account = "account"
  /// Issuer enrollment challenge.
  case enrollmentChallenge = "enrollment-challenge"
  /// Enrollment incarnation identity.
  case enrollmentID = "enrollment-id"
  /// App Attest enrollment assertion client data.
  case enrollmentKeyBinding = "enrollment-key-binding"
  /// Wallet incarnation identity.
  case walletID = "wallet-id"
  /// Scheme-root-signed signer certificate transcript.
  case certificateBody = "certificate-body"
  /// Signer certificate digest.
  case certificate = "certificate"
  /// Count-prefixed ordered certificate digests.
  case certificateSet = "certificate-set"
  /// Issuer-signed credential transcript.
  case credentialBody = "credential-body"
  /// Credential digest.
  case credential = "credential"
  /// Signed scheme policy transcript.
  case schemePolicyBody = "scheme-policy-body"
  /// Scheme policy digest.
  case schemePolicy = "scheme-policy"
  /// Signed fee schedule transcript.
  case feeScheduleBody = "fee-schedule-body"
  /// Fee schedule digest.
  case feeSchedule = "fee-schedule"
  /// Signed blacklist transcript.
  case blacklistBody = "blacklist-body"
  /// Blacklist digest.
  case blacklist = "blacklist"
  /// Blacklist gap leaf.
  case blacklistLeaf = "blacklist-leaf"
  /// Blacklist tree node.
  case blacklistNode = "blacklist-node"
  /// Signed quota share transcript.
  case quotaShareBody = "quota-share-body"
  /// Quota share digest.
  case quotaShare = "quota-share"
  /// Quota window leaf.
  case quotaWindow = "quota-window"
  /// Quota window tree node.
  case quotaNode = "quota-node"
  /// Signed time anchor transcript.
  case timeAnchorBody = "time-anchor-body"
  /// Time anchor digest.
  case timeAnchor = "time-anchor"
  /// Payer-signed Offer transcript.
  case offerBody = "offer-body"
  /// Session control transcript.
  case sessionControlBody = "session-control-body"
  /// Receiver-signed Request transcript.
  case requestBody = "request-body"
  /// Request digest.
  case request = "request"
  /// Credit identity over the Request body transcript.
  case credit = "credit"
  /// Positional Send verification dependencies.
  case dependencies = "dependencies"
  /// Transition statement.
  case statement = "statement"
  /// Transition or CreditStatus proof bytes.
  case proof = "proof"
  /// Provider commit receipt transcript.
  case receiptBody = "receipt-body"
  /// Provider commit receipt digest.
  case receipt = "receipt"
  /// Complete state package.
  case `package` = "package"
  /// Complete canonical Payment.
  case payment = "payment"
  /// Read-only CreditStatus statement.
  case creditStatusStatement = "credit-status-statement"
  /// Delivery evidence.
  case credited = "credited"
  /// Provider operation identity.
  case operationID = "operation-id"
  /// Receipt-free output descriptor.
  case output = "output"
  /// Local recovery capsule frame.
  case capsule = "capsule"
  /// Local provider marker frame.
  case marker = "marker"
  /// Local completion record frame.
  case completion = "completion"
  /// Signed load voucher transcript.
  case voucherBody = "voucher-body"
  /// Load voucher digest.
  case voucher = "voucher"
  /// Unload claim nullifier.
  case unloadNullifier = "unload-nullifier"
  /// Wallet-key ledger control transcript.
  case ledgerControlBody = "ledger-control-body"
  /// Payment-key possession transcript.
  case renewalChallenge = "renewal-challenge"
  /// Payment-key binding of a newly attested key.
  case renewalKeyBinding = "renewal-key-binding"
  /// App Attest renewal assertion client data.
  case renewalAssertion = "renewal-assertion"
  /// Signed artifact manifest transcript.
  case artifactManifestBody = "artifact-manifest-body"
  /// Artifact manifest digest.
  case artifactManifest = "artifact-manifest"
  /// Signed load/unload charge quote transcript.
  case chargeQuoteBody = "charge-quote-body"
  /// Charge quote digest.
  case chargeQuote = "charge-quote"
  /// Original platform evidence bytes.
  case evidence = "evidence"
}

/// Peer message kind carried by the canonical envelope; the raw value is the Norito wire tag.
public enum KagemushaWalletMessageKindV1: UInt32, CaseIterable, Sendable {
  /// Payer session hint.
  case offer = 1
  /// Receiver setup quote.
  case request = 2
  /// Complete committed Payment.
  case payment = 3
  /// Delivery evidence.
  case credited = 4
  /// Nonmonetary session control.
  case sessionControl = 5
  /// Nonmonetary policy data.
  case policyData = 6

  /// Maximum complete canonical envelope frame of this kind, header and padding included.
  public var maximumFrameBytes: Int {
    switch self {
    case .offer, .sessionControl:
      KagemushaWalletWireV1.sessionMaximumBytes
    case .request, .payment, .credited, .policyData:
      KagemushaWalletWireV1.messageMaximumBytes
    }
  }

  /// Maximum complete `kgm1:` text of this kind.
  public var maximumTextBytes: Int {
    switch self {
    case .offer, .sessionControl:
      KagemushaWalletWireV1.sessionTextMaximumBytes
    case .request, .payment, .credited, .policyData:
      KagemushaWalletWireV1.messageTextMaximumBytes
    }
  }

  /// Field path, from the variant fields, to the message's top-level `version: u16`.
  ///
  /// Offer and Request carry it in their signed body; the other kinds carry it first.
  var versionFieldPath: [Int] {
    switch self {
    case .offer, .request: [0, 0, 0]
    case .payment, .credited, .sessionControl, .policyData: [0, 0]
    }
  }

  /// Field path, from the variant fields, to the scheme checked at decode (design C5): the body
  /// scheme of Offer and Request, the Request body scheme of Payment, the receiver credential
  /// body scheme of Credited, and the message's own scheme field otherwise.
  var schemeFieldPath: [Int] {
    switch self {
    case .offer, .request: [0, 0, 1]
    case .payment: [0, 1, 0, 1]
    case .credited: [0, 3, 0, 1]
    case .sessionControl, .policyData: [0, 1]
    }
  }
}

/// One canonical envelope frame whose header, versions, per-kind bound and scheme were checked.
///
/// Structural only: the caller must still run the typed decoder, `validate()` and signature
/// verification before acting on the message.
public struct KagemushaWalletEnvelopeFrameV1: Equatable, Sendable {
  /// Message kind selected by the envelope's wire tag.
  public let kind: KagemushaWalletMessageKindV1
  /// Scheme identity checked at decode (32 bytes).
  public let schemeID: Data
  /// Complete canonical frame bytes.
  public let canonicalBytes: Data
}

/// KAGEMUSHA wallet V1 wire constants, digests, signature rule, envelope header and `kgm1:` text.
public enum KagemushaWalletWireV1 {
  /// Version carried by every wallet V1 object and transcript.
  public static let version: UInt16 = 1
  /// Prefix of every wallet V1 digest preimage.
  public static let digestPrefix = Data("iroha:kagemusha:wallet:v1:".utf8)
  /// Text transport discriminator.
  public static let textPrefix = "kgm1:"
  /// Maximum complete canonical envelope frame for Offer and SessionControl.
  public static let sessionMaximumBytes = 2_048
  /// Maximum complete canonical envelope frame for Request, Payment, Credited and PolicyData.
  public static let messageMaximumBytes = 10_000
  /// Maximum complete `kgm1:` text of a session-bounded envelope (2_736).
  public static let sessionTextMaximumBytes = constantTextMaximumBytes(sessionMaximumBytes)
  /// Maximum complete `kgm1:` text of a message-bounded envelope (13_339).
  public static let messageTextMaximumBytes = constantTextMaximumBytes(messageMaximumBytes)
  /// Norito frame name of the single peer-message envelope; its schema hash is derived from it.
  public static let envelopeFrameName =
    "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEnvelopeV1"
  /// Archived payload alignment of the envelope (it contains `u128`), which fixes its padding.
  public static let envelopePayloadAlignment = 16
  /// Bytes of one digest, nonce or scheme identity.
  public static let digestBytes = 32
  /// Bytes of one uncompressed SEC1 P-256 public key.
  public static let publicKeyBytes = 65
  /// Bytes of one fixed-width `r || s` signature.
  public static let signatureBytes = 64
  /// P-256 group order `n`, big-endian.
  public static let groupOrder: [UInt8] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
  ]
  /// `floor(n / 2)`, big-endian; a canonical signature has `1 <= s <= halfOrder`.
  public static let halfOrder: [UInt8] = [
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
  ]

  private static let base64URLAlphabet = Array(
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_".utf8)

  // MARK: Bounds

  /// Exact unpadded base64url length of `count` raw bytes, or `nil` on overflow or a negative
  /// count.
  public static func unpaddedBase64URLLength(_ count: Int) -> Int? {
    guard count >= 0 else { return nil }
    let (groups, overflow) = (count / 3).multipliedReportingOverflow(by: 4)
    guard !overflow else { return nil }
    let tail: Int
    switch count % 3 {
    case 0: tail = 0
    case 1: tail = 2
    default: tail = 3
    }
    let (total, tailOverflow) = groups.addingReportingOverflow(tail)
    return tailOverflow ? nil : total
  }

  /// Maximum `kgm1:` text length of a frame of at most `frameBytes` bytes, or `nil` on overflow
  /// or a negative count.
  public static func textMaximumBytes(forFrameBytes frameBytes: Int) -> Int? {
    guard let body = unpaddedBase64URLLength(frameBytes) else { return nil }
    let (total, overflow) = textPrefix.utf8.count.addingReportingOverflow(body)
    return overflow ? nil : total
  }

  /// Text bound of a protocol frame constant; the constants are small, so this cannot fail.
  private static func constantTextMaximumBytes(_ frameBytes: Int) -> Int {
    guard let total = textMaximumBytes(forFrameBytes: frameBytes) else {
      preconditionFailure("KAGEMUSHA wallet V1 text bound overflow")
    }
    return total
  }

  // MARK: Digests

  /// Exact SHA-256 preimage of `H(role, body)`; the ECDSA message of a signed body.
  ///
  /// The layout is `prefix || role || 0x00 || LE64(len(body)) || body`.
  public static func preimage(role: KagemushaWalletDigestRoleV1, body: Data) -> Data {
    let label = Data(role.rawValue.utf8)
    var preimage = Data()
    preimage.reserveCapacity(digestPrefix.count + label.count + 9 + body.count)
    preimage.append(digestPrefix)
    preimage.append(label)
    preimage.append(0)
    preimage.append(contentsOf: littleEndianLength(body.count))
    preimage.append(body)
    return preimage
  }

  /// Domain-separated digest `H(role, body)` (§8).
  public static func digest(role: KagemushaWalletDigestRoleV1, body: Data) -> Data {
    var hasher = SHA256()
    hasher.update(data: digestPrefix)
    hasher.update(data: Data(role.rawValue.utf8))
    hasher.update(data: Data([0]))
    hasher.update(data: Data(littleEndianLength(body.count)))
    hasher.update(data: body)
    return Data(hasher.finalize())
  }

  /// Digest of one signed object: `H(role, e || signature)`, where `e` is its body digest.
  ///
  /// - Throws: ``KagemushaWalletWireErrorV1/invalidField(_:)`` for a body digest that is not
  ///   32 bytes or a signature that is not canonical low-S.
  public static func signedObjectDigest(
    role: KagemushaWalletDigestRoleV1,
    bodyDigest: Data,
    signature: Data
  ) throws -> Data {
    guard bodyDigest.count == digestBytes else {
      throw KagemushaWalletWireErrorV1.invalidField("body_digest")
    }
    guard isCanonicalLowSSignature(signature) else {
      throw KagemushaWalletWireErrorV1.invalidField("signature")
    }
    var body = Data(bodyDigest)
    body.append(signature)
    return digest(role: role, body: body)
  }

  // MARK: Signatures

  /// Whether `signature` is `r || s` with `1 <= r < n` and `1 <= s <= floor(n / 2)`.
  ///
  /// CryptoKit accepts a high-S signature, so every received signature must pass this check
  /// before ``verifySignature(publicKey:preimage:signature:)`` hands it to CryptoKit.
  public static func isCanonicalLowSSignature(_ signature: Data) -> Bool {
    guard signature.count == signatureBytes else { return false }
    let bytes = [UInt8](signature)
    let r = Array(bytes[0..<32])
    let s = Array(bytes[32..<64])
    return !isZero(r) && compareBigEndian(r, groupOrder) < 0
      && !isZero(s) && compareBigEndian(s, halfOrder) <= 0
  }

  /// Whether `publicKey` is a 65-byte uncompressed SEC1 point on P-256.
  public static func isValidPublicKey(_ publicKey: Data) -> Bool {
    publicKey.count == publicKeyBytes && publicKey.first == 0x04
      && (try? P256.Signing.PublicKey(x963Representation: publicKey)) != nil
  }

  /// Verify a received low-S ECDSA-P256-SHA256 signature over the exact `preimage`.
  ///
  /// The raw low-S check runs first; CryptoKit then checks the ECDSA equation with
  /// `e = SHA-256(preimage)`. Received bytes are never normalized.
  public static func verifySignature(publicKey: Data, preimage: Data, signature: Data) -> Bool {
    guard
      isCanonicalLowSSignature(signature),
      isValidPublicKey(publicKey),
      let key = try? P256.Signing.PublicKey(x963Representation: publicKey),
      let parsed = try? P256.Signing.ECDSASignature(rawRepresentation: signature)
    else {
      return false
    }
    return key.isValidSignature(parsed, for: preimage)
  }

  /// Verify a received low-S signature over the preimage of `H(role, body)` under `publicKey`.
  public static func verifySignature(
    publicKey: Data,
    role: KagemushaWalletDigestRoleV1,
    body: Data,
    signature: Data
  ) -> Bool {
    verifySignature(
      publicKey: publicKey,
      preimage: preimage(role: role, body: body),
      signature: signature)
  }

  // MARK: Text

  /// `kgm1:` text of one canonical frame: the prefix and unpadded base64url.
  public static func encodeText(_ frame: Data) -> String {
    let bytes = [UInt8](frame)
    var out = Array(textPrefix.utf8)
    out.reserveCapacity(out.count + (unpaddedBase64URLLength(bytes.count) ?? 0))
    var index = 0
    while bytes.count - index >= 3 {
      let value =
        UInt32(bytes[index]) << 16 | UInt32(bytes[index + 1]) << 8 | UInt32(bytes[index + 2])
      appendSextets(value, count: 4, to: &out)
      index += 3
    }
    switch bytes.count - index {
    case 1:
      appendSextets(UInt32(bytes[index]) << 16, count: 2, to: &out)
    case 2:
      appendSextets(UInt32(bytes[index]) << 16 | UInt32(bytes[index + 1]) << 8, count: 3, to: &out)
    default:
      break
    }
    return String(decoding: out, as: UTF8.self)
  }

  /// Strictly decode `kgm1:` text into canonical frame bytes (§8, design §4.7).
  ///
  /// Same rules and order as the Rust `kagemusha_wallet_text_decode_v1`: at most the largest
  /// text bound, the exact prefix, a non-empty body of only the base64url alphabet without
  /// padding or whitespace, a length that is not `1 mod 4`, and re-encode equality.
  ///
  /// - Throws: ``KagemushaWalletWireErrorV1`` for any other form.
  public static func decodeText(_ text: String) throws -> Data {
    let textBytes = Array(text.utf8)
    guard textBytes.count <= messageTextMaximumBytes else {
      throw KagemushaWalletWireErrorV1.encodedSizeExceeded(
        actual: textBytes.count, maximum: messageTextMaximumBytes)
    }
    let prefix = Array(textPrefix.utf8)
    guard textBytes.count >= prefix.count, Array(textBytes[0..<prefix.count]) == prefix else {
      throw KagemushaWalletWireErrorV1.invalidField("text.prefix")
    }
    let body = Array(textBytes[prefix.count...])
    guard !body.isEmpty else {
      throw KagemushaWalletWireErrorV1.invalidField("text.body")
    }
    let sextets = body.map(base64URLSextet)
    guard sextets.allSatisfy({ $0 != nil }) else {
      throw KagemushaWalletWireErrorV1.invalidField("text.alphabet")
    }
    guard body.count % 4 != 1 else {
      throw KagemushaWalletWireErrorV1.invalidField("text.length")
    }
    var frame = [UInt8]()
    frame.reserveCapacity(body.count * 3 / 4)
    var accumulator: UInt32 = 0
    var bits = 0
    for case let sextet? in sextets {
      accumulator = (accumulator << 6) | UInt32(sextet)
      bits += 6
      if bits >= 8 {
        bits -= 8
        frame.append(UInt8(truncatingIfNeeded: accumulator >> UInt32(bits)))
        accumulator &= (UInt32(1) << UInt32(bits)) - 1
      }
    }
    let decoded = Data(frame)
    guard encodeText(decoded) == text else {
      throw KagemushaWalletWireErrorV1.invalidField("text.base64url")
    }
    return decoded
  }

  // MARK: Envelope

  /// Validate one canonical envelope frame for `expectedSchemeID` (design §0 order).
  ///
  /// Rejects, in order: a frame above the largest bound (before any parsing); a Norito header
  /// that is malformed, has nonzero padding or a CRC64 mismatch (`frame.header`); another
  /// schema hash than the envelope frame name's (`frame.schema`); flags other than
  /// `COMPACT_LEN` (`frame.flags`); padding other than the envelope's 16-byte payload alignment
  /// (`frame.padding`); a malformed field layout; another envelope or message version; an
  /// unknown message tag; a frame above its kind's bound; and another scheme.
  ///
  /// - Throws: ``KagemushaWalletWireErrorV1``.
  public static func validateEnvelope(
    _ frame: Data,
    expectedSchemeID: Data
  ) throws -> KagemushaWalletEnvelopeFrameV1 {
    guard frame.count <= messageMaximumBytes else {
      throw KagemushaWalletWireErrorV1.encodedSizeExceeded(
        actual: frame.count, maximum: messageMaximumBytes)
    }
    guard expectedSchemeID.count == digestBytes else {
      throw KagemushaWalletWireErrorV1.invalidField("expected_scheme_id")
    }
    let canonical = Data(frame)
    guard let decoded = noritoDecodeFrame(canonical) else {
      throw KagemushaWalletWireErrorV1.invalidField("frame.header")
    }
    guard decoded.header.schema == noritoSchemaHash(forTypeName: envelopeFrameName) else {
      throw KagemushaWalletWireErrorV1.invalidField("frame.schema")
    }
    guard decoded.header.compression == .none, decoded.header.flags == NoritoHeader.compactLen
    else {
      throw KagemushaWalletWireErrorV1.invalidField("frame.flags")
    }
    guard
      let padding = noritoHeaderPaddingLength(payloadAlignment: envelopePayloadAlignment),
      decoded.paddingLength == padding,
      decoded.header.length == UInt64(decoded.payload.count)
    else {
      throw KagemushaWalletWireErrorV1.invalidField("frame.padding")
    }

    let payload = [UInt8](decoded.payload)
    let envelopeFields = try structFields(payload, in: 0..<payload.count)
    guard envelopeFields.count == 2 else {
      throw KagemushaWalletWireErrorV1.invalidField("envelope.fields")
    }
    let envelopeVersion = try versionValue(payload, in: envelopeFields[0], field: "envelope.version")
    guard envelopeVersion == version else {
      throw KagemushaWalletWireErrorV1.unsupportedVersion(
        field: "envelope.version", version: envelopeVersion)
    }

    let message = envelopeFields[1]
    guard message.count >= 4 else {
      throw KagemushaWalletWireErrorV1.invalidField("message.tag")
    }
    let tag =
      UInt32(payload[message.lowerBound])
      | UInt32(payload[message.lowerBound + 1]) << 8
      | UInt32(payload[message.lowerBound + 2]) << 16
      | UInt32(payload[message.lowerBound + 3]) << 24
    guard let kind = KagemushaWalletMessageKindV1(rawValue: tag) else {
      throw KagemushaWalletWireErrorV1.invalidField("message.tag")
    }
    let variant = (message.lowerBound + 4)..<message.upperBound
    guard try structFields(payload, in: variant).count == 1 else {
      throw KagemushaWalletWireErrorV1.invalidField("message.fields")
    }
    let messageVersion = try versionValue(
      payload,
      in: fieldPath(payload, in: variant, path: kind.versionFieldPath),
      field: "message.version")
    guard messageVersion == version else {
      throw KagemushaWalletWireErrorV1.unsupportedVersion(
        field: "message.version", version: messageVersion)
    }

    guard canonical.count <= kind.maximumFrameBytes else {
      throw KagemushaWalletWireErrorV1.encodedSizeExceeded(
        actual: canonical.count, maximum: kind.maximumFrameBytes)
    }

    let schemeRange = try fieldPath(payload, in: variant, path: kind.schemeFieldPath)
    guard schemeRange.count == digestBytes else {
      throw KagemushaWalletWireErrorV1.invalidField("envelope.scheme_id")
    }
    let schemeID = Data(payload[schemeRange])
    guard schemeID == expectedSchemeID else {
      throw KagemushaWalletWireErrorV1.schemeMismatch(field: "envelope.scheme_id")
    }
    return KagemushaWalletEnvelopeFrameV1(kind: kind, schemeID: schemeID, canonicalBytes: canonical)
  }

  /// Strictly decode `kgm1:` text and validate the envelope frame it carries.
  ///
  /// - Throws: what ``decodeText(_:)`` and ``validateEnvelope(_:expectedSchemeID:)`` throw.
  public static func decodeEnvelopeText(
    _ text: String,
    expectedSchemeID: Data
  ) throws -> KagemushaWalletEnvelopeFrameV1 {
    try validateEnvelope(decodeText(text), expectedSchemeID: expectedSchemeID)
  }

  /// Validate a canonical envelope frame and encode its `kgm1:` text.
  ///
  /// - Throws: what ``validateEnvelope(_:expectedSchemeID:)`` throws.
  public static func encodeEnvelopeText(_ frame: Data, expectedSchemeID: Data) throws -> String {
    encodeText(try validateEnvelope(frame, expectedSchemeID: expectedSchemeID).canonicalBytes)
  }

  // MARK: Private helpers

  private static func littleEndianLength(_ count: Int) -> [UInt8] {
    // A Swift collection count is never negative, so the conversion is exact.
    withUnsafeBytes(of: UInt64(count).littleEndian, Array.init)
  }

  private static func isZero(_ bytes: [UInt8]) -> Bool {
    bytes.allSatisfy { $0 == 0 }
  }

  private static func compareBigEndian(_ lhs: [UInt8], _ rhs: [UInt8]) -> Int {
    for (left, right) in zip(lhs, rhs) where left != right {
      return left < right ? -1 : 1
    }
    return 0
  }

  private static func appendSextets(_ value: UInt32, count: Int, to out: inout [UInt8]) {
    for position in 0..<count {
      let shift = UInt32(18 - 6 * position)
      out.append(base64URLAlphabet[Int((value >> shift) & 0x3f)])
    }
  }

  private static func base64URLSextet(_ byte: UInt8) -> UInt8? {
    switch byte {
    case 0x41...0x5a: return byte - 0x41
    case 0x61...0x7a: return byte - 0x61 + 26
    case 0x30...0x39: return byte - 0x30 + 52
    case 0x2d: return 62
    case 0x5f: return 63
    default: return nil
    }
  }

  /// Read one canonical Norito compact length (unsigned LEB128, shortest form) that must fit
  /// in the bytes remaining before `end`.
  private static func readCompactLength(
    _ bytes: [UInt8],
    at offset: inout Int,
    end: Int
  ) throws -> Int {
    var value: UInt64 = 0
    var shift: UInt64 = 0
    var groups = 0
    while true {
      guard offset < end else {
        throw KagemushaWalletWireErrorV1.invalidField("frame.length")
      }
      let byte = bytes[offset]
      offset += 1
      groups += 1
      let group = UInt64(byte & 0x7f)
      guard shift < 64, shift < 63 || group <= 1 else {
        throw KagemushaWalletWireErrorV1.invalidField("frame.varint")
      }
      value |= group << shift
      if byte & 0x80 == 0 {
        guard groups == 1 || byte != 0 else {
          throw KagemushaWalletWireErrorV1.invalidField("frame.varint")
        }
        break
      }
      shift += 7
    }
    guard value <= UInt64(end - offset) else {
      throw KagemushaWalletWireErrorV1.invalidField("frame.length")
    }
    return Int(value)
  }

  /// Every `[len][payload]` field of the struct encoded in exactly `range`.
  private static func structFields(_ bytes: [UInt8], in range: Range<Int>) throws -> [Range<Int>] {
    var fields: [Range<Int>] = []
    var offset = range.lowerBound
    while offset < range.upperBound {
      let length = try readCompactLength(bytes, at: &offset, end: range.upperBound)
      fields.append(offset..<(offset + length))
      offset += length
    }
    return fields
  }

  /// Field `index` of the struct encoded in `range`; earlier fields are skipped by length.
  private static func structField(
    _ bytes: [UInt8],
    in range: Range<Int>,
    index: Int
  ) throws -> Range<Int> {
    var offset = range.lowerBound
    for position in 0...index {
      let length = try readCompactLength(bytes, at: &offset, end: range.upperBound)
      if position == index {
        return offset..<(offset + length)
      }
      offset += length
    }
    throw KagemushaWalletWireErrorV1.invalidField("frame.length")
  }

  private static func fieldPath(
    _ bytes: [UInt8],
    in range: Range<Int>,
    path: [Int]
  ) throws -> Range<Int> {
    try path.reduce(range) { current, index in
      try structField(bytes, in: current, index: index)
    }
  }

  private static func versionValue(
    _ bytes: [UInt8],
    in range: Range<Int>,
    field: String
  ) throws -> UInt16 {
    guard range.count == 2 else {
      throw KagemushaWalletWireErrorV1.invalidField(field)
    }
    return UInt16(bytes[range.lowerBound]) | UInt16(bytes[range.lowerBound + 1]) << 8
  }
}
