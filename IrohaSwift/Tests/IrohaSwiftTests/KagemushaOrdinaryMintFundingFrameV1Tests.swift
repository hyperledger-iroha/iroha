import Foundation
import XCTest
@testable import IrohaSwift

/// Canonical transport tests with inert originals. No proof, debit or Native owner qualification.
final class KagemushaOrdinaryMintFundingFrameV1Tests: XCTestCase {
  private let digest = Data(repeating: 7, count: 32)
  private let prepared = [Data([1]), Data([2]), Data(repeating: 3, count: 64),
    Data([4]), Data(), Data([5]), Data([6]), Data([7])]

  func testAllSeventeenResponseShapesPreservePhaseAndOwnerCorrelation() throws {
    XCTAssertEqual(KagemushaOrdinaryMintFundingPhaseV1.allCases.count, 17)
    XCTAssertNil(KagemushaOrdinaryMintFundingPhaseV1(rawValue: 0))
    XCTAssertNil(KagemushaOrdinaryMintFundingPhaseV1(rawValue: 18))
    for phase in KagemushaOrdinaryMintFundingPhaseV1.allCases {
      let fields: [Data]
      switch phase {
      case .prepareMint: fields = [digest, Data([1]), Data([2])]
      case .proveMint, .signTransaction: fields = [digest]
      case .signAccountConsent: fields = [Data(repeating: 1, count: 64)]
      case .preparePreDebit, .fencePreDebit, .recoverPreDebit: fields = prepared
      case .readFinality: fields = [Data([0]), Data()]
      case .recoverPlatform: fields = [Data([0]), digest, Data([1]), Data([2]), Data()]
      case .originalPlatformCounter: fields = [Data([5]), Data()]
      case .retainedProgress: fields = [digest, Data([0])]
      case .fencePlatform, .retainPlatformOriginal, .retainCoreDecision, .submitTransaction,
        .refreshAccountClock, .acknowledgeRetainedPlatformOriginal: fields = []
      }
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(phase, handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(phase,
        handle: 19, response: frame), fields)
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(phase,
        handle: 20, response: frame))
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(phase,
        handle: 0, response: frame))
    }
  }

  func testAllRequestPhasesUseOnlyTheirExactOriginalRoles() throws {
    for phase in KagemushaOrdinaryMintFundingPhaseV1.allCases {
      let originals: [Data]
      switch phase {
      case .prepareMint: originals = [Data([1] + Array(repeating: UInt8(0), count: 15))]
      case .retainPlatformOriginal: originals = [Data([1])]
      case .retainCoreDecision: originals = Array(repeating: Data([1]), count: 5)
      default: originals = []
      }
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(phase,
        handle: 19, originals: originals)
      let archive = try XCTUnwrap(noritoDecodeFrame(frame))
      XCTAssertEqual(archive.header.schema,
        noritoSchemaHash(forTypeName: "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingRequestV1"))
      var reader = CanonicalNoritoReader(data: archive.payload)
      XCTAssertEqual(try reader.readCompactField(), CompactNorito.encodeUInt16(1))
      XCTAssertEqual(try reader.readCompactField(), Data([phase.rawValue]))
      XCTAssertEqual(try reader.readCompactField(), CompactNorito.encodeUInt64(19))
      XCTAssertEqual(try reader.readCompactField(),
        try CompactNorito.encodeVec(originals, encode: CompactNorito.encodeBytesVec))
      XCTAssertEqual(reader.remaining(), 0)
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(phase,
        handle: 0, originals: originals))
    }
  }

  func testPositiveAmountUsesFullLittleEndianUInt128WithoutNarrowing() throws {
    for amount in [Data([1] + Array(repeating: UInt8(0), count: 15)),
      Data(Array(repeating: UInt8(0), count: 15) + [1]), Data(repeating: 255, count: 16)] {
      _ = try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.prepareMint,
        handle: 19, originals: [amount])
    }
    for amount in [Data(), Data(repeating: 1, count: 15), Data(repeating: 1, count: 17),
      Data(repeating: 0, count: 16)] {
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.prepareMint,
        handle: 19, originals: [amount]))
    }
  }

  func testMalformedOriginalsDoNotDispatchOrCloseTheRetainedOwner() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.prepareMint,
      originals: [Data(repeating: 0, count: 16)]))
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.retainCoreDecision,
      originals: Array(repeating: Data([1]), count: 4)))
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.retainPlatformOriginal,
      originals: [Data(repeating: 1, count: 4_097)]))
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.acknowledgeRetainedPlatformOriginal,
      originals: [Data([1])]))
    XCTAssertEqual(endpoint.invocations, 0)
    XCTAssertEqual(endpoint.closes, 0)
    endpoint.response = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.refreshAccountClock,
      handle: 19, fields: [])
    let adapter: any KagemushaNativeMintFundingCoreCoordinatorV1 = KagemushaNativeCoreCoordinatorAdapterV1(bridge: bridge)
    XCTAssertEqual(try adapter.invokeOrdinaryMintFunding(.refreshAccountClock, originals: []), [])
    XCTAssertEqual(endpoint.invocations, 1)
    XCTAssertEqual(endpoint.request, try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.refreshAccountClock,
      handle: 19, originals: []))
  }

  func testEightPreDebitOriginalsArePreservedIncludingSeparateFinancialControls() throws {
    for phase in [KagemushaOrdinaryMintFundingPhaseV1.preparePreDebit, .fencePreDebit, .recoverPreDebit] {
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(phase, handle: 19, fields: prepared)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(phase,
        handle: 19, response: frame), prepared)
      for index in [0, 1, 2, 3, 5, 6, 7] {
        var invalid = prepared; invalid[index] = Data()
        XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(phase,
          handle: 19, fields: invalid))
      }
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(phase,
        handle: 19, fields: Array(prepared.dropLast())))
    }
  }

  func testCompleteNodePacketUsesDedicatedBoundAndRetainsEveryOriginal() throws {
    let packet = Data(repeating: 9, count: 300_000)
    let originals = [Data([1]), Data([2]), Data([3]), Data([4]), packet]
    let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.retainCoreDecision,
      handle: 19, originals: originals)
    XCTAssertGreaterThan(frame.count, KagemushaCoreCoordinatorFrameV1.maximumRequestBytes)
    var reader = CanonicalNoritoReader(data: try XCTUnwrap(noritoDecodeFrame(frame)).payload)
    for _ in 0..<3 { _ = try reader.readCompactField() }
    XCTAssertEqual(try reader.readCompactField(),
      try CompactNorito.encodeVec(originals, encode: CompactNorito.encodeBytesVec))
    XCTAssertEqual(KagemushaOrdinaryMintFundingFrameV1.frameMaximum,
      60 * 1024 * 1024 + (16 * 1024 * 1024 + 4_096) + 32_768 + 16_384 + 131_072 + 4_096)
    for index in 0..<5 {
      var invalid = originals; invalid[index] = Data()
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.retainCoreDecision,
        handle: 19, originals: invalid))
    }
  }

  func testReadOnlyPlatformRecoveryCannotReclassifyUnknownAsRetainedOriginal() throws {
    for status in UInt8(0)...3 {
      let raw = status < 2 ? Data() : Data([9])
      let fields = [Data([status]), digest, Data([1]), Data([2]), raw]
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.recoverPlatform,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.recoverPlatform,
        handle: 19, response: frame), fields)
      var wrong = fields; wrong[4] = status < 2 ? Data([9]) : Data()
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.recoverPlatform,
        handle: 19, fields: wrong))
    }
  }

  func testPendingFinalityCannotCarryAnAppliedOriginal() throws {
    for fields in [[Data([0]), Data()], [Data([1]), Data([9])]] {
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.readFinality,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.readFinality,
        handle: 19, response: frame), fields)
    }
    for fields in [[Data([0]), Data([9])], [Data([1]), Data()], [Data([2]), Data([9])]] {
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.readFinality,
        handle: 19, fields: fields))
    }
  }

  func testActualAppleCounterUsesAllFourBytesAndAndroidHasNoFabricatedFloor() throws {
    for fields in [[Data([4]), Data(repeating: 0, count: 4)],
      [Data([4]), Data(repeating: 255, count: 4)], [Data([5]), Data()]] {
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.originalPlatformCounter,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.originalPlatformCounter,
        handle: 19, response: frame), fields)
    }
    for fields in [[Data([4]), Data()], [Data([5]), Data(repeating: 0, count: 4)],
      [Data([0]), Data()], [Data([4]), Data(repeating: 1, count: 16)]] {
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.originalPlatformCounter,
        handle: 19, fields: fields))
    }
    XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.originalPlatformCounter,
      handle: 19, originals: [Data(repeating: 255, count: 4)]))
  }

  func testCorruptSchemaVersionFlagsAndTrailingPayloadRefuse() throws {
    let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.refreshAccountClock,
      handle: 19, fields: [])
    var corrupt = frame; corrupt[31] ^= 1
    var retiredFlags = frame; retiredFlags[39] = 0
    var trailing = try XCTUnwrap(noritoDecodeFrame(frame)).payload; trailing.append(0)
    let trailingFrame = noritoEncode(typeName: "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingResponseV1",
      payload: trailing, flags: NoritoHeader.compactLen, payloadAlignment: 8)
    let wrongSchema = try uncheckedResponse(.refreshAccountClock, fields: [],
      type: "connect_norito_bridge::KagemushaOrdinaryNativeIncomingResponseV1")
    let wrongVersion = try uncheckedResponse(.refreshAccountClock, fields: [], version: 2)
    let tooMany = try uncheckedResponse(.preparePreDebit, fields: prepared + [Data([9])])
    for invalid in [corrupt, retiredFlags, trailingFrame, wrongSchema, wrongVersion, tooMany] {
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.refreshAccountClock,
        handle: 19, response: invalid))
    }
    XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.preparePreDebit,
      handle: 19, response: tooMany))
  }

  func testSubstitutedResponseClosesBeforeTeardownAndNeverRedispatches() throws {
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    endpoint.response = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.submitTransaction,
      handle: 19, fields: [])
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.refreshAccountClock))
    XCTAssertEqual(endpoint.closes, 1)
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.refreshAccountClock))
    XCTAssertEqual(endpoint.invocations, 1)
  }

  func testNativeErrorRemainsExactEvenWhenTeardownAlsoFails() throws {
    let endpoint = Endpoint(); endpoint.failure = .nativeFailure(-310)
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.submitTransaction)) {
      XCTAssertEqual($0 as? KagemushaCoreCoordinatorErrorV1, .nativeFailure(-310))
    }
    XCTAssertEqual(endpoint.closes, 1)
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.submitTransaction))
    XCTAssertEqual(endpoint.invocations, 1)
  }

  func testRetainedProgressIsReadOnlyAndRejectsMalformedOperationOrStage() throws {
    for stage in UInt8(0)...12 {
      let fields = [digest, Data([stage])]
      let frame = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.retainedProgress,
        handle: 19, fields: fields)
      XCTAssertEqual(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.retainedProgress,
        handle: 19, response: frame), fields)
    }
    let malformed: [[Data]] = [[digest], [digest, Data([0]), Data()],
      [Data(repeating: 0, count: 32), Data([0])], [Data(repeating: 1, count: 31), Data([0])],
      [digest, Data()], [digest, Data([0, 1])], [digest, Data([13])], [digest, Data([255])]]
    for fields in malformed {
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.retainedProgress,
        handle: 19, fields: fields))
      let frame = try uncheckedResponse(.retainedProgress, fields: fields)
      XCTAssertThrowsError(try KagemushaOrdinaryMintFundingFrameV1.decodeResponse(.retainedProgress,
        handle: 19, response: frame))
    }
    let endpoint = Endpoint()
    let bridge = try KagemushaCoreCoordinatorBridgeV1.openEndpoint(storagePath: "/durable/store", endpoint: endpoint)
    // An offered operation cannot select or renew the retained Native operation.
    XCTAssertThrowsError(try bridge.invokeOrdinaryMintFunding(.retainedProgress, originals: [digest]))
    XCTAssertEqual(endpoint.invocations, 0)
    XCTAssertEqual(endpoint.closes, 0)
    endpoint.response = try KagemushaOrdinaryMintFundingFrameV1.encodeResponse(.retainedProgress,
      handle: 19, fields: [digest, Data([12])])
    XCTAssertEqual(try bridge.invokeOrdinaryMintFunding(.retainedProgress), [digest, Data([12])])
    XCTAssertEqual(endpoint.request, try KagemushaOrdinaryMintFundingFrameV1.encodeRequest(.retainedProgress,
      handle: 19, originals: []))
    XCTAssertEqual(endpoint.invocations, 1)
    XCTAssertEqual(endpoint.closes, 0)
  }

  private func uncheckedResponse(_ phase: KagemushaOrdinaryMintFundingPhaseV1,
    fields: [Data], version: UInt16 = 1,
    type: String = "connect_norito_bridge::KagemushaOrdinaryNativeMintFundingResponseV1") throws -> Data {
    var writer = CompactNoritoWriter()
    writer.writeField(CompactNorito.encodeUInt16(version))
    writer.writeField(Data([phase.rawValue]))
    writer.writeField(CompactNorito.encodeUInt64(19))
    // This helper is used only with small known fixture blobs.
    writer.writeField(try CompactNorito.encodeVec(fields, encode: CompactNorito.encodeBytesVec))
    return noritoEncode(typeName: type, payload: writer.data,
      flags: NoritoHeader.compactLen, payloadAlignment: 8)
  }

  private final class Endpoint: KagemushaCoreCoordinatorEndpointV1 {
    var response = Data(), request = Data()
    var failure: KagemushaCoreCoordinatorErrorV1?
    var invocations = 0, closes = 0
    func contract() throws -> [UInt32] { [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21] }
    func install(storagePath: Data) throws {}
    func open(storagePath: Data) throws -> UInt64 { 19 }
    func invoke(handle: UInt64, method: UInt8, request: Data) throws -> Data {
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    func invokeIncoming(request: Data) throws -> Data { throw KagemushaCoreCoordinatorErrorV1.unavailable }
    func invokeIntegrity(phase: UInt8, handle: UInt64, original: Data) throws -> Data {
      throw KagemushaCoreCoordinatorErrorV1.unavailable
    }
    func invokeMintFunding(request: Data) throws -> Data {
      invocations += 1
      self.request = request
      if let failure { throw failure }
      return response
    }
    func close(handle: UInt64) throws {
      closes += 1
      if failure != nil { throw KagemushaCoreCoordinatorErrorV1.nativeFailure(-311) }
    }
  }
}
