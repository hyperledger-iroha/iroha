import Foundation
@testable import IrohaSwift

enum TestNetworkIds {
    static let canonical = try! NetworkId(
        literal: "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"
    )

    static let other: NetworkId = {
        var bytes = canonical.bytes
        bytes[0] ^= 0x01
        return try! NetworkId(bytes: bytes)
    }()
}
