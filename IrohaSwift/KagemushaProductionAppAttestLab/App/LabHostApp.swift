//! Physical-device host for a real production-environment App Attest capture.

import CryptoKit
import DeviceCheck
import Foundation
import SwiftUI

private let requestSchema = "iroha.kagemusha.ios.app_attest_capture_request.v1"
private let captureSchema = "iroha.kagemusha.ios.app_attest_physical_capture.v1"
private let directoryName = "kagemusha-production-app-attest"
private let maximumInputBytes = 256 * 1024
private let maximumObjectBytes = 128 * 1024

private enum CaptureFailure: Error, CustomStringConvertible {
    case invalid(String)

    var description: String {
        switch self {
        case let .invalid(message):
            return message
        }
    }
}

private func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    guard condition() else {
        throw CaptureFailure.invalid(message)
    }
}

private func sha256(_ data: Data) -> Data {
    Data(SHA256.hash(data: data))
}

private func hex(_ data: Data) -> String {
    data.map { String(format: "%02x", $0) }.joined()
}

private func canonicalJSON(_ value: Any) throws -> Data {
    try validateCanonicalJSONValue(value)
    var encoded = try JSONSerialization.data(
        withJSONObject: value,
        options: [.sortedKeys, .withoutEscapingSlashes]
    )
    encoded.append(0x0A)
    return encoded
}

private func validateCanonicalJSONValue(_ value: Any) throws {
    switch value {
    case let dictionary as [String: Any]:
        for (key, child) in dictionary {
            try require(
                !key.isEmpty && key.unicodeScalars.allSatisfy { $0.value >= 0x20 && $0.value <= 0x7E },
                "JSON object key is not canonical ASCII"
            )
            try validateCanonicalJSONValue(child)
        }
    case let array as [Any]:
        try require(array.count <= 1_024, "JSON array exceeds the capture bound")
        for child in array {
            try validateCanonicalJSONValue(child)
        }
    case let string as String:
        try require(
            string.unicodeScalars.allSatisfy { $0.value >= 0x20 && $0.value <= 0x7E },
            "JSON string is not canonical ASCII"
        )
    case let number as NSNumber:
        try require(!CFNumberIsFloatType(number), "floating-point JSON is forbidden")
    case is NSNull:
        break
    default:
        throw CaptureFailure.invalid("unsupported JSON value in capture request")
    }
}

private func documentsDirectory() throws -> URL {
    guard let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first else {
        throw CaptureFailure.invalid("application Documents directory is unavailable")
    }
    return url.appendingPathComponent(directoryName, isDirectory: true)
}

private func readRequest() throws -> (Data, [String: Any], String) {
    let requestURL = try documentsDirectory().appendingPathComponent("request-v1.json")
    let data = try Data(contentsOf: requestURL, options: [.mappedIfSafe])
    try require(!data.isEmpty && data.count <= maximumInputBytes, "capture request size is outside bounds")
    guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw CaptureFailure.invalid("capture request is not a JSON object")
    }
    let canonicalRequest = try canonicalJSON(value)
    try require(canonicalRequest == data, "capture request is not canonical JSON")
    let fields = Set(value.keys)
    try require(
        fields == ["schema", "version", "attestation_client_data_base64", "assertion_client_data_template"],
        "capture request fields are not exact"
    )
    try require(value["schema"] as? String == requestSchema, "capture request schema is not exact")
    try require((value["version"] as? NSNumber)?.intValue == 1, "capture request version is not 1")
    guard
        let attestationBase64 = value["attestation_client_data_base64"] as? String,
        let attestationClientData = Data(base64Encoded: attestationBase64),
        !attestationClientData.isEmpty,
        attestationClientData.count <= maximumInputBytes,
        attestationClientData.base64EncodedString() == attestationBase64,
        let assertionTemplate = value["assertion_client_data_template"] as? [String: Any]
    else {
        throw CaptureFailure.invalid("capture request client data is invalid")
    }
    try require(
        assertionTemplate["attestation_object_sha256"] == nil && assertionTemplate["key_id"] == nil,
        "assertion template must leave device-bound fields unset"
    )
    try validateCanonicalJSONValue(assertionTemplate)
    return (attestationClientData, assertionTemplate, hex(sha256(data)))
}

private func generateKey(_ service: DCAppAttestService) async throws -> String {
    try await withCheckedThrowingContinuation { continuation in
        service.generateKey { keyID, error in
            if let error {
                continuation.resume(throwing: error)
            } else if let keyID, !keyID.isEmpty {
                continuation.resume(returning: keyID)
            } else {
                continuation.resume(throwing: CaptureFailure.invalid("App Attest returned no key identifier"))
            }
        }
    }
}

private func attest(
    _ service: DCAppAttestService,
    keyID: String,
    clientData: Data
) async throws -> Data {
    try await withCheckedThrowingContinuation { continuation in
        service.attestKey(keyID, clientDataHash: sha256(clientData)) { object, error in
            if let error {
                continuation.resume(throwing: error)
            } else if let object, !object.isEmpty, object.count <= maximumObjectBytes {
                continuation.resume(returning: object)
            } else {
                continuation.resume(throwing: CaptureFailure.invalid("App Attest returned no bounded attestation object"))
            }
        }
    }
}

private func assertKey(
    _ service: DCAppAttestService,
    keyID: String,
    clientData: Data
) async throws -> Data {
    try await withCheckedThrowingContinuation { continuation in
        service.generateAssertion(keyID, clientDataHash: sha256(clientData)) { object, error in
            if let error {
                continuation.resume(throwing: error)
            } else if let object, !object.isEmpty, object.count <= maximumObjectBytes {
                continuation.resume(returning: object)
            } else {
                continuation.resume(throwing: CaptureFailure.invalid("App Attest returned no bounded assertion object"))
            }
        }
    }
}

private func writeCapture(_ value: [String: Any], requestDigest: String) throws {
    try require(
        requestDigest.count == 64
            && requestDigest.unicodeScalars.allSatisfy {
                (0x30 ... 0x39).contains($0.value) || (0x61 ... 0x66).contains($0.value)
            },
        "capture request digest is not canonical SHA-256"
    )
    let directory = try documentsDirectory()
    try FileManager.default.createDirectory(
        at: directory,
        withIntermediateDirectories: true,
        attributes: [.posixPermissions: NSNumber(value: 0o700)]
    )
    let output = directory.appendingPathComponent("capture-\(requestDigest).json")
    try canonicalJSON(value).write(to: output, options: [.atomic, .completeFileProtection])
}

@MainActor
private final class CaptureController: ObservableObject {
    @Published var status = "Waiting to start"
    private var started = false

    func start() {
        guard !started else { return }
        started = true
        Task { await capture() }
    }

    private func capture() async {
        let service = DCAppAttestService.shared
        let startedAt = Int64(Date().timeIntervalSince1970 * 1_000)
        var requestDigest: String?
        do {
            try require(service.isSupported, "DCAppAttestService is not supported on this device")
            let (attestationClientData, template, digest) = try readRequest()
            requestDigest = digest
            status = "Generating Secure Enclave App Attest key"
            let keyID = try await generateKey(service)
            guard let keyIDBytes = Data(base64Encoded: keyID), keyIDBytes.count == 32 else {
                throw CaptureFailure.invalid("App Attest key identifier is not canonical 32-byte Base64")
            }
            status = "Requesting Apple key attestation"
            let attestationObject = try await attest(
                service,
                keyID: keyID,
                clientData: attestationClientData
            )
            var assertionClientDataValue = template
            assertionClientDataValue["attestation_object_sha256"] = hex(sha256(attestationObject))
            assertionClientDataValue["key_id"] = keyID
            let assertionClientData = try canonicalJSON(assertionClientDataValue)
            status = "Generating App Attest assertion"
            let assertionObject = try await assertKey(
                service,
                keyID: keyID,
                clientData: assertionClientData
            )
            let bundle = Bundle.main
            try writeCapture([
                "schema": captureSchema,
                "version": 1,
                "status": "captured",
                "app_attest_supported": true,
                "requested_environment": "production",
                "started_at_unix_ms": startedAt,
                "captured_at_unix_ms": Int64(Date().timeIntervalSince1970 * 1_000),
                "bundle_id": bundle.bundleIdentifier ?? "",
                "bundle_version": bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "",
                "key_id": keyID,
                "attestation_client_data_base64": attestationClientData.base64EncodedString(),
                "attestation_object_base64": attestationObject.base64EncodedString(),
                "assertion_client_data_base64": assertionClientData.base64EncodedString(),
                "assertion_object_base64": assertionObject.base64EncodedString(),
            ], requestDigest: digest)
            status = "Production App Attest capture completed"
        } catch {
            let nsError = error as NSError
            if let requestDigest {
                try? writeCapture([
                    "schema": captureSchema,
                    "version": 1,
                    "status": "failed",
                    "app_attest_supported": service.isSupported,
                    "requested_environment": "production",
                    "started_at_unix_ms": startedAt,
                    "captured_at_unix_ms": Int64(Date().timeIntervalSince1970 * 1_000),
                    "bundle_id": Bundle.main.bundleIdentifier ?? "",
                    "bundle_version": Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "",
                    "error_domain": nsError.domain,
                    "error_code": nsError.code,
                    "error_description": nsError.localizedDescription,
                ], requestDigest: requestDigest)
            }
            status = "Capture failed: \(nsError.domain) \(nsError.code)"
        }
    }
}

private struct ContentView: View {
    @StateObject private var controller = CaptureController()

    var body: some View {
        VStack(spacing: 18) {
            Text("Kagemusha App Attest")
                .font(.title2)
            ProgressView()
            Text(controller.status)
                .multilineTextAlignment(.center)
                .padding()
        }
        .task { controller.start() }
    }
}

@main
private struct KagemushaProductionAppAttestLabApp: App {
    var body: some Scene {
        WindowGroup { ContentView() }
    }
}
