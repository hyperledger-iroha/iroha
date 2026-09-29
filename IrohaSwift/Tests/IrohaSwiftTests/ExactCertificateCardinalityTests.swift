import Foundation
import XCTest
@testable import IrohaSwift

final class ExactCertificateCardinalityTests: XCTestCase {
    func testNativeStatusCannotAdmitAQuorumProjectionOfAnyCardinality() throws {
        let source = try XCTUnwrap(JSONSerialization.jsonObject(with: NativeStatusFixtures.json("observer")) as? [String: Any])
        for count in [0, 2, 3, 4, 7] {
            var value = source
            value["last_commit_qc"] = ["validator_count": 4, "signer_count": count,
                                      "min_signers": 3, "signed_power": count, "total_power": 4]
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(JSONSerialization.data(withJSONObject: value)))
        }
    }
}
