import CryptoKit
import XCTest
@testable import IrohaSwift

/// Canonical-account admission coverage for the expensive receiver-lineage proof route.
final class ToriiKagemushaReceiverLineageAuthTests: XCTestCase {
    private let seed = Data(repeating: 0x52, count: 32)
    private let timestampMs: UInt64 = 4_102_444_801_000
    private let nonce = "swift-offline-lineage-auth"

    override func tearDown() {
        StubURLProtocol.handler = nil
        super.tearDown()
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testReceiverLineageBindsExactNetworkTargetAndNoritoBody() async throws {
        #if canImport(Darwin)
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 bridge is not linked in this test host"
        )
        let payment = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let query = try KagemushaRecipientLineageQueryV2(
            networkID: payment.payload.networkID,
            recipient: payment.payload.recipient,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
            receiverDeviceID: payment.payload.receiverDeviceID,
            assetDefinitionID: payment.payload.assetDefinitionID,
            trustedCheckpointHeight: 1
        )
        let accountId = try Keypair(privateKeyBytes: seed)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
        let auth = ToriiCanonicalRequestAuth(
            accountId: accountId,
            privateKey: seed,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let responseBody = Data([0x4e, 0x52, 0x54, 0x31])
        var captured: URLRequest?
        StubURLProtocol.handler = { request in
            captured = request
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/x-norito"]
            )!
            return (response, responseBody)
        }
        let client = ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: session,
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )

        let received = try await client.getKagemushaRecipientRegistrationLineage(
            query: query,
            canonicalAuth: auth
        )
        XCTAssertEqual(received, responseBody)
        let request = try XCTUnwrap(captured)
        let requestURL = try XCTUnwrap(request.url)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(requestURL.path, "/v1/offline/receiver-lineage")
        XCTAssertEqual(toriiClientTestBodyData(from: request), query.noritoArchive)
        XCTAssertEqual(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
            try AccountAddress.parseEncoded(accountId).canonicalHex()
        )
        let signature = try XCTUnwrap(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
        )
        let signatureData = try XCTUnwrap(Data(base64Encoded: signature))
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        func verifies(
            networkId: NetworkId,
            method: String,
            url: URL,
            body: Data
        ) throws -> Bool {
            let message = try ToriiCanonicalRequest.signatureMessage(
                networkId: networkId,
                method: method,
                url: url,
                body: body,
                timestampMs: timestampMs,
                nonce: nonce
            )
            return publicKey.isValidSignature(signatureData, for: message)
        }
        XCTAssertTrue(try verifies(
            networkId: TestNetworkIds.canonical,
            method: "POST",
            url: requestURL,
            body: query.noritoArchive
        ))
        XCTAssertFalse(try verifies(
            networkId: TestNetworkIds.other,
            method: "POST",
            url: requestURL,
            body: query.noritoArchive
        ))
        XCTAssertFalse(try verifies(
            networkId: TestNetworkIds.canonical,
            method: "GET",
            url: requestURL,
            body: query.noritoArchive
        ))
        XCTAssertFalse(try verifies(
            networkId: TestNetworkIds.canonical,
            method: "POST",
            url: URL(string: "https://torii.example/v1/offline/readiness")!,
            body: query.noritoArchive
        ))
        XCTAssertFalse(try verifies(
            networkId: TestNetworkIds.canonical,
            method: "POST",
            url: requestURL,
            body: query.noritoArchive + Data([0])
        ))
        #endif
    }
}
