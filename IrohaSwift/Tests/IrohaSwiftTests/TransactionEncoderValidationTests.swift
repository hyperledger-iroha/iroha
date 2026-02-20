import Foundation
import XCTest
@testable import IrohaSwift

private func canonicalOwnerLiteral() throws -> String {
    let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
    let address = try AccountAddress.fromAccount(domain: AccountAddress.defaultDomainName,
                                                 publicKey: keypair.publicKey)
    let ih58 = try address.toIH58(networkPrefix: 0x02F1)
    return ih58
}

private func noncanonicalOwnerLiteral() throws -> String {
    let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
    let address = try AccountAddress.fromAccount(domain: AccountAddress.defaultDomainName,
                                                 publicKey: keypair.publicKey)
    let canonicalHex = try address.canonicalHex()
    return canonicalHex
}

@available(macOS 10.15, iOS 13.0, *)
private func canonicalAuthorityLiteral(from signingKey: SigningKey,
                                       domain: String = AccountAddress.defaultDomainName) throws -> String {
    let publicKey = try signingKey.publicKey()
    let address = try AccountAddress.fromAccount(domain: domain, publicKey: publicKey)
    let ih58 = try address.toIH58(networkPrefix: 0x02F1)
    return ih58
}

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

    func testMetadataTargetAcceptsShorthandAssetId() throws {
        let target = try TransactionInputValidator.sanitizeMetadataTarget(
            .asset("rose##alice@wonderland")
        )
        guard case let .asset(assetId) = target else {
            return XCTFail("expected asset target")
        }
        XCTAssertEqual(assetId, "rose#wonderland#alice@wonderland")
    }

    func testCastZkBallotRejectsIncompleteLockHints() throws {
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON(["owner": owner])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)

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
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
            "root_hint": "not-hex",
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)

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
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
            "root_hint_hex": "0x" + String(repeating: "Cc", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        )
    }

    func testCastZkBallotAcceptsCanonicalHints() throws {
        let owner = try canonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
            "root_hint": "0x" + String(repeating: "Cc", count: 32),
            "nullifier": "blake2b32:" + String(repeating: "DD", count: 32),
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)

        do {
            _ = try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                               signingKey: signingKey,
                                                               creationTimeMs: 1)
        } catch SwiftTransactionEncoderError.nativeBridgeError(.governance) {
            throw XCTSkip("governance encoder unavailable in linked native bridge")
        }
    }

    func testCastZkBallotRejectsNoncanonicalOwner() throws {
        let owner = try noncanonicalOwnerLiteral()
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 4, count: 32))
        let authority = try canonicalAuthorityLiteral(from: signingKey)
        let publicInputs = try NoritoJSON.fromJSONObject([
            "owner": owner,
            "amount": "1",
            "duration_blocks": 1,
        ])
        let request = CastZkBallotRequest(chainId: "chain",
                                          authority: authority,
                                          electionId: "election-1",
                                          proofB64: "AAAA",
                                          publicInputs: publicInputs,
                                          ttlMs: nil)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeCastZkBallot(request: request,
                                                           signingKey: signingKey,
                                                           creationTimeMs: 1)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .invalidZkBallotPublicInputs("owner must be a canonical account id"))
        }
    }
}
