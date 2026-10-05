import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Consumes `fixtures/kagemusha/wallet_v1_vectors.json`, written by the Rust
/// `kagemusha_wallet_v1` vector test, and checks the Swift wallet wire helpers against it.
final class KagemushaWalletVectorsV1Tests: XCTestCase {
  private typealias Wire = KagemushaWalletWireV1

  /// Signed objects whose digest is `H(role, e || signature)` over the matching `-body` role.
  private static let signedObjectRoles: [KagemushaWalletDigestRoleV1] = [
    .certificate, .credential, .schemePolicy, .feeSchedule, .blacklist, .quotaShare,
    .timeAnchor, .request, .receipt, .voucher, .artifactManifest, .chargeQuote,
  ]

  // MARK: Constants and bounds

  func testConstantsMatchFixtureBounds() throws {
    let fixture = try loadFixture()
    let bounds = try object(fixture, "bounds")
    XCTAssertEqual(try int(fixture, "fixture_version"), 1)
    XCTAssertEqual(try int(bounds, "version"), Int(Wire.version))
    XCTAssertEqual(try int(bounds, "session_max_bytes"), Wire.sessionMaximumBytes)
    XCTAssertEqual(try int(bounds, "message_max_bytes"), Wire.messageMaximumBytes)
    XCTAssertEqual(try int(bounds, "session_text_max_bytes"), Wire.sessionTextMaximumBytes)
    XCTAssertEqual(try int(bounds, "message_text_max_bytes"), Wire.messageTextMaximumBytes)
    XCTAssertEqual(try string(bounds, "text_prefix"), Wire.textPrefix)
    XCTAssertEqual(Wire.sessionMaximumBytes, 2_048)
    XCTAssertEqual(Wire.messageMaximumBytes, 10_000)
    XCTAssertEqual(Wire.sessionTextMaximumBytes, 2_736)
    XCTAssertEqual(Wire.messageTextMaximumBytes, 13_339)
    XCTAssertEqual(
      try int(bounds, "lineage_max_bytes"), KagemushaWalletMessageKindV1.lineage.maximumFrameBytes)
    // σ and Ω carry no pinned byte caps until the artifact set fixes their sizes (owner
    // question Q6); the frames that carry them are the only bound.
    XCTAssertNil(bounds["proof_max_bytes"])
    XCTAssertNil(bounds["credit_status_proof_max_bytes"])
    XCTAssertFalse(try string(bounds, "proof_caps").isEmpty)
    let frameCaps = try Dictionary(
      uniqueKeysWithValues: objects(fixture, "frames").map {
        (try string($0, "type"), try int($0, "max_bytes"))
      })
    XCTAssertEqual(
      try int(bounds, "credential_max_bytes"), frameCaps["KagemushaWalletCredentialV1"])
    XCTAssertEqual(
      try int(bounds, "fold_record_max_bytes"), frameCaps["KagemushaWalletFoldRecordV1"])

    XCTAssertEqual(try string(fixture, "domain_prefix"), String(decoding: Wire.digestPrefix, as: UTF8.self))
    XCTAssertEqual(try string(fixture, "domain_prefix_hex"), hex(Wire.digestPrefix))

    let header = try object(fixture, "norito_header")
    XCTAssertEqual(try int(header, "header_bytes"), NoritoHeader.encodedLength)
    XCTAssertEqual(try hexData(string(header, "magic_hex")), NoritoHeader.magic)
    XCTAssertEqual(try int(header, "major"), Int(NoritoHeader.versionMajor))
    XCTAssertEqual(try int(header, "minor"), Int(NoritoHeader.versionMinor))
    XCTAssertEqual(try int(header, "compression"), Int(NoritoCompression.none.rawValue))
    XCTAssertEqual(noritoHeaderPaddingLength(payloadAlignment: Wire.envelopePayloadAlignment), 8)

    let boundaries = try object(fixture, "signature_boundaries")
    XCTAssertEqual(try bytes(boundaries, "order_hex"), Wire.groupOrder)
    XCTAssertEqual(try bytes(boundaries, "half_order_hex"), Wire.halfOrder)
  }

  func testBase64URLLengthAndTextBoundsAreExact() {
    XCTAssertEqual(Wire.unpaddedBase64URLLength(0), 0)
    XCTAssertEqual(Wire.unpaddedBase64URLLength(1), 2)
    XCTAssertEqual(Wire.unpaddedBase64URLLength(2), 3)
    XCTAssertEqual(Wire.unpaddedBase64URLLength(3), 4)
    XCTAssertEqual(Wire.unpaddedBase64URLLength(2_048), 2_731)
    XCTAssertEqual(Wire.unpaddedBase64URLLength(10_000), 13_334)
    XCTAssertNil(Wire.unpaddedBase64URLLength(-1))
    XCTAssertNil(Wire.unpaddedBase64URLLength(Int.max))
    XCTAssertEqual(Wire.textMaximumBytes(forFrameBytes: 2_048), 2_736)
    XCTAssertEqual(Wire.textMaximumBytes(forFrameBytes: 10_000), 13_339)
    XCTAssertEqual(Wire.textMaximumBytes(forFrameBytes: 0), 5)
    XCTAssertNil(Wire.textMaximumBytes(forFrameBytes: -1))
    XCTAssertNil(Wire.textMaximumBytes(forFrameBytes: Int.max))
    for length in 0..<64 {
      XCTAssertEqual(
        Wire.unpaddedBase64URLLength(length),
        Wire.encodeText(Data(repeating: 0xa5, count: length)).utf8.count - Wire.textPrefix.utf8.count)
    }
  }

  func testMessageKindsMatchRustTagsAndBounds() throws {
    let fixture = try loadFixture()
    let tags = try objects(object(fixture, "enum_tags"), "KagemushaWalletMessageV1")
    XCTAssertEqual(tags.count, KagemushaWalletMessageKindV1.allCases.count)
    for row in tags {
      let kind = try XCTUnwrap(KagemushaWalletMessageKindV1(rawValue: UInt32(int(row, "tag"))))
      XCTAssertEqual(kindName(kind), try string(row, "variant"))
    }
    for kind in KagemushaWalletMessageKindV1.allCases {
      switch kind {
      case .offer, .sessionControl:
        XCTAssertEqual(kind.maximumFrameBytes, 2_048)
        XCTAssertEqual(kind.maximumTextBytes, 2_736)
      case .request, .payment, .credited, .policyData, .lineage:
        XCTAssertEqual(kind.maximumFrameBytes, 10_000)
        XCTAssertEqual(kind.maximumTextBytes, 13_339)
      }
    }
    XCTAssertEqual(KagemushaWalletMessageKindV1.allCases.map(\.rawValue), [1, 2, 3, 4, 5, 6, 7])
  }

  func testFrameSchemaHashesFollowFrameNames() throws {
    let frames = try objects(loadFixture(), "frames")
    XCTAssertFalse(frames.isEmpty)
    var sawEnvelope = false
    for frame in frames {
      let name = try string(frame, "frame_name")
      XCTAssertEqual(
        noritoSchemaHash(forTypeName: name), try bytes(frame, "frame_hash_hex"), name)
      XCTAssertEqual(
        name, "iroha_data_model::kagemusha::kagemusha_wallet_v1::" + (try string(frame, "type")))
      if name == Wire.envelopeFrameName {
        sawEnvelope = true
        XCTAssertEqual(try int(frame, "max_bytes"), Wire.messageMaximumBytes)
      }
    }
    XCTAssertTrue(sawEnvelope)
  }

  // MARK: Digests

  func testDigestVectorsRecomputeEveryRole() throws {
    let vectors = try objects(loadFixture(), "digests")
    // Rust's current ALL table has 55 SHA-256 roles; step-relation values use Poseidon.
    XCTAssertEqual(KagemushaWalletDigestRoleV1.allCases.count, 55)
    XCTAssertEqual(vectors.count, KagemushaWalletDigestRoleV1.allCases.count)
    XCTAssertEqual(
      Set(try vectors.map { try string($0, "role") }),
      Set(KagemushaWalletDigestRoleV1.allCases.map(\.rawValue)))
    XCTAssertEqual(
      try vectors.map { try string($0, "role") },
      KagemushaWalletDigestRoleV1.allCases.map(\.rawValue))
    // Pre-split and superseded SHA-256 roles are gone; current step values use Poseidon.
    for retired in [
      "dependencies", "credit-status-statement", "credit", "proof", "step-proof", "payment",
      "blacklist-leaf", "blacklist-node", "quota-window", "quota-node",
    ] {
      XCTAssertNil(KagemushaWalletDigestRoleV1(rawValue: retired), retired)
    }
    for vector in vectors {
      let roleName = try string(vector, "role")
      let role = try XCTUnwrap(KagemushaWalletDigestRoleV1(rawValue: roleName), roleName)
      let body = try hexData(string(vector, "body_hex"))
      let preimage = try hexData(string(vector, "preimage_hex"))
      let expected = try hexData(string(vector, "digest_hex"))
      _ = try XCTUnwrap(vector["stand_in_proof"] as? Bool, roleName)
      XCTAssertEqual(Wire.preimage(role: role, body: body), preimage, roleName)
      XCTAssertEqual(Wire.digest(role: role, body: body), expected, roleName)
      XCTAssertEqual(Data(SHA256.hash(data: preimage)), expected, roleName)

      var flipped = body
      if flipped.isEmpty {
        flipped.append(0)
      } else {
        flipped[flipped.startIndex] ^= 0x01
      }
      XCTAssertNotEqual(Wire.digest(role: role, body: flipped), expected, roleName)
      let otherRole: KagemushaWalletDigestRoleV1 = role == .scheme ? .relation : .scheme
      XCTAssertNotEqual(Wire.digest(role: otherRole, body: body), expected, roleName)
    }
  }

  func testSignedObjectDigestsBindBodyDigestAndSignature() throws {
    let fixture = try loadFixture()
    let digests = try Dictionary(
      uniqueKeysWithValues: objects(fixture, "digests").map { (try string($0, "role"), $0) })
    // A role may sign more than one vector object (the Send and the Receive receipt).
    let signatures = try Dictionary(
      grouping: objects(fixture, "signatures"), by: { try string($0, "role") })
    XCTAssertEqual(signatures["receipt-body"]?.count, 2)
    for role in Self.signedObjectRoles {
      let bodyRoleName = role.rawValue + "-body"
      let bodyRole = try XCTUnwrap(KagemushaWalletDigestRoleV1(rawValue: bodyRoleName))
      let objectVector = try XCTUnwrap(digests[role.rawValue], role.rawValue)
      let bodyVector = try XCTUnwrap(digests[bodyRoleName], bodyRoleName)

      let transcript = try hexData(string(objectVector, "body_hex"))
      XCTAssertEqual(transcript.count, 96, role.rawValue)
      let bodyDigest = Data(transcript.prefix(32))
      let signature = Data(transcript.suffix(64))
      let signatureVector = try XCTUnwrap(
        signatures[bodyRoleName]?.first {
          (try? hexData(string($0, "signature_hex"))) == signature
        },
        bodyRoleName)
      let body = try hexData(string(bodyVector, "body_hex"))
      XCTAssertEqual(bodyDigest, Wire.digest(role: bodyRole, body: body), role.rawValue)
      XCTAssertEqual(bodyDigest, try hexData(string(signatureVector, "e_hex")), role.rawValue)
      XCTAssertEqual(signature, try hexData(string(signatureVector, "signature_hex")))
      XCTAssertTrue(
        Wire.verifySignature(
          publicKey: try hexData(string(signatureVector, "public_key_hex")),
          role: bodyRole, body: body, signature: signature),
        role.rawValue)
      XCTAssertEqual(
        try Wire.signedObjectDigest(role: role, bodyDigest: bodyDigest, signature: signature),
        try hexData(string(objectVector, "digest_hex")),
        role.rawValue)

      let twin = try hexData(string(object(signatureVector, "high_s_twin"), "signature_hex"))
      assertWireError(
        try Wire.signedObjectDigest(role: role, bodyDigest: bodyDigest, signature: twin),
        .invalidField("signature"))
      assertWireError(
        try Wire.signedObjectDigest(
          role: role, bodyDigest: bodyDigest.prefix(31), signature: signature),
        .invalidField("body_digest"))
    }
  }

  // MARK: Signatures

  func testFixedKeysDeriveFromScalars() throws {
    let keys = try objects(loadFixture(), "keys")
    XCTAssertFalse(keys.isEmpty)
    for key in keys {
      let name = try string(key, "name")
      let scalar = try hexData(string(key, "scalar_hex"))
      let publicKey = try hexData(string(key, "public_key_hex"))
      let derived = try P256.Signing.PrivateKey(rawRepresentation: scalar).publicKey
      XCTAssertEqual(derived.x963Representation, publicKey, name)
      XCTAssertTrue(Wire.isValidPublicKey(publicKey), name)

      let sec1 = [UInt8](publicKey)
      let compressed = Data([0x02 | (sec1[64] & 1)] + sec1[1..<33])
      XCTAssertFalse(Wire.isValidPublicKey(compressed), name)
      XCTAssertFalse(Wire.isValidPublicKey(publicKey.prefix(64)), name)
      var wrongPrefix = publicKey
      wrongPrefix[wrongPrefix.startIndex] = 0x05
      XCTAssertFalse(Wire.isValidPublicKey(wrongPrefix), name)
      var offCurve = publicKey
      offCurve[offCurve.index(before: offCurve.endIndex)] ^= 0x01
      XCTAssertFalse(Wire.isValidPublicKey(offCurve), name)
    }
  }

  func testSignatureVectorsRejectHighSTwinBeforeCryptoKit() throws {
    let fixture = try loadFixture()
    XCTAssertEqual(
      try string(fixture, "signature_rule"),
      "ECDSA-P256-SHA256 over preimage_hex; RFC 6979 from the fixed scalars, frozen to low S; "
        + "a consumer accepts iff codec_ok and verify_ok")
    let keys = try objects(fixture, "keys").map { try hexData(string($0, "public_key_hex")) }
    let vectors = try objects(fixture, "signatures")
    XCTAssertEqual(vectors.count, 18)
    for vector in vectors {
      let label = try string(vector, "object")
      let role = try XCTUnwrap(KagemushaWalletDigestRoleV1(rawValue: string(vector, "role")))
      let publicKey = try hexData(string(vector, "public_key_hex"))
      let preimage = try hexData(string(vector, "preimage_hex"))
      let signature = try hexData(string(vector, "signature_hex"))
      let codecOK = try bool(vector, "codec_ok")
      let verifyOK = try bool(vector, "verify_ok")
      XCTAssertTrue(codecOK && verifyOK, label)

      // The preimage is exactly prefix || role || 0x00 || LE64(len) || body.
      let body = try bodyOfPreimage(preimage, role: role)
      XCTAssertEqual(Wire.preimage(role: role, body: body), preimage, label)
      let e = try hexData(string(vector, "e_hex"))
      XCTAssertEqual(Data(SHA256.hash(data: preimage)), e, label)
      XCTAssertEqual(Wire.digest(role: role, body: body), e, label)

      XCTAssertTrue(Wire.isValidPublicKey(publicKey), label)
      XCTAssertEqual(Wire.isCanonicalLowSSignature(signature), codecOK, label)
      XCTAssertEqual(cryptoKitEquation(publicKey, preimage, signature), verifyOK, label)
      XCTAssertTrue(
        Wire.verifySignature(publicKey: publicKey, preimage: preimage, signature: signature), label)
      XCTAssertTrue(
        Wire.verifySignature(publicKey: publicKey, role: role, body: body, signature: signature),
        label)

      // CryptoKit accepts the high-S twin; the raw low-S check rejects it first.
      let twinVector = try object(vector, "high_s_twin")
      let twin = try hexData(string(twinVector, "signature_hex"))
      XCTAssertFalse(try bool(twinVector, "codec_ok"), label)
      XCTAssertTrue(try bool(twinVector, "verify_ok"), label)
      XCTAssertFalse(Wire.isCanonicalLowSSignature(twin), label)
      XCTAssertTrue(cryptoKitEquation(publicKey, preimage, twin), label)
      XCTAssertFalse(
        Wire.verifySignature(publicKey: publicKey, preimage: preimage, signature: twin), label)
      XCTAssertFalse(
        Wire.verifySignature(publicKey: publicKey, role: role, body: body, signature: twin), label)
      XCTAssertEqual(
        try P256.Signing.ECDSASignature(
          derRepresentation: hexData(string(twinVector, "der_hex"))
        ).rawRepresentation,
        twin, label)
      XCTAssertEqual(try hexData(string(twinVector, "frozen_signature_hex")), signature, label)
      XCTAssertEqual(twin.prefix(32), signature.prefix(32), label)
      XCTAssertEqual(
        addBigEndian(Array(signature.suffix(32)), Array(twin.suffix(32))), Wire.groupOrder, label)

      // The signature binds the exact preimage and key.
      var tampered = preimage
      tampered[tampered.index(before: tampered.endIndex)] ^= 0x01
      XCTAssertFalse(
        Wire.verifySignature(publicKey: publicKey, preimage: tampered, signature: signature),
        label)
      for other in keys where other != publicKey {
        XCTAssertFalse(
          Wire.verifySignature(publicKey: other, preimage: preimage, signature: signature), label)
      }
      XCTAssertFalse(
        Wire.verifySignature(
          publicKey: publicKey, preimage: preimage, signature: signature.prefix(63)),
        label)
    }
  }

  func testSignatureBoundaryCasesSeparateCodecFromEquation() throws {
    let boundary = try object(loadFixture(), "signature_boundaries")
    let role = try XCTUnwrap(KagemushaWalletDigestRoleV1(rawValue: string(boundary, "role")))
    let body = try hexData(string(boundary, "body_hex"))
    let preimage = try hexData(string(boundary, "preimage_hex"))
    let publicKey = try hexData(string(boundary, "public_key_hex"))
    XCTAssertEqual(Wire.preimage(role: role, body: body), preimage)
    XCTAssertEqual(Data(SHA256.hash(data: preimage)), try hexData(string(boundary, "e_hex")))
    XCTAssertEqual(
      try P256.Signing.PrivateKey(rawRepresentation: hexData(string(boundary, "d_hex")))
        .publicKey.x963Representation,
      publicKey)
    let half = try bytes(boundary, "half_order_hex")
    let halfPlusOne = try bytes(boundary, "half_order_plus_one_hex")
    XCTAssertEqual(addBigEndian(half, halfPlusOne), Wire.groupOrder)

    let expected: [String: (Bool, Bool)] = [
      "s_half_order": (true, true),
      "s_half_order_plus_one_high_s_twin": (false, true),
      "r_zero": (false, false),
      "s_zero": (false, false),
      "r_order": (false, false),
      "s_order": (false, false),
    ]
    let cases = try objects(boundary, "cases")
    XCTAssertEqual(Set(try cases.map { try string($0, "name") }), Set(expected.keys))
    for row in cases {
      let name = try string(row, "name")
      let signature = try hexData(string(row, "signature_hex"))
      let codecOK = try bool(row, "codec_ok")
      let verifyOK = try bool(row, "verify_ok")
      let pinned = try XCTUnwrap(expected[name])
      XCTAssertEqual(codecOK, pinned.0, name)
      XCTAssertEqual(verifyOK, pinned.1, name)
      XCTAssertEqual(Wire.isCanonicalLowSSignature(signature), codecOK, name)
      XCTAssertEqual(cryptoKitEquation(publicKey, preimage, signature), verifyOK, name)
      XCTAssertEqual(
        Wire.verifySignature(publicKey: publicKey, preimage: preimage, signature: signature),
        codecOK && verifyOK, name)
      XCTAssertEqual(
        Wire.verifySignature(publicKey: publicKey, role: role, body: body, signature: signature),
        codecOK && verifyOK, name)
      if name == "s_half_order" {
        XCTAssertEqual(Array(signature.prefix(32)), try bytes(boundary, "r_hex"))
        XCTAssertEqual(Array(signature.suffix(32)), half)
      }
    }
  }

  // MARK: Envelopes

  func testEnvelopeVectorsValidateHeaderBoundAndScheme() throws {
    let vectors = try objects(loadFixture(), "envelopes")
    var kinds = Set<KagemushaWalletMessageKindV1>()
    for vector in vectors {
      let label = try string(vector, "variant")
      let frame = try hexData(string(vector, "canonical_hex"))
      let scheme = try hexData(string(vector, "scheme_id_hex"))
      let kind = try XCTUnwrap(
        KagemushaWalletMessageKindV1(rawValue: UInt32(int(vector, "tag"))), label)
      kinds.insert(kind)
      _ = try XCTUnwrap(vector["stand_in_proof"] as? Bool, label)
      XCTAssertEqual(kindName(kind), try string(vector, "kind"), label)
      XCTAssertEqual(kind.maximumFrameBytes, try int(vector, "bound"), label)
      XCTAssertEqual(kind.maximumTextBytes, try int(vector, "text_bound"), label)
      XCTAssertEqual(frame.count, try int(vector, "frame_len"), label)
      XCTAssertLessThanOrEqual(frame.count, kind.maximumFrameBytes, label)

      // Header: schema hash from the frame name, flags, padding, CRC64 and length.
      let frameName = try string(vector, "frame_name")
      XCTAssertEqual(frameName, Wire.envelopeFrameName, label)
      let schemaHash = try bytes(vector, "schema_hash_hex")
      XCTAssertEqual(noritoSchemaHash(forTypeName: frameName), schemaHash, label)
      let decoded = try XCTUnwrap(noritoDecodeFrame(frame), label)
      XCTAssertEqual(decoded.header.schema, schemaHash, label)
      XCTAssertEqual(Int(decoded.header.flags), try int(vector, "flags"), label)
      XCTAssertEqual(decoded.paddingLength, try int(vector, "padding_len"), label)
      XCTAssertEqual(decoded.payload.count, try int(vector, "payload_len"), label)
      XCTAssertEqual(decoded.header.length, UInt64(decoded.payload.count), label)
      let crc = try string(vector, "crc64_hex")
      XCTAssertEqual(String(format: "%016llx", crc64ECMA(decoded.payload)), crc, label)
      XCTAssertEqual(String(format: "%016llx", decoded.header.checksum), crc, label)

      let validated = try Wire.validateEnvelope(frame, expectedSchemeID: scheme)
      XCTAssertEqual(validated.kind, kind, label)
      XCTAssertEqual(validated.schemeID, scheme, label)
      XCTAssertEqual(validated.canonicalBytes, frame, label)

      var otherScheme = scheme
      otherScheme[otherScheme.startIndex] ^= 0x01
      assertWireError(
        try Wire.validateEnvelope(frame, expectedSchemeID: otherScheme),
        .schemeMismatch(field: "envelope.scheme_id"))
      // The scheme-agnostic inspection a carrier applies returns the same frame and scheme.
      XCTAssertEqual(try Wire.inspectEnvelope(frame), validated, label)
      assertWireError(
        try Wire.validateEnvelope(frame, expectedSchemeID: scheme.prefix(31)),
        .invalidField("expected_scheme_id"))

      // Strict `kgm1:` round trip.
      let text = try string(vector, "text")
      XCTAssertLessThanOrEqual(text.utf8.count, kind.maximumTextBytes, label)
      XCTAssertEqual(Wire.encodeText(frame), text, label)
      XCTAssertEqual(try Wire.decodeText(text), frame, label)
      XCTAssertEqual(try Wire.decodeEnvelopeText(text, expectedSchemeID: scheme), validated, label)
      XCTAssertEqual(try Wire.encodeEnvelopeText(frame, expectedSchemeID: scheme), text, label)
      assertWireError(
        try Wire.decodeEnvelopeText(text, expectedSchemeID: otherScheme),
        .schemeMismatch(field: "envelope.scheme_id"))
    }
    XCTAssertEqual(kinds, Set(KagemushaWalletMessageKindV1.allCases))
  }

  func testEnvelopeEveryByteFlipIsRejected() throws {
    for vector in try objects(loadFixture(), "envelopes") {
      let label = try string(vector, "variant")
      let frame = try hexData(string(vector, "canonical_hex"))
      let scheme = try hexData(string(vector, "scheme_id_hex"))
      var accepted: [Int] = []
      for index in 0..<frame.count {
        var flipped = frame
        flipped[index] ^= 0x01
        if (try? Wire.validateEnvelope(flipped, expectedSchemeID: scheme)) != nil {
          accepted.append(index)
        }
      }
      XCTAssertEqual(accepted, [], label)
      XCTAssertThrowsError(try Wire.validateEnvelope(frame.dropLast(), expectedSchemeID: scheme))
      var extended = frame
      extended.append(0)
      XCTAssertThrowsError(try Wire.validateEnvelope(extended, expectedSchemeID: scheme))
    }
  }

  func testEnvelopeBoundsAcceptLimitAndRejectLimitPlusOne() throws {
    let scheme = [UInt8](repeating: 0x5c, count: 32)
    for kind in KagemushaWalletMessageKindV1.allCases {
      let label = kindName(kind)
      let bound = kind.maximumFrameBytes
      let atLimit = try sizedEnvelope(kind: kind, scheme: scheme, frameBytes: bound)
      let validated = try Wire.validateEnvelope(atLimit, expectedSchemeID: Data(scheme))
      XCTAssertEqual(validated.kind, kind, label)
      let atLimitText = Wire.encodeText(atLimit)
      XCTAssertEqual(atLimitText.utf8.count, kind.maximumTextBytes, label)
      XCTAssertEqual(
        try Wire.decodeEnvelopeText(atLimitText, expectedSchemeID: Data(scheme)), validated, label)

      let overLimit = try sizedEnvelope(kind: kind, scheme: scheme, frameBytes: bound + 1)
      assertWireError(
        try Wire.validateEnvelope(overLimit, expectedSchemeID: Data(scheme)),
        .encodedSizeExceeded(actual: bound + 1, maximum: bound))
      assertWireError(
        try Wire.inspectEnvelope(overLimit),
        .encodedSizeExceeded(actual: bound + 1, maximum: bound))
      let overLimitText = Wire.encodeText(overLimit)
      XCTAssertEqual(overLimitText.utf8.count, kind.maximumTextBytes + 1, label)
      XCTAssertThrowsError(
        try Wire.decodeEnvelopeText(overLimitText, expectedSchemeID: Data(scheme)), label)
      assertWireError(
        try Wire.encodeEnvelopeText(overLimit, expectedSchemeID: Data(scheme)),
        .encodedSizeExceeded(actual: bound + 1, maximum: bound))

      // Decode order: versions before the per-kind bound, the bound before the scheme.
      var wrongVersion = SyntheticEnvelope(kind: kind, scheme: scheme)
      wrongVersion.messageVersion = 2
      let wrongVersionFrame = try sized(wrongVersion, frameBytes: bound + 1)
      if bound < Wire.messageMaximumBytes {
        assertWireError(
          try Wire.validateEnvelope(wrongVersionFrame, expectedSchemeID: Data(scheme)),
          .unsupportedVersion(field: "message.version", version: 2))
      }
      var otherScheme = scheme
      otherScheme[0] ^= 0x01
      assertWireError(
        try Wire.validateEnvelope(overLimit, expectedSchemeID: Data(otherScheme)),
        .encodedSizeExceeded(actual: bound + 1, maximum: bound))
    }
    // The byte cap applies before any header parsing.
    assertWireError(
      try Wire.validateEnvelope(Data(repeating: 0, count: 10_001), expectedSchemeID: Data(scheme)),
      .encodedSizeExceeded(actual: 10_001, maximum: 10_000))
  }

  func testEnvelopeStructuralRejections() throws {
    let scheme = [UInt8](repeating: 0x3a, count: 32)
    let expected = Data(scheme)
    for kind in KagemushaWalletMessageKindV1.allCases {
      let label = kindName(kind)
      let base = SyntheticEnvelope(kind: kind, scheme: scheme, fillerBytes: 17)
      XCTAssertEqual(
        try Wire.validateEnvelope(base.frame(), expectedSchemeID: expected).kind, kind, label)

      var envelopeVersion = base
      envelopeVersion.envelopeVersion = 2
      assertWireError(
        try Wire.validateEnvelope(envelopeVersion.frame(), expectedSchemeID: expected),
        .unsupportedVersion(field: "envelope.version", version: 2))

      var messageVersion = base
      messageVersion.messageVersion = 0
      assertWireError(
        try Wire.validateEnvelope(messageVersion.frame(), expectedSchemeID: expected),
        .unsupportedVersion(field: "message.version", version: 0))

      var shortScheme = base
      shortScheme.scheme = Array(scheme.prefix(31))
      assertWireError(
        try Wire.validateEnvelope(shortScheme.frame(), expectedSchemeID: expected),
        .invalidField("envelope.scheme_id"))

      var trailing = base
      trailing.trailingEnvelopeField = true
      assertWireError(
        try Wire.validateEnvelope(trailing.frame(), expectedSchemeID: expected),
        .invalidField("envelope.fields"))

      var schema = base
      schema.typeName = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletMessageV1"
      assertWireError(
        try Wire.validateEnvelope(schema.frame(), expectedSchemeID: expected),
        .invalidField("frame.schema"))

      var fixedWidth = base
      fixedWidth.flags = 0
      assertWireError(
        try Wire.validateEnvelope(fixedWidth.frame(), expectedSchemeID: expected),
        .invalidField("frame.flags"))

      for alignment in [1, 8, 32] {
        var padding = base
        padding.payloadAlignment = alignment
        assertWireError(
          try Wire.validateEnvelope(padding.frame(), expectedSchemeID: expected),
          .invalidField("frame.padding"))
      }

      var corrupted = try base.frame()
      corrupted[corrupted.index(before: corrupted.endIndex)] ^= 0x01
      assertWireError(
        try Wire.validateEnvelope(corrupted, expectedSchemeID: expected),
        .invalidField("frame.header"))
    }

    for tag: UInt32 in [0, 8, 0x0100_0001] {
      var unknown = SyntheticEnvelope(kind: .offer, scheme: scheme)
      unknown.tag = tag
      assertWireError(
        try Wire.validateEnvelope(unknown.frame(), expectedSchemeID: expected),
        .invalidField("message.tag"))
    }

    let version = compactField([1, 0])
    let offer = compactField(compactField(compactField([1, 0]) + compactField(scheme)))
    let cases: [(String, [UInt8])] = [
      ("message.tag", version + compactField([1, 0, 0])),
      ("message.fields", version + compactField(le32(1) + offer + compactField([0]))),
      ("envelope.version", compactField([1, 0, 0]) + compactField(le32(1) + offer)),
      ("envelope.fields", version),
      ("frame.length", [0x05, 1, 0]),
      ("frame.length", version + [0x80]),
      ("frame.varint", [0x82, 0x00, 1, 0] + compactField(le32(1) + offer)),
      ("frame.varint", [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]),
      ("frame.length", version + compactField(le32(1) + compactField([0x05, 1]))),
    ]
    for (field, payload) in cases {
      let frame = noritoEncode(
        typeName: Wire.envelopeFrameName, payload: Data(payload),
        flags: NoritoHeader.compactLen, payloadAlignment: Wire.envelopePayloadAlignment)
      assertWireError(
        try Wire.validateEnvelope(frame, expectedSchemeID: expected), .invalidField(field))
    }
  }

  func testEnvelopeVectorsCarryTheSplitLineageLayouts() throws {
    let fixture = try loadFixture()
    let vectors = try Dictionary(
      uniqueKeysWithValues: objects(fixture, "envelopes").map { (try string($0, "variant"), $0) })
    let offer = try vectorMessage(XCTUnwrap(vectors["Offer"]))
    let request = try vectorMessage(XCTUnwrap(vectors["Request"]))
    XCTAssertEqual(offer.fields.count, 4)
    XCTAssertEqual(request.fields.count, 5)

    // Compact Payment {version, request: {body, signature}, payer_payment_key,
    // payer_credential_digest, send}: the signed Request is exactly the Request message's body
    // and signature, and the Send package's lineage slot is Present (Ω(pred)).
    let payment = try vectorMessage(XCTUnwrap(vectors["Payment"]))
    XCTAssertEqual(payment.fields.count, 5)
    let signedRequest = try XCTUnwrap(compactFields(payment.payload, payment.fields[1]))
    XCTAssertEqual(signedRequest.count, 2)
    XCTAssertEqual(payment.bytes(signedRequest[0]), request.bytes(request.fields[0]))
    XCTAssertEqual(payment.bytes(signedRequest[1]), request.bytes(request.fields[4]))
    XCTAssertTrue(Wire.isValidPublicKey(payment.bytes(payment.fields[2])))
    XCTAssertEqual(payment.fields[3].count, Wire.digestBytes)
    let send = try XCTUnwrap(compactFields(payment.payload, payment.fields[4]))
    XCTAssertEqual(send.count, 5)
    XCTAssertEqual(payment.tag(send[2]), 1)
    let paymentObject = try XCTUnwrap(
      objects(fixture, "objects").first { $0["type"] as? String == "KagemushaWalletPaymentV1" })
    let paymentFrame = try XCTUnwrap(
      noritoDecodeFrame(hexData(string(paymentObject, "canonical_hex"))))
    XCTAssertEqual(payment.bytes(payment.message), paymentFrame.payload)

    // Credited {version, scheme_id, evidence}: Receive 1 {package} with an empty lineage slot,
    // or Status 2 {CreditStatus}; the scheme the envelope is checked against is its own field.
    for (variant, tag) in [("Credited::Receive", UInt32(1)), ("Credited::Status", 2)] {
      let vector = try XCTUnwrap(vectors[variant], variant)
      let credited = try vectorMessage(vector)
      XCTAssertEqual(credited.fields.count, 3, variant)
      XCTAssertEqual(
        credited.bytes(credited.fields[1]), try hexData(string(vector, "scheme_id_hex")), variant)
      XCTAssertEqual(credited.tag(credited.fields[2]), tag, variant)
    }
    let receive = try vectorMessage(XCTUnwrap(vectors["Credited::Receive"]))
    let evidence = receive.fields[2]
    let evidenceFields = try XCTUnwrap(
      compactFields(receive.payload, (evidence.lowerBound + 4)..<evidence.upperBound))
    XCTAssertEqual(evidenceFields.count, 1)
    let receivePackage = try XCTUnwrap(compactFields(receive.payload, evidenceFields[0]))
    XCTAssertEqual(receivePackage.count, 5)
    XCTAssertEqual(receive.tag(receivePackage[2]), 0)

    // Lineage {version, lineage: {public, proof}}: Ω's 13 public outputs name the Offer's
    // payer wallet and a valid payment key.
    let lineage = try vectorMessage(XCTUnwrap(vectors["Lineage"]))
    XCTAssertEqual(lineage.fields.count, 2)
    let omega = try XCTUnwrap(compactFields(lineage.payload, lineage.fields[1]))
    XCTAssertEqual(omega.count, 2)
    let omegaPublic = try XCTUnwrap(compactFields(lineage.payload, omega[0]))
    XCTAssertEqual(omegaPublic.count, 13)
    let offerBody = try XCTUnwrap(compactFields(offer.payload, offer.fields[0]))
    XCTAssertEqual(lineage.bytes(omegaPublic[4]), offer.bytes(offerBody[3]))
    XCTAssertTrue(Wire.isValidPublicKey(lineage.bytes(omegaPublic[6])))
  }

  // MARK: Object frames

  func testObjectFramesDecodeAndReencodeByteIdentically() throws {
    let fixture = try loadFixture()
    let caps = try Dictionary(
      uniqueKeysWithValues: objects(fixture, "frames").map {
        (try string($0, "type"), try int($0, "max_bytes"))
      })
    let vectors = try objects(fixture, "objects")
    var types = Set<String>()
    for vector in vectors {
      let type = try string(vector, "type")
      let label = type + " " + (try string(vector, "variant"))
      types.insert(type)
      _ = try XCTUnwrap(vector["stand_in_proof"] as? Bool, label)
      let name = try string(vector, "frame_name")
      XCTAssertEqual(name, "iroha_data_model::kagemusha::kagemusha_wallet_v1::" + type, label)
      let frame = try hexData(string(vector, "canonical_hex"))
      XCTAssertEqual(frame.count, try int(vector, "frame_len"), label)
      if let cap = caps[type] {
        XCTAssertLessThanOrEqual(frame.count, cap, label)
      }
      let decoded = try XCTUnwrap(noritoDecodeFrame(frame), label)
      XCTAssertEqual(decoded.header.schema, noritoSchemaHash(forTypeName: name), label)
      XCTAssertEqual(decoded.header.compression, .none, label)
      XCTAssertEqual(decoded.header.flags, NoritoHeader.compactLen, label)
      XCTAssertEqual(decoded.header.length, UInt64(decoded.payload.count), label)
      let payload = [UInt8](decoded.payload)
      let fields = try XCTUnwrap(compactFields(payload, 0..<payload.count), label)
      XCTAssertFalse(fields.isEmpty, label)
      // The padding is that of the archived payload alignment (16 when the payload contains a
      // u128, none otherwise), so the frame re-encodes byte for byte.
      let alignment = try XCTUnwrap(
        [1, 16].first { noritoHeaderPaddingLength(payloadAlignment: $0) == decoded.paddingLength },
        label)
      XCTAssertEqual(
        noritoEncode(
          typeName: name, payload: decoded.payload, flags: decoded.header.flags,
          payloadAlignment: alignment),
        frame, label)
      var flipped = frame
      flipped[flipped.index(before: flipped.endIndex)] ^= 0x01
      XCTAssertNil(noritoDecodeFrame(flipped), label)
    }
    XCTAssertTrue(
      types.isSuperset(of: [
        "KagemushaWalletPaymentV1", "KagemushaWalletPackageV1", "KagemushaWalletMarkerV1",
        "KagemushaWalletRecoveryCapsuleV1", "KagemushaWalletCompletionRecordV1",
        "KagemushaWalletFoldRecordV1",
      ]))
  }

  // MARK: Field encodings

  /// Pasta `Fp` modulus `p`, little-endian: the σ field of the step relations.
  private static let fieldModulus: [UInt8] = [
    0x01, 0x00, 0x00, 0x00, 0xed, 0x30, 0x2d, 0x99, 0x1b, 0xf9, 0x4c, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
  ]

  func testFieldEncodingsFollowTheElementRule() throws {
    let fixture = try loadFixture()
    let encodings = try object(fixture, "field_encodings")
    XCTAssertEqual(try bytes(encodings, "modulus_le_hex"), Self.fieldModulus)
    XCTAssertFalse(isCanonicalFieldElement(Data(Self.fieldModulus), modulus: Self.fieldModulus))
    var belowModulus = Self.fieldModulus
    belowModulus[0] -= 1
    XCTAssertTrue(isCanonicalFieldElement(Data(belowModulus), modulus: Self.fieldModulus))

    let domains = try objects(encodings, "domains")
    // The current Rust Poseidon table, in declaration order (poseidon.rs).
    let expectedDomains = [
      ("core", "kgwcore1"), ("rest", "kgwrest1"), ("statement", "kgwstmt1"),
      ("credit_id", "kgwcrdt1"), ("send_chain", "kgwschn1"), ("recv_chain", "kgwrchn1"),
      ("consumed_credit_leaf", "kgwccrd1"), ("pending_outgoing_leaf", "kgwpout1"),
      ("load_recovery_leaf", "kgwload1"), ("redeem_recovery_leaf", "kgwrdm_1"),
      ("fee_claim_leaf", "kgwfee_1"), ("quota_usage_leaf", "kgwquse1"),
      ("credit_digest_leaf", "kgwcdig1"), ("sparse_empty_leaf", "kgwsmte1"),
      ("sparse_node", "kgwsmtn1"), ("blacklist_leaf", "kgwblkl1"),
      ("blacklist_node", "kgwblkn1"), ("quota_window_leaf", "kgwqwin1"),
      ("quota_node", "kgwqwnd1"), ("proof_digest", "kgwprf_1"),
      ("step_proof_digest", "kgwstep1"), ("payment_digest", "kgwpay_1"),
    ]
    XCTAssertEqual(domains.count, expectedDomains.count)
    XCTAssertEqual(try domains.map { try string($0, "use") }, expectedDomains.map { $0.0 })
    XCTAssertEqual(try domains.map { try string($0, "ascii") }, expectedDomains.map { $0.1 })
    XCTAssertEqual(Set(try domains.map { try string($0, "use") }).count, domains.count)
    XCTAssertEqual(Set(try domains.map { try string($0, "ascii") }).count, domains.count)
    for domain in domains {
      let ascii = try string(domain, "ascii")
      XCTAssertEqual(ascii.utf8.count, 8, ascii)
      XCTAssertTrue(ascii.hasPrefix("kgw"), ascii)
      XCTAssertEqual(try hexData(string(domain, "u64_le_hex")), Data(ascii.utf8), ascii)
    }

    let sendStatement = try object(encodings, "send_statement")
    let receiveStatement = try object(encodings, "receive_statement")
    let state = try object(encodings, "receive_successor_state")
    let send = try fieldElements(sendStatement, "items")
    let receive = try fieldElements(receiveStatement, "items")
    let core = try fieldElements(state, "core_items")
    let rest = try fieldElements(state, "rest_items")
    let sendChain = try fieldElements(encodings, "send_chain_append_from_empty")
    let recvChain = try fieldElements(encodings, "recv_chain_append")
    let consumed = try fieldElements(encodings, "consumed_credit_leaf")
    let pending = try fieldElements(encodings, "pending_outgoing_leaf")
    let feeClaim = try fieldElements(encodings, "fee_claim_leaf")
    let creditDigest = try fieldElements(encodings, "credit_digest_leaf")
    let lists: [(String, [Data], Int)] = [
      ("send_statement", send, 28), ("receive_statement", receive, 28), ("core", core, 32),
      ("rest", rest, 13), ("send_chain", sendChain, 9), ("recv_chain", recvChain, 5),
      ("consumed_credit_leaf", consumed, 3), ("pending_outgoing_leaf", pending, 8),
      ("fee_claim_leaf", feeClaim, 4), ("credit_digest_leaf", creditDigest, 3),
    ]
    for (name, items, count) in lists {
      XCTAssertEqual(items.count, count, name)
      for item in items {
        XCTAssertTrue(isCanonicalFieldElement(item, modulus: Self.fieldModulus), name)
      }
    }
    guard lists.allSatisfy({ $0.1.count == $0.2 }) else { return }

    // Statement items are the limb rule applied to the exact 440-byte statement transcript.
    XCTAssertEqual(
      try statementFieldElements(hexData(string(sendStatement, "statement_hex"))), send)
    XCTAssertEqual(
      try statementFieldElements(hexData(string(receiveStatement, "statement_hex"))), receive)

    // One credit throughout: the chain entries and map leaves repeat the effects' elements.
    let digests = try Dictionary(
      uniqueKeysWithValues: objects(fixture, "digests").map { (try string($0, "role"), $0) })
    func digestLimbs(_ role: String) throws -> [Data] {
      try fieldDigestLimbs(hexData(string(XCTUnwrap(digests[role], role), "digest_hex")))
    }
    // A credit identity and Payment digest each occupy one canonical Poseidon field.
    let poseidon = try object(fixture, "poseidon")
    let creditID = try object(object(poseidon, "credit_id"), "poseidon")
    let credit = [try hexData(string(creditID, "digest_hex"))]
    let paymentDigest = try hexData(string(object(poseidon, "payment_digest"), "digest_hex"))
    let sendDescriptor = Array(send[18..<26])
    XCTAssertEqual(Array(send[18..<19]), credit)
    XCTAssertEqual(Array(send[24..<26]), try digestLimbs("request"))
    XCTAssertEqual(Array(receive[18..<19]), credit)
    XCTAssertEqual(receive[21], send[22])
    XCTAssertEqual(sendChain, [Data(count: 32)] + sendDescriptor)
    XCTAssertEqual(Array(recvChain.dropFirst()), Array(receive[18..<22]))
    XCTAssertEqual(pending, sendDescriptor)
    XCTAssertEqual(consumed, credit + [receive[21], receive[10]])
    XCTAssertEqual(Array(feeClaim.prefix(2)), credit + [send[23]])
    XCTAssertEqual(Array(feeClaim.suffix(2)), try digestLimbs("fee-schedule"))
    XCTAssertEqual(Array(creditDigest.prefix(1)), credit)
    XCTAssertEqual(creditDigest[1], paymentDigest)
    XCTAssertEqual(creditDigest[2], Data(count: 32))

    // Scheme and asset moved into the core; every step opens them, while rest holds policies.
    XCTAssertEqual(core[0], receive[9])
    XCTAssertEqual(Array(core[5..<7]), Array(send[19..<21]))
    XCTAssertEqual(Array(core[7..<9]), Array(receive[7..<9]))
    XCTAssertEqual(core[11], receive[10])
    XCTAssertEqual(core[13], receive[11])
    XCTAssertEqual(Array(core[1..<3]), Array(receive[3..<5]))
    XCTAssertEqual(Array(core[3..<5]), Array(receive[5..<7]))
    XCTAssertEqual(core[22], receive[12])
    XCTAssertEqual(core[10], receive[13])
    XCTAssertEqual(try hexData(string(state, "commitment_hex")), receive[16])
    let stateFrame = try XCTUnwrap(noritoDecodeFrame(hexData(string(state, "state_hex"))))
    XCTAssertEqual(
      stateFrame.header.schema,
      noritoSchemaHash(
        forTypeName: "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletStateV1"))
    XCTAssertEqual(stateFrame.header.flags, NoritoHeader.compactLen)
    let decodedState = try stateFieldElements(stateFrame.payload)
    XCTAssertEqual(decodedState.core, core)
    XCTAssertEqual(decodedState.rest, rest)
  }

  // MARK: Text

  func testTextCodecIsStrict() throws {
    XCTAssertEqual(Wire.encodeText(Data()), "kgm1:")
    XCTAssertEqual(Wire.encodeText(Data([0xfb, 0xff])), "kgm1:-_8")
    XCTAssertEqual(try Wire.decodeText("kgm1:-_8"), Data([0xfb, 0xff]))
    XCTAssertEqual(try Wire.decodeText("kgm1:AA"), Data([0]))
    XCTAssertEqual(try Wire.decodeText("kgm1:AAA"), Data([0, 0]))
    XCTAssertEqual(try Wire.decodeText("kgm1:AAAA"), Data([0, 0, 0]))
    // An empty frame encodes to the bare prefix, which decoding rejects (`text.body`) like Rust.
    for length in 1..<70 {
      let frame = Data((0..<length).map { UInt8(truncatingIfNeeded: $0 &* 37 &+ 11) })
      XCTAssertEqual(try Wire.decodeText(Wire.encodeText(frame)), frame)
      XCTAssertEqual(
        String(Wire.encodeText(frame).dropFirst(5)),
        frame.base64EncodedString()
          .replacingOccurrences(of: "+", with: "-")
          .replacingOccurrences(of: "/", with: "_")
          .replacingOccurrences(of: "=", with: ""))
    }

    let rejected: [(String, KagemushaWalletWireErrorV1)] = [
      ("", .invalidField("text.prefix")),
      ("kgm1", .invalidField("text.prefix")),
      ("KGM1:AAAA", .invalidField("text.prefix")),
      (" kgm1:AAAA", .invalidField("text.prefix")),
      ("kgm2:AAAA", .invalidField("text.prefix")),
      ("kgm1:", .invalidField("text.body")),
      ("kgm1:AA==", .invalidField("text.alphabet")),
      ("kgm1:AAA=", .invalidField("text.alphabet")),
      ("kgm1:AA+A", .invalidField("text.alphabet")),
      ("kgm1:AA/A", .invalidField("text.alphabet")),
      ("kgm1:AA A", .invalidField("text.alphabet")),
      ("kgm1:AAAA\n", .invalidField("text.alphabet")),
      ("kgm1:AA\u{00e9}", .invalidField("text.alphabet")),
      ("kgm1:A", .invalidField("text.length")),
      ("kgm1:AAAAA", .invalidField("text.length")),
      ("kgm1:AB", .invalidField("text.base64url")),
      ("kgm1:AAB", .invalidField("text.base64url")),
      ("kgm1:AAAAAB", .invalidField("text.base64url")),
    ]
    for (text, error) in rejected {
      assertWireError(try Wire.decodeText(text), error)
    }

    // A real vector text with one character outside the alphabet.
    let vector = try XCTUnwrap(objects(loadFixture(), "envelopes").first)
    var characters = Array(try string(vector, "text"))
    characters[characters.count / 2] = "+"
    assertWireError(try Wire.decodeText(String(characters)), .invalidField("text.alphabet"))
  }

  func testTextBoundIsCheckedBeforeAnyOtherRule() throws {
    let maximum = "kgm1:" + String(repeating: "A", count: Wire.messageTextMaximumBytes - 5)
    XCTAssertEqual(maximum.utf8.count, 13_339)
    XCTAssertEqual(try Wire.decodeText(maximum), Data(repeating: 0, count: 10_000))
    assertWireError(
      try Wire.decodeText(maximum + "A"),
      .encodedSizeExceeded(actual: 13_340, maximum: 13_339))
    assertWireError(
      try Wire.decodeText("XXXX:" + String(repeating: "=", count: 13_335)),
      .encodedSizeExceeded(actual: 13_340, maximum: 13_339))
    let scheme = [UInt8](repeating: 0x11, count: 32)
    let overCap = try sizedEnvelope(kind: .payment, scheme: scheme, frameBytes: 10_001)
    assertWireError(
      try Wire.decodeEnvelopeText(Wire.encodeText(overCap), expectedSchemeID: Data(scheme)),
      .encodedSizeExceeded(actual: 13_340, maximum: 13_339))
    let overSession = try sizedEnvelope(kind: .offer, scheme: scheme, frameBytes: 2_049)
    let overSessionText = Wire.encodeText(overSession)
    XCTAssertEqual(overSessionText.utf8.count, 2_737)
    assertWireError(
      try Wire.decodeEnvelopeText(overSessionText, expectedSchemeID: Data(scheme)),
      .encodedSizeExceeded(actual: 2_049, maximum: 2_048))
  }
}

// MARK: - Synthetic envelopes

/// Structurally valid envelope frame with the field paths the validator reads; its contents are
/// not a valid typed message.
private struct SyntheticEnvelope {
  var kind: KagemushaWalletMessageKindV1
  var scheme: [UInt8]
  var fillerBytes = 0
  var envelopeVersion: UInt16 = 1
  var messageVersion: UInt16 = 1
  var tag: UInt32?
  var trailingEnvelopeField = false
  var typeName = KagemushaWalletWireV1.envelopeFrameName
  var flags = NoritoHeader.compactLen
  var payloadAlignment = KagemushaWalletWireV1.envelopePayloadAlignment

  func payload() -> [UInt8] {
    let version = compactField(le16(messageVersion))
    let leaf =
      version + compactField(scheme)
      + compactField([UInt8](repeating: 0xa5, count: fillerBytes))
    let variant: [UInt8]
    switch kind {
    case .offer, .request:
      variant = compactField(compactField(leaf))
    case .payment, .lineage:
      variant = compactField(version + compactField(compactField(leaf)))
    case .credited, .sessionControl, .policyData:
      variant = compactField(leaf)
    }
    var payload =
      compactField(le16(envelopeVersion)) + compactField(le32(tag ?? kind.rawValue) + variant)
    if trailingEnvelopeField {
      payload += compactField([0])
    }
    return payload
  }

  func frame() throws -> Data {
    noritoEncode(
      typeName: typeName, payload: Data(payload()), flags: flags,
      payloadAlignment: payloadAlignment)
  }
}

private enum SyntheticFailure: Error {
  case unreachableSize(Int)
}

private func sized(_ template: SyntheticEnvelope, frameBytes: Int) throws -> Data {
  var envelope = template
  envelope.fillerBytes = 0
  for _ in 0..<16 {
    let frame = try envelope.frame()
    if frame.count == frameBytes { return frame }
    envelope.fillerBytes += frameBytes - frame.count
    guard envelope.fillerBytes >= 0 else { break }
  }
  throw SyntheticFailure.unreachableSize(frameBytes)
}

private func sizedEnvelope(
  kind: KagemushaWalletMessageKindV1,
  scheme: [UInt8],
  frameBytes: Int
) throws -> Data {
  try sized(SyntheticEnvelope(kind: kind, scheme: scheme), frameBytes: frameBytes)
}

/// `[compact length][content]` with a canonical unsigned LEB128 length.
private func compactField(_ content: [UInt8]) -> [UInt8] {
  var length = UInt64(content.count)
  var prefix: [UInt8] = []
  repeat {
    var byte = UInt8(length & 0x7f)
    length >>= 7
    if length != 0 { byte |= 0x80 }
    prefix.append(byte)
  } while length != 0
  return prefix + content
}

private func le16(_ value: UInt16) -> [UInt8] {
  withUnsafeBytes(of: value.littleEndian, Array.init)
}

private func le32(_ value: UInt32) -> [UInt8] {
  withUnsafeBytes(of: value.littleEndian, Array.init)
}

// MARK: - Vector layouts

/// Every `[compact length][content]` field of the struct encoded in exactly `range`, or `nil`
/// when a length is truncated or overruns the range.
private func compactFields(_ bytes: [UInt8], _ range: Range<Int>) -> [Range<Int>]? {
  var fields: [Range<Int>] = []
  var offset = range.lowerBound
  while offset < range.upperBound {
    var length = 0
    var shift = 0
    while true {
      guard offset < range.upperBound, shift < 63 else { return nil }
      let byte = bytes[offset]
      offset += 1
      length |= Int(byte & 0x7f) << shift
      shift += 7
      if byte & 0x80 == 0 { break }
    }
    guard length <= range.upperBound - offset else { return nil }
    fields.append(offset..<(offset + length))
    offset += length
  }
  return fields
}

/// The message struct of one envelope vector, with the payload its field ranges index.
private struct VectorMessage {
  let payload: [UInt8]
  /// Range of the message struct: the envelope variant's single field.
  let message: Range<Int>
  /// Field ranges of the message struct.
  let fields: [Range<Int>]

  func bytes(_ range: Range<Int>) -> Data { Data(payload[range]) }

  /// Little-endian `u32` enum tag at the start of `range`, or `nil` when it is shorter.
  func tag(_ range: Range<Int>) -> UInt32? {
    guard range.count >= 4 else { return nil }
    return (0..<4).reduce(UInt32(0)) {
      $0 | UInt32(payload[range.lowerBound + $1]) << (8 * UInt32($1))
    }
  }
}

private func vectorMessage(_ vector: [String: Any]) throws -> VectorMessage {
  let frame = try hexData(string(vector, "canonical_hex"))
  guard let decoded = noritoDecodeFrame(frame) else {
    throw WalletFixtureFailure.malformed("envelope frame")
  }
  let payload = [UInt8](decoded.payload)
  guard
    let envelope = compactFields(payload, 0..<payload.count), envelope.count == 2,
    envelope[1].count >= 4,
    let variant = compactFields(payload, (envelope[1].lowerBound + 4)..<envelope[1].upperBound),
    variant.count == 1,
    let fields = compactFields(payload, variant[0])
  else {
    throw WalletFixtureFailure.malformed("envelope fields")
  }
  return VectorMessage(payload: payload, message: variant[0], fields: fields)
}

// MARK: - σ-field elements

/// Whether `value` is a 32-byte little-endian integer below `modulus` (little-endian).
private func isCanonicalFieldElement(_ value: Data, modulus: [UInt8]) -> Bool {
  let bytes = [UInt8](value)
  guard bytes.count == 32, modulus.count == 32 else { return false }
  for index in stride(from: 31, through: 0, by: -1) where bytes[index] != modulus[index] {
    return bytes[index] < modulus[index]
  }
  return false
}

/// One integer element: the little-endian integer bytes zero-filled to 32.
private func fieldInteger(_ littleEndian: [UInt8]) -> Data {
  Data(littleEndian + [UInt8](repeating: 0, count: 32 - littleEndian.count))
}

/// The two elements of a 32-byte digest or identifier: its 128-bit limbs, low half first.
private func fieldDigestLimbs(_ digest: Data) -> [Data] {
  let bytes = [UInt8](digest)
  return [fieldInteger(Array(bytes.prefix(16))), fieldInteger(Array(bytes.dropFirst(16)))]
}

private func fieldElements(_ value: [String: Any], _ key: String) throws -> [Data] {
  guard let items = value[key] as? [String] else { throw WalletFixtureFailure.malformed(key) }
  return try items.map(hexData)
}

/// σ-field elements of one 440-byte `statement` transcript, in the Rust `field_items` order:
/// version; relation, scheme, asset and credential limbs; successor lifecycle, sequence and
/// `next_load`; enabled controls; lineage `burned_total`; lineage pending-outgoing root,
/// predecessor and successor as one element each; the effect tag; and the effect's elements
/// zero-filled to 10. Only the Send (3) and Receive (4) effects the vectors carry are mirrored.
/// SHA-256 values contribute two limbs; the Poseidon `credit_id` contributes one element.
private func statementFieldElements(_ transcript: Data) throws -> [Data] {
  let bytes = [UInt8](transcript)
  guard bytes.count == 440 else { throw WalletFixtureFailure.malformed("statement length") }
  var offset = 0
  func take(_ count: Int) -> [UInt8] {
    defer { offset += count }
    return Array(bytes[offset..<(offset + count)])
  }
  let version = take(2)
  let scheme = take(32)
  let relation = take(32)
  let credential = take(32)
  let asset = take(32)
  let lifecycle = take(1)
  let sequence = take(16)
  let nextLoad = take(16)
  let controls = take(4)
  let burned = take(16)
  let lineageRoot = take(32)
  let predecessor = take(32)
  let successor = take(32)
  let tag = take(1)
  let union = take(160)
  var items = [fieldInteger(version)]
  for digest in [relation, scheme, asset, credential] {
    items += fieldDigestLimbs(Data(digest))
  }
  items += [lifecycle, sequence, nextLoad, controls, burned].map(fieldInteger)
  items += [lineageRoot, predecessor, successor].map { Data($0) }
  items.append(fieldInteger(tag))
  let layouts: [UInt8: [FieldEncoding]] = [
    3: [.poseidon, .digest, .integer(16), .integer(16), .integer(16), .digest,
      .integer(8), .integer(8)],
    4: [.poseidon, .digest, .integer(16)],
  ]
  guard let layout = layouts[tag[0]] else { throw WalletFixtureFailure.malformed("effect tag") }
  var cursor = 0
  var effect: [Data] = []
  for encoding in layout {
    let field = Array(union[cursor..<(cursor + encoding.width)])
    cursor += encoding.width
    effect += try encodedFieldElements(field, encoding)
  }
  guard union[cursor...].allSatisfy({ $0 == 0 }), effect.count <= 10 else {
    throw WalletFixtureFailure.malformed("effect fill")
  }
  return items + effect + Array(repeating: Data(count: 32), count: 10 - effect.count)
}

/// Current Rust element kinds: integers, two-limb SHA-256 values and one-field Poseidon values.
private enum FieldEncoding {
  case integer(Int)
  case digest
  case poseidon

  var width: Int {
    switch self {
    case .integer(let width): width
    case .digest, .poseidon: 32
    }
  }
}

private func encodedFieldElements(_ field: [UInt8], _ encoding: FieldEncoding) throws -> [Data] {
  guard field.count == encoding.width else {
    throw WalletFixtureFailure.malformed("field width")
  }
  switch encoding {
  case .integer: return [fieldInteger(field)]
  case .digest: return fieldDigestLimbs(Data(field))
  case .poseidon:
    guard KagemushaWalletWireV1.isCanonicalFieldValue(Data(field)) else {
      throw WalletFixtureFailure.malformed("Poseidon field")
    }
    return [Data(field)]
  }
}

/// Re-derive every core/rest element from the current compact Norito state record.
/// Rust state.rs owns these field orders: scheme/asset and blacklist age are core fields.
private func stateFieldElements(_ payload: Data) throws -> (core: [Data], rest: [Data]) {
  let bytes = [UInt8](payload)
  guard let state = compactFields(bytes, 0..<bytes.count), state.count == 3,
    Array(bytes[state[0]]) == le16(1)
  else { throw WalletFixtureFailure.malformed("state fields") }
  let coreLayout: [FieldEncoding] = [
    .integer(4), .digest, .digest, .digest, .digest,
    .integer(16), .integer(16), .integer(16), .integer(16), .integer(16), .integer(16),
    .poseidon, .poseidon, .poseidon, .poseidon, .poseidon, .poseidon, .poseidon,
    .integer(4), .poseidon, .integer(8), .poseidon,
    .integer(8), .integer(8), .integer(8), .integer(8), .integer(8), .poseidon,
  ]
  let restLayout: [FieldEncoding] = [
    .integer(4), .integer(8), .digest, .digest, .digest, .digest, .integer(8), .digest,
  ]
  func elements(_ range: Range<Int>, _ layout: [FieldEncoding]) throws -> [Data] {
    guard let fields = compactFields(bytes, range), fields.count == layout.count else {
      throw WalletFixtureFailure.malformed("state layout")
    }
    return try zip(fields, layout).flatMap { range, encoding in
      try encodedFieldElements(Array(bytes[range]), encoding)
    }
  }
  return (try elements(state[1], coreLayout), try elements(state[2], restLayout))
}

// MARK: - Fixture and assertion helpers

private enum WalletFixtureFailure: Error {
  case missingFixture
  case malformed(String)
}

private func kindName(_ kind: KagemushaWalletMessageKindV1) -> String {
  switch kind {
  case .offer: "Offer"
  case .request: "Request"
  case .payment: "Payment"
  case .credited: "Credited"
  case .sessionControl: "SessionControl"
  case .policyData: "PolicyData"
  case .lineage: "Lineage"
  }
}

/// Locates `fixtures/kagemusha/wallet_v1_vectors.json` by walking up from this source file.
private func walletFixtureURL() throws -> URL {
  var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
  for _ in 0..<8 {
    let candidate = directory.appendingPathComponent("fixtures/kagemusha/wallet_v1_vectors.json")
    if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
    directory.deleteLastPathComponent()
  }
  throw WalletFixtureFailure.missingFixture
}

private func loadFixture() throws -> [String: Any] {
  let data = try Data(contentsOf: walletFixtureURL())
  guard let fixture = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
    throw WalletFixtureFailure.malformed("root")
  }
  return fixture
}

private func object(_ value: [String: Any], _ key: String) throws -> [String: Any] {
  guard let object = value[key] as? [String: Any] else {
    throw WalletFixtureFailure.malformed(key)
  }
  return object
}

private func objects(_ value: [String: Any], _ key: String) throws -> [[String: Any]] {
  guard let array = value[key] as? [[String: Any]] else {
    throw WalletFixtureFailure.malformed(key)
  }
  return array
}

private func string(_ value: [String: Any], _ key: String) throws -> String {
  guard let text = value[key] as? String else { throw WalletFixtureFailure.malformed(key) }
  return text
}

private func int(_ value: [String: Any], _ key: String) throws -> Int {
  guard let number = value[key] as? NSNumber else { throw WalletFixtureFailure.malformed(key) }
  return number.intValue
}

private func bool(_ value: [String: Any], _ key: String) throws -> Bool {
  guard let flag = value[key] as? Bool else { throw WalletFixtureFailure.malformed(key) }
  return flag
}

private func bytes(_ value: [String: Any], _ key: String) throws -> [UInt8] {
  [UInt8](try hexData(string(value, key)))
}

private func hexData(_ text: String) throws -> Data {
  let digits = Array(text.utf8)
  guard digits.count % 2 == 0 else { throw WalletFixtureFailure.malformed("odd hex") }
  func nibble(_ digit: UInt8) throws -> UInt8 {
    switch digit {
    case 0x30...0x39: return digit - 0x30
    case 0x61...0x66: return digit - 0x61 + 10
    default: throw WalletFixtureFailure.malformed("hex digit")
    }
  }
  var out = Data(capacity: digits.count / 2)
  var index = 0
  while index < digits.count {
    out.append(try nibble(digits[index]) << 4 | nibble(digits[index + 1]))
    index += 2
  }
  return out
}

private func hex(_ data: Data) -> String {
  data.map { String(format: "%02x", $0) }.joined()
}

/// Body of a wallet preimage `prefix || role || 0x00 || LE64(len) || body`, checking the frame.
private func bodyOfPreimage(_ preimage: Data, role: KagemushaWalletDigestRoleV1) throws -> Data {
  let bytes = [UInt8](preimage)
  let head = [UInt8](KagemushaWalletWireV1.digestPrefix) + Array(role.rawValue.utf8) + [0]
  guard bytes.count >= head.count + 8, Array(bytes[0..<head.count]) == head else {
    throw WalletFixtureFailure.malformed("preimage head")
  }
  var length: UInt64 = 0
  for (shift, byte) in bytes[head.count..<(head.count + 8)].enumerated() {
    length |= UInt64(byte) << UInt64(8 * shift)
  }
  let body = Array(bytes[(head.count + 8)...])
  guard length == UInt64(body.count) else { throw WalletFixtureFailure.malformed("preimage len") }
  return Data(body)
}

/// The ECDSA equation as CryptoKit checks it, scalars taken as they are (high S accepted).
private func cryptoKitEquation(_ publicKey: Data, _ preimage: Data, _ signature: Data) -> Bool {
  guard
    let key = try? P256.Signing.PublicKey(x963Representation: publicKey),
    let parsed = try? P256.Signing.ECDSASignature(rawRepresentation: signature)
  else {
    return false
  }
  return key.isValidSignature(parsed, for: preimage)
}

/// Big-endian sum of two equal-width integers, or `nil` on a carry out.
private func addBigEndian(_ lhs: [UInt8], _ rhs: [UInt8]) -> [UInt8]? {
  guard lhs.count == rhs.count else { return nil }
  var out = [UInt8](repeating: 0, count: lhs.count)
  var carry = 0
  for index in stride(from: lhs.count - 1, through: 0, by: -1) {
    let sum = Int(lhs[index]) + Int(rhs[index]) + carry
    out[index] = UInt8(sum & 0xff)
    carry = sum >> 8
  }
  return carry == 0 ? out : nil
}

private func assertWireError<T>(
  _ expression: @autoclosure () throws -> T,
  _ expected: KagemushaWalletWireErrorV1,
  file: StaticString = #filePath,
  line: UInt = #line
) {
  XCTAssertThrowsError(try expression(), file: file, line: line) { error in
    XCTAssertEqual(error as? KagemushaWalletWireErrorV1, expected, file: file, line: line)
  }
}
