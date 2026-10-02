// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaOrdinaryEnrollmentOriginalWireV1Tests: XCTestCase {
  func testFIChallengeRejectsForeignOperationZeroMessageAndNoncanonicalOriginal() throws {
    let operation = Data(repeating: 3, count: 32)
    let original = Data([1, 2, 3]), message = Data(repeating: 9, count: 32)
    var fields: [String: Any] = ["challenge_id": OrdinaryEnrollmentWire.hex(operation),
      "canonical_challenge_base64": original.base64EncodedString(),
      "account_signing_message_base64": message.base64EncodedString(), "expires_at_ms": UInt64.max]
    let accepted = try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation)
    XCTAssertEqual(accepted.challenge, original); XCTAssertEqual(accepted.message, message)
    fields["challenge_id"] = OrdinaryEnrollmentWire.hex(Data(repeating: 4, count: 32))
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation))
    fields["challenge_id"] = OrdinaryEnrollmentWire.hex(operation)
    fields["account_signing_message_base64"] = Data(repeating: 0, count: 32).base64EncodedString()
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation))
    fields["account_signing_message_base64"] = message.base64EncodedString()
    fields["canonical_challenge_base64"] = original.base64EncodedString() + "\n"
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation))
    fields["canonical_challenge_base64"] = original.base64EncodedString()
    fields["expires_at_ms"] = 0
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation))
    fields["expires_at_ms"] = true
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailChallenge(OrdinaryEnrollmentWire.json(fields), operation: operation))
  }

  func testRawOriginalDigestSizeAndFICertificateCorrelation() throws {
    let raw = Data(repeating: 7, count: 314)
    var fields: [String: Any] = ["raw_admission_base64": raw.base64EncodedString(),
      "raw_admission_sha256_hex": OrdinaryEnrollmentWire.hex(Data(SHA256.hash(data: raw)))]
    XCTAssertEqual(try OrdinaryEnrollmentWire.original(OrdinaryEnrollmentWire.json(fields),
      field: "raw_admission_base64", digestField: "raw_admission_sha256_hex", exact: 314, maximum: 314), raw)
    fields["raw_admission_sha256_hex"] = String(repeating: "0", count: 64)
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.original(OrdinaryEnrollmentWire.json(fields),
      field: "raw_admission_base64", digestField: "raw_admission_sha256_hex", exact: 314, maximum: 314))
    let short = Data(raw.dropLast())
    fields["raw_admission_base64"] = short.base64EncodedString()
    fields["raw_admission_sha256_hex"] = OrdinaryEnrollmentWire.hex(Data(SHA256.hash(data: short)))
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.original(OrdinaryEnrollmentWire.json(fields),
      field: "raw_admission_base64", digestField: "raw_admission_sha256_hex", exact: 314, maximum: 314))
    let operation = Data(repeating: 3, count: 32), certificate = Data([8, 9]), id = Data(repeating: 6, count: 32)
    var final: [String: Any] = ["challenge_id": OrdinaryEnrollmentWire.hex(operation),
      "canonical_certificate_base64": certificate.base64EncodedString(), "enrollment_id_hex": OrdinaryEnrollmentWire.hex(id)]
    let accepted = try OrdinaryEnrollmentWire.retailCertificate(OrdinaryEnrollmentWire.json(final), operation: operation)
    XCTAssertEqual(accepted.certificate, certificate); XCTAssertEqual(accepted.enrollmentID, id)
    final["challenge_id"] = OrdinaryEnrollmentWire.hex(id)
    XCTAssertThrowsError(try OrdinaryEnrollmentWire.retailCertificate(OrdinaryEnrollmentWire.json(final), operation: operation))
  }
}
