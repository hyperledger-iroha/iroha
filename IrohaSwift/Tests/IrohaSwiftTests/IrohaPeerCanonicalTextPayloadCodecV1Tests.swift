import Foundation
import XCTest
@testable import IrohaSwift

final class IrohaPeerCanonicalTextPayloadCodecV1Tests: XCTestCase {
    func testOfflineNoteExactUTF8RoundTripUsesConservativeBounds() throws {
        let signedWalletText = "pk2off2:eyJsaW5lYWdlIjoiY2Fub25pY2FsIn0.署名"
        let bytes = try IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
            for: signedWalletText,
            profile: .offlineNote
        )
        XCTAssertEqual(bytes, Data(signedWalletText.utf8))
        XCTAssertEqual(
            try IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
                from: bytes,
                profile: .offlineNote
            ),
            signedWalletText
        )
        XCTAssertEqual(
            try IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
                for: .offlineNote
            ),
            24_576
        )
        XCTAssertThrowsError(
            try IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
                for: .kagemusha
            )
        ) {
            XCTAssertEqual(
                $0 as? IrohaPeerCanonicalTextPayloadCodecErrorV1,
                .unsupportedProfile(.kagemusha)
            )
        }
        let canonicalLimited = IrohaPeerWireLimitsV1(
            maximumCanonicalBytes: 7,
            maximumOfflineNoteEncodedBytes: 9,
            maximumKagemushaEncodedBytes: 8
        )
        XCTAssertEqual(
            try IrohaPeerCanonicalTextPayloadCodecV1.maximumCanonicalTextBytes(
                for: .offlineNote,
                limits: canonicalLimited
            ),
            7
        )
    }

    func testRejectsEmptyInvalidUTF8AndProfileOversizeInput() throws {
        let profile = IrohaPeerWireProfileV1.offlineNote
        do {
            XCTAssertThrowsError(
                try IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
                    for: "",
                    profile: profile
                )
            ) {
                XCTAssertEqual(
                    $0 as? IrohaPeerCanonicalTextPayloadCodecErrorV1,
                    .emptyPayload
                )
            }
            XCTAssertThrowsError(
                try IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
                    from: Data(),
                    profile: profile
                )
            )
            XCTAssertThrowsError(
                try IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
                    from: Data([0xC3, 0x28]),
                    profile: profile
                )
            ) {
                XCTAssertEqual(
                    $0 as? IrohaPeerCanonicalTextPayloadCodecErrorV1,
                    .invalidUTF8
                )
            }
            let maximum = try IrohaPeerCanonicalTextPayloadCodecV1
                .maximumCanonicalTextBytes(for: profile)
            XCTAssertThrowsError(
                try IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
                    for: String(repeating: "a", count: maximum + 1),
                    profile: profile
                )
            )
            XCTAssertThrowsError(
                try IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
                    from: Data(repeating: 0x61, count: maximum + 1),
                    profile: profile
                )
            )
        }

        for operation in [
            { try IrohaPeerCanonicalTextPayloadCodecV1.canonicalBytes(
                for: "not-a-native-archive",
                profile: .kagemusha
            ) as Any },
            { try IrohaPeerCanonicalTextPayloadCodecV1.canonicalText(
                from: Data("not-a-native-archive".utf8),
                profile: .kagemusha
            ) as Any }
        ] {
            XCTAssertThrowsError(try operation()) {
                XCTAssertEqual(
                    $0 as? IrohaPeerCanonicalTextPayloadCodecErrorV1,
                    .unsupportedProfile(.kagemusha)
                )
            }
        }
    }
}
