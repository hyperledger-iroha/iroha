import XCTest
@testable import IrohaSwift

final class TransactionInputValidatorTests: XCTestCase {
    func testValidateRejectsEmptyChainId() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "   ",
                                                   authorityId: "alice@wonderland",
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

    func testValidateRejectsMalformedAssetDefinition() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice@wonderland",
                                                   assetDefinitionId: "rose")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetDefinitionId("rose"))
        }
    }

    func testValidateRejectsAssetDefinitionWithReservedCharacters() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice@wonderland",
                                                   assetDefinitionId: "rose$#wonderland")
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetDefinitionId("rose$#wonderland"))
        }
    }

    func testValidateTrimsWhitespace() throws {
        let ids = try TransactionInputValidator.validate(
            chainId: " 0000 ",
            authorityId: " alice@wonderland ",
            assetDefinitionId: " rose#wonderland ",
            accountIds: [.init(field: "destination", value: " bob@wonderland ")]
        )
        XCTAssertEqual(ids.chainId, "0000")
        XCTAssertEqual(ids.authorityId, "alice@wonderland")
        XCTAssertEqual(ids.assetDefinitionId, "rose#wonderland")
        XCTAssertEqual(ids.accountIds["destination"], "bob@wonderland")
    }

    func testSanitizeMetadataTargetRejectsMalformedAssetId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("rose#wonderland"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("rose#wonderland"))
        }
    }

    func testSanitizeMetadataTargetRejectsAssetNameWithReservedCharacters() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("ro$se#wonderland#alice@wonderland"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("ro$se#wonderland#alice@wonderland"))
        }
    }

    func testSanitizeMetadataTargetTrimsAccountAndDomainIds() throws {
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.account("  alice@wonderland  "))
        XCTAssertEqual(target.objectId, "alice@wonderland")

        let domainTarget = try TransactionInputValidator.sanitizeMetadataTarget(.domain("  wonderland  "))
        XCTAssertEqual(domainTarget.objectId, "wonderland")
    }

    func testSanitizeAssetIdRejectsSharedDomainShorthand() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("xor##alice@wonderland"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("xor##alice@wonderland"))
        }
    }

    func testValidateAcceptsIh58Authority() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let ih58 = try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: ih58)
        XCTAssertEqual(ids.authorityId, ih58)
    }

    func testValidateAcceptsUaidAuthority() throws {
        let uaidHex = String(repeating: "0", count: 63) + "f"
        let literal = "uaid:\(uaidHex)"
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: literal)
        XCTAssertEqual(ids.authorityId, literal)
    }
}
