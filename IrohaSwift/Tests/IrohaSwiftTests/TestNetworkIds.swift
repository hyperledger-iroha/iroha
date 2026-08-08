import Foundation
@testable import IrohaSwift

enum TestNetworkIds {
    static let canonical = try! NetworkId(
        literal: "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    )

    static let other: NetworkId = {
        var bytes = canonical.bytes
        bytes[0] ^= 0x01
        return try! NetworkId(bytes: bytes)
    }()
}
