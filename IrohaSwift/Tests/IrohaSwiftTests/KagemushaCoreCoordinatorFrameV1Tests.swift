import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCoreCoordinatorFrameV1Tests: XCTestCase {
  private func recoveryFields() throws -> [Data] {
    let key = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x41, count: 32))
    let literal = try AccountAddress.fromAccount(publicKey: key.publicKey.rawRepresentation).toI105(networkPrefix: 753)
    let runtime = try KagemushaRetailEnrollmentRuntimeProjectionV1(fiID: "mibank", ledgerDataspaceID: 10,
      authenticationNamespace: "mibank.bpng", networkID: Data(repeating: 0x35, count: 32),
      asset: KagemushaAssetDefinitionIDV1("839FV3NJC8NfgWQvghXU2hEFQm9a"),
      assetIncarnation: KagemushaAssetIncarnationV1(bytes: Data(repeating: 0x37, count: 32)), scale: 2)
    let owner = try KagemushaRetailEnrollmentOwnerProjectionV1(accountID: .init(literal),
      runtime: runtime, laneID: Data(repeating: 0x38, count: 32))
    let challenge = try KagemushaEnrolledOpenAccountChallengeV1(enrollmentID: owner.enrollmentID(),
      owner: owner, nonce: Data(repeating: 0x39, count: 32),
      authoritySource: .initialCertificate(certificateDigest: Data(repeating: 0x40, count: 32)),
      releaseID: Data(repeating: 0x41, count: 32), hardwarePolicyDigest: Data(repeating: 0x42, count: 32),
      coreAuthorizationKeyReference: Data(repeating: 0x43, count: 32))
    return [Data([7, 0, 0, 0, 0, 0, 0, 0]), try challenge.canonicalBytes(),
      challenge.accountSigningMessage(), try KagemushaDeviceOperationCodecV1.encodeControlCommand(.readActiveHardwareCredential),
      challenge.nonce]
  }

  func testRecoveredNativeChallengeRequiresExactCanonicalMessageNonceAndReadCommand() throws {
    let fields = try recoveryFields()
    let begin = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(9)])
    let response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: begin, fields: fields)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(.initialEnrollment,
      requestFrame: begin, responseFrame: response), fields)
    XCTAssertEqual(try KagemushaEnrolledRecoveryAttemptV1(nativeFields: fields).accountSigningMessage, fields[2])
    for index in 0..<5 {
      var changed = fields
      if index == 0 { changed[0] = Data(repeating: 0, count: 8) }
      else if index == 1 { changed[1].append(0) }
      else if index == 3 { changed[3] = Data([1]) }
      else { changed[index] = Data(repeating: 0x49, count: 32) }
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
        requestFrame: begin, fields: changed), "phase9 field \(index)")
    }
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(9), fields[0]]))
    for changed in [Array(fields.dropLast()), fields + [Data([1])]] {
      XCTAssertThrowsError(try KagemushaEnrolledRecoveryAttemptV1(nativeFields: changed))
    }
  }

  func testRecoveredCompletionAndCancellationKeepExactAttemptAndOriginalFrameBounds() throws {
    let attempt = try recoveryFields()[0]
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(10), attempt, Data(repeating: 0x51, count: 64),
      Data(repeating: 0x52, count: 65_716)]
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: fields)
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: request, fields: [attempt]))
    for index in 1...3 {
      var changed = fields
      changed[index] = index == 1 ? Data(repeating: 0, count: 8)
        : index == 2 ? Data(repeating: 1, count: 63) : Data(repeating: 1, count: 65_717)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: changed))
    }
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: request, fields: [Data([8, 0, 0, 0, 0, 0, 0, 0])]))
    let cancel = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(11), attempt])
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: cancel, fields: []))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: cancel, fields: [attempt]))
  }

  func testNativePreparationVerificationRequiresExactTicketFrameAndReturnedNonce() throws {
    let nonce = Data(repeating: 0x22, count: 32)
    var preparation = Data(repeating: 0x11, count: 273)
    preparation.replaceSubrange(49..<81, with: nonce)
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(8), Data([7, 0, 0, 0, 0, 0, 0, 0]), preparation]
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: fields)
    let response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: request, fields: [nonce])
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(.initialEnrollment,
      requestFrame: request, responseFrame: response), [nonce])
    for malformed in [Array(fields.dropLast()), fields + [Data([1])],
                      [fields[0], Data(repeating: 0, count: 8), preparation],
                      [fields[0], fields[1], Data(preparation.dropLast())]] {
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
        fields: malformed))
    }
    for malformed in [[], [Data(repeating: 0, count: 32)], [Data(repeating: 0x33, count: 32)], [nonce, nonce]] {
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
        requestFrame: request, fields: malformed))
    }
  }

  func testCoordinatorMethodsMatchSharedCurrentSchemaVectors() throws {
    let cases = try fixtures()
    // The maintained native producer supplies structural diagnostics for all
    // first-release methods; these archives grant no native authority.
    XCTAssertEqual(Set(cases.map { $0.method.rawValue }), Set(UInt8(1)...UInt8(18)))
    XCTAssertEqual(cases.count, 25)
    for item in cases {
      let request = try KagemushaCoreCoordinatorFrameV1.decodeRequest(item.method, frame: item.request)
      let response = try KagemushaCoreCoordinatorFrameV1.decodeResponse(item.method, requestFrame: item.request, responseFrame: item.response)
      XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.encodeRequest(item.method, fields: request), item.request, item.name)
      XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.encodeResponse(item.method, requestFrame: item.request, fields: response), item.response, item.name)
    }
  }

  func testTruncationRetiredSchemaAndInvalidLengthsFailClosed() throws {
    for item in try fixtures() {
      for length in item.request.indices {
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.decodeRequest(item.method, frame: item.request.prefix(length)), item.name)
      }
      var mutations = [item.request + Data([0])]
      for (index, value): (Int, UInt8) in [(8, 1), (12, 1), (10, 17)] {
        var bytes = item.request
        bytes[index] = value
        mutations.append(bytes)
      }
      var invalidLength = item.request
      if invalidLength.count >= 20 {
        invalidLength.replaceSubrange(16..<20, with: Data(repeating: 255, count: 4))
      } else {
        // The policy read has no request fields. Claiming one oversized field
        // must still be rejected without constructing an invalid Data range.
        invalidLength[10] = 1
        invalidLength.append(Data(repeating: 255, count: 4))
      }
      mutations.append(invalidLength)
      for bytes in mutations {
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.decodeRequest(item.method, frame: bytes))
      }
      for length in item.response.indices {
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.decodeResponse(item.method, requestFrame: item.request, responseFrame: item.response.prefix(length)), item.name)
      }
    }
  }

  func testClosedFieldCountsAndAllCorrelatedOutputsRejectSubstitution() throws {
    let indexes = ["reserve": 0, "begin-send": 0, "begin-redeem": 0, "installed-terminal": 0,
      "recover-sender": 0, "recover-terminal": 1, "release-send": 3, "release-redeem": 3,
      "app-attest-ack": 0, "outgoing-state-proof-export": 0]
    for item in try fixtures() {
      let request = try KagemushaCoreCoordinatorFrameV1.decodeRequest(item.method, frame: item.request)
      if !request.isEmpty {
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(item.method, fields: Array(request.dropLast())))
      } else {
        XCTAssertEqual(item.method, .authenticatedHardwarePolicy)
        XCTAssertEqual(item.request.count, 16)
      }
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(item.method, fields: request + [Data([1])]))
      var response = try KagemushaCoreCoordinatorFrameV1.decodeResponse(item.method, requestFrame: item.request, responseFrame: item.response)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(item.method, requestFrame: item.request, fields: response + [Data([1])]))
      let correlatedIndex: Int?
      switch item.method {
      case .prepareIncomingFold: correlatedIndex = 1
      case .completeIncomingFold, .stageIncomingOriginal: correlatedIndex = 0
      default: correlatedIndex = indexes[item.name]
      }
      if let index = correlatedIndex {
        response[index][0] ^= 1
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(item.method, requestFrame: item.request, fields: response), item.name)
      }
    }
  }

  func testOversizedFieldsAndFramesFailClosed() throws {
    let id = Data(repeating: 7, count: 32)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.reserveOperationID, fields: [KagemushaCoreCoordinatorFrameV1.u32(22), id, Data(repeating: 0, count: KagemushaCoreCoordinatorFrameV1.maximumFieldBytes + 1)]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptInstalledTerminal, fields: Array(repeating: Data(repeating: 0, count: 65536), count: 5)))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.reserveOperationID, frame: Data(repeating: 0, count: 262145)))
  }

  func testInitialEnrollmentBoundsTicketAndProofRetryCorrelation() throws {
    let ticket = KagemushaCoreCoordinatorFrameV1.u32(7) + KagemushaCoreCoordinatorFrameV1.u32(0)
    let begin = [KagemushaCoreCoordinatorFrameV1.u32(1), Data("i105example".utf8)]
    let beginFrame = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: begin)
    let response = [ticket, Data(repeating: 0x44, count: 32),
      Data(repeating: 0x45, count: 32), Data(repeating: 0x46, count: 32),
      Data(repeating: 0x47, count: 32), Data(repeating: 0x48, count: 32),
      KagemushaCoreCoordinatorFrameV1.u32(120_007) + KagemushaCoreCoordinatorFrameV1.u32(0)]
    let responseFrame = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: beginFrame, fields: response)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(.initialEnrollment,
      requestFrame: beginFrame, responseFrame: responseFrame), response)
    let readSelection = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(7), begin[1]])
    let retainedSelection = try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: readSelection, fields: response)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(.initialEnrollment,
      requestFrame: readSelection, responseFrame: retainedSelection), response)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: beginFrame, fields: [ticket, response[1], response[2], response[3], Data([0x45])]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: beginFrame, fields: Array(response.prefix(5))))
    var zeroDeadline = response
    zeroDeadline[6] = Data(repeating: 0, count: 8)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: beginFrame, fields: zeroDeadline))
    let challenge = [KagemushaCoreCoordinatorFrameV1.u32(2), ticket,
      Data(repeating: 0x51, count: 273), Data([0x52]), Data([0x53]), Data([0x54]),
      Data(repeating: 0x55, count: 32), Data(repeating: 0x56, count: 32),
      Data(repeating: 0x57, count: 32), Data([0x58]), ticket]
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: challenge))
    var shortPreparation = challenge
    shortPreparation[2].removeLast()
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: shortPreparation))
    let prepare = [KagemushaCoreCoordinatorFrameV1.u32(3), ticket,
      Data(repeating: 0x46, count: 64), Data(repeating: 0x47, count: 65_716)]
    XCTAssertNoThrow(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: prepare))
    var oversize = prepare
    oversize[3].append(0x48)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment, fields: oversize))
    let read = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.initialEnrollment,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(4), ticket])
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.initialEnrollment,
      requestFrame: read, fields: [Data(repeating: 0, count: 8), Data(repeating: 1, count: 32), Data([1])]))
  }

  func testAppAttestCommitAcknowledgmentBindsOriginalBytesAndCounter() throws {
    let method = KagemushaCoreCoordinatorMethodV1.acknowledgeCommittedAppAttest
    let domain = Data("iroha:kagemusha:v1:hardware-transition-selection\0".utf8)
    var selection = domain + Data([0x93, 0x01, 0, 0, 0, 0, 0, 0]) + Data([1, 0])
    for _ in 0..<7 { selection.append(Data(repeating: 0x42, count: 32)) }
    selection.append(Data([1, 0, 0, 0, 0, 0, 0, 0]))
    selection.append(Data(repeating: 0x42, count: 32))
    selection.append(Data([1, 0, 0, 0, 0, 0, 0, 0, 1])) // Generation, MintFold.
    selection.append(Data(repeating: 0x42, count: 32))
    selection.append(Data(repeating: 0, count: 64))
    selection.append(4)
    selection.append(Data(repeating: 0, count: 15))
    selection.append(5)
    selection.append(Data(repeating: 0, count: 15))
    // Public synthetic assertion shape only: the frame test grants no native authority.
    let auth = Data(repeating: 0x42, count: 32) + Data([0x40, 0, 0, 0, 9])
    let key = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 1, count: 32))
    let nonce = Data(SHA256.hash(data: auth + Data(SHA256.hash(data: selection))))
    let signature = try key.signature(for: nonce).derRepresentation
    var raw = Data([0xa2, 0x71])
    raw.append(Data("authenticatorData".utf8))
    raw.append(Data([0x58, 37])); raw.append(auth)
    raw.append(0x69); raw.append(Data("signature".utf8))
    raw.append(Data([0x58, UInt8(signature.count)])); raw.append(signature)
    let request = [Data(repeating: 0x11, count: 32), Data("app-attest-key".utf8),
      selection, raw, KagemushaCoreCoordinatorFrameV1.u32(4),
      Data(repeating: 0x33, count: 32), Data(repeating: 0x44, count: 32)]
    let encoded = try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: request)
    let response = [request[0], Data(SHA256.hash(data: request[1])),
      Data(SHA256.hash(data: request[2])), Data(SHA256.hash(data: request[3])),
      KagemushaCoreCoordinatorFrameV1.u32(9), request[5], request[6]]
    let reply = try KagemushaCoreCoordinatorFrameV1.encodeResponse(
      method, requestFrame: encoded, fields: response)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(
      method, requestFrame: encoded, responseFrame: reply), response)
    for index in response.indices {
      var changed = response
      changed[index][0] ^= 1
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(
        method, requestFrame: encoded, fields: changed))
    }
    for index in request.indices {
      var changed = request
      changed[index] = Data()
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: changed))
    }
    var exhausted = request
    exhausted[4] = KagemushaCoreCoordinatorFrameV1.u32(UInt32.max)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: exhausted))
  }

  func testOutgoingStateProofExportBindsOriginalOperationAndBoundsArchives() throws {
    let method = KagemushaCoreCoordinatorMethodV1.exportOutgoingStateProof
    let operationID = Data(repeating: 0x66, count: 32)
    let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields: [operationID])
    let fields = [operationID, Data([0x81]), Data([0x82])]
    let response = try KagemushaCoreCoordinatorFrameV1.encodeResponse(method,
      requestFrame: request, fields: fields)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeResponse(method,
      requestFrame: request, responseFrame: response), fields)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(method,
      fields: [Data(repeating: 0, count: 32)]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(method,
      requestFrame: request, fields: [Data(repeating: 0x67, count: 32), fields[1], fields[2]]))
    for index in 1...2 {
      var oversized = fields
      oversized[index] = Data(repeating: 0x83, count: index == 1 ? 4097 : 6529)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(method,
        requestFrame: request, fields: oversized))
    }
  }

  func testAuthenticatedReplyRequiresFullLowSAuthenticatorAndRetiresNineFields() throws {
    let id = Data(repeating: 7, count: 32)
    var scalar = Data(repeating: 0, count: 32)
    scalar[31] = 1
    let signature = scalar + scalar
    let fields = [KagemushaCoreCoordinatorFrameV1.u32(5), id, Data([1]), Data([2]), signature,
      KagemushaCoreCoordinatorFrameV1.u32(1), id, Data([3]), Data([4]), KagemushaCoreCoordinatorFrameV1.u32(0xffff)]
    let frame = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: fields)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.acceptAuthenticatedReply, frame: frame)[4], signature)
    var retired = fields
    retired.remove(at: 4)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: retired))
    for invalid in [Data(), Data(repeating: 1, count: 63), Data(repeating: 0, count: 64), Data(repeating: 0xff, count: 64)] {
      var mutation = fields
      mutation[4] = invalid
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: mutation))
    }
  }

  func testReleaseAcceptanceRequiresExactOriginalResponseAndRejectsTenFieldLayout() throws {
    let id = Data(repeating: 7, count: 32)
    var scalar = Data(repeating: 0, count: 32); scalar[31] = 1
    let signature = scalar + scalar
    let reply = Data([0x12])
    let original = try testSignedDeviceResponseFrame(operation: 12, status: .success,
      requestID: id, payload: reply, authenticator: signature)
    let retired = [KagemushaCoreCoordinatorFrameV1.u32(12), id, Data([1]), reply, signature,
      KagemushaCoreCoordinatorFrameV1.u32(1), id, Data([3]), Data([4]), KagemushaCoreCoordinatorFrameV1.u32(0xffff)]
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: retired))
    let fields = retired + [original]
    let frame = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: fields)
    XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.acceptAuthenticatedReply, frame: frame)[10], original)
    for index in [1, 3, 4, 10] {
      var changed = fields
      if index == 10 { changed[index].removeLast() } else { changed[index][0] ^= 1 }
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptAuthenticatedReply, fields: changed))
    }
  }

  func testObservationCodecVectorsAndClosedReadInventory() throws {
    let commands: [KagemushaDeviceControlCommandV1] = [.readActiveHardwareCredential,
      .readTrustedTimeOrLease, .readPendingCreditWatermark(watermark: nil, target: .drainAll), .recoverWalletSnapshot]
    var vectors: [String: String] = [:]
    for command in commands {
      let canonical = try KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
      vectors[String(command.operation)] = canonical.map { String(format: "%02x", $0) }.joined()
      let fields = [KagemushaCoreCoordinatorFrameV1.u32(UInt32(command.operation)), canonical]
      let request = try KagemushaCoreCoordinatorFrameV1.encodeRequest(.beginObservation, fields: fields)
      XCTAssertEqual(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.beginObservation, frame: request), fields)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.reserveOperationID,
        fields: [fields[0], Data(repeating: 1, count: 32), canonical]))
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(.beginObservation,
        requestFrame: request, fields: [Data(repeating: 0, count: 32)]))
    }
    try JSONSerialization.data(withJSONObject: vectors, options: [.prettyPrinted, .sortedKeys])
      .write(to: URL(fileURLWithPath: "/tmp/swift-observation-command-vectors.json"))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.beginObservation,
      fields: [KagemushaCoreCoordinatorFrameV1.u32(20), Data([1])]))
  }

  private struct Fixture {
    let name: String
    let method: KagemushaCoreCoordinatorMethodV1
    let request: Data
    let response: Data
  }

  private func fixtures() throws -> [Fixture] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let fixture = directory.appendingPathComponent("fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv")
      if FileManager.default.fileExists(atPath: fixture.path) {
        return try String(contentsOf: fixture, encoding: .utf8).split(separator: "\n")
          .filter { !$0.hasPrefix("#") }.map { line in
            let columns = line.split(separator: "\t", omittingEmptySubsequences: false)
            guard columns.count == 4, let code = UInt8(columns[1]), let method = KagemushaCoreCoordinatorMethodV1(rawValue: code) else {
              throw KagemushaCoreCoordinatorErrorV1.invalidFrame("bad fixture row")
            }
            return Fixture(name: String(columns[0]), method: method, request: try hex(columns[2]), response: try hex(columns[3]))
          }
      }
      directory.deleteLastPathComponent()
    }
    throw KagemushaCoreCoordinatorErrorV1.invalidFrame("missing frame fixture")
  }

  private func hex(_ text: Substring) throws -> Data {
    let characters = Array(text)
    guard characters.count.isMultiple(of: 2) else { throw KagemushaCoreCoordinatorErrorV1.invalidFrame("bad hex") }
    return try Data(stride(from: 0, to: characters.count, by: 2).map {
      guard let value = UInt8(String(characters[$0...$0 + 1]), radix: 16) else {
        throw KagemushaCoreCoordinatorErrorV1.invalidFrame("bad hex")
      }
      return value
    })
  }
}
