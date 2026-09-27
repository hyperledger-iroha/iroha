import Foundation
import XCTest

@testable import IrohaSwift

final class ContractStateValueProofV1Tests: XCTestCase {
    private let path = "sc/alpha/Balance"
    private let root = ContractStateValueProofV1Tests.bytes("194a8961806570284bf970836427142baa1ddcab853f1ee2c3ae672e1da8acb3")
    private let sibling = ContractStateValueProofV1Tests.bytes("482931df820458f6bf299fa0e88e37f2378b3e8558c2c254ad03755ed8790947")

    func testIndependentTwoLeafVectorBindsExactAccumulatedValue() throws {
        // Python hashlib.blake2b(digest_size=32) reference, with Iroha's marker.
        let siblingLiteral = try NetworkId(bytes: Data(sibling)).literal
        let step = ContractStateProofStepV1(bit: 0, prefix: [UInt8](repeating: 0, count: 32), sibling: siblingLiteral)
        let proof = ContractStateValueInclusionProofV1(
            version: 1, path: path, value: Array("one".utf8), leafCount: 2, steps: [step]
        )
        XCTAssertTrue(proof.verify(expectedPath: path, trustedRoot: Data(root)))
        XCTAssertFalse(proof.verify(expectedPath: "sc/beta/Balance", trustedRoot: Data(root)))
        XCTAssertFalse(proof.verify(expectedPath: path, trustedRoot: Data(sibling)))
        XCTAssertFalse(ContractStateValueInclusionProofV1(
            version: 1, path: path, value: Array("two".utf8), leafCount: 2, steps: [step]
        ).verify(expectedPath: path, trustedRoot: Data(root)))
        XCTAssertFalse(ContractStateValueInclusionProofV1(
            version: 1, path: path, value: Array("one".utf8), leafCount: 1, steps: [step]
        ).verify(expectedPath: path, trustedRoot: Data(root)))
    }

    func testDecoderRejectsUnknownFieldsAndNonCanonicalSibling() throws {
        let siblingLiteral = try NetworkId(bytes: Data(sibling)).literal
        let valid = """
        {"version":1,"path":"sc/alpha/Balance","value":[111,110,101],"leaf_count":2,
         "steps":[{"bit":0,"prefix":\(String(describing: [UInt8](repeating: 0, count: 32))),"sibling":"\(siblingLiteral)"}]}
        """
        let decoded = try ContractStateValueInclusionProofV1.parseJson(Data(valid.utf8))
        XCTAssertTrue(decoded.verify(expectedPath: path, trustedRoot: Data(root)))
        let extra = valid.replacingOccurrences(of: "\"version\":1", with: "\"version\":1,\"unknown\":0")
        XCTAssertThrowsError(try ContractStateValueInclusionProofV1.parseJson(Data(extra.utf8)))
        let duplicate = valid.replacingOccurrences(of: "\"version\":1", with: "\"version\":1,\"version\":1")
        XCTAssertThrowsError(try ContractStateValueInclusionProofV1.parseJson(Data(duplicate.utf8)))
        let badSibling = valid.replacingOccurrences(of: siblingLiteral, with: siblingLiteral.lowercased())
        let bad = try ContractStateValueInclusionProofV1.parseJson(Data(badSibling.utf8))
        XCTAssertFalse(bad.verify(expectedPath: path, trustedRoot: Data(root)))
    }

    private static func bytes(_ hex: String) -> [UInt8] {
        let chars = Array(hex.utf8)
        return stride(from: 0, to: chars.count, by: 2).map { index in
            UInt8(String(decoding: chars[index..<(index + 2)], as: UTF8.self), radix: 16)!
        }
    }
}
