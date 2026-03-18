import Foundation

public struct ComputeSimulationResult {
    public let payloadHashLiteral: String
    public let responseHashLiteral: String?
    public let responseBase64: String?
}

public enum ComputeSimulatorError: Error, LocalizedError, Equatable {
    case missingRoute
    case invalidPayloadHash(expected: String, actual: String)
    case unsupportedEntrypoint(String)
    case decodeFailure(String)

    public var errorDescription: String? {
        switch self {
        case .missingRoute:
            return "compute route missing from manifest"
        case let .invalidPayloadHash(expected, actual):
            return "payload hash mismatch: expected \(expected), computed \(actual)"
        case let .unsupportedEntrypoint(name):
            return "unsupported compute entrypoint \(name)"
        case let .decodeFailure(message):
            return message
        }
    }
}

public enum ComputeSimulator {
    public static func loadFixtures(
        manifestPath: URL,
        callPath: URL,
        payloadPath: URL
    ) throws -> (manifest: [String: Any], call: [String: Any], payload: Data) {
        let manifest = try decodeJson(at: manifestPath, description: "compute manifest")
        let call = try decodeJson(at: callPath, description: "compute call")
        let payload = try Data(contentsOf: payloadPath).trimmingTrailingWhitespace()
        return (manifest, call, payload)
    }

    public static func simulate(
        manifest: [String: Any],
        call: [String: Any],
        payload: Data
    ) throws -> ComputeSimulationResult {
        guard let route = resolveRoute(manifest: manifest, call: call) else {
            throw ComputeSimulatorError.missingRoute
        }
        let payloadHash = hashLiteral(for: payload)
        let expectedHash = (call["request"] as? [String: Any])?["payload_hash"] as? String
        if let expectedHash, expectedHash != payloadHash {
            throw ComputeSimulatorError.invalidPayloadHash(expected: expectedHash, actual: payloadHash)
        }

        let gasLimit: UInt64
        if let raw = call["gas_limit"] {
            guard let parsed = StrictJSONNumber.uint64(from: raw) else {
                throw ComputeSimulatorError.decodeFailure("gas_limit must be an unsigned integer")
            }
            gasLimit = parsed
        } else {
            gasLimit = 0
        }
        let maxResponseBytes: UInt64
        if let raw = call["max_response_bytes"] {
            guard let parsed = StrictJSONNumber.uint64(from: raw) else {
                throw ComputeSimulatorError.decodeFailure("max_response_bytes must be an unsigned integer")
            }
            maxResponseBytes = parsed
        } else {
            maxResponseBytes = 0
        }
        let entrypoint = route["entrypoint"] as? String ?? "echo"
        let response = try runEntrypoint(
            name: entrypoint,
            payload: payload,
            gasLimit: gasLimit,
            maxResponseBytes: maxResponseBytes
        )

        let responseHash = response.map(hashLiteral(for:))
        let responseBase64 = response?.base64EncodedString()
        return ComputeSimulationResult(
            payloadHashLiteral: payloadHash,
            responseHashLiteral: responseHash,
            responseBase64: responseBase64
        )
    }

    private static func decodeJson(at url: URL, description: String) throws -> [String: Any] {
        let data = try Data(contentsOf: url)
        guard
            let decoded = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            throw ComputeSimulatorError.decodeFailure("failed to decode \(description) at \(url.path)")
        }
        return decoded
    }

    private static func resolveRoute(
        manifest: [String: Any],
        call: [String: Any]
    ) -> [String: Any]? {
        guard
            let routes = manifest["routes"] as? [[String: Any]],
            let targetRoute = call["route"] as? [String: Any],
            let service = targetRoute["service"] as? String,
            let method = targetRoute["method"] as? String
        else {
            return nil
        }
        return routes.first(where: { route in
            guard let id = route["id"] as? [String: Any] else { return false }
            return id["service"] as? String == service && id["method"] as? String == method
        })
    }

    private static func runEntrypoint(
        name: String,
        payload: Data,
        gasLimit: UInt64,
        maxResponseBytes: UInt64
    ) throws -> Data? {
        var response: Data?
        switch name {
        case "echo", "quote_entry":
            response = payload
        case "uppercase":
            if let upper = String(data: payload, encoding: .utf8) {
                response = upper.uppercased().data(using: .utf8)
            } else {
                response = payload
            }
        case "sha3":
            response = hashBytes(payload)
        default:
            throw ComputeSimulatorError.unsupportedEntrypoint(name)
        }

        let cycles = UInt64(payload.count) &* 10_000 &+ 5_000
        if cycles > gasLimit {
            response = nil
        }
        if let response, response.count > maxResponseBytes {
            return nil
        }
        return response
    }

    private static func hashBytes(_ payload: Data) -> Data {
        var digest = Blake2b.hash256(payload)
        if !digest.isEmpty {
            digest[digest.count - 1] |= 1
        }
        return digest
    }

    private static func hashLiteral(for payload: Data) -> String {
        let digest = hashBytes(payload)
        let checksumBody = digest.map { String(format: "%02X", $0) }.joined()
        let checksum = crc16(tag: "hash", body: checksumBody)
        return "hash:\(checksumBody)#\(String(format: "%04X", checksum))"
    }

    private static func crc16(tag: String, body: String) -> UInt16 {
        var crc: UInt16 = 0xffff
        let processByte: (UInt8) -> Void = { byte in
            crc ^= UInt16(byte & 0xff) << 8
            for _ in 0..<8 {
                if crc & 0x8000 != 0 {
                    crc = (crc << 1) ^ 0x1021
                } else {
                    crc = crc << 1
                }
            }
            crc &= 0xffff
        }

        for byte in tag.utf8 {
            processByte(byte)
        }
        processByte(UInt8(ascii: ":"))
        for byte in body.utf8 {
            processByte(byte)
        }
        return crc & 0xffff
    }
}

private extension Data {
    /// Trims trailing whitespace/newline bytes so fixture files with a final newline hash correctly.
    func trimmingTrailingWhitespace() -> Data {
        guard !isEmpty else { return self }
        var end = count
        while end > 0 {
            let byte = self[index(startIndex, offsetBy: end - 1)]
            if byte == 0x20 || byte == 0x0a || byte == 0x0d || byte == 0x09 {
                end -= 1
                continue
            }
            break
        }
        if end == count { return self }
        return self[startIndex..<index(startIndex, offsetBy: end)]
    }
}
