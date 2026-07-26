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

    func testBalanceAssetRejectsMalformedPresentDefinition() {
        let json: [String: Any] = [
            "asset_id": encodedUsdAssetID,
            "asset_definition_id": 7,
            "quantity": "1",
        ]

        XCTAssertThrowsError(try ConnectBalanceAsset(json: json)) { error in
            guard case ConnectEnvelopeError.invalidPayload = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
        }
    }

    func testBalanceAssetRejectsNoncanonicalQuantity() {
        for quantity in ["-1", "+1", "01", "1.0", " 1"] {
            let json: [String: Any] = [
                "asset_id": encodedUsdAssetID,
                "quantity": quantity
            ]

            XCTAssertThrowsError(try ConnectBalanceAsset(json: json), quantity) { error in
                guard case ConnectEnvelopeError.invalidPayload = error else {
                    XCTFail("Expected invalidPayload for \(quantity), got \(error)")
                    return
                }
            }
        }
    }

    func testBalanceAssetCodableRoundTripsCanonicalQuantity() throws {
        let data = Data(
            #"{"asset_id":"5ywNgSPQ5KyuQh7SwaZmwMW4GTXu","quantity":"1.25","precision":2}"#.utf8
        )

        let decoded = try JSONDecoder().decode(ConnectBalanceAsset.self, from: data)

        XCTAssertEqual(decoded.assetId, encodedUsdAssetID)
        XCTAssertEqual(decoded.quantity, "1.25")
        XCTAssertEqual(decoded.precision, 2)
        let roundTripped = try JSONDecoder().decode(
            ConnectBalanceAsset.self,
            from: JSONEncoder().encode(decoded)
        )
        XCTAssertEqual(roundTripped, decoded)
    }

    func testBalanceAssetCodableRejectsLossyAndNoncanonicalQuantities() {
        let overflow = String(repeating: "9", count: 155)
        let scale29 = "0." + String(repeating: "0", count: 28) + "1"
        let quantityFragments = [
            "1",
            "true",
            #""+1""#,
            #""01""#,
            #""1.0""#,
            #""1.""#,
            #""1e0""#,
            #"" 1""#,
            #""1 ""#,
            #""-0""#,
            #""-1""#,
            "\"\(scale29)\"",
            "\"\(overflow)\""
        ]

        for fragment in quantityFragments {
            let data = Data(
                "{\"asset_id\":\"\(encodedUsdAssetID)\",\"quantity\":\(fragment)}".utf8
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ConnectBalanceAsset.self, from: data),
                fragment
            )
        }
    }

    func testBalanceAssetCodableRejectsInvalidValuesWhenEncoding() {
        for quantity in ["-1", "+1", "01", "1.0", "1e0", " 1"] {
            let asset = ConnectBalanceAsset(assetId: encodedUsdAssetID, quantity: quantity)
            XCTAssertThrowsError(try JSONEncoder().encode(asset), quantity)
        }

        let invalidPrecision = ConnectBalanceAsset(
            assetId: encodedUsdAssetID,
            quantity: "1",
            precision: -1
        )
        XCTAssertThrowsError(try JSONEncoder().encode(invalidPrecision))
    }

    func testBalanceSnapshotRejectsFractionalLastUpdated() {
        let asset: [String: Any] = [
            "asset_id": encodedUsdAssetID,
            "quantity": "1"
        ]
        let json: [String: Any] = [
            "account_id": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
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
