import Foundation

func validateConnectSessionResponse(_ response: ToriiConnectSessionResponse,
                                    expectedNode: String?) throws -> ToriiConnectSessionResponse {
    guard response.extra.isEmpty else {
        throw ToriiClientError.invalidPayload("Connect session response contains unsupported fields")
    }
    for (field, token) in [
        ("token_app", response.tokenApp),
        ("token_wallet", response.tokenWallet),
        ("token_management", response.tokenManagement),
        ("token_relay", response.tokenRelay)
    ] {
        _ = try decodeConnectBase64URL(token, byteCount: 32, field: field)
    }
    try validateConnectLaunchURI(response.walletURI,
                                 role: "wallet",
                                 token: response.tokenWallet,
                                 response: response,
                                 expectedNode: expectedNode)
    try validateConnectLaunchURI(response.appURI,
                                 role: "app",
                                 token: response.tokenApp,
                                 response: response,
                                 expectedNode: expectedNode)
    return response
}

private func validateConnectLaunchURI(_ literal: String,
                                      role: String,
                                      token: String,
                                      response: ToriiConnectSessionResponse,
                                      expectedNode: String?) throws {
    guard let components = URLComponents(string: literal),
          components.scheme == "iroha",
          components.host == "connect",
          components.path.isEmpty,
          components.fragment == nil else {
        throw ToriiClientError.invalidPayload("Connect \(role) URI must use iroha://connect")
    }
    let allowed = Set(["sid", "network_id", "app_pk", "nonce", "node", "v", "role", "token", "relay"])
    var query: [String: String] = [:]
    for item in components.queryItems ?? [] {
        guard allowed.contains(item.name), query[item.name] == nil, let value = item.value else {
            throw ToriiClientError.invalidPayload(
                "Connect \(role) URI has duplicate or unsupported parameters"
            )
        }
        query[item.name] = value
    }
    let expected: [String: String] = [
        "sid": response.sid,
        "network_id": response.networkID.literal,
        "app_pk": connectBase64URL(response.appPublicKey),
        "nonce": connectBase64URL(response.nonce),
        "node": expectedNode ?? "",
        "v": "1",
        "role": role,
        "token": token,
        "relay": response.tokenRelay
    ]
    guard query == expected else {
        throw ToriiClientError.invalidPayload(
            "Connect \(role) URI substituted the canonical session identity"
        )
    }
}
