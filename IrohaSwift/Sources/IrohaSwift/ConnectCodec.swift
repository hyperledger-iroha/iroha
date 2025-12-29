import Foundation

public enum ConnectCodecError: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case encodeFailed
    case decodeFailed

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "NoritoBridge connect codec is unavailable."
            )
        case .encodeFailed:
            return "NoritoBridge failed to encode the provided Connect frame."
        case .decodeFailed:
            return "NoritoBridge failed to decode the provided Connect frame bytes."
        }
    }
}

public enum ConnectCodec {
    public static func encode(_ frame: ConnectFrame) throws -> Data {
        guard NoritoNativeBridge.shared.isConnectCodecAvailable else {
            throw ConnectCodecError.bridgeUnavailable
        }
        guard let native = NoritoNativeBridge.shared.encodeConnectFrame(frame) else {
            throw ConnectCodecError.encodeFailed
        }
        return native
    }

    public static func decode(_ data: Data) throws -> ConnectFrame {
        guard NoritoNativeBridge.shared.isConnectCodecAvailable else {
            throw ConnectCodecError.bridgeUnavailable
        }
        guard let frame = NoritoNativeBridge.shared.decodeConnectFrame(data) else {
            throw ConnectCodecError.decodeFailed
        }
        return frame
    }
}

extension ConnectCodec {
    static func encodePermissionsJSON(_ permissions: ConnectPermissions?) -> Data? {
        guard let permissions else { return nil }
        var json: [String: Any] = [
            "methods": permissions.methods,
            "events": permissions.events
        ]
        if let resources = permissions.resources {
            json["resources"] = resources
        }
        return try? JSONSerialization.data(withJSONObject: json, options: [])
    }

    static func decodePermissionsJSON(_ data: Data) -> ConnectPermissions? {
        guard !data.isEmpty else { return nil }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        if object.isEmpty { return nil }
        let methods = (object["methods"] as? [Any])?.compactMap { $0 as? String } ?? []
        let events = (object["events"] as? [Any])?.compactMap { $0 as? String } ?? []
        let resourcesArray = (object["resources"] as? [Any])?.compactMap { $0 as? String }
        return ConnectPermissions(methods: methods, events: events, resources: resourcesArray)
    }

    static func encodeAppMetadataJSON(_ metadata: ConnectAppMetadata?) -> Data? {
        guard let metadata else { return nil }
        var json: [String: Any] = [:]
        if let name = metadata.name?.trimmingCharacters(in: .whitespacesAndNewlines), !name.isEmpty {
            json["name"] = name
        }
        if let iconURL = metadata.iconURL?.trimmingCharacters(in: .whitespacesAndNewlines), !iconURL.isEmpty {
            json["url"] = iconURL
        }
        // TODO: Map description/icon_hash once Connect metadata exposes a dedicated description field.
        guard !json.isEmpty else { return nil }
        return try? JSONSerialization.data(withJSONObject: json, options: [])
    }

    static func decodeAppMetadataJSON(_ data: Data) -> ConnectAppMetadata? {
        guard !data.isEmpty else { return nil }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        if object.isEmpty { return nil }
        func value(for key: String) -> String? {
            guard let raw = object[key] as? String else { return nil }
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        }
        let name = value(for: "name")
        let iconURL = value(for: "url")
        return ConnectAppMetadata(name: name, iconURL: iconURL, description: nil)
    }

    static func encodeProofJSON(_ proof: ConnectSignInProof?) -> Data? {
        guard let proof else { return nil }
        var json: [String: Any] = [:]
        if let domain = proof.domain { json["domain"] = domain }
        if let uri = proof.uri { json["uri"] = uri }
        if let statement = proof.statement { json["statement"] = statement }
        if let issuedAt = proof.issuedAt { json["issued_at"] = issuedAt }
        if let nonce = proof.nonce { json["nonce"] = nonce }
        guard !json.isEmpty else { return nil }
        return try? JSONSerialization.data(withJSONObject: json, options: [])
    }

    static func decodeProofJSON(_ data: Data) -> ConnectSignInProof? {
        guard !data.isEmpty else { return nil }
        guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        if object.isEmpty { return nil }
        func value(for key: String) -> String? {
            guard let raw = object[key] as? String, !raw.isEmpty else { return nil }
            return raw
        }
        return ConnectSignInProof(domain: value(for: "domain"),
                                  uri: value(for: "uri"),
                                  statement: value(for: "statement"),
                                  issuedAt: value(for: "issued_at"),
                                  nonce: value(for: "nonce"))
    }
}
