//! Two-launch, no-network physical-iPhone evidence harness for the candidate-only bridge.

import CryptoKit
import Darwin
import Foundation
import Network
import NoritoBridgeCandidateLab
import Security
import UIKit
import XCTest

private let inputDirectoryName = "kagemusha-candidate-input"
private let outputDirectoryName = "kagemusha-candidate-output"
private let resourceCeilingBytes: UInt64 = 6 * 1024 * 1024 * 1024
private let maximumArchiveBytes = 96 * 1024 * 1024

private enum LabFailure: Error, CustomStringConvertible {
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
        throw LabFailure.invalid(message)
    }
}

private func hex(_ bytes: some Sequence<UInt8>) -> String {
    bytes.map { String(format: "%02x", $0) }.joined()
}

private func sha256(_ data: Data) -> String {
    hex(SHA256.hash(data: data))
}

private func sha256(_ bytes: [UInt8]) -> String {
    sha256(Data(bytes))
}

private func canonicalJSON(_ value: Any) throws -> Data {
    try require(JSONSerialization.isValidJSONObject(value), "value is not valid JSON")
    var data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    data.append(0x0a)
    return data
}

private func recordedAtUTC() -> String {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter.string(from: Date())
}

private func documentsDirectory() throws -> URL {
    let urls = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
    try require(urls.count == 1, "application Documents directory is unavailable")
    // iOS commonly presents the data container below `/var`, whose system
    // prefix may resolve to `/private/var`. Canonicalize that trusted system
    // prefix once so the stricter descendant checks can reject any later
    // symlink traversal without rejecting the physical app container itself.
    return urls[0].resolvingSymlinksInPath().standardizedFileURL
}

private func inputDirectory() throws -> URL {
    try documentsDirectory().appendingPathComponent(inputDirectoryName, isDirectory: true)
}

private func outputDirectory() throws -> URL {
    try documentsDirectory().appendingPathComponent(outputDirectoryName, isDirectory: true)
}

private func validateDirectory(_ url: URL, ownerPrivate: Bool) throws {
    var metadata = stat()
    try require(lstat(url.path, &metadata) == 0, "directory is unavailable: \(url.lastPathComponent)")
    try require(
        (metadata.st_mode & mode_t(S_IFMT)) == mode_t(S_IFDIR),
        "path is not a directory: \(url.lastPathComponent)"
    )
    try require(metadata.st_uid == getuid(), "directory owner differs from the test process")
    if ownerPrivate {
        try require(
            (metadata.st_mode & mode_t(0o077)) == 0,
            "directory is not owner-private: \(url.lastPathComponent)"
        )
    }
    var resolved = [CChar](repeating: 0, count: Int(PATH_MAX))
    try require(realpath(url.path, &resolved) != nil, "directory cannot be canonicalized")
    try require(
        String(cString: resolved) == url.path,
        "directory path traverses a symbolic link: \(url.lastPathComponent)"
    )
}

private func readRegular(_ url: URL, maximum: Int, ownerPrivate: Bool = true) throws -> Data {
    try require(maximum > 0, "invalid read ceiling")
    var before = stat()
    try require(lstat(url.path, &before) == 0, "input is unavailable: \(url.lastPathComponent)")
    try require(
        (before.st_mode & mode_t(S_IFMT)) == mode_t(S_IFREG),
        "input is not a regular file: \(url.lastPathComponent)"
    )
    try require(before.st_nlink == 1, "input has multiple hard links: \(url.lastPathComponent)")
    try require(before.st_uid == getuid(), "input owner differs from the test process")
    if ownerPrivate {
        try require(
            (before.st_mode & mode_t(0o077)) == 0,
            "input is not owner-private: \(url.lastPathComponent)"
        )
    }
    try require(
        before.st_size > 0 && before.st_size <= off_t(maximum),
        "input size is outside its bound: \(url.lastPathComponent)"
    )
    let descriptor = open(url.path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW)
    try require(descriptor >= 0, "input cannot be opened safely: \(url.lastPathComponent)")
    defer { close(descriptor) }
    var opened = stat()
    try require(fstat(descriptor, &opened) == 0, "input metadata cannot be read")
    try require(
        before.st_dev == opened.st_dev
            && before.st_ino == opened.st_ino
            && before.st_size == opened.st_size,
        "input changed while opening: \(url.lastPathComponent)"
    )
    var result = Data()
    result.reserveCapacity(Int(opened.st_size))
    var buffer = [UInt8](repeating: 0, count: min(1024 * 1024, maximum))
    while true {
        let count = read(descriptor, &buffer, buffer.count)
        try require(count >= 0, "input read failed: \(url.lastPathComponent)")
        if count == 0 {
            break
        }
        try require(
            result.count <= maximum - count,
            "input grew beyond its bound: \(url.lastPathComponent)"
        )
        result.append(contentsOf: buffer[0..<count])
    }
    var after = stat()
    try require(lstat(url.path, &after) == 0, "input disappeared while reading")
    try require(
        opened.st_dev == after.st_dev
            && opened.st_ino == after.st_ino
            && opened.st_size == after.st_size
            && result.count == Int(opened.st_size),
        "input changed while reading: \(url.lastPathComponent)"
    )
    return result
}

private func fsyncDirectory(_ directory: URL) throws {
    let descriptor = open(directory.path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW)
    try require(descriptor >= 0, "output directory cannot be opened for fsync")
    defer { close(descriptor) }
    try require(fsync(descriptor) == 0, "output directory fsync failed")
}

private func writeDurably(_ data: Data, to destination: URL) throws {
    try require(!data.isEmpty, "refusing to persist empty evidence")
    let directory = destination.deletingLastPathComponent()
    try validateDirectory(directory, ownerPrivate: true)
    let temporary = directory.appendingPathComponent(
        ".\(destination.lastPathComponent).\(getpid()).\(UUID().uuidString)"
    )
    let descriptor = open(
        temporary.path,
        O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW,
        mode_t(0o600)
    )
    try require(descriptor >= 0, "temporary evidence file cannot be created")
    var closed = false
    defer {
        if !closed {
            close(descriptor)
        }
        unlink(temporary.path)
    }
    try data.withUnsafeBytes { raw in
        guard let base = raw.baseAddress else {
            throw LabFailure.invalid("evidence bytes are unavailable")
        }
        var offset = 0
        while offset < raw.count {
            let count = write(descriptor, base.advanced(by: offset), raw.count - offset)
            try require(count > 0, "evidence write failed")
            offset += count
        }
    }
    try require(fsync(descriptor) == 0, "evidence file fsync failed")
    try require(close(descriptor) == 0, "evidence file close failed")
    closed = true
    try require(rename(temporary.path, destination.path) == 0, "atomic evidence rename failed")
    try fsyncDirectory(directory)
    let reopened = try readRegular(destination, maximum: max(data.count, 1))
    try require(reopened == data, "durably reopened evidence differs from written bytes")
}

private func fileSHA256(_ url: URL, maximum: UInt64 = 2 * 1024 * 1024 * 1024) throws -> String {
    var metadata = stat()
    try require(lstat(url.path, &metadata) == 0, "code artifact is unavailable")
    try require(
        (metadata.st_mode & mode_t(S_IFMT)) == mode_t(S_IFREG)
            && metadata.st_nlink == 1
            && metadata.st_size > 0
            && UInt64(metadata.st_size) <= maximum,
        "code artifact is not a bounded singly-linked regular file"
    )
    let descriptor = open(url.path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW)
    try require(descriptor >= 0, "code artifact cannot be opened")
    defer { close(descriptor) }
    var hasher = SHA256()
    var buffer = [UInt8](repeating: 0, count: 1024 * 1024)
    while true {
        let count = read(descriptor, &buffer, buffer.count)
        try require(count >= 0, "code artifact read failed")
        if count == 0 {
            break
        }
        hasher.update(data: Data(buffer[0..<count]))
    }
    return hex(hasher.finalize())
}

private func random32() throws -> [UInt8] {
    var bytes = [UInt8](repeating: 0, count: 32)
    try require(
        SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess,
        "secure random generation failed"
    )
    try require(bytes.contains(where: { $0 != 0 }), "secure random output was all zero")
    return bytes
}

private func sysctlString(_ name: String) throws -> String {
    var size = 0
    try require(sysctlbyname(name, nil, &size, nil, 0) == 0 && size > 1, "sysctl size failed")
    var bytes = [CChar](repeating: 0, count: size)
    try require(
        sysctlbyname(name, &bytes, &size, nil, 0) == 0,
        "sysctl value failed"
    )
    return String(cString: bytes)
}

private func bootSessionSHA256() throws -> String {
    var bootTime = timeval()
    var size = MemoryLayout<timeval>.size
    try require(
        sysctlbyname("kern.boottime", &bootTime, &size, nil, 0) == 0
            && size == MemoryLayout<timeval>.size,
        "kern.boottime is unavailable"
    )
    return sha256(Data("\(bootTime.tv_sec):\(bootTime.tv_usec)".utf8))
}

private struct Session {
    static let keys: Set<String> = [
        "schema",
        "version",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "topup_finality_roster_sha256",
        "scenario_inventory_sha256",
        "native_build_manifest_sha256",
        "native_library_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "reviewed_source_closure_descriptor_sha256",
        "device_udid_sha256",
        "device_ecid_sha256",
        "device_serial_sha256",
        "expected_hardware_model",
        "expected_board_config",
        "expected_os_version",
        "expected_os_build",
    ]

    let candidateRecordSHA256: String
    let candidateManifestSHA256: String
    let rosterSHA256: String
    let scenarioInventorySHA256: String
    let nativeBuildManifestSHA256: String
    let nativeLibrarySHA256: String
    let sourceCommit: String
    let sourceTreeSHA256: String
    let reviewedSourceClosureSHA256: String
    let deviceUDIDSHA256: String
    let deviceECIDSHA256: String
    let deviceSerialSHA256: String
    let expectedHardwareModel: String
    let expectedBoardConfig: String
    let expectedOSVersion: String
    let expectedOSBuild: String

    init(data: Data) throws {
        let raw = try JSONSerialization.jsonObject(with: data)
        guard let object = raw as? [String: Any] else {
            throw LabFailure.invalid("session manifest is not an object")
        }
        try require(Set(object.keys) == Self.keys, "session manifest fields are not exact")
        try require(
            object["schema"] as? String == "iroha.kagemusha.ios_device_lab.session.v1",
            "session schema is not exact"
        )
        try require((object["version"] as? NSNumber)?.intValue == 1, "session version is not exact")
        try require(object["source_repo_dirty"] as? Bool == false, "session must bind clean source")
        func string(_ key: String) throws -> String {
            guard let value = object[key] as? String, !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines) else {
                throw LabFailure.invalid("invalid session field: \(key)")
            }
            return value
        }
        func digest(_ key: String) throws -> String {
            let value = try string(key)
            try require(
                value.range(of: "^[0-9a-f]{64}$", options: .regularExpression) != nil
                    && value != String(repeating: "0", count: 64),
                "invalid session SHA-256: \(key)"
            )
            return value
        }
        candidateRecordSHA256 = try digest("candidate_record_sha256")
        candidateManifestSHA256 = try digest("candidate_manifest_sha256")
        rosterSHA256 = try digest("topup_finality_roster_sha256")
        scenarioInventorySHA256 = try digest("scenario_inventory_sha256")
        nativeBuildManifestSHA256 = try digest("native_build_manifest_sha256")
        nativeLibrarySHA256 = try digest("native_library_sha256")
        sourceCommit = try string("source_commit")
        try require(
            sourceCommit.range(of: "^[0-9a-f]{40}$", options: .regularExpression) != nil,
            "source commit is not lowercase git hex"
        )
        sourceTreeSHA256 = try digest("source_tree_sha256")
        reviewedSourceClosureSHA256 = try digest("reviewed_source_closure_descriptor_sha256")
        deviceUDIDSHA256 = try digest("device_udid_sha256")
        deviceECIDSHA256 = try digest("device_ecid_sha256")
        deviceSerialSHA256 = try digest("device_serial_sha256")
        expectedHardwareModel = try string("expected_hardware_model")
        expectedBoardConfig = try string("expected_board_config")
        expectedOSVersion = try string("expected_os_version")
        expectedOSBuild = try string("expected_os_build")
    }
}

private final class CountingURLProtocol: URLProtocol {
    private static let lock = NSLock()
    private static var count = 0

    static func reset() {
        lock.lock()
        count = 0
        lock.unlock()
    }

    static func observedCount() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }

    override class func canInit(with request: URLRequest) -> Bool {
        _ = request
        lock.lock()
        count += 1
        lock.unlock()
        return false
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        client?.urlProtocol(self, didFailWithError: LabFailure.invalid("network request blocked"))
    }

    override func stopLoading() {}
}

private final class OfflinePathWindow {
    private let monitor = NWPathMonitor()
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.kagemusha.path-monitor")
    private let lock = NSLock()
    private let firstSample = DispatchSemaphore(value: 0)
    private var signaled = false
    private var samples: [[String: Any]] = []

    func start() throws {
        monitor.pathUpdateHandler = { [weak self] path in
            self?.record(path, label: "callback")
        }
        monitor.start(queue: queue)
        try require(
            firstSample.wait(timeout: .now() + 20) == .success,
            "NWPathMonitor did not produce an initial sample"
        )
    }

    func sample(_ label: String) {
        record(monitor.currentPath, label: label)
    }

    func finish() throws -> [[String: Any]] {
        sample("after")
        monitor.cancel()
        queue.sync {}
        lock.lock()
        let result = samples
        lock.unlock()
        try require(result.count >= 5, "offline window did not contain enough real path samples")
        for sample in result {
            try require(sample["status"] as? String == "unsatisfied", "network path was not offline")
        }
        return result
    }

    private func record(_ path: NWPath, label: String) {
        let status: String
        switch path.status {
        case .satisfied:
            status = "satisfied"
        case .requiresConnection:
            status = "requires_connection"
        case .unsatisfied:
            status = "unsatisfied"
        @unknown default:
            status = "unknown"
        }
        let entry: [String: Any] = [
            "label": label,
            "monotonic_nanos": NSNumber(value: DispatchTime.now().uptimeNanoseconds),
            "status": status,
            "expensive": path.isExpensive,
            "constrained": path.isConstrained,
            "wifi": path.usesInterfaceType(.wifi),
            "cellular": path.usesInterfaceType(.cellular),
            "wired_ethernet": path.usesInterfaceType(.wiredEthernet),
            "loopback": path.usesInterfaceType(.loopback),
        ]
        lock.lock()
        samples.append(entry)
        if !signaled {
            signaled = true
            firstSample.signal()
        }
        lock.unlock()
    }
}

private func callProofPhase(
    candidate: URL,
    roster: URL,
    artifactRoot: URL,
    scenario: URL,
    launchNonce: [UInt8]
) throws -> Data {
    let candidatePath = [UInt8](candidate.path.utf8)
    let rosterPath = [UInt8](roster.path.utf8)
    let artifactPath = [UInt8](artifactRoot.path.utf8)
    let scenarioPath = [UInt8](scenario.path.utf8)
    var output: UnsafeMutablePointer<UInt8>?
    var outputLength: UInt = 0
    let status = candidatePath.withUnsafeBufferPointer { candidateBuffer in
        rosterPath.withUnsafeBufferPointer { rosterBuffer in
            artifactPath.withUnsafeBufferPointer { artifactBuffer in
                scenarioPath.withUnsafeBufferPointer { scenarioBuffer in
                    launchNonce.withUnsafeBufferPointer { nonceBuffer in
                        connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1(
                            candidateBuffer.baseAddress,
                            UInt(candidateBuffer.count),
                            rosterBuffer.baseAddress,
                            UInt(rosterBuffer.count),
                            artifactBuffer.baseAddress,
                            UInt(artifactBuffer.count),
                            scenarioBuffer.baseAddress,
                            UInt(scenarioBuffer.count),
                            nonceBuffer.baseAddress,
                            UInt(nonceBuffer.count),
                            &output,
                            &outputLength
                        )
                    }
                }
            }
        }
    }
    defer {
        if let output {
            connect_norito_free(output)
        }
    }
    try require(status == 0, "native proof phase rejected with status \(status)")
    try require(
        output != nil && outputLength > 0 && outputLength <= UInt(maximumArchiveBytes),
        "native proof phase returned an invalid checkpoint"
    )
    return Data(bytes: output!, count: Int(outputLength))
}

private func callRestartPhase(
    candidate: URL,
    roster: URL,
    artifactRoot: URL,
    scenario: URL,
    checkpoint: Data,
    launchNonce: [UInt8]
) throws -> Data {
    let candidatePath = [UInt8](candidate.path.utf8)
    let rosterPath = [UInt8](roster.path.utf8)
    let artifactPath = [UInt8](artifactRoot.path.utf8)
    let scenarioPath = [UInt8](scenario.path.utf8)
    var output: UnsafeMutablePointer<UInt8>?
    var outputLength: UInt = 0
    let status = candidatePath.withUnsafeBufferPointer { candidateBuffer in
        rosterPath.withUnsafeBufferPointer { rosterBuffer in
            artifactPath.withUnsafeBufferPointer { artifactBuffer in
                scenarioPath.withUnsafeBufferPointer { scenarioBuffer in
                    checkpoint.withUnsafeBytes { checkpointBuffer in
                        launchNonce.withUnsafeBufferPointer { nonceBuffer in
                            connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1(
                                candidateBuffer.baseAddress,
                                UInt(candidateBuffer.count),
                                rosterBuffer.baseAddress,
                                UInt(rosterBuffer.count),
                                artifactBuffer.baseAddress,
                                UInt(artifactBuffer.count),
                                scenarioBuffer.baseAddress,
                                UInt(scenarioBuffer.count),
                                checkpointBuffer.bindMemory(to: UInt8.self).baseAddress,
                                UInt(checkpointBuffer.count),
                                nonceBuffer.baseAddress,
                                UInt(nonceBuffer.count),
                                &output,
                                &outputLength
                            )
                        }
                    }
                }
            }
        }
    }
    defer {
        if let output {
            connect_norito_free(output)
        }
    }
    try require(status == 0, "native restart phase rejected with status \(status)")
    try require(
        output != nil && outputLength > 0 && outputLength <= UInt(maximumArchiveBytes),
        "native restart phase returned an invalid transcript"
    )
    return Data(bytes: output!, count: Int(outputLength))
}

private func codeIdentity() throws -> [String: Any] {
    guard
        let appExecutable = Bundle.main.executableURL,
        let testExecutable = Bundle(for: KagemushaCandidateEvidenceLabTests.self).executableURL
    else {
        throw LabFailure.invalid("signed app/test executables are unavailable")
    }
    return [
        "app_bundle_id": Bundle.main.bundleIdentifier ?? "",
        "app_version": Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "",
        "app_build": Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "",
        "app_executable_sha256": try fileSHA256(appExecutable),
        "test_bundle_id": Bundle(for: KagemushaCandidateEvidenceLabTests.self).bundleIdentifier ?? "",
        "test_executable_sha256": try fileSHA256(testExecutable),
    ]
}

private func deviceIdentity(_ session: Session) throws -> [String: Any] {
    #if targetEnvironment(simulator)
    throw LabFailure.invalid("candidate evidence cannot run in a simulator")
    #else
    let model = try sysctlString("hw.machine")
    let osBuild = try sysctlString("kern.osversion")
    let osVersion = UIDevice.current.systemVersion
    try require(model == session.expectedHardwareModel, "physical device hardware model changed")
    try require(osVersion == session.expectedOSVersion, "physical device OS version changed")
    try require(osBuild == session.expectedOSBuild, "physical device OS build changed")
    guard let vendorIdentifier = UIDevice.current.identifierForVendor?.uuidString else {
        throw LabFailure.invalid("identifierForVendor is unavailable")
    }
    return [
        "physical": true,
        "simulator": false,
        "platform": "ios",
        "hardware_model": model,
        "board_config": session.expectedBoardConfig,
        "os_version": osVersion,
        "os_build": osBuild,
        "udid_sha256": session.deviceUDIDSHA256,
        "ecid_sha256": session.deviceECIDSHA256,
        "serial_sha256": session.deviceSerialSHA256,
        "identifier_for_vendor_sha256": sha256(Data(vendorIdentifier.utf8)),
        "boot_session_sha256": try bootSessionSHA256(),
    ]
    #endif
}

private func validateStagedInputs(_ root: URL, session: Session) throws {
    try validateDirectory(root, ownerPrivate: true)
    let candidate = try readRegular(
        root.appendingPathComponent("candidate-v4.norito"),
        maximum: 1024 * 1024
    )
    try require(sha256(candidate) == session.candidateRecordSHA256, "candidate digest changed")
    let candidateManifest = try readRegular(
        root.appendingPathComponent("candidate-manifest-v4.norito"),
        maximum: 1024 * 1024
    )
    try require(
        sha256(candidateManifest) == session.candidateManifestSHA256,
        "candidate manifest digest changed"
    )
    let roster = try readRegular(
        root.appendingPathComponent("topup-finality-roster-v4.norito"),
        maximum: 16 * 1024 * 1024
    )
    try require(sha256(roster) == session.rosterSHA256, "roster digest changed")
    let nativeManifest = try readRegular(
        root.appendingPathComponent("native-build-manifest.json"),
        maximum: 1024 * 1024
    )
    try require(
        sha256(nativeManifest) == session.nativeBuildManifestSHA256,
        "native build manifest digest changed"
    )
    let closure = try readRegular(
        root.appendingPathComponent("reviewed-source-closure-v1.json"),
        maximum: 64 * 1024 * 1024
    )
    try require(
        sha256(closure) == session.reviewedSourceClosureSHA256,
        "reviewed source closure digest changed"
    )
    try validateDirectory(root.appendingPathComponent("artifacts"), ownerPrivate: true)
    try validateDirectory(root.appendingPathComponent("scenario"), ownerPrivate: true)
}

private func commonReceipt(
    phase: String,
    session: Session,
    launchNonce: [UInt8],
    installIdentity: Data,
    checkpoint: Data,
    networkSamples: [[String: Any]],
    observedRequests: Int
) throws -> [String: Any] {
    try require(observedRequests == 0, "URL loading system observed a network request")
    return [
        "schema": "iroha.kagemusha.ios_device_lab.launch_receipt.v1",
        "version": 1,
        "phase": phase,
        "process_id": Int(getpid()),
        "launch_nonce_sha256": sha256(launchNonce),
        "recorded_at_utc": recordedAtUTC(),
        "monotonic_nanos": NSNumber(value: DispatchTime.now().uptimeNanoseconds),
        "resource_ceiling_bytes": NSNumber(value: resourceCeilingBytes),
        "candidate_record_sha256": session.candidateRecordSHA256,
        "candidate_manifest_sha256": session.candidateManifestSHA256,
        "topup_finality_roster_sha256": session.rosterSHA256,
        "scenario_inventory_sha256": session.scenarioInventorySHA256,
        "native_build_manifest_sha256": session.nativeBuildManifestSHA256,
        "native_library_sha256": session.nativeLibrarySHA256,
        "source_commit": session.sourceCommit,
        "source_tree_sha256": session.sourceTreeSHA256,
        "source_repo_dirty": false,
        "reviewed_source_closure_descriptor_sha256": session.reviewedSourceClosureSHA256,
        "install_identity_sha256": sha256(installIdentity),
        "checkpoint_size_bytes": checkpoint.count,
        "checkpoint_sha256": sha256(checkpoint),
        "device": try deviceIdentity(session),
        "code_identity": try codeIdentity(),
        "network_monitor": "NWPathMonitor",
        "network_samples": networkSamples,
        "url_protocol_observed_request_count": observedRequests,
        "device_attestation_policy": "taira-testnet-physical-ios-xcode-paired-v1",
        "app_attest_used": false,
    ]
}

final class KagemushaCandidateEvidenceLabTests: XCTestCase {
    func testProofPhase() throws {
        #if targetEnvironment(simulator)
        throw XCTSkip("simulator is explicitly unsupported")
        #else
        let input = try inputDirectory()
        let output = try outputDirectory()
        let manager = FileManager.default
        if manager.fileExists(atPath: output.path) {
            try manager.removeItem(at: output)
        }
        try manager.createDirectory(
            at: output,
            withIntermediateDirectories: false,
            attributes: [.posixPermissions: NSNumber(value: 0o700)]
        )
        try validateDirectory(output, ownerPrivate: true)
        let sessionData = try readRegular(
            input.appendingPathComponent("session-v1.json"),
            maximum: 1024 * 1024
        )
        let session = try Session(data: sessionData)
        try validateStagedInputs(input, session: session)
        let launchNonce = try random32()
        let installIdentity = Data(try random32())
        let installIdentityURL = output.appendingPathComponent("install-identity-v1.bin")
        try writeDurably(installIdentity, to: installIdentityURL)

        CountingURLProtocol.reset()
        try require(
            URLProtocol.registerClass(CountingURLProtocol.self),
            "URLProtocol request observer could not be registered"
        )
        defer { URLProtocol.unregisterClass(CountingURLProtocol.self) }
        let pathWindow = OfflinePathWindow()
        try pathWindow.start()
        pathWindow.sample("before")
        pathWindow.sample("through_before_native")
        let checkpoint = try callProofPhase(
            candidate: input.appendingPathComponent("candidate-v4.norito"),
            roster: input.appendingPathComponent("topup-finality-roster-v4.norito"),
            artifactRoot: input.appendingPathComponent("artifacts", isDirectory: true),
            scenario: input.appendingPathComponent("scenario", isDirectory: true),
            launchNonce: launchNonce
        )
        pathWindow.sample("through_after_native")
        let networkSamples = try pathWindow.finish()
        let observedRequests = CountingURLProtocol.observedCount()
        let checkpointURL = output.appendingPathComponent("checkpoint-v1.norito")
        try writeDurably(checkpoint, to: checkpointURL)
        let reopenedCheckpoint = try readRegular(
            checkpointURL,
            maximum: maximumArchiveBytes
        )
        try require(reopenedCheckpoint == checkpoint, "checkpoint reopen is not exact")
        let receipt = try commonReceipt(
            phase: "proof",
            session: session,
            launchNonce: launchNonce,
            installIdentity: installIdentity,
            checkpoint: reopenedCheckpoint,
            networkSamples: networkSamples,
            observedRequests: observedRequests
        )
        try writeDurably(
            canonicalJSON(receipt),
            to: output.appendingPathComponent("proof-launch-receipt-v1.json")
        )
        #endif
    }

    func testRestartPhase() throws {
        #if targetEnvironment(simulator)
        throw XCTSkip("simulator is explicitly unsupported")
        #else
        let input = try inputDirectory()
        let output = try outputDirectory()
        try validateDirectory(output, ownerPrivate: true)
        let sessionData = try readRegular(
            input.appendingPathComponent("session-v1.json"),
            maximum: 1024 * 1024
        )
        let session = try Session(data: sessionData)
        try validateStagedInputs(input, session: session)
        let checkpoint = try readRegular(
            output.appendingPathComponent("checkpoint-v1.norito"),
            maximum: maximumArchiveBytes
        )
        let installIdentity = try readRegular(
            output.appendingPathComponent("install-identity-v1.bin"),
            maximum: 32
        )
        try require(installIdentity.count == 32, "install identity has the wrong size")
        let proofReceiptData = try readRegular(
            output.appendingPathComponent("proof-launch-receipt-v1.json"),
            maximum: 4 * 1024 * 1024
        )
        guard
            let proofReceipt = try JSONSerialization.jsonObject(with: proofReceiptData)
                as? [String: Any],
            let proofPID = (proofReceipt["process_id"] as? NSNumber)?.int32Value,
            let proofNonceSHA256 = proofReceipt["launch_nonce_sha256"] as? String
        else {
            throw LabFailure.invalid("proof launch receipt is malformed")
        }
        try require(proofReceipt["phase"] as? String == "proof", "proof receipt phase is not exact")
        try require(
            proofReceipt["checkpoint_sha256"] as? String == sha256(checkpoint)
                && (proofReceipt["checkpoint_size_bytes"] as? NSNumber)?.intValue
                    == checkpoint.count,
            "proof receipt does not bind the reopened checkpoint"
        )
        try require(
            proofReceipt["install_identity_sha256"] as? String == sha256(installIdentity),
            "proof receipt does not bind the persisted install identity"
        )
        try require(
            proofReceipt["candidate_record_sha256"] as? String
                == session.candidateRecordSHA256
                && proofReceipt["source_tree_sha256"] as? String
                    == session.sourceTreeSHA256
                && proofReceipt["reviewed_source_closure_descriptor_sha256"] as? String
                    == session.reviewedSourceClosureSHA256,
            "proof receipt does not bind the current candidate/source closure"
        )
        try require(proofPID != getpid(), "restart phase reused the proof process")
        let launchNonce = try random32()
        try require(sha256(launchNonce) != proofNonceSHA256, "restart launch nonce was reused")

        CountingURLProtocol.reset()
        try require(
            URLProtocol.registerClass(CountingURLProtocol.self),
            "URLProtocol request observer could not be registered"
        )
        defer { URLProtocol.unregisterClass(CountingURLProtocol.self) }
        let pathWindow = OfflinePathWindow()
        try pathWindow.start()
        pathWindow.sample("before")
        pathWindow.sample("through_before_native")
        let transcript = try callRestartPhase(
            candidate: input.appendingPathComponent("candidate-v4.norito"),
            roster: input.appendingPathComponent("topup-finality-roster-v4.norito"),
            artifactRoot: input.appendingPathComponent("artifacts", isDirectory: true),
            scenario: input.appendingPathComponent("scenario", isDirectory: true),
            checkpoint: checkpoint,
            launchNonce: launchNonce
        )
        pathWindow.sample("through_after_native")
        let networkSamples = try pathWindow.finish()
        let observedRequests = CountingURLProtocol.observedCount()
        guard let native = try JSONSerialization.jsonObject(with: transcript) as? [String: Any] else {
            throw LabFailure.invalid("native transcript is not a JSON object")
        }
        try require(
            native["schema"] as? String
                == "iroha.kagemusha.ios_device_lab.native_transcript.v1",
            "native transcript schema is not exact"
        )
        try require(
            (native["exact_operation_count"] as? NSNumber)?.intValue == 28
                && (native["causal_events"] as? [Any])?.count == 28,
            "native transcript does not contain the exact lifecycle"
        )
        try require(
            (native["duplicate_error_code"] as? NSNumber)?.intValue == -311,
            "native transcript did not observe exact duplicate rejection -311"
        )
        try require(
            native["final_unspent_atomic_units"] as? String == "0",
            "native transcript retained unspent value"
        )
        try require(
            (native["resource_ceiling_bytes"] as? NSNumber)?.uint64Value
                == resourceCeilingBytes,
            "native transcript changed the fixed resource policy"
        )
        try require(
            (native["proof_process_id"] as? NSNumber)?.int32Value == proofPID
                && (native["restart_process_id"] as? NSNumber)?.int32Value == getpid(),
            "native transcript process identities do not close over both launches"
        )
        try require(
            native["candidate_record_sha256"] as? String == session.candidateRecordSHA256
                && native["candidate_manifest_sha256"] as? String
                    == session.candidateManifestSHA256
                && native["scenario_inventory_sha256"] as? String
                    == session.scenarioInventorySHA256
                && native["source_commit"] as? String == session.sourceCommit
                && native["source_tree_sha256"] as? String == session.sourceTreeSHA256
                && native["reviewed_source_closure_descriptor_sha256"] as? String
                    == session.reviewedSourceClosureSHA256,
            "native transcript does not bind the current candidate/source/scenario"
        )
        try require(
            native["proof_launch_nonce_sha256"] as? String == proofNonceSHA256
                && native["restart_launch_nonce_sha256"] as? String
                    == sha256(launchNonce),
            "native transcript launch nonces differ from both process receipts"
        )
        try require(
            (native["artifact_inventory"] as? [Any])?.count == 8,
            "native transcript does not contain the exact artifact inventory"
        )
        let transcriptURL = output.appendingPathComponent("native-transcript-v1.json")
        try writeDurably(transcript, to: transcriptURL)
        var receipt = try commonReceipt(
            phase: "restart",
            session: session,
            launchNonce: launchNonce,
            installIdentity: installIdentity,
            checkpoint: checkpoint,
            networkSamples: networkSamples,
            observedRequests: observedRequests
        )
        receipt["native_transcript_size_bytes"] = transcript.count
        receipt["native_transcript_sha256"] = sha256(transcript)
        receipt["proof_launch_receipt_sha256"] = sha256(proofReceiptData)
        try writeDurably(
            canonicalJSON(receipt),
            to: output.appendingPathComponent("restart-launch-receipt-v1.json")
        )
        #endif
    }
}
