import Foundation
import XCTest
@testable import IrohaSwift

final class SccpReplayV1Tests: XCTestCase {
    func testLocalFinalV1ReplayVectorRejectsMalleableWitnesses() throws {
        let domainHash = try SccpReplayV1.domainHash(
            source: .soraTaira,
            target: .ethereumMainnet,
            boundary: .evmDestinationMint,
            routeRevision: 7,
            routeConfigurationHash: Data(repeating: 0x44, count: 32),
            actor: try .evm(Data(repeating: 0x33, count: 20))
        )
        XCTAssertEqual(
            SccpV1.encodeLowerHex(domainHash),
            "ebc495541ef2265beebe7ee9e4e8764595c2a55ed67dc6d0a8ff69ccd3ff3228"
        )

        let replayID = Data(repeating: 0x11, count: 32)
        let key = try SccpReplayV1.replayKey(domainHash: domainHash, replayId: replayID)
        XCTAssertEqual(
            SccpV1.encodeLowerHex(key),
            "035bcebe9423edd4f1b945bae54905e0f0860bcc54718d372b1a58797ce614d4"
        )
        XCTAssertEqual(key.first, 3)

        var amount = Data(repeating: 0, count: 16)
        amount[15] = 9
        let recordDigest = try SccpReplayV1.recordDigest(
            operation: .evmDestinationMint,
            replayId: replayID,
            payloadSHA256: Data(repeating: 0x22, count: 32),
            amountScale9BE: amount,
            principal: try .evm(Data(repeating: 0x33, count: 20)),
            auxiliaryIdentitySHA256: Data(repeating: 0x55, count: 32)
        )
        XCTAssertEqual(
            SccpV1.encodeLowerHex(recordDigest),
            "bb0a7e99f5d2d136375e46ba231903611366ea85ec0e10130488a085fa05bf4f"
        )

        let empty = SccpReplayV1.emptyHashes()
        XCTAssertEqual(
            SccpV1.encodeLowerHex(empty[0]),
            "6841d062186b649a505eb694ebce936fe978c5530596882a70c6e04303c88d43"
        )
        XCTAssertEqual(
            SccpV1.encodeLowerHex(empty[SccpReplayV1.depth]),
            "cefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f"
        )

        let emptyWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: empty[SccpReplayV1.depth],
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: Data(repeating: 0, count: 32),
            siblings: []
        )
        let nonmembership = try SccpReplayV1.rootFromWitness(
            key: key,
            recordDigest: Data(repeating: 0, count: 32),
            witness: emptyWitness
        )
        XCTAssertTrue(nonmembership.matchesExpectedRoot)

        let occupiedExpected = try SccpV1.decodeLowerHex(
            "ec10fe878a6429557c7af279b8cb6fa5cc51165f4e6a54fb27ed6ad8525caf91"
        )
        let occupiedWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: occupiedExpected,
            priorRecordDigest: recordDigest,
            siblingBitmap: Data(repeating: 0, count: 32),
            siblings: []
        )
        let membership = try SccpReplayV1.rootFromWitness(
            key: key,
            recordDigest: recordDigest,
            witness: occupiedWitness
        )
        XCTAssertTrue(membership.matchesExpectedRoot)
        XCTAssertEqual(SccpV1.encodeLowerHex(membership.root), SccpV1.encodeLowerHex(occupiedExpected))

        var reservedBitmap = Data(repeating: 0, count: 32)
        reservedBitmap[0] = 1
        let reservedWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: empty[SccpReplayV1.depth],
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: reservedBitmap,
            siblings: [Data(repeating: 0xaa, count: 32)]
        )
        XCTAssertThrowsError(try SccpReplayV1.rootFromWitness(
            key: key,
            recordDigest: Data(repeating: 0, count: 32),
            witness: reservedWitness
        ))

        var explicitDefaultBitmap = Data(repeating: 0, count: 32)
        explicitDefaultBitmap[31] = 1
        let explicitDefaultWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: empty[SccpReplayV1.depth],
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: explicitDefaultBitmap,
            siblings: [empty[0]]
        )
        XCTAssertThrowsError(try SccpReplayV1.rootFromWitness(
            key: key,
            recordDigest: Data(repeating: 0, count: 32),
            witness: explicitDefaultWitness
        ))

        XCTAssertNoThrow(try SccpReplayV1.verifyAgainstCurrentRoot(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: emptyWitness,
            currentRoot: empty[SccpReplayV1.depth]
        ))
        let zeroExpectedWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: Data(repeating: 0, count: 32),
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: Data(repeating: 0, count: 32),
            siblings: []
        )
        XCTAssertFalse(try SccpReplayV1.rootFromWitness(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: zeroExpectedWitness
        ).matchesExpectedRoot)
        XCTAssertThrowsError(try SccpReplayV1.verifyAgainstCurrentRoot(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: zeroExpectedWitness,
            currentRoot: Data(repeating: 0, count: 32)
        ))
        var zeroSiblingBitmap = Data(repeating: 0, count: 32)
        zeroSiblingBitmap[31] = 1
        let zeroSiblingWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: empty[SccpReplayV1.depth],
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: zeroSiblingBitmap,
            siblings: [Data(repeating: 0, count: 32)]
        )
        let zeroSiblingRoot = try SccpReplayV1.rootFromWitness(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: zeroSiblingWitness
        ).root
        let boundZeroSiblingWitness = try SccpSparseMerkleWitnessV1(
            expectedShardRoot: zeroSiblingRoot,
            priorRecordDigest: Data(repeating: 0, count: 32),
            siblingBitmap: zeroSiblingBitmap,
            siblings: [Data(repeating: 0, count: 32)]
        )
        XCTAssertNoThrow(try SccpReplayV1.verifyAgainstCurrentRoot(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: boundZeroSiblingWitness,
            currentRoot: zeroSiblingRoot
        ))
        XCTAssertThrowsError(try SccpReplayV1.verifyAgainstCurrentRoot(
            key: Data(repeating: 0, count: 32),
            recordDigest: Data(repeating: 0, count: 32),
            witness: boundZeroSiblingWitness,
            currentRoot: Data(repeating: 0x77, count: 32)
        ))
    }

    func testReplayOperationPrincipalAndTonDirectionAreExact() throws {
        XCTAssertEqual(SccpReplayBoundaryV1.tonWalletBurnAuthorization.rawValue, 0x35)
        XCTAssertEqual(SccpReplayBoundaryV1.tonWalletBurnLock.rawValue, 0x36)
        XCTAssertEqual(SccpReplayBoundaryV1.tonWalletBurnRefund.rawValue, 0x37)
        let tonActor = SccpReplayActorV1.ton(
            workchain: 0,
            account: Data(repeating: 0x66, count: 32)
        )
        for boundary in [
            SccpReplayBoundaryV1.tonBridgeInboundMint,
            .tonMasterMint,
            .tonWalletMintCredit,
        ] {
            XCTAssertNoThrow(try SccpReplayV1.domainHash(
                source: .soraTaira,
                target: .tonMainnet,
                boundary: boundary,
                routeRevision: 7,
                routeConfigurationHash: Data(repeating: 0x44, count: 32),
                actor: tonActor
            ))
        }
        for boundary in [
            SccpReplayBoundaryV1.tonBridgeOutboundBurn,
            .tonMasterBurn,
            .tonWalletBurnAuthorization,
            .tonWalletBurnLock,
            .tonWalletBurnRefund,
        ] {
            XCTAssertNoThrow(try SccpReplayV1.domainHash(
                source: .tonMainnet,
                target: .soraTaira,
                boundary: boundary,
                routeRevision: 7,
                routeConfigurationHash: Data(repeating: 0x44, count: 32),
                actor: tonActor
            ))
            XCTAssertThrowsError(try SccpReplayV1.domainHash(
                source: .soraTaira,
                target: .tonMainnet,
                boundary: boundary,
                routeRevision: 7,
                routeConfigurationHash: Data(repeating: 0x44, count: 32),
                actor: tonActor
            ))
        }
        var amount = Data(repeating: 0, count: 16)
        amount[15] = 9
        XCTAssertThrowsError(try SccpReplayV1.recordDigest(
            operation: .soraOutboundLock,
            replayId: Data(repeating: 0x11, count: 32),
            payloadSHA256: Data(repeating: 0x22, count: 32),
            amountScale9BE: amount,
            principal: try .evm(Data(repeating: 0x33, count: 20)),
            auxiliaryIdentitySHA256: Data(repeating: 0x55, count: 32)
        ))
    }

}
