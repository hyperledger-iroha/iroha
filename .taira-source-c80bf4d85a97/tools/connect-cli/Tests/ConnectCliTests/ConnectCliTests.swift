//! Tests for connect-cli SID parsing helpers.

import XCTest
@testable import connect_cli

final class ConnectCliTests: XCTestCase {
    func testDecodeSidBase64UrlAcceptsValidSid() {
        let sid = Data(repeating: 0x11, count: sidByteCount)
        let base64 = sid.base64EncodedString()
        let base64Url = base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        XCTAssertEqual(decodeSidBase64Url(base64Url), sid)
    }

    func testDecodeSidBase64UrlRejectsPadding() {
        let sid = Data(repeating: 0x22, count: sidByteCount)
        let base64 = sid.base64EncodedString()
        let paddedBase64Url = base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")

        XCTAssertNil(decodeSidBase64Url(paddedBase64Url))
    }

    func testDecodeSidBase64UrlRejectsWrongLength() {
        let sid = Data(repeating: 0x33, count: sidByteCount - 1)
        let base64 = sid.base64EncodedString()
        let base64Url = base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")

        XCTAssertNil(decodeSidBase64Url(base64Url))
    }
}
