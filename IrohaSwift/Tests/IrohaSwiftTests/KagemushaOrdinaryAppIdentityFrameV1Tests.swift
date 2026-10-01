import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Real Rust C bytes through structural parsers and finite native frame grammar.
/// No native handle, issuer verdict, fake bridge or device capability is constructed.
final class KagemushaOrdinaryAppIdentityFrameV1Tests:XCTestCase {
  func testInputFreeSelectorIsClosedAndDoesNotGenerateCallerIdentity() throws {
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.preparedOrdinaryAppIdentity.rawValue,21)
    let q=[n(11)],id=Data(repeating:0x44,count:32)
    try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[id])
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q+[id]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[Data(repeating:0,count:32)]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[id,id]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(12)]))
  }
  func testBothActualRustCOriginalsMatchPhase1ProjectionAndIssuerSignature() throws {
    for row in try vectors() {
      let f=try fields(row),c=try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2])
      let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f,enrollmentID:c.enrollmentID)
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse([n(1),c.enrollmentID],f)
      XCTAssertEqual(p.generationChallenge,try value(row,"attestation_challenge_hex"))
      XCTAssertEqual(p.signedChallenge,try value(row,"challenge_transport_hex"))
      let issuer=try Curve25519.Signing.PublicKey(rawRepresentation:value(row,"issuer_public_key_hex"))
      XCTAssertTrue(issuer.isValidSignature(Data(p.signedChallenge.suffix(64)),for:p.signingBytes))
      if p.platform == 5 { XCTAssertEqual(p.originalAlias,row["key_alias"] as? String);XCTAssertEqual(p.androidLevelsMask,3) }
      else { XCTAssertTrue(p.originalAlias.isEmpty);XCTAssertEqual(p.androidLevelsMask,0) }
    }
  }
  func testOriginalTransportChallengeAliasAndPlatformSubstitutionsReject() throws {
    for row in try vectors() {
      let f=try fields(row),id=try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2]).enrollmentID
      var bad=f;bad[1][100] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad,enrollmentID:id))
      bad=f;bad[3]=Data(repeating:0x44,count:32)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad,enrollmentID:id))
      bad=f;bad[4]=Data([f[4][0] == 4 ? 5 : 4])
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad,enrollmentID:id))
      bad=f;bad[5]=Data("caller-key-alias".utf8)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad,enrollmentID:id))
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f,enrollmentID:Data(repeating:0x55,count:32)))
    }
  }
  func testOneCompleteAttestationFitsTwoExactChunksAndRejectsPartialOrExtraChunks() throws {
    let row=try XCTUnwrap(vectors().first),point=try value(row,"attested_public_key_sec1_hex")
    for length in [1,65_535,65_536,65_537,131_072] {
      let raw=Data(repeating:0x23,count:length),q=[n(5),ticket,point,Data(raw.prefix(65_536)),Data(raw.dropFirst(65_536))]
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,
        [Data(SHA256.hash(data:raw)),Data(SHA256.hash(data:point))])
    }
    for chunks in [[Data(),Data()],[Data([1]),Data([2])],[Data(repeating:1,count:65_537),Data()],
      [Data(repeating:1,count:65_536),Data(repeating:1,count:65_537)]] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(5),ticket,point]+chunks))
    }
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(6),ticket,Data(repeating:1,count:314)]))
  }
  func testRawRecoveryChunksRequireExactIndexTotalAndFiniteLengths() throws {
    let hash=Data(repeating:0x41,count:32)
    for total in [1,65_536,65_537,131_072] {
      for i in 0..<((total+65_535)/65_536) {
        let length=min(65_536,max(0,total-i*65_536)),q=[n(10),ticket,n(UInt32(i))]
        let response=[n(UInt32(i)),Data(repeating:0x17,count:length),hash,n(UInt32(total))]
        try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,response)
        var bad=response;bad[0]=n(UInt32(1-i))
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
        bad=response;bad[1].append(0)
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
      }
    }
    // Native rejects start>=total; an empty second chunk is never a present original.
    for total in [1,65_535,65_536] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(
        [n(10),ticket,n(1)],[n(1),Data(),hash,n(UInt32(total))]))
    }
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(10),ticket,n(2)]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse([n(10),ticket,n(0)],[n(0),Data(),hash,n(0)]))
  }
  func testUnknownDeviceRecoveryNeverSuppliesInventedOriginal() throws {
    let row=try appleRow(),f=try fields(row),c=try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2])
    let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f,enrollmentID:c.enrollmentID)
    let key=Data(try XCTUnwrap(row["key_alias"] as? String).utf8)
    XCTAssertEqual(try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(0),.generate)
    XCTAssertEqual(try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(2),.attest)
    for state:UInt8 in [4,5] { XCTAssertEqual(try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(state),.complete) }
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(6))
    for state:UInt8 in [1,3] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityRecoveryActionV1.classify(state)) { error in
        XCTAssertEqual(error as? KagemushaAppAttestEvidenceErrorV1,.assertionOutcomeUnknown)
      }
      let r=[Data([state]),state==3 ? key : Data(),Data(),Data(),n(0),Data(),Data()]
      let decoded=try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(r,original:p)
      XCTAssertTrue(decoded.point.isEmpty);XCTAssertTrue(decoded.rawAdmission.isEmpty)
      var bad=r;bad[5]=Data(repeating:1,count:314)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(bad,original:p))
    }
    let point=try value(row,"attested_public_key_sec1_hex"),rawSHA=try value(row,"raw_platform_evidence_digest_hex")
    let r=[Data([4]),key,point,rawSHA,n(99),Data(),Data()]
    _ = try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(r,original:p)
    var bad=r;bad[1]=Data(Data(repeating:0x66,count:32).base64EncodedString().utf8)
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityRecoveryProjectionV1(bad,original:p))
  }
  func testRawAdmissionStructuralJoinKeepsExactCIntervalPointAndPendingScope() throws {
    let row=try appleRow(),f=try fields(row),c=try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2])
    let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f,enrollmentID:c.enrollmentID)
    let point=try value(row,"attested_public_key_sec1_hex"),rawSHA=try value(row,"raw_platform_evidence_digest_hex")
    let metadata=try KagemushaOrdinaryAppIdentityRecoveryProjectionV1([Data([4]),
      Data(try XCTUnwrap(row["key_alias"] as? String).utf8),point,rawSHA,n(99),Data(),Data()],original:p)
    // Nonzero signature bytes test only a projection join, not Ed authentication.
    var a=Data([1,0,1])+p.generationChallenge+c.appAuthorityPolicyDigest+Data([2,3])+point
    a += Data(SHA256.hash(data:point))+rawSHA+Data(repeating:0x27,count:32)+n(0)+u64(c.issuedAtMS)+u64(c.expiresAtMS)
    a += Data(repeating:0x29,count:64);XCTAssertEqual(a.count,314)
    let joined=try KagemushaOrdinaryAppIdentityRawProjectionV1(a,original:p,retained:metadata)
    let expected=Data(SHA256.hash(data:Data("iroha:kagemusha:v1:pending-raw-app-identity-scope\0".utf8)+p.nativeScope+u64(314)+a))
    XCTAssertEqual(joined.pendingScope(nativeScope:p.nativeScope),expected)
    for offset in [3,35,67,68,69,134,166,230,234,242] {
      var bad=a;bad[offset] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityRawProjectionV1(bad,original:p,retained:metadata))
    }
  }
  private var ticket:Data { Data([9])+Data(repeating:0,count:7) }
  private func n(_ v:UInt32)->Data { withUnsafeBytes(of:v.littleEndian){Data($0)} }
  private func u64(_ v:UInt64)->Data { withUnsafeBytes(of:v.littleEndian){Data($0)} }
  private func fields(_ row:[String:Any]) throws -> [Data] {
    let platform:UInt8=(row["platform_tag"] as? Int)==1 ? 5 : 4
    return [ticket,try value(row,"challenge_transport_hex"),try value(row,"challenge_signing_hex"),
      try value(row,"attestation_challenge_hex"),Data([platform]),
      platform==5 ? Data(try XCTUnwrap(row["key_alias"] as? String).utf8) : Data(),
      Data([platform==5 ? 3 : 0]),Data(repeating:0x99,count:32)]
  }
  private func appleRow() throws -> [String:Any] { try XCTUnwrap(vectors().first{($0["platform_tag"] as? Int)==2}) }
  private func vectors() throws -> [[String:Any]] {
    let path=URL(fileURLWithPath:#filePath).deletingLastPathComponent()
      .appendingPathComponent("Fixtures/kagemusha_ordinary_enrollment_native_vectors_v1.json")
    let raw=try Data(contentsOf:path)
    XCTAssertEqual(Data(SHA256.hash(data:raw)),try hex("23e39d05c50a1e32c95ccd5bed97724205e325d01b9d014b3a37a747ab9d9d67"))
    let json=try XCTUnwrap(JSONSerialization.jsonObject(with:raw) as? [String:Any])
    XCTAssertEqual(json["codec_only"] as? Bool,true)
    for key in ["native_authority","hardware_qualified","monetary_authority"] { XCTAssertEqual(json[key] as? Bool,false) }
    let rows=try XCTUnwrap(json["enrollment_vectors"] as? [[String:Any]]);XCTAssertEqual(rows.count,2);return rows
  }
  private func value(_ row:[String:Any],_ key:String) throws -> Data { try hex(XCTUnwrap(row[key] as? String)) }
  private func hex(_ text:String) throws -> Data {
    let chars=Array(text);guard chars.count%2==0 else { throw KagemushaCoreCoordinatorErrorV1.unavailable };var d=Data()
    for i in stride(from:0,to:chars.count,by:2) { d.append(try XCTUnwrap(UInt8(String(chars[i...i+1]),radix:16))) };return d
  }
}
