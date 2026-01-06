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

    static func decodePermissionsJSON(_ data: Data) throws -> ConnectPermissions? {
        guard !data.isEmpty else { return nil }
        let json = try JSONSerialization.jsonObject(with: data)
        guard let object = json as? [String: Any] else { throw ConnectCodecError.decodeFailed }
        if object.isEmpty { return nil }
        let allowedKeys: Set<String> = ["methods", "events", "resources"]
        for key in object.keys where !allowedKeys.contains(key) {
            throw ConnectCodecError.decodeFailed
        }
        guard let methodsRaw = object["methods"],
              let eventsRaw = object["events"] else {
            throw ConnectCodecError.decodeFailed
        }
        let methods = try decodeStringArray(methodsRaw)
        let events = try decodeStringArray(eventsRaw)
        let resources = try decodeOptionalStringArray(object["resources"])
        return ConnectPermissions(methods: methods, events: events, resources: resources)
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

    static func decodeAppMetadataJSON(_ data: Data) throws -> ConnectAppMetadata? {
        guard !data.isEmpty else { return nil }
        let json = try JSONSerialization.jsonObject(with: data)
        guard let object = json as? [String: Any] else { throw ConnectCodecError.decodeFailed }
        if object.isEmpty { return nil }
        let allowedKeys: Set<String> = ["name", "url"]
        for key in object.keys where !allowedKeys.contains(key) {
            throw ConnectCodecError.decodeFailed
        }
        let name = try decodeOptionalString(object["name"])
        let iconURL = try decodeOptionalString(object["url"])
        guard name != nil || iconURL != nil else { return nil }
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

    static func decodeProofJSON(_ data: Data) throws -> ConnectSignInProof? {
        guard !data.isEmpty else { return nil }
        let json = try JSONSerialization.jsonObject(with: data)
        guard let object = json as? [String: Any] else { throw ConnectCodecError.decodeFailed }
        if object.isEmpty { return nil }
        let allowedKeys: Set<String> = ["domain", "uri", "statement", "issued_at", "nonce"]
        for key in object.keys where !allowedKeys.contains(key) {
            throw ConnectCodecError.decodeFailed
        }
        let domain = try decodeOptionalString(object["domain"])
        let uri = try decodeOptionalString(object["uri"])
        let statement = try decodeOptionalString(object["statement"])
        let issuedAt = try decodeOptionalString(object["issued_at"])
        let nonce = try decodeOptionalString(object["nonce"])
        guard domain != nil || uri != nil || statement != nil || issuedAt != nil || nonce != nil else { return nil }
        return ConnectSignInProof(domain: domain,
                                  uri: uri,
                                  statement: statement,
                                  issuedAt: issuedAt,
                                  nonce: nonce)
    }

    private static func decodeStringArray(_ value: Any) throws -> [String] {
        guard let array = value as? [Any] else { throw ConnectCodecError.decodeFailed }
        var strings: [String] = []
        strings.reserveCapacity(array.count)
        for item in array {
            guard let string = item as? String else { throw ConnectCodecError.decodeFailed }
            strings.append(string)
        }
        return strings
    }

    private static func decodeOptionalStringArray(_ value: Any?) throws -> [String]? {
        guard let value else { return nil }
        if value is NSNull { return nil }
        return try decodeStringArray(value)
    }

    private static func decodeOptionalString(_ value: Any?) throws -> String? {
        guard let value else { return nil }
        if value is NSNull { return nil }
        guard let raw = value as? String else { throw ConnectCodecError.decodeFailed }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
