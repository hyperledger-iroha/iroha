import Foundation
import XCTest
@testable import IrohaSwift

/// Durable type names preserve nominal empty products without accepting malformed declarations.
final class KotodamaStateTypesTests: XCTestCase {
    private func decode(_ typeName: String) throws -> ToriiContractManifest {
        let payload = #"{"states":[{"name":"Stored","type_name":"\#(typeName)"}]}"#
        return try JSONDecoder().decode(ToriiContractManifest.self, from: Data(payload.utf8))
    }

    func testEmptyProductsKeepTheirNominalSpellingInDurableSchemas() throws {
        for typeName in [
            "Empty{}", "Other{}", "Transfer{}", "List<Empty{}, 2>", "List<List<Empty{}, 2>, 2>",
            "Envelope{empty: Empty{}}", "StateMap<int, Empty{}>",
            "std/math@1.0.0::Math::Empty{}",
        ] {
            let manifest = try decode(typeName)
            XCTAssertEqual(manifest.states?.first?.typeName, typeName)
            let encoded = try JSONEncoder().encode(manifest)
            let roundtrip = try JSONDecoder().decode(ToriiContractManifest.self, from: encoded)
            XCTAssertEqual(roundtrip.states?.first?.typeName, typeName)
        }
    }

    func testMalformedEmptyProductsAndReservedShapesAreRejected() {
        for typeName in [
            "{}", "Empty{", "Empty{ }", "Empty{,}", "Empty{: int}",
            "Empty{field: int, }", "Empty{}trailing", "List<Empty{},2>",
            "List<Empty{}, 0>", "Envelope{empty: Empty{}, empty: Empty{}}",
            "StatePage{}", "Option{}", "int{}",
        ] {
            XCTAssertThrowsError(try decode(typeName), typeName)
        }
    }
}
