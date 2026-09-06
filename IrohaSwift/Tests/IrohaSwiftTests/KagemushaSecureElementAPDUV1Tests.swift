import CryptoKit
import Foundation
import XCTest

@testable import IrohaSwift

final class KagemushaSecureElementAPDUV1Tests: XCTestCase {
  #if !OFFLINE_SECURE_ELEMENT_CREDENTIAL
    func testAppleCredentialBoundaryIsUnavailableWithoutExplicitCompileGate() async throws {
      let configuration = try KagemushaSecureElementCredentialConfigurationV1.foundation(
        productConfigurationIdentifier: UUID(
          uuidString: "00112233-4455-6677-8899-AABBCCDDEEFF"
        )!,
        displayName: "KAGEMUSHA",
        releaseID: Data(repeating: 0x44, count: 32)
      )
      let opened = await KagemushaSecureElementCredentialSessionV1.openIfAvailable(
        configuration: configuration
      )
      XCTAssertNil(opened)
    }
  #endif

  func testCapabilityProbePreservesFoundationDiagnosticAllocation() throws {
    let capabilities = Data((1...96).map(UInt8.init))
    let channel = ScriptedAPDUChannel(
      response: Data(repeating: 7, count: 116),
      capabilities: capabilities
    )
    let endpoint = KagemushaSecureElementAPDUEndpointV1(channel: channel)
    XCTAssertEqual(try endpoint.capabilities(), capabilities)
    XCTAssertEqual(channel.commands, [Data([0x80, 0x11, 0, 0, 96])])
    XCTAssertFalse(channel.commands.contains { $0[1] == 0x10 })
    XCTAssertFalse(channel.commands.contains { (0x20...0x27).contains($0[1]) })
  }

  func testCommandAndResponseAreChunkedDeterministicallyAndDigestBound() throws {
    let command = Data((0..<(80 + 500)).map { UInt8(truncatingIfNeeded: $0 * 17) })
    let response = Data((0..<(116 + 509)).map { UInt8(truncatingIfNeeded: $0 * 29) })
    let channel = ScriptedAPDUChannel(response: response)
    let endpoint = KagemushaSecureElementAPDUEndpointV1(channel: channel)

    XCTAssertEqual(try endpoint.execute(command), response)
    XCTAssertEqual(channel.receivedCommand, command)
    XCTAssertEqual(channel.writeSizes, [224, 224, 132])
    XCTAssertEqual(channel.readSizes, [224, 224, 177])
    XCTAssertEqual(channel.writeIndexes, [0, 1, 2])
    XCTAssertEqual(channel.readIndexes, [0, 1, 2])
    XCTAssertEqual(channel.abortCount, 0)
  }

  func testBadResponseDigestFailsClosedAndAbortsTransport() throws {
    let channel = ScriptedAPDUChannel(response: Data(repeating: 7, count: 116))
    channel.corruptResponseDigest = true
    let endpoint = KagemushaSecureElementAPDUEndpointV1(channel: channel)
    XCTAssertThrowsError(try endpoint.execute(Data(repeating: 3, count: 80)))
    XCTAssertEqual(channel.abortCount, 1)
  }

  func testFoundationDiagnosticCannotBecomeAvailableProvider() throws {
    let channel = FoundationDiagnosticChannel()
    let endpoint = KagemushaSecureElementAPDUEndpointV1(channel: channel)
    XCTAssertThrowsError(
      try KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(endpoint)
    )
  }
}

private final class FoundationDiagnosticChannel: KagemushaSecureElementAPDUChannelV1 {
  func transceive(_: Data) throws -> Data {
    Data([0x4f, 0x44, 0x4a, 0x30, 1, 0, 0, 0, 0x90, 0])
  }

  func close() {}
}

private final class ScriptedAPDUChannel: KagemushaSecureElementAPDUChannelV1 {
  let response: Data
  let capabilities: Data
  var commands: [Data] = []
  var writeSizes: [Int] = []
  var readSizes: [Int] = []
  var writeIndexes: [Int] = []
  var readIndexes: [Int] = []
  var receivedCommand = Data()
  var corruptResponseDigest = false
  var abortCount = 0
  private var declaredLength = 0
  private var expectedCommandDigest = Data()

  init(
    response: Data,
    capabilities: Data = Data((1...96).map(UInt8.init))
  ) {
    self.response = response
    self.capabilities = capabilities
  }

  func transceive(_ command: Data) throws -> Data {
    commands.append(command)
    switch command[1] {
    case 0x11:
      return success(capabilities)
    case 0x12:
      let data = Data(command.dropFirst(5))
      declaredLength = Int(readUInt32LE(data))
      expectedCommandDigest = Data(data[4..<36])
      receivedCommand.removeAll(keepingCapacity: true)
      return success()
    case 0x13:
      writeIndexes.append(index(command))
      let data = Data(command.dropFirst(5))
      writeSizes.append(data.count)
      receivedCommand.append(data)
      return success()
    case 0x14:
      XCTAssertEqual(declaredLength, receivedCommand.count)
      XCTAssertEqual(expectedCommandDigest, sha256(receivedCommand))
      var digest = sha256(response)
      if corruptResponseDigest { digest[0] ^= 1 }
      var metadata = uint32LE(UInt32(response.count))
      metadata.append(digest)
      return success(metadata)
    case 0x15:
      let chunkIndex = index(command)
      readIndexes.append(chunkIndex)
      let encodedLength = Int(command[4])
      let count = encodedLength == 0 ? 256 : encodedLength
      let offset = chunkIndex * 224
      let upper = min(offset + count, response.count)
      let chunk = Data(response[offset..<upper])
      readSizes.append(chunk.count)
      return success(chunk)
    case 0x16:
      abortCount += 1
      return success()
    default:
      XCTFail("unexpected instruction")
      return Data([0x6d, 0x00])
    }
  }

  func close() {}

  private func index(_ command: Data) -> Int {
    Int(command[2]) << 8 | Int(command[3])
  }

  private func success(_ data: Data = Data()) -> Data {
    data + Data([0x90, 0])
  }

  private func sha256(_ data: Data) -> Data {
    Data(SHA256.hash(data: data))
  }

  private func readUInt32LE(_ data: Data) -> UInt32 {
    (0..<4).reduce(UInt32(0)) { value, index in
      value | UInt32(data[index]) << UInt32(index * 8)
    }
  }

  private func uint32LE(_ value: UInt32) -> Data {
    Data(
      (0..<4).map { index in
        UInt8(truncatingIfNeeded: value >> UInt32(index * 8))
      })
  }
}
