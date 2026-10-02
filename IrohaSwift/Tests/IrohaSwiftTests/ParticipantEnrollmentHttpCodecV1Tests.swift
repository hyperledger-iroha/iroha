// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Public non-session fixture only; the canonical AccountAddress ABI bridge is mandatory.
/// No Native owner, lease refresh, nonce consumption, FI session or issuer is constructed.
final class ParticipantEnrollmentHttpCodecV1Tests: XCTestCase {
    private typealias Codec = ParticipantEnrollmentHttpCodecV1
    private func load() throws -> [String: Any] {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("fixtures/kagemusha/participant_enrollment_http_v1.json"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }
    private func text(_ object: [String: Any], _ name: String) throws -> String { try XCTUnwrap(object[name] as? String) }
    private func vectors(_ f: [String: Any]) throws -> [[String: Any]] { try XCTUnwrap(f["vectors"] as? [[String: Any]]) }
    private func bytes(_ hex: String) throws -> Data {
        let chars = Array(hex.utf8); XCTAssertEqual(chars.count % 2, 0); var out = Data()
        for i in stride(from: 0, to: chars.count, by: 2) {
            let value = try XCTUnwrap(UInt8(String(decoding: chars[i..<(i + 2)], as: UTF8.self), radix: 16)); out.append(value)
        }
        return out
    }
    private func fieldBytes(_ f: [String: Any], _ name: String) throws -> Data { try bytes(text(f, name)) }
    private func operation(_ v: [String: Any]) throws -> Codec.Operation {
        switch try text(v, "operation") {
        case "prepare": return .prepare
        case "raw_attestation": return .rawAttestation
        case "certificate": return .certificate
        default: XCTFail("Unknown public fixture operation"); throw Codec.CodecError.invalidOriginal
        }
    }
    private func context(_ v: [String: Any], _ f: [String: Any], namespace: String? = nil,
        actor: String? = nil, target: URL? = nil, network: NetworkId? = nil) throws -> Codec.Context {
        try Codec.context(networkID: network ?? NetworkId(bytes: fieldBytes(f, "network_id_hex")),
            authenticationNamespace: namespace ?? text(f, "authentication_namespace"), actorID: actor ?? text(f, "actor_id"),
            operation: operation(v), target: target ?? XCTUnwrap(URL(string: text(v, "target"))))
    }
    private func request(_ v: [String: Any], _ f: [String: Any], body: Data? = nil, session: Data? = nil) throws -> Codec.Request {
        try Codec.originalRequest(context: context(v, f), signatoryI105: text(f, "signatory_i105"), walletI105: text(f, "wallet_i105"),
            requestID: text(f, "request_id"), idempotencyKey: text(f, "idempotency_key"), timestampMS: XCTUnwrap(UInt64(text(v, "timestamp_ms"))),
            nonce: text(f, "nonce"), originalBody: body ?? fieldBytes(f, "body_hex"), sessionSHA256: session ?? fieldBytes(f, "session_sha256_hex"))
    }
    private func headers(_ v: [String: Any]) throws -> [Codec.Header] {
        try XCTUnwrap(v["headers"] as? [[String: Any]]).map { h in
            try Codec.Header(name: text(h, "name"), value: bytes(text(h, "value_hex")))
        }
    }
    private func decode(_ v: [String: Any], _ f: [String: Any], h: [Codec.Header]? = nil,
        body: Data? = nil, method: String = "POST", path: String? = nil, c: Codec.Context? = nil) throws -> Codec.ReceivedOriginal {
        try Codec.decode(context: c ?? context(v, f), actualMethod: method, actualRequestTarget: path ?? text(v, "path"),
            originalBody: body ?? fieldBytes(f, "body_hex"), headers: h ?? headers(v))
    }
    private func replace(_ v: [String: Any], _ index: Int, _ value: Data) throws -> [Codec.Header] {
        try headers(v).enumerated().map { i, h in
            if i == index { return try Codec.Header(name: h.name, value: value) }
            return h
        }
    }

    func testAllThreePurposesMatchExactPublicMessagesAndNineHeaders() throws {
        let f = try load(), all = try vectors(f); XCTAssertEqual(all.count, 3)
        for v in all {
            let r = try request(v, f)
            XCTAssertEqual(try r.signingMessage(), try bytes(text(v, "message_hex")))
            XCTAssertEqual(try decode(v, f).request.originalBody(), try fieldBytes(f, "body_hex"))
            XCTAssertEqual(try decode(v, f).originalSignature(), try bytes(text(v, "signature_hex")))
            let encoded = try Codec.encodeHeaders(request: r, originalSignature: bytes(text(v, "signature_hex")), authorization: fieldBytes(f, "authorization_hex"))
            XCTAssertEqual(encoded.count, 9)
            for (actual, expected) in zip(encoded, try headers(v)) {
                XCTAssertEqual(actual.name, expected.name); XCTAssertEqual(actual.originalValue(), expected.originalValue())
            }
        }
    }
    func testWrongPurposeGenericPrehashAndRetailSignaturesAreRefused() throws {
        let f = try load(), all = try vectors(f)
        for v in all {
            for other in all {
                if try text(other, "operation") == text(v, "operation") { continue }
                XCTAssertThrowsError(try decode(v, f, h: replace(v, 7, Data(text(other, "signature_hex").utf8))))
            }
            for name in ["negative_generic_subject_signature_hex", "negative_iroha_prehash_signature_hex", "negative_retail_raw32_signature_hex"] {
                XCTAssertThrowsError(try decode(v, f, h: replace(v, 7, Data(text(v, name).utf8))))
            }
        }
    }
    func testEveryActualHeaderMissingOrDuplicatedIsRefusedIncludingMixedCase() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first), h = try headers(v)
        for i in h.indices {
            XCTAssertThrowsError(try decode(v, f, h: h.enumerated().filter { $0.offset != i }.map { $0.element }))
            XCTAssertThrowsError(try decode(v, f, h: h + [h[i]]))
            XCTAssertThrowsError(try decode(v, f, h: h + [try Codec.Header(name: h[i].name.uppercased(), value: h[i].originalValue())]))
        }
        _ = try decode(v, f, h: h.map { try Codec.Header(name: $0.name.uppercased(), value: $0.originalValue()) })
        XCTAssertThrowsError(try decode(v, f, h: h + [try Codec.Header(name: "X-Iroha-Enrollment-Unknown", value: Data([49]))]))
        _ = try decode(v, f, h: h + [try Codec.Header(name: "X-Dataspace-Id", value: Data("is2".utf8))])
    }
    func testOriginalBodyAndEverySignedContextMutationAreRefused() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first)
        for i in 0...8 {
            var changed = try headers(v)[i].originalValue(); changed[changed.count - 1] = changed.last == 48 ? 49 : 48
            XCTAssertThrowsError(try decode(v, f, h: replace(v, i, changed)))
        }
        var body = try fieldBytes(f, "body_hex"); body[0] = 91
        XCTAssertThrowsError(try decode(v, f, body: body))
        let reserialized = try XCTUnwrap(String(data: fieldBytes(f, "body_hex"), encoding: .utf8)).trimmingCharacters(in: .whitespacesAndNewlines)
        XCTAssertThrowsError(try decode(v, f, body: Data(reserialized.utf8)))
        XCTAssertThrowsError(try decode(v, f, method: "post")); XCTAssertThrowsError(try decode(v, f, method: "GET"))
        XCTAssertThrowsError(try decode(v, f, path: text(v, "path") + "?offered=1"))
        XCTAssertThrowsError(try decode(v, f, c: context(v, f, namespace: "unit-fi-b")))
        XCTAssertThrowsError(try decode(v, f, c: context(v, f, actor: "other-test-actor")))
        var otherNetwork = try fieldBytes(f, "network_id_hex"); otherNetwork[0] = 1
        XCTAssertThrowsError(try decode(v, f, c: context(v, f, network: NetworkId(bytes: otherNetwork))))
        let otherTarget = try XCTUnwrap(URL(string: text(v, "target").replacingOccurrences(of: "unit-fi.invalid", with: "other-fi.invalid")))
        XCTAssertThrowsError(try decode(v, f, c: context(v, f, target: otherTarget)))
        let otherMount = try XCTUnwrap(URL(string: text(v, "target").replacingOccurrences(of: "/public-mount/", with: "/other-mount/")))
        XCTAssertThrowsError(try decode(v, f, path: otherMount.path, c: context(v, f, target: otherMount)))
        XCTAssertThrowsError(try Codec.originalRequest(context: context(v, f), signatoryI105: text(f, "wallet_i105"), walletI105: text(f, "signatory_i105"),
            requestID: text(f, "request_id"), idempotencyKey: text(f, "idempotency_key"), timestampMS: XCTUnwrap(UInt64(text(v, "timestamp_ms"))),
            nonce: text(f, "nonce"), originalBody: fieldBytes(f, "body_hex"), sessionSHA256: fieldBytes(f, "session_sha256_hex")))
    }
    func testCanonicalUtf8HexDecimalAndVisibleWhitespaceAreEnforced() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first)
        for i in 0...7 { XCTAssertThrowsError(try decode(v, f, h: replace(v, i, Data([0xc3, 0x28])))) }
        for i in 0...7 {
            let old = try headers(v)[i].originalValue()
            XCTAssertThrowsError(try decode(v, f, h: replace(v, i, Data([32]) + old)))
            XCTAssertThrowsError(try decode(v, f, h: replace(v, i, old + Data([32]))))
        }
        for bad in [try "0" + text(v, "timestamp_ms"), "18446744073709551616", "30000", "18446744073709521616", "+90001"] {
            XCTAssertThrowsError(try decode(v, f, h: replace(v, 5, Data(bad.utf8))))
        }
        for bad in [String(repeating: "AB", count: 32), String(repeating: "12", count: 31), try "0x" + text(f, "nonce")] {
            XCTAssertThrowsError(try decode(v, f, h: replace(v, 6, Data(bad.utf8))))
        }
        for bad in [try text(v, "signature_hex").uppercased(), "AA==", String(repeating: "00", count: 64)] {
            XCTAssertThrowsError(try decode(v, f, h: replace(v, 7, Data(bad.utf8))))
        }
        XCTAssertThrowsError(try decode(v, f, h: replace(v, 1, Data(text(f, "signatory_canonical_hex").utf8))))
    }
    func testBoundsRejectIncompleteOversizedAndNonAsciiOriginals() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first)
        XCTAssertThrowsError(try request(v, f, body: Data())); XCTAssertThrowsError(try request(v, f, body: Data(repeating: 0, count: 256 * 1024 + 1)))
        XCTAssertThrowsError(try request(v, f, session: Data(repeating: 0, count: 32))); XCTAssertThrowsError(try request(v, f, session: Data(repeating: 0, count: 31)))
        for auth in [Data(), Data(repeating: 65, count: 8193), Data([31]), Data([127])] { XCTAssertThrowsError(try Codec.authorizationDigest(auth)) }
        XCTAssertThrowsError(try decode(v, f, h: replace(v, 3, Data(repeating: 65, count: 8193))))
        XCTAssertThrowsError(try context(v, f, namespace: String(repeating: "a", count: 257))); XCTAssertThrowsError(try context(v, f, actor: "é"))
        let binary = Data([0xff, 0, 0x80]); XCTAssertEqual(try request(v, f, body: binary).originalBody(), binary)
    }
    func testAuthorizationSpacesAreHashedExactlyAndNeverTrimmed() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first), auth = try fieldBytes(f, "authorization_hex")
        let spaced = Data([32]) + auth + Data([32])
        XCTAssertEqual(try Codec.authorizationDigest(spaced), Data(SHA256.hash(data: spaced)))
        XCTAssertThrowsError(try decode(v, f, h: replace(v, 8, spaced)))
        XCTAssertThrowsError(try Codec.encodeHeaders(request: request(v, f), originalSignature: bytes(text(v, "signature_hex")), authorization: spaced))
    }
    func testMutableInputsAndReturnedDataCannotReplaceRetainedOriginals() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first)
        var body = try fieldBytes(f, "body_hex"), session = try fieldBytes(f, "session_sha256_hex")
        let r = try request(v, f, body: body, session: session); body[0] = 0; session[0] = 0
        var returnedBody = r.originalBody(); returnedBody[0] = 0
        var returnedSession = r.sessionSHA256(); returnedSession[0] = 0
        XCTAssertEqual(try r.signingMessage(), try bytes(text(v, "message_hex")))
        var signature = try bytes(text(v, "signature_hex")), auth = try fieldBytes(f, "authorization_hex")
        let h = try Codec.encodeHeaders(request: r, originalSignature: signature, authorization: auth); signature[0] = 0; auth[0] = 0
        for header in h { var returned = header.originalValue(); returned[0] = 0 }
        let received = try decode(v, f, h: h); var returned = received.originalSignature(); returned[0] = 0
        XCTAssertEqual(received.originalSignature(), try bytes(text(v, "signature_hex")))
    }
    func testSignedNonUtf8BodyAndAuthorizationSpacesRetainExactOriginals() throws {
        let f = try load(), auxiliary = try XCTUnwrap(f["auxiliary_originals"] as? [[String: Any]])
        for v in auxiliary {
            let body = try fieldBytes(v, "body_hex"), auth = try fieldBytes(v, "authorization_hex")
            let r = try request(v, f, body: body, session: fieldBytes(v, "session_sha256_hex"))
            XCTAssertEqual(try r.signingMessage(), try bytes(text(v, "message_hex")))
            let h = try Codec.encodeHeaders(request: r, originalSignature: bytes(text(v, "signature_hex")), authorization: auth)
            XCTAssertEqual(h.last?.originalValue(), auth)
            XCTAssertEqual(try decode(v, f, h: h, body: body).request.originalBody(), body)
        }
    }
    func testOfferedOrNoncanonicalTargetsCannotReplaceExactMountedTarget() throws {
        let f = try load(), v = try XCTUnwrap(vectors(f).first), target = try text(v, "target")
        for bad in [target + "?x=1", target + "#x", target.replacingOccurrences(of: "https://", with: "http://"),
            target.replacingOccurrences(of: "unit-fi.invalid", with: "user@unit-fi.invalid"),
            target.replacingOccurrences(of: "unit-fi.invalid", with: "unit-fi.invalid:443"),
            target.replacingOccurrences(of: "/public-mount/", with: "/public-mount/../"),
            target.replacingOccurrences(of: "/public-mount/", with: "/public-mount/%2e%2e/")] {
            XCTAssertThrowsError(try context(v, f, target: XCTUnwrap(URL(string: bad))))
        }
    }
}
