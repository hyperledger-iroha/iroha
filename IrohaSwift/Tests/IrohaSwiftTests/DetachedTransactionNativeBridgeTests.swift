import Foundation
import XCTest
@testable import IrohaSwift

final class DetachedTransactionNativeBridgeTests: XCTestCase {
    private let hashA = String(repeating: "11", count: 32)
    private let hashB = String(repeating: "22", count: 32)
    private let hashC = String(repeating: "33", count: 32)

    func testContractInspectionDecodesTypedBindingsAndLosslessMetadataIntegers() throws {
        let json = Data(
            """
            {
              "schema":"iroha.detached_transaction_scaffold.v1",
              "payload_signing_hash_hex":"\(hashA)",
              "authority":"sorau-authority",
              "chain":"chain",
              "creation_time_ms":18446744073709551615,
              "time_to_live_ms":60000,
              "metadata":{"signed":-9223372036854775808,"unsigned":18446744073709551615,"nested":{"ok":true}},
              "entrypoint_hash_hex":"\(hashB)",
              "executable":{"kind":"contract_call","contract_address":"sorac1contract","entrypoint":"pay","arguments_b64":"AQID"}
            }
            """.utf8
        )
        let inspection = try DetachedTransactionBridgeJSONCodec.decodeInspection(json)
        XCTAssertEqual(inspection.payloadSigningHash, Data(repeating: 0x11, count: 32))
        XCTAssertEqual(inspection.entrypointHash, Data(repeating: 0x22, count: 32))
        XCTAssertEqual(inspection.creationTimeMs, UInt64.max)
        XCTAssertEqual(inspection.metadata["signed"], .signedInteger(Int64.min))
        XCTAssertEqual(inspection.metadata["unsigned"], .unsignedInteger(UInt64.max))
        guard case let .contractCall(call) = inspection.executable else {
            return XCTFail("expected contract call")
        }
        XCTAssertEqual(call.contractAddress, "sorac1contract")
        XCTAssertEqual(call.entrypoint, "pay")
        XCTAssertEqual(call.arguments, Data([1, 2, 3]))
    }

    func testAssetTransferInspectionDecodesGlobalAndDataspaceScopes() throws {
        for (scopeJSON, expectedScope) in [
            ("{\"kind\":\"global\"}", DetachedAssetScopeInspection.global),
            ("{\"kind\":\"dataspace\",\"dataspace_id\":42}", .dataspace(42))
        ] {
            let json = Data(
                """
                {"schema":"iroha.detached_transaction_scaffold.v1","payload_signing_hash_hex":"\(hashA)","authority":"source","chain":"chain","creation_time_ms":1,"time_to_live_ms":null,"metadata":{},"entrypoint_hash_hex":"\(hashB)","executable":{"kind":"asset_transfer","asset_definition_id":"asset","asset_scope":\(scopeJSON),"source_asset_id":"asset#source","source_account_id":"source","destination_account_id":"destination","amount":"1.25"}}
                """.utf8
            )
            let inspection = try DetachedTransactionBridgeJSONCodec.decodeInspection(json)
            guard case let .assetTransfer(transfer) = inspection.executable else {
                return XCTFail("expected asset transfer")
            }
            XCTAssertEqual(transfer.assetScope, expectedScope)
            XCTAssertEqual(transfer.amount, "1.25")
        }
    }

    func testInspectionRejectsSchemaHashBase64AndScopeSubstitution() {
        let valid = """
        {"schema":"iroha.detached_transaction_scaffold.v1","payload_signing_hash_hex":"\(hashA)","authority":"a","chain":"c","creation_time_ms":1,"time_to_live_ms":null,"metadata":{},"entrypoint_hash_hex":"\(hashB)","executable":{"kind":"contract_call","contract_address":"x","entrypoint":"y","arguments_b64":null}}
        """
        let hostile = [
            valid.replacingOccurrences(of: "iroha.detached_transaction_scaffold.v1", with: "other"),
            valid.replacingOccurrences(of: hashA, with: String(repeating: "AA", count: 32)),
            valid.replacingOccurrences(of: hashB, with: "22"),
            valid.replacingOccurrences(of: "\"arguments_b64\":null", with: "\"arguments_b64\":\"AQI\""),
            valid.replacingOccurrences(of: "\"kind\":\"contract_call\"", with: "\"kind\":\"ivm\""),
            valid.replacingOccurrences(of: "\"schema\":", with: "\"future\":true,\"schema\":"),
            valid.replacingOccurrences(
                of: "\"schema\":\"iroha.detached_transaction_scaffold.v1\"",
                with: "\"schema\":\"other\",\"schema\":\"iroha.detached_transaction_scaffold.v1\""
            )
        ]
        for candidate in hostile {
            XCTAssertThrowsError(
                try DetachedTransactionBridgeJSONCodec.decodeInspection(Data(candidate.utf8))
            )
        }

        let impossibleScope = valid
            .replacingOccurrences(
                of: "{\"kind\":\"contract_call\",\"contract_address\":\"x\",\"entrypoint\":\"y\",\"arguments_b64\":null}",
                with: "{\"kind\":\"asset_transfer\",\"asset_definition_id\":\"d\",\"asset_scope\":{\"kind\":\"global\",\"dataspace_id\":1},\"source_asset_id\":\"s\",\"source_account_id\":\"a\",\"destination_account_id\":\"b\",\"amount\":\"1\"}"
            )
        XCTAssertThrowsError(
            try DetachedTransactionBridgeJSONCodec.decodeInspection(Data(impossibleScope.utf8))
        )
    }

    func testFinalizationRequiresExactSchemaAndLowercaseHashes() throws {
        let valid = Data(
            """
            {"schema":"iroha.detached_transaction_finalization.v1","payload_signing_hash_hex":"\(hashA)","transaction_hash_hex":"\(hashB)","entrypoint_hash_hex":"\(hashC)"}
            """.utf8
        )
        let finalization = try DetachedTransactionBridgeJSONCodec.decodeFinalization(valid)
        XCTAssertEqual(finalization.payloadSigningHash, Data(repeating: 0x11, count: 32))
        XCTAssertEqual(finalization.transactionHash, Data(repeating: 0x22, count: 32))
        XCTAssertEqual(finalization.entrypointHash, Data(repeating: 0x33, count: 32))

        for candidate in [
            String(decoding: valid, as: UTF8.self).replacingOccurrences(
                of: "iroha.detached_transaction_finalization.v1",
                with: "v0"
            ),
            String(decoding: valid, as: UTF8.self).replacingOccurrences(of: hashA, with: hashA.uppercased()),
            String(decoding: valid, as: UTF8.self).replacingOccurrences(
                of: hashC,
                with: "zz" + String(hashC.dropFirst(2))
            ),
            String(decoding: valid, as: UTF8.self).replacingOccurrences(
                of: "\"schema\":",
                with: "\"future\":true,\"schema\":"
            )
        ] {
            XCTAssertThrowsError(
                try DetachedTransactionBridgeJSONCodec.decodeFinalization(Data(candidate.utf8))
            )
        }
    }

    func testBridgeStatusMapsNewFailClosedErrors() {
        XCTAssertEqual(NativeBridgeError.fromStatus(-501), .detachedTransactionScaffold)
        XCTAssertEqual(NativeBridgeError.fromStatus(-502), .detachedTransactionSignature)
        XCTAssertEqual(NativeBridgeError.fromStatus(-503), .canonicalJSON)
    }

    func testPublicModelsAreSendable() {
        func requireSendable<T: Sendable>(_: T.Type) {}
        requireSendable(DetachedTransactionScaffoldInspection.self)
        requireSendable(DetachedTransactionFinalizationResult.self)
        requireSendable(CanonicalJSONBlake3Result.self)
        requireSendable(NativeBridgeJSONValue.self)
    }
}
