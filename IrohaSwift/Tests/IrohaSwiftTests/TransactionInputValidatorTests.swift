import XCTest
@testable import IrohaSwift

final class TransactionInputValidatorTests: XCTestCase {
    private func ih58(seed: UInt8 = 1,
                      domain: String = AccountAddress.defaultDomainName) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        let address = try AccountAddress.fromAccount(domain: domain, publicKey: keypair.publicKey)
        return try address.toIH58(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    func testValidateRejectsEmptyChainId() throws {
        let authority = try ih58(seed: 1)
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "   ",
                                                   authorityId: authority,
                                                   assetDefinitionId: "rose#wonderland")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
    }

    func testValidateRejectsMalformedAuthority() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice",
                                                   assetDefinitionId: "rose#wonderland")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice"))
        }
    }

    func testValidateRejectsAuthorityWithReservedCharacters() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice#bad@wonderland",
                                                   assetDefinitionId: "rose#wonderland")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice#bad@wonderland"))
        }
    }

    func testValidateRejectsMalformedAssetDefinition() throws {
        let authority = try ih58(seed: 2)
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: authority,
                                                   assetDefinitionId: "rose")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetDefinitionId("rose"))
        }
    }

    func testValidateRejectsAssetDefinitionWithReservedCharacters() throws {
        let authority = try ih58(seed: 3)
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: authority,
                                                   assetDefinitionId: "rose$#wonderland")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetDefinitionId("rose$#wonderland"))
        }
    }

    func testValidateTrimsWhitespace() throws {
        let authority = try ih58(seed: 4)
        let destination = try ih58(seed: 5)
        let ids = try TransactionInputValidator.validate(
            chainId: " 0000 ",
            authorityId: " \(authority) ",
            assetDefinitionId: " rose#wonderland ",
            accountIds: [.init(field: "destination", value: " \(destination) ")]
        )
        XCTAssertEqual(ids.chainId, "0000")
        XCTAssertEqual(ids.authorityId, authority)
        XCTAssertEqual(ids.assetDefinitionId, "rose#wonderland")
        XCTAssertEqual(ids.accountIds["destination"], destination)
    }

    func testSanitizeMetadataTargetRejectsMalformedAssetId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("rose#wonderland"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("rose#wonderland"))
        }
    }

    func testSanitizeMetadataTargetRejectsTextualAssetId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("ro$se#wonderland#alice@wonderland"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("ro$se#wonderland#alice@wonderland"))
        }
    }

    func testSanitizeMetadataTargetTrimsAccountAndDomainIds() throws {
        let authority = try ih58(seed: 6)
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.account("  \(authority)  "))
        XCTAssertEqual(target.objectId, authority)

        let domainTarget = try TransactionInputValidator.sanitizeMetadataTarget(.domain("  wonderland  "))
        XCTAssertEqual(domainTarget.objectId, "wonderland")
    }

    func testSanitizeAssetIdAcceptsNoritoHexLiteral() throws {
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.asset("norito:0A0B"))
        XCTAssertEqual(target.objectId, "norito:0a0b")
    }

    func testValidateAcceptsIh58Authority() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let ih58 = try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: ih58)
        XCTAssertEqual(ids.authorityId, ih58)
    }

    func testValidateAcceptsCompressedAuthorityAndCanonicalizesToIh58() throws {
        let address = try AccountAddress.fromAccount(domain: "wonderland",
                                                     publicKey: Data(repeating: 0xAD, count: 32))
        let compressed = try address.toCompressedSora()
        let ih58 = try address.toIH58(networkPrefix: AccountId.defaultNetworkPrefix)
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: compressed)
        XCTAssertEqual(ids.authorityId, ih58)
    }

    func testValidateRejectsIh58WithDomainSuffix() throws {
        let publicKey = Data(repeating: 0xAC, count: 32)
        let ih58 = try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")
        let literal = "\(ih58)@wonderland"
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: literal)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: literal))
        }
    }

    func testValidateRejectsUaidAuthority() throws {
        let uaidHex = String(repeating: "0", count: 63) + "f"
        let literal = "uaid:\(uaidHex)"
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: literal)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: literal))
        }
    }
}
