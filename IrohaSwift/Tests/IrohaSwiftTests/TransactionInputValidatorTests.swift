import XCTest
@testable import IrohaSwift

final class TransactionInputValidatorTests: XCTestCase {
    private let sampleAid = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"

    private func i105(seed: UInt8 = 1,
                      domain: String = AccountAddress.defaultDomainName) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        let address = try AccountAddress.fromAccount(publicKey: keypair.publicKey)
        return try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    func testValidateRejectsEmptyChainId() throws {
        let authority = try i105(seed: 1)
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "   ",
                                                   authorityId: authority,
                                                   assetDefinitionId: sampleAid)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
    }

    func testValidateRejectsMalformedAuthority() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice",
                                                   assetDefinitionId: sampleAid)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice"))
        }
    }

    func testValidateRejectsAuthorityWithReservedCharacters() {
        XCTAssertThrowsError(
            try TransactionInputValidator.validate(chainId: "0000",
                                                   authorityId: "alice#bad@hbl.sbp",
                                                   assetDefinitionId: sampleAid)
        ) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAccountId(field: "authority", value: "alice#bad@hbl.sbp"))
        }
    }

    func testValidateRejectsMalformedAssetDefinition() throws {
        let authority = try i105(seed: 2)
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
        let authority = try i105(seed: 3)
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
        let authority = try i105(seed: 4)
        let destination = try i105(seed: 5)
        let ids = try TransactionInputValidator.validate(
            chainId: " 0000 ",
            authorityId: " \(authority) ",
            assetDefinitionId: " \(sampleAid) ",
            accountIds: [.init(field: "destination", value: " \(destination) ")]
        )
        XCTAssertEqual(ids.chainId, "0000")
        XCTAssertEqual(ids.authorityId, authority)
        XCTAssertEqual(ids.assetDefinitionId, sampleAid)
        XCTAssertEqual(ids.accountIds["destination"], destination)
    }

    func testSanitizeMetadataTargetRejectsMalformedAssetId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
        }
    }

    func testSanitizeMetadataTargetRejectsTextualAssetId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.asset("ro$se#wonderland#alice@hbl.sbp"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedAssetId("ro$se#wonderland#alice@hbl.sbp"))
        }
    }

    func testSanitizeMetadataTargetRejectsMalformedRwaId() {
        XCTAssertThrowsError(try TransactionInputValidator.sanitizeMetadataTarget(.rwa("lot-001"))) { error in
            XCTAssertEqual(error as? TransactionInputError,
                           .malformedRwaId(field: "target", value: "lot-001"))
        }
    }

    func testSanitizeMetadataTargetTrimsAccountAndDomainIds() throws {
        let authority = try i105(seed: 6)
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.account("  \(authority)  "))
        XCTAssertEqual(target.objectId, authority)

        let domainTarget = try TransactionInputValidator.sanitizeMetadataTarget(.domain("  wonderland  "))
        XCTAssertEqual(domainTarget.objectId, "wonderland")
    }

    func testSanitizeAssetIdAcceptsCanonicalPublicLiteral() throws {
        let literal =
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.asset(literal))
        XCTAssertEqual(target.objectId, literal)
    }

    func testSanitizeRwaIdAcceptsCanonicalPublicLiteral() throws {
        let literal =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities"
        let uppercaseHashLiteral =
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF$commodities"
        let target = try TransactionInputValidator.sanitizeMetadataTarget(.rwa(uppercaseHashLiteral))
        XCTAssertEqual(target.objectId, literal)
    }

    func testValidateAcceptsI105Authority() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let i105 = try AccountId.makeI105(publicKey: publicKey)
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: i105)
        XCTAssertEqual(ids.authorityId, i105)
    }

    func testValidateAcceptsI105DefaultAuthorityAndCanonicalizesToI105() throws {
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 0xAD, count: 32))
        let i105Default = try address.toI105Default()
        let i105 = try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        let ids = try TransactionInputValidator.validate(chainId: "0000",
                                                         authorityId: i105Default)
        XCTAssertEqual(ids.authorityId, i105)
    }

    func testValidateRejectsI105WithDomainSuffix() throws {
        let publicKey = Data(repeating: 0xAC, count: 32)
        let i105 = try AccountId.makeI105(publicKey: publicKey)
        let literal = "\(i105)@hbl.sbp"
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
