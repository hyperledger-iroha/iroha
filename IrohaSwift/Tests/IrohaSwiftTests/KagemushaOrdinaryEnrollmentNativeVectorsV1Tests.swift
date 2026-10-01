import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Actual Rust C/E bytes through called structural parsers; no native capability is constructed.
final class KagemushaOrdinaryEnrollmentNativeVectorsV1Tests: XCTestCase {
  func testBothActualNativeEnrollmentVectorsMatchExactCAndEAndPlatformAlias() throws {
    for row in try vectors() {
      let f = try fields(row), c = try KagemushaOrdinaryAppEnrollmentProjectionV1(f[7])
      let p = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
        approvalID: nil, enrollmentChallengeHash: Data(SHA256.hash(data:c.canonicalSigningBytes)))
      XCTAssertEqual(p.generationChallenge, try value(row,"attestation_challenge_hex"))
      XCTAssertEqual(Data(SHA256.hash(data: p.signingBytes)), try value(row,"possession_sha256_hex"))
      XCTAssertEqual(p.signingBytes, try value(row,"possession_signing_hex"))
      XCTAssertEqual(p.keyAlias, row["key_alias"] as? String)
      XCTAssertEqual(p.platform, c.platform)
      if c.platform == 5 { XCTAssertEqual(c.androidKeyAlias,p.keyAlias) }
      else { XCTAssertNil(c.androidKeyAlias); XCTAssertEqual(p.appleCounterFloor,0) }
      let transport = try value(row,"challenge_transport_hex")
      XCTAssertEqual(transport.count,515)
      XCTAssertEqual(Data(transport.prefix(451)), Data(f[7].suffix(451)))
      let issuer = try Curve25519.Signing.PublicKey(rawRepresentation: value(row,"issuer_public_key_hex"))
      XCTAssertTrue(issuer.isValidSignature(try value(row,"issuer_signature_hex"),for:f[7]))
      XCTAssertEqual(Data(transport.suffix(64)),try value(row,"issuer_signature_hex"))
    }
  }

  func testActualNativeEnrollmentRejectsWrongPurposeAttemptIDAndAliasSubstitution() throws {
    for row in try vectors() {
      let original = try fields(row), c = try KagemushaOrdinaryAppEnrollmentProjectionV1(original[7])
      let start = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8).count+8
      var bad = original; bad[1][start+2] = 2
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:bad,
        approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:c.canonicalSigningBytes))))
      bad=original;bad[1].replaceSubrange((start+3)..<(start+35),with:c.enrollmentID)
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:bad,
        approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:c.canonicalSigningBytes))))
      bad=original;bad[3]=Data("substituted-original-alias".utf8)
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:bad,
        approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:c.canonicalSigningBytes))))
      // The signed original C cannot be replaced by its Norito archive.
      bad=original;bad[7]=try value(row,"challenge_archive_hex")
      bad[4]=Data(SHA256.hash(data:bad[7]))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:bad,
        approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:c.canonicalSigningBytes))))
    }
  }

  private func fields(_ row:[String:Any]) throws -> [Data] {
    let nativePlatform:UInt8 = (row["platform_tag"] as? Int)==1 ? 5 : 4
    return [Data([9])+Data(repeating:0,count:7),try value(row,"possession_signing_hex"),
      Data([nativePlatform]),Data(try XCTUnwrap(row["key_alias"] as? String).utf8),
      try value(row,"attestation_challenge_hex"),try value(row,"attested_public_key_sec1_hex"),
      try value(row,"attested_key_id_hex"),try value(row,"challenge_signing_hex"),Data(),
      Data(repeating:0x99,count:32),nativePlatform==4 ? Data(repeating:0,count:4) : Data(),
      Data([nativePlatform==4 ? 0 : 3]),Data(repeating:0x88,count:32),Data()]
  }
  private func vectors() throws -> [[String:Any]] {
    let path=URL(fileURLWithPath:#filePath).deletingLastPathComponent()
      .appendingPathComponent("Fixtures/kagemusha_ordinary_enrollment_native_vectors_v1.json")
    let raw=try Data(contentsOf:path)
    XCTAssertEqual(Data(SHA256.hash(data:raw)),try hex("23e39d05c50a1e32c95ccd5bed97724205e325d01b9d014b3a37a747ab9d9d67"))
    let root=try XCTUnwrap(JSONSerialization.jsonObject(with:raw) as? [String:Any])
    XCTAssertEqual(root["codec_only"] as? Bool,true)
    for flag in ["native_authority","hardware_qualified","monetary_authority"] {
      XCTAssertEqual(root[flag] as? Bool,false)
    }
    let rows=try XCTUnwrap(root["enrollment_vectors"] as? [[String:Any]])
    XCTAssertEqual(rows.count,2);return rows
  }
  private func value(_ row:[String:Any],_ key:String) throws -> Data {
    try hex(XCTUnwrap(row[key] as? String))
  }
  private func hex(_ text:String) throws -> Data {
    guard text.count%2==0 else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture hex") }
    let chars=Array(text);var result=Data()
    for i in stride(from:0,to:chars.count,by:2) {
      guard let byte=UInt8(String(chars[i...i+1]),radix:16) else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("fixture hex")
      }
      result.append(byte)
    }
    return result
  }
}
