import Foundation
import XCTest
@testable import IrohaSwift

final class OfflinePetalStreamTests: XCTestCase {
    func testPetalStreamRoundTrip() throws {
        let payload = makePayload(length: 128)
        let grid = try OfflinePetalStreamEncoder.encodeGrid(payload)
        let decoded = try OfflinePetalStreamDecoder.decodeGrid(grid)
        XCTAssertEqual(decoded, payload)
    }

    func testPetalStreamSampleRoundTrip() throws {
        let payload = makePayload(length: 200)
        let grid = try OfflinePetalStreamEncoder.encodeGrid(payload)
        let image = renderGrid(grid: grid, cellSize: 4)
        let samples = try OfflinePetalStreamSampler.sampleGridFromRGBA(
            rgba: image.data,
            width: image.width,
            height: image.height,
            gridSize: grid.gridSize
        )
        let decoded = try OfflinePetalStreamDecoder.decodeSamples(samples)
        XCTAssertEqual(decoded, payload)
    }

    func testPetalStreamEncodeGridsUsesMaxPayload() throws {
        let small = makePayload(length: 16)
        let large = makePayload(length: 220)
        let result = try OfflinePetalStreamEncoder.encodeGrids([small, large])
        XCTAssertEqual(result.grids.count, 2)
        XCTAssertEqual(result.gridSize, result.grids[0].gridSize)
        XCTAssertEqual(result.gridSize, result.grids[1].gridSize)
    }

    private func makePayload(length: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: length)
        for index in bytes.indices {
            bytes[index] = UInt8((index * 17 + 13) % 256)
        }
        return Data(bytes)
    }

    private func renderGrid(grid: OfflinePetalStreamGrid, cellSize: Int) -> (data: Data, width: Int, height: Int) {
        let size = Int(grid.gridSize) * cellSize
        var data = [UInt8](repeating: 0, count: size * size * 4)
        for y in 0..<Int(grid.gridSize) {
            for x in 0..<Int(grid.gridSize) {
                let idx = y * Int(grid.gridSize) + x
                let value: UInt8 = grid.cells[idx] ? 0 : 255
                for oy in 0..<cellSize {
                    for ox in 0..<cellSize {
                        let px = x * cellSize + ox
                        let py = y * cellSize + oy
                        let offset = (py * size + px) * 4
                        data[offset] = value
                        data[offset + 1] = value
                        data[offset + 2] = value
                        data[offset + 3] = 255
                    }
                }
            }
        }
        return (Data(data), size, size)
    }
}
