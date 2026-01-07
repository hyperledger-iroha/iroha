import Foundation
import XCTest
@testable import IrohaSwift

final class TransactionEncoderValidationTests: XCTestCase {
    func testSetMetadataRejectsMalformedAuthority() throws {
        let value = try NoritoJSON(["profile": "demo"])
        let request = SetMetadataRequest(chainId: "chain",
                                         authority: "alice",
                                         target: .account("bob@wonderland"),
                                         key: "profile",
                                         value: value,
                                         ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 1, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeSetMetadata(request: request,
                                                          signingKey: signingKey,
                                                          creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice"))
        }
    }

    func testPersistCouncilRejectsInvalidMemberAccount() throws {
        let request = PersistCouncilRequest(chainId: "chain",
                                            authority: "alice@wonderland",
                                            epoch: 1,
                                            members: ["bob"],
                                            candidatesCount: 1,
                                            derivedBy: .vrf,
                                            ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 2, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodePersistCouncil(request: request,
                                                             signingKey: signingKey,
                                                             creationTimeMs: 10)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "members[0]", value: "bob"))
        }
    }

    func testRemoveMetadataRejectsMalformedAssetTarget() throws {
        let request = RemoveMetadataRequest(chainId: "chain",
                                            authority: "alice@wonderland",
                                            target: .asset("rose#wonderland"),
                                            key: "profile",
                                            ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 3, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeRemoveMetadata(request: request,
                                                             signingKey: signingKey,
                                                             creationTimeMs: 5)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("rose#wonderland"))
        }
    }

    func testCastZkBallotRejectsIncompleteLockHints() throws {
        let publicInputs = try NoritoJSON(["owner": "alice@wonderland"])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: "alice@wonderland",
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("lock hints must include owner, amount, duration_blocks"))
        }
    }

    func testCastZkBallotRejectsInvalidRootHintHex() throws {
        let publicInputs = try NoritoJSON([
            "owner": "alice@wonderland",
            "amount": "1",
            "duration_blocks": 1,
            "root_hint": "not-hex",
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: "alice@wonderland",
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("root_hint must be 32-byte hex"))
        }
    }

    func testCastZkBallotRejectsDeprecatedAliases() throws {
        let publicInputs = try NoritoJSON([
            "owner": "alice@wonderland",
            "amount": "1",
            "duration_blocks": 1,
            "root_hint_hex": "0x" + String(repeating: "Cc", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: "alice@wonderland",
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        )
    }

    func testCastZkBallotAcceptsCanonicalHints() throws {
        let publicInputs = try NoritoJSON([
            "owner": "alice@wonderland",
            "amount": "1",
            "duration_blocks": 1,
            "root_hint": "0x" + String(repeating: "Cc", count: 32),
            "nullifier": "blake2b32:" + String(repeating: "DD", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: "alice@wonderland",
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))

        XCTAssertNoThrow(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        )
    }
}
