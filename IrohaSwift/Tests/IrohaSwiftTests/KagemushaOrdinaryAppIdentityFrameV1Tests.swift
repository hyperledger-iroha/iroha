import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Real Rust C bytes through structural parsers and finite native frame grammar.
/// Scripted endpoint tests exercise correlation only; they establish no native, device or monetary authority.
final class KagemushaOrdinaryAppIdentityFrameV1Tests:XCTestCase {
  func testInputFreeSelectorIsClosedAndDoesNotGenerateCallerIdentity() throws {
    XCTAssertEqual(KagemushaCoreCoordinatorMethodV1.preparedOrdinaryAppIdentity.rawValue,21)
    let q=[n(11)],id=Data(repeating:0x44,count:32)
    try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[id])
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q+[id]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[Data(repeating:0,count:32)]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[id,id]))
    try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(12)])
    for extra in [id,Data(),Data("caller-alias".utf8)] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(12),extra]))
    }
    for retired in [[n(1)],[n(1),id],[n(1),ticket]] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(retired))
    }
  }
  func testBothActualRustCOriginalsMatchExplicitPhase13ProjectionAndIssuerSignature() throws {
    for row in try vectors() {
      let f=try fields(row),c=try KagemushaOrdinaryAppEnrollmentProjectionV1(f[2])
      let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f)
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse([n(13),reservationTicket,f[1]],f)
      XCTAssertEqual(p.challenge.enrollmentID,c.enrollmentID)
      XCTAssertEqual(p.ticket,ticket)
      XCTAssertNotEqual(p.ticket,reservationTicket)
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
      let f=try fields(row)
      var bad=f;bad[1][100] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad))
      bad=f;bad[3]=Data(repeating:0x44,count:32)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad))
      bad=f;bad[4]=Data([f[4][0] == 4 ? 5 : 4])
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad))
      bad=f;bad[5]=Data("caller-key-alias".utf8)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityPreparedProjectionV1(bad))
      // A structurally valid response cannot substitute the explicitly offered signed original.
      var offered=f[1];offered[offered.count-1] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(
        [n(13),reservationTicket,offered],f))
    }
  }
  func testReservationCarrierRequiresExactCardinalityTicketSelectorsAccountAndNonceUUID() throws {
    let q=[n(12)]
    for row in try vectors() {
      let f=try reservationFields(row)
      let p=try KagemushaOrdinaryAppIdentityReservationProjectionV1(f)
      XCTAssertEqual(p.fields,f)
      XCTAssertEqual(p.ticket,reservationTicket)
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,f)
      for wrongCount in [[Data](),Array(f.dropLast()),f+[Data()]] {
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityReservationProjectionV1(wrongCount))
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,wrongCount))
      }
      for wrongTicket in [Data(),Data(repeating:0,count:8),Data(repeating:1,count:7),Data(repeating:1,count:9)] {
        var bad=f;bad[0]=wrongTicket
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
      }
      for index in 2...6 {
        for wrongDigest in [Data(repeating:0,count:32),Data(repeating:1,count:31),Data(repeating:1,count:33)] {
          var bad=f;bad[index]=wrongDigest
          XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
        }
      }
      // The carrier is bounded public text; this grammar check grants no account admission.
      for account in [Data("fixture-口座".utf8),Data(repeating:0x61,count:2048)] {
        var valid=f;valid[1]=account
        try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,valid)
      }
      for account in [Data(),Data([0xc0,0x80]),Data("fixture\u{0}account".utf8),Data(repeating:0x61,count:2049)] {
        var bad=f;bad[1]=account
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
      }
      var bad=f;bad[7]=Data("00000000-0000-4000-8000-000000000000".utf8)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
      bad=f;bad[2][0] ^= 1
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
      // Exact lowercase UUIDv4 formatting also holds when nonce bytes contain hex letters.
      var valid=f;valid[2]=Data(repeating:0xab,count:32);valid[7]=Data(nonceUUID(valid[2]).utf8)
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,valid)
      bad=valid;bad[7]=Data(nonceUUID(valid[2]).uppercased().utf8)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,bad))
    }
  }
  func testExplicitPreparationIntakeRequiresOriginal515AndCompleteRequest() throws {
    for row in try vectors() {
      let f=try fields(row),q=[n(13),reservationTicket,f[1]]
      try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q)
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(Array(q.dropLast())))
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q+[Data()]))
      for offered in [Data(),Data(f[1].prefix(514)),f[1]+Data([0]),try value(row,"challenge_archive_hex")] {
        XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(13),reservationTicket,offered]))
      }
    }
  }
  func testOriginalPolicyReadbackAllowsAbsentAndExact16KiBButRejectsOverflowOrExtraFields() throws {
    let q=[n(14),reservationTicket]
    try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q)
    for original in [Data(),Data(repeating:0x23,count:16_384)] {
      try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,[original])
    }
    for fields in [[Data](),[Data(),Data()],[Data(repeating:0x23,count:16_385)]] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateResponse(q,fields))
    }
  }
  func testRawAdmissionRequiresExplicit314ByteOriginalWithoutGrantingAuthority() throws {
    // Arbitrary bytes test only the frame width, never an issuer signature or native admission.
    let original=Data(repeating:0x23,count:314),q=[n(6),ticket,original]
    try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q)
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest([n(6),ticket]))
    XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(q+[Data()]))
    for length in [0,313,315] {
      XCTAssertThrowsError(try KagemushaOrdinaryAppIdentityFrameV1.validateRequest(
        [n(6),ticket,Data(repeating:0x23,count:length)]))
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
    let row=try appleRow(),f=try fields(row)
    let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f)
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
    let p=try KagemushaOrdinaryAppIdentityPreparedProjectionV1(f)
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
  func testNativeReservationUsesCopiedOriginalsDistinctTicketsAndExactRetry() throws {
    for row in try vectors() {
      let carrier=try reservationFields(row),prepared=try fields(row)
      XCTAssertNotEqual(carrier[0],prepared[0])
      let endpoint=ReservationEndpoint(carrier:carrier,prepared:prepared)
      let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
      let reservation=try bridge.reserveOriginalIdentity()
      var copied=try reservation.releaseID();copied[0] ^= 1
      XCTAssertEqual(try reservation.releaseID(),carrier[3])
      XCTAssertEqual(try reservation.originalPlayIntegrityPolicyBytes(),Data())
      let first=try reservation.acceptOriginalSignedPreparation(prepared[1])
      let second=try reservation.acceptOriginalSignedPreparation(prepared[1])
      XCTAssertEqual(try first.originalSignedPreparationBytes(),prepared[1])
      XCTAssertEqual(try second.originalChallengeSigningBytes(),prepared[2])
      XCTAssertEqual(endpoint.phases.filter{$0==13}.count,2)
      XCTAssertTrue(endpoint.phases.allSatisfy{[12,13,14,8].contains($0)})
      XCTAssertEqual(endpoint.closeCalls,0)
      try bridge.close()
    }
  }
  func testChangedPreparationRetryClosesBeforeAnotherNativeIntake() throws {
    let row=try appleRow(),carrier=try reservationFields(row),prepared=try fields(row)
    let endpoint=ReservationEndpoint(carrier:carrier,prepared:prepared)
    let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
    let reservation=try bridge.reserveOriginalIdentity()
    _ = try reservation.acceptOriginalSignedPreparation(prepared[1])
    var changed=prepared[1];changed[changed.count-1] ^= 1
    XCTAssertThrowsError(try reservation.acceptOriginalSignedPreparation(changed))
    XCTAssertEqual(endpoint.phases.filter{$0==13}.count,1)
    XCTAssertEqual(endpoint.closeCalls,1)
    let calls=endpoint.phases.count
    XCTAssertThrowsError(try reservation.clientNonce())
    XCTAssertEqual(endpoint.phases.count,calls)
  }
  func testNativeReservationSubstitutionClosesBeforePolicyOrPreparationDispatch() throws {
    for phase in [UInt32(13),14] {
      let row=try appleRow(),carrier=try reservationFields(row),prepared=try fields(row)
      let endpoint=ReservationEndpoint(carrier:carrier,prepared:prepared)
      let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
      let reservation=try bridge.reserveOriginalIdentity()
      endpoint.carrier[3]=Data(repeating:0x65,count:32)
      if phase==13 { XCTAssertThrowsError(try reservation.acceptOriginalSignedPreparation(prepared[1])) }
      else { XCTAssertThrowsError(try reservation.originalPlayIntegrityPolicyBytes()) }
      XCTAssertFalse(endpoint.phases.contains(phase))
      XCTAssertEqual(endpoint.closeCalls,1)
    }
  }
  func testLostNativePreparationResponseFreezesTheSameReservation() throws {
    let row=try appleRow(),endpoint=ReservationEndpoint(carrier:try reservationFields(row),prepared:try fields(row))
    let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
    let reservation=try bridge.reserveOriginalIdentity()
    endpoint.losePreparationResponse=true
    XCTAssertThrowsError(try reservation.acceptOriginalSignedPreparation(endpoint.prepared[1]))
    let calls=endpoint.phases.count
    XCTAssertThrowsError(try reservation.acceptOriginalSignedPreparation(endpoint.prepared[1]))
    XCTAssertEqual(endpoint.phases.count,calls)
    XCTAssertEqual(endpoint.phases.filter{$0==13}.count,1)
    XCTAssertEqual(endpoint.closeCalls,1)
  }
  func testPreparedScopeSubstitutionRevokesBeforeOriginalExposure() throws {
    let row=try appleRow(),endpoint=ReservationEndpoint(carrier:try reservationFields(row),prepared:try fields(row))
    let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
    let reservation=try bridge.reserveOriginalIdentity()
    let prepared=try reservation.acceptOriginalSignedPreparation(endpoint.prepared[1])
    endpoint.substitutePreparedScope=true
    XCTAssertThrowsError(try prepared.originalSignedPreparationBytes())
    let calls=endpoint.phases.count
    XCTAssertThrowsError(try prepared.originalChallengeSigningBytes())
    XCTAssertEqual(endpoint.phases.count,calls)
    XCTAssertEqual(endpoint.closeCalls,1)
  }
  func testApplePossessionAPIRejectsAndroidBeforeAnyNativePossessionDispatch() throws {
    let row=try XCTUnwrap(vectors().first{($0["platform_tag"] as? Int)==1})
    let endpoint=ReservationEndpoint(carrier:try reservationFields(row),prepared:try fields(row))
    let bridge=try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath:"/durable/store",endpoint:endpoint)
    let reservation=try bridge.reserveOriginalIdentity()
    let prepared=try reservation.acceptOriginalSignedPreparation(endpoint.prepared[1])
    let calls=endpoint.phases.count
    XCTAssertThrowsError(try prepared.preparePendingAppAttestPossession())
    XCTAssertEqual(endpoint.phases.count,calls)
    XCTAssertFalse(endpoint.phases.contains(20))
    XCTAssertEqual(endpoint.closeCalls,1)
  }
  /// Mapping control only: native cryptographic admission and physical qualification are separate.
  private final class ReservationEndpoint:KagemushaCoreCoordinatorEndpointV1 {
    var carrier:[Data]
    let prepared:[Data]
    var phases=[UInt32]()
    var closeCalls=0
    var losePreparationResponse=false
    var substitutePreparedScope=false
    init(carrier:[Data],prepared:[Data]) { self.carrier=carrier.map{Data($0)};self.prepared=prepared.map{Data($0)} }
    func contract() throws -> [UInt32] { [2,25,3,6,54,8,7,22,16,0xffff,1,21] }
    func install(storagePath:Data) throws {}
    func open(storagePath:Data) throws -> UInt64 { 1 }
    func close(handle:UInt64) throws { XCTAssertEqual(handle,1);closeCalls += 1 }
    func invoke(handle:UInt64,method:UInt8,request:Data) throws -> Data {
      XCTAssertEqual(handle,1);XCTAssertEqual(method,21)
      let q=try KagemushaCoreCoordinatorFrameV1.decodeRequest(.preparedOrdinaryAppIdentity,frame:request)
      let phase=KagemushaAppPlatformPreparedProjectionV1.u32(q[0]);phases.append(phase)
      let r:[Data]
      switch phase {
      case 12:XCTAssertEqual(q.count,1);r=carrier
      case 13:
        XCTAssertEqual(q[1],carrier[0]);XCTAssertEqual(q[2],prepared[1])
        if losePreparationResponse { throw KagemushaCoreCoordinatorErrorV1.unavailable }
        r=prepared
      case 14:XCTAssertEqual(q[1],carrier[0]);r=[Data()]
      case 8:
        XCTAssertEqual(q[1],prepared[0])
        r=[substitutePreparedScope ? Data(repeating:0x76,count:32) : prepared[7],prepared[3]]
      default:throw KagemushaCoreCoordinatorErrorV1.invalidFrame("unexpected scripted identity phase")
      }
      return try KagemushaCoreCoordinatorFrameV1.encodeResponse(.preparedOrdinaryAppIdentity,
        requestFrame:request,fields:r)
    }
  }
  private var reservationTicket:Data { Data([6])+Data(repeating:0,count:7) }
  private func reservationFields(_ row:[String:Any]) throws -> [Data] {
    let signed=try value(row,"challenge_transport_hex")
    func selector(_ index:Int)->Data { Data(signed[(3+index*32)..<(35+index*32)]) }
    let nonce=selector(1)
    return [reservationTicket,Data("fixture-account".utf8),nonce,selector(6),selector(7),
      selector(5),selector(11),Data(nonceUUID(nonce).utf8)]
  }
  private func nonceUUID(_ nonce:Data)->String {
    var bytes=Array(nonce.prefix(16))
    bytes[6]=(bytes[6]&0x0f)|0x40;bytes[8]=(bytes[8]&0x3f)|0x80
    let uuid=UUID(uuid:(bytes[0],bytes[1],bytes[2],bytes[3],bytes[4],bytes[5],bytes[6],bytes[7],
      bytes[8],bytes[9],bytes[10],bytes[11],bytes[12],bytes[13],bytes[14],bytes[15]))
    return uuid.uuidString.lowercased()
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
    XCTAssertEqual(Data(SHA256.hash(data:raw)),try hex("d398cbdbdb79d5216f202457404d32381d867caf208aeb360c9b8157a56b5e7a"))
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
