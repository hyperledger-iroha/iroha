import XCTest
@testable import IrohaSwift

final class ConnectEventsTests: XCTestCase {
    private let encodedUsdAssetID =
        "5ywNgSPQ5KyuQh7SwaZmwMW4GTXu"

    func testBalanceAssetRejectsFractionalPrecision() {
        let json: [String: Any] = [
            "asset_id": encodedUsdAssetID,
            "quantity": "1",
            "precision": 1.5
        ]

        XCTAssertThrowsError(try ConnectBalanceAsset(json: json)) { error in
            guard case ConnectEnvelopeError.invalidPayload = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
        }
    }

    func testBalanceSnapshotRejectsFractionalLastUpdated() {
        let asset: [String: Any] = [
            "asset_id": encodedUsdAssetID,
            "quantity": "1"
        ]
        let json: [String: Any] = [
            "account_id": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
            "assets": [asset],
            "last_updated_ms": 1.25
        ]

        XCTAssertThrowsError(try ConnectBalanceSnapshot(json: json)) { error in
            guard case ConnectEnvelopeError.invalidPayload = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
        }
    }
}
