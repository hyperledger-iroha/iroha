import Foundation
import XCTest
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

func toriiClientTestBodyData(from request: URLRequest) -> Data? {
    if let data = request.httpBody {
        return data
    }
    guard let stream = request.httpBodyStream else { return nil }
    stream.open()
    defer { stream.close() }
    var buffer = [UInt8](repeating: 0, count: 1024)
    var data = Data()
    while stream.hasBytesAvailable {
        let read = stream.read(&buffer, maxLength: buffer.count)
        if read <= 0 { break }
        data.append(buffer, count: read)
    }
    return data.isEmpty ? nil : data
}

func toriiClientTestBodyJSON(from request: URLRequest) -> [String: Any] {
    guard let data = toriiClientTestBodyData(from: request),
          let object = try? JSONSerialization.jsonObject(with: data),
          let dictionary = object as? [String: Any] else {
        return [:]
    }
    return dictionary
}

func toriiClientTestBase64URL(_ data: Data) -> String {
    data.base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
}

struct ToriiClientTestConnectResponse {
    let payload: [String: Any]
    let tokenWallet: String
    let tokenManagement: String
    let tokenRelay: String
}

func toriiClientTestConnectSessionResponse(sid: String,
                                           networkID: String,
                                           appPublicKey: String,
                                           nonce: String,
                                           node: String) -> ToriiClientTestConnectResponse {
    let tokenApp = toriiClientTestBase64URL(Data(repeating: 0xA1, count: 32))
    let tokenWallet = toriiClientTestBase64URL(Data(repeating: 0xB2, count: 32))
    let tokenManagement = toriiClientTestBase64URL(Data(repeating: 0xC3, count: 32))
    let tokenRelay = toriiClientTestBase64URL(Data(repeating: 0xD4, count: 32))
    func uri(role: String, token: String) -> String {
        var components = URLComponents()
        components.scheme = "iroha"
        components.host = "connect"
        components.queryItems = [
            URLQueryItem(name: "sid", value: sid),
            URLQueryItem(name: "network_id", value: networkID),
            URLQueryItem(name: "app_pk", value: appPublicKey),
            URLQueryItem(name: "nonce", value: nonce),
            URLQueryItem(name: "node", value: node),
            URLQueryItem(name: "v", value: "1"),
            URLQueryItem(name: "role", value: role),
            URLQueryItem(name: "token", value: token),
            URLQueryItem(name: "relay", value: tokenRelay)
        ]
        return components.string!
    }
    return ToriiClientTestConnectResponse(
        payload: [
            "sid": sid,
            "network_id": networkID,
            "app_pk": appPublicKey,
            "nonce": nonce,
            "wallet_uri": uri(role: "wallet", token: tokenWallet),
            "app_uri": uri(role: "app", token: tokenApp),
            "token_app": tokenApp,
            "token_wallet": tokenWallet,
            "token_management": tokenManagement,
            "token_relay": tokenRelay
        ],
        tokenWallet: tokenWallet,
        tokenManagement: tokenManagement,
        tokenRelay: tokenRelay
    )
}

func toriiClientTestNoncanonicalBase64PadBitAlias(_ encoded: String) -> String {
    XCTAssertTrue(encoded.hasSuffix("=="))
    let alphabet = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".utf8)
    var bytes = Array(encoded.utf8)
    let index = bytes.count - 3
    let value = alphabet.firstIndex(of: bytes[index])!
    bytes[index] = alphabet[value ^ 0x01]
    return String(decoding: bytes, as: UTF8.self)
}
