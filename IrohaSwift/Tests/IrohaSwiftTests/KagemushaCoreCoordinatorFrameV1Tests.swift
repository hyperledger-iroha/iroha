import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCoreCoordinatorFrameV1Tests: XCTestCase {
  func testAllNativeMethodsMatchSharedCurrentSchemaVectors() throws {
    let cases = try fixtures()
    XCTAssertEqual(Set(cases.map { $0.method.rawValue }), Set(UInt8(1)...UInt8(10)))
    XCTAssertEqual(cases.count, 14)
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
      invalidLength.replaceSubrange(16..<20, with: Data(repeating: 255, count: 4))
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
      "recover-sender": 0, "recover-terminal": 1, "release-send": 3, "release-redeem": 3]
    for item in try fixtures() {
      let request = try KagemushaCoreCoordinatorFrameV1.decodeRequest(item.method, frame: item.request)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(item.method, fields: Array(request.dropLast())))
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(item.method, fields: request + [Data([1])]))
      var response = try KagemushaCoreCoordinatorFrameV1.decodeResponse(item.method, requestFrame: item.request, responseFrame: item.response)
      XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(item.method, requestFrame: item.request, fields: response + [Data([1])]))
      if let index = indexes[item.name] {
        response[index][0] = 0x7f
        XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeResponse(item.method, requestFrame: item.request, fields: response), item.name)
      }
    }
  }

  func testOversizedFieldsAndFramesFailClosed() throws {
    let id = Data(repeating: 7, count: 32)
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.reserveOperationID, fields: [KagemushaCoreCoordinatorFrameV1.u32(22), id, Data(repeating: 0, count: 65537)]))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.encodeRequest(.acceptInstalledTerminal, fields: Array(repeating: Data(repeating: 0, count: 65536), count: 5)))
    XCTAssertThrowsError(try KagemushaCoreCoordinatorFrameV1.decodeRequest(.reserveOperationID, frame: Data(repeating: 0, count: 262145)))
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
