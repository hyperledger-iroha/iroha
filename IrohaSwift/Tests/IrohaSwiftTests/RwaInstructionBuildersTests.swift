import XCTest
@testable import IrohaSwift

final class RwaInstructionBuildersTests: XCTestCase {
    private let fixtureRwaId =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities"
    private let fixtureRwaIdUpperHash =
        "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF$commodities"

    private func i105(seed: UInt8) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        return try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    func testRegisterRwaWrapsRawNewRwaObject() throws {
        let rwa = try NoritoJSON.fromJSONObject([
            "domain": "commodities",
            "quantity": "10.5",
            "spec": ["scale": 1],
            "primary_reference": "vault-cert-001",
            "metadata": ["origin": "AE"],
            "parents": [],
            "controls": ["freeze_enabled": true],
        ])
        let payload = try RwaInstructionBuilders.registerRwa(rwa: rwa)
        let object = try JSONSerialization.jsonObject(with: payload.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["RegisterRwa"] as? [String: Any],
            let innerRwa = inner["rwa"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(innerRwa["domain"] as? String, "commodities")
        XCTAssertEqual(innerRwa["quantity"] as? String, "10.5")
        XCTAssertEqual(innerRwa["primary_reference"] as? String, "vault-cert-001")
    }

    func testTransferRwaCanonicalizesAccountsAndRwaId() throws {
        let source = try i105(seed: 1)
        let destination = try i105(seed: 2)
        let payload = try RwaInstructionBuilders.transferRwa(
            sourceAccountId: source,
            rwaId: fixtureRwaIdUpperHash,
            quantity: "3.25",
            destinationAccountId: destination
        )
        let object = try JSONSerialization.jsonObject(with: payload.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["TransferRwa"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(inner["source"] as? String, source)
        XCTAssertEqual(inner["rwa"] as? String, fixtureRwaId)
        XCTAssertEqual(inner["quantity"] as? String, "3.25")
        XCTAssertEqual(inner["destination"] as? String, destination)
    }

    func testMergeRwasWrapsRawMergeObject() throws {
        let merge = try NoritoJSON.fromJSONObject([
            "parents": [
                ["rwa": fixtureRwaId, "quantity": "1.5"],
            ],
            "primary_reference": "blend-cert-007",
            "status": "blended",
            "metadata": ["grade": "A"],
        ])
        let payload = try RwaInstructionBuilders.mergeRwas(merge: merge)
        let object = try JSONSerialization.jsonObject(with: payload.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["MergeRwas"] as? [String: Any],
            let parents = inner["parents"] as? [[String: Any]]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(parents.first?["rwa"] as? String, fixtureRwaId)
        XCTAssertEqual(inner["primary_reference"] as? String, "blend-cert-007")
        XCTAssertEqual(inner["status"] as? String, "blended")
    }

    func testSetRwaControlsWrapsRawPolicyObject() throws {
        let controls = try NoritoJSON.fromJSONObject([
            "controller_accounts": [try i105(seed: 3)],
            "freeze_enabled": true,
            "hold_enabled": true,
            "force_transfer_enabled": false,
            "redeem_enabled": true,
        ])
        let payload = try RwaInstructionBuilders.setRwaControls(rwaId: fixtureRwaId, controls: controls)
        let object = try JSONSerialization.jsonObject(with: payload.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["SetRwaControls"] as? [String: Any],
            let innerControls = inner["controls"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(inner["rwa"] as? String, fixtureRwaId)
        XCTAssertEqual(innerControls["freeze_enabled"] as? Bool, true)
        XCTAssertEqual(innerControls["hold_enabled"] as? Bool, true)
        XCTAssertEqual(innerControls["redeem_enabled"] as? Bool, true)
    }

    func testScalarRwaOperationsRoundTripShapes() throws {
        let destination = try i105(seed: 4)
        let redeem = try RwaInstructionBuilders.redeemRwa(rwaId: fixtureRwaId, quantity: "2")
        let freeze = try RwaInstructionBuilders.freezeRwa(rwaId: fixtureRwaId)
        let unfreeze = try RwaInstructionBuilders.unfreezeRwa(rwaId: fixtureRwaId)
        let hold = try RwaInstructionBuilders.holdRwa(rwaId: fixtureRwaId, quantity: "3")
        let release = try RwaInstructionBuilders.releaseRwa(rwaId: fixtureRwaId, quantity: "1")
        let force = try RwaInstructionBuilders.forceTransferRwa(
            rwaId: fixtureRwaId,
            quantity: "4",
            destinationAccountId: destination
        )

        let redeemObject = try JSONSerialization.jsonObject(with: redeem.data, options: []) as? [String: Any]
        let holdObject = try JSONSerialization.jsonObject(with: hold.data, options: []) as? [String: Any]
        let forceObject = try JSONSerialization.jsonObject(with: force.data, options: []) as? [String: Any]

        XCTAssertEqual((redeemObject?["RedeemRwa"] as? [String: Any])?["quantity"] as? String, "2")
        XCTAssertEqual((holdObject?["HoldRwa"] as? [String: Any])?["quantity"] as? String, "3")
        XCTAssertEqual((forceObject?["ForceTransferRwa"] as? [String: Any])?["destination"] as? String, destination)

        XCTAssertNotNil(try JSONSerialization.jsonObject(with: freeze.data, options: []))
        XCTAssertNotNil(try JSONSerialization.jsonObject(with: unfreeze.data, options: []))
        XCTAssertNotNil(try JSONSerialization.jsonObject(with: release.data, options: []))
    }

    func testRejectsInvalidRwaIdAndQuantity() {
        XCTAssertThrowsError(try RwaInstructionBuilders.freezeRwa(rwaId: "bad-rwa")) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedRwaId(field: "rwa", value: "bad-rwa"))
        }

        XCTAssertThrowsError(try RwaInstructionBuilders.holdRwa(rwaId: fixtureRwaId, quantity: "  ")) { error in
            XCTAssertEqual(error as? RwaInstructionBuilderError,
                           .invalidQuantity(field: "quantity"))
        }
    }

    func testIrohaSDKConvenienceHelpersProduceSamePayloads() throws {
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let source = try i105(seed: 5)
        let destination = try i105(seed: 6)
        let controls = try NoritoJSON.fromJSONObject(["freeze_enabled": true])

        let transferSDK = try sdk.buildTransferRwa(sourceAccountId: source,
                                                   rwaId: fixtureRwaId,
                                                   quantity: "1",
                                                   destinationAccountId: destination)
        let transferDirect = try RwaInstructionBuilders.transferRwa(sourceAccountId: source,
                                                                    rwaId: fixtureRwaId,
                                                                    quantity: "1",
                                                                    destinationAccountId: destination)
        XCTAssertEqual(transferSDK.data, transferDirect.data)

        let controlsSDK = try sdk.buildSetRwaControls(rwaId: fixtureRwaId, controls: controls)
        let controlsDirect = try RwaInstructionBuilders.setRwaControls(rwaId: fixtureRwaId, controls: controls)
        XCTAssertEqual(controlsSDK.data, controlsDirect.data)
    }
}
