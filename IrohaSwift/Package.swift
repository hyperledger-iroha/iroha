// swift-tools-version:5.9
import Foundation
import CoreFoundation
import PackageDescription
#if canImport(CryptoKit)
import CryptoKit
#endif
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bridgeRelativePath = "../dist/NoritoBridge.xcframework"
let requiredBridgeAbiVersion = 22
let requiredWorkspaceCargoLockSha256 =
    "179f589da420c024725efd9a65adb9c1e34085fa022cc01a8c67bb2262e93bf7"
let requiredPrivacyReleaseCargoLockSha256 =
    "31b5af592c235ce7a24e9ea219ceaa5c2f74400b650c5121182425d93e39811d"
let requiredBridgeRequiredSymbolsSha256 =
    "7c3bf8ff069072318286078dc07789a8d34f17e7cf4290ce91870072325b019d"
let requiredBridgeForbiddenSymbolsSha256 =
    "e2b5affd16c74bd43802648a7d9d521db3a588db43a717d00a0dbb72cf004380"
let requiredBridgeProductionRolesSha256 =
    "bf354c216b682a2bf5ba00226c7a09d5d6f9f8777b2312b900f39ddb5148d9bb"
let requiredBridgePrivacyEnvironmentProfilesSha256 =
    "40c38b02d510ee8dfdc37b3f3943e8c12568f1b8ce1cff301f2aa067f63c5f44"
let requiredBridgeManifestFields: Set<String> = [
    "version",
    "native_bridge_abi_version",
    "privacy_production_enabled",
    "cargo_features",
    "build_environment",
    "source_commit",
    "source_tree_dirty",
    "source_fingerprint_sha256",
    "workspace_cargo_lock_sha256",
    "build_cargo_lock_sha256",
    "build_cargo_lock_authority",
    "bridge_header_sha256",
    "required_symbols",
    "forbidden_symbols",
    "kagemusha_mobile_artifact_roles",
    "hashes",
]
let requiredBridgeSliceIdentifiers: Set<String> = [
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64_x86_64",
]
let requiredBridgeBuildEnvironmentFields: Set<String> = [
    "schema",
    "hermetic_runner_schema",
    "hermetic_runner_sha256",
    "environment_profiles",
    "cargo_build_jobs",
    "rust_toolchain_channel",
    "cargo_release",
    "cargo_commit_hash",
    "cargo_binary_sha256",
    "rustc_release",
    "rustc_commit_hash",
    "rustc_binary_sha256",
    "rustdoc_release",
    "rustdoc_commit_hash",
    "rustdoc_binary_sha256",
    "python_version",
    "python_binary_sha256",
    "git_version",
    "git_binary_sha256",
    "rustup_version",
    "rustup_binary_sha256",
    "xcode_version",
    "xcode_build_version",
    "iphoneos_sdk_version",
    "iphonesimulator_sdk_version",
    "macosx_sdk_version",
    "iphoneos_deployment_target",
    "iphonesimulator_deployment_target",
    "macosx_deployment_target",
]
let requiredPrivacyBuildEnvironment: Set<String> = [
    "CARGO",
    "CARGO_BUILD_JOBS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "CARGO_INCREMENTAL",
    "CARGO_NET_OFFLINE",
    "CARGO_TARGET_DIR",
    "DEVELOPER_DIR",
    "HOME",
    "IROHA_PRIVACY_AUTHENTICATED_APPLE_CARGO_PROFILE",
    "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGET",
    "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_GIT_LINK_STATE",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_REGISTRY_LINK_STATE",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR",
    "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE",
    "IROHA_PRIVACY_AUTHENTICATED_DEVELOPER_DIR",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTC_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTC_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_PATH",
    "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SEAL",
    "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR",
    "IROHA_PRIVACY_AUTHENTICATED_SDKROOT",
    "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE",
    "IROHA_PRIVACY_CARGO_AUDIT_PATH",
    "IROHA_PRIVACY_CARGO_LOCKFILE_PATH",
    "IROHA_PRIVACY_LOCKFILE_PYTHON_BIN",
    "IROHA_PRIVACY_REAL_CARGO",
    "IROHA_PRIVACY_SDK_ROOT",
    "LANG",
    "LC_ALL",
    "NORITO_SKIP_BINDINGS_SYNC",
    "PATH",
    "RUSTC_BOOTSTRAP",
    "SDKROOT",
    "TMPDIR",
]
let requiredPrivacyEnvironmentProfileAllowLists: [String: [String]] = [
    "privacy-apple-ios-device-arm64": Array(
        requiredPrivacyBuildEnvironment.union(["IPHONEOS_DEPLOYMENT_TARGET"])
    ).sorted(),
    "privacy-apple-ios-simulator-arm64": Array(
        requiredPrivacyBuildEnvironment.union([
            "IPHONEOS_DEPLOYMENT_TARGET",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        ])
    ).sorted(),
    "privacy-apple-ios-simulator-x86_64": Array(
        requiredPrivacyBuildEnvironment.union([
            "IPHONEOS_DEPLOYMENT_TARGET",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        ])
    ).sorted(),
    "privacy-apple-macos-arm64": Array(
        requiredPrivacyBuildEnvironment.union(["MACOSX_DEPLOYMENT_TARGET"])
    ).sorted(),
    "privacy-apple-macos-x86_64": Array(
        requiredPrivacyBuildEnvironment.union(["MACOSX_DEPLOYMENT_TARGET"])
    ).sorted(),
]
let requiredBridgeTopLevelEntries: Set<String> = [
    ".privacy-production-enabled",
    "Info.plist",
    "NoritoBridge.artifacts.json",
    "ios-arm64",
    "ios-arm64_x86_64-simulator",
    "macos-arm64_x86_64",
]
let requiredBridgeHeaderEntries: Set<String> = [
    "NoritoBridge.h",
    "connect_norito_bridge.h",
    "module.modulemap",
]
let repositoryDirectory = packageDirectory.deletingLastPathComponent().standardizedFileURL
let configuredArtifactDirectory = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_APPLE_ARTIFACT_DIR"
]
let requireExternalArtifactInput = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"
]
guard requireExternalArtifactInput == nil
    || requireExternalArtifactInput == "0"
    || requireExternalArtifactInput == "1"
else {
    fatalError(
        "error: MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT must be exactly 0 or 1."
    )
}
let requireExternalArtifact = requireExternalArtifactInput == "1"
let requirePrivacyProductionArtifactInput = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT"
]
guard requirePrivacyProductionArtifactInput == nil
    || requirePrivacyProductionArtifactInput == "0"
    || requirePrivacyProductionArtifactInput == "1"
else {
    fatalError(
        "error: MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT must be exactly 0 or 1."
    )
}
let requirePrivacyProductionArtifact =
    requirePrivacyProductionArtifactInput == "1"
if requirePrivacyProductionArtifact, !requireExternalArtifact {
    fatalError(
        "error: MOBILE_SDK_REQUIRE_PRIVACY_PRODUCTION_APPLE_ARTIFACT=1 requires MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1."
    )
}
if requireExternalArtifact, configuredArtifactDirectory == nil {
    fatalError(
        """
        error: external Apple artifact validation requires MOBILE_SDK_APPLE_ARTIFACT_DIR \
        to identify an artifact directory outside the Iroha source tree.
        """
    )
}

func relativePath(from base: URL, to destination: URL) -> String {
    let baseComponents = base.pathComponents
    let destinationComponents = destination.pathComponents
    var commonComponentCount = 0
    while
        commonComponentCount < baseComponents.count,
        commonComponentCount < destinationComponents.count,
        baseComponents[commonComponentCount] == destinationComponents[commonComponentCount]
    {
        commonComponentCount += 1
    }
    let parentComponents = Array(
        repeating: "..",
        count: baseComponents.count - commonComponentCount
    )
    let childComponents = destinationComponents.dropFirst(commonComponentCount)
    return (parentComponents + childComponents).joined(separator: "/")
}

func canonicalExistingFilesystemPath(_ path: String) -> String? {
#if canImport(Darwin) || canImport(Glibc)
    path.withCString { encodedPath in
        guard let resolvedPath = realpath(encodedPath, nil) else {
            return nil
        }
        defer { free(resolvedPath) }
        return String(cString: resolvedPath)
    }
#else
    guard FileManager.default.fileExists(atPath: path) else { return nil }
    return URL(fileURLWithPath: path)
        .resolvingSymlinksInPath()
        .standardizedFileURL.path
#endif
}

func filesystemType(at url: URL) -> FileAttributeType? {
    guard let attributes = try? FileManager.default.attributesOfItem(
        atPath: url.path
    ) else {
        return nil
    }
    return attributes[.type] as? FileAttributeType
}

func isNonSymbolicRegularFile(_ url: URL) -> Bool {
    filesystemType(at: url) == .typeRegular
}

func isNonSymbolicDirectory(_ url: URL) -> Bool {
    filesystemType(at: url) == .typeDirectory
}

func exactDirectoryEntries(at url: URL, expected: Set<String>) -> Bool {
    guard isNonSymbolicDirectory(url),
          let entries = try? FileManager.default.contentsOfDirectory(
              atPath: url.path
          ) else {
        return false
    }
    return Set(entries) == expected
}

func boundedRegularFileData(at url: URL, maximumByteCount: UInt64) -> Data? {
    guard isNonSymbolicRegularFile(url),
          let attributes = try? FileManager.default.attributesOfItem(
              atPath: url.path
          ),
          let size = attributes[.size] as? NSNumber,
          size.uint64Value > 0,
          size.uint64Value <= maximumByteCount else {
        return nil
    }
    return try? Data(contentsOf: url, options: [.mappedIfSafe])
}

func canonicalLowercaseHex(_ value: Any?, byteCount: Int) -> String? {
    guard let value = value as? String,
          value.utf8.count == byteCount * 2,
          value.utf8.allSatisfy({
              (0x30 ... 0x39).contains($0) || (0x61 ... 0x66).contains($0)
          }) else {
        return nil
    }
    return value
}

func matchesEntireString(_ value: Any?, pattern: String) -> Bool {
    guard let value = value as? String,
          let expression = try? NSRegularExpression(pattern: pattern) else {
        return false
    }
    let range = NSRange(value.startIndex..<value.endIndex, in: value)
    guard let match = expression.firstMatch(in: value, range: range) else {
        return false
    }
    return match.range == range
}

func sourceHasExactlyOneMatch(at url: URL, pattern: String) -> Bool {
    guard isNonSymbolicRegularFile(url),
          let source = try? String(contentsOf: url, encoding: .utf8),
          let expression = try? NSRegularExpression(pattern: pattern) else {
        return false
    }
    let range = NSRange(source.startIndex..<source.endIndex, in: source)
    return expression.numberOfMatches(in: source, range: range) == 1
}

struct StrictJSONDuplicateKeyScanner {
    let bytes: [UInt8]
    var index = 0

    mutating func scan() -> Bool {
        guard parseValue() else { return false }
        skipWhitespace()
        return index == bytes.count
    }

    mutating func skipWhitespace() {
        while index < bytes.count,
              bytes[index] == 0x20 || bytes[index] == 0x09
                || bytes[index] == 0x0A || bytes[index] == 0x0D {
            index += 1
        }
    }

    mutating func consume(_ byte: UInt8) -> Bool {
        skipWhitespace()
        guard index < bytes.count, bytes[index] == byte else { return false }
        index += 1
        return true
    }

    mutating func parseValue() -> Bool {
        skipWhitespace()
        guard index < bytes.count else { return false }
        switch bytes[index] {
        case 0x7B:
            return parseObject()
        case 0x5B:
            return parseArray()
        case 0x22:
            return parseString() != nil
        default:
            let start = index
            while index < bytes.count,
                  ![0x20, 0x09, 0x0A, 0x0D, 0x2C, 0x5D, 0x7D]
                    .contains(bytes[index]) {
                index += 1
            }
            return index > start
        }
    }

    mutating func parseObject() -> Bool {
        guard consume(0x7B) else { return false }
        skipWhitespace()
        if index < bytes.count, bytes[index] == 0x7D {
            index += 1
            return true
        }
        var keys = Set<String>()
        while true {
            guard let key = parseString(), keys.insert(key).inserted,
                  consume(0x3A), parseValue() else {
                return false
            }
            skipWhitespace()
            guard index < bytes.count else { return false }
            if bytes[index] == 0x7D {
                index += 1
                return true
            }
            guard bytes[index] == 0x2C else { return false }
            index += 1
        }
    }

    mutating func parseArray() -> Bool {
        guard consume(0x5B) else { return false }
        skipWhitespace()
        if index < bytes.count, bytes[index] == 0x5D {
            index += 1
            return true
        }
        while true {
            guard parseValue() else { return false }
            skipWhitespace()
            guard index < bytes.count else { return false }
            if bytes[index] == 0x5D {
                index += 1
                return true
            }
            guard bytes[index] == 0x2C else { return false }
            index += 1
        }
    }

    mutating func parseString() -> String? {
        skipWhitespace()
        guard index < bytes.count, bytes[index] == 0x22 else { return nil }
        let start = index
        index += 1
        while index < bytes.count {
            if bytes[index] == 0x22 {
                index += 1
                let encoded = Data(bytes[start..<index])
                return try? JSONSerialization.jsonObject(
                    with: encoded,
                    options: [.fragmentsAllowed]
                ) as? String
            }
            if bytes[index] == 0x5C {
                index += 1
                guard index < bytes.count else { return nil }
            }
            index += 1
        }
        return nil
    }
}

func hasNoDuplicateJSONMembers(_ data: Data) -> Bool {
    var scanner = StrictJSONDuplicateKeyScanner(bytes: Array(data))
    return scanner.scan()
}

func canonicalJSONBoolean(_ value: Any?) -> Bool? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) == CFBooleanGetTypeID() else {
        return nil
    }
    return number.boolValue
}

func canonicalJSONInteger(_ value: Any?) -> Int? {
    guard let number = value as? NSNumber,
          CFGetTypeID(number) != CFBooleanGetTypeID(),
          !["f", "d"].contains(String(cString: number.objCType)) else {
        return nil
    }
    return number.intValue
}

func hasOnlyCanonicalJSONNumberTypes(_ value: Any) -> Bool {
    if let dictionary = value as? [String: Any] {
        return dictionary.values.allSatisfy(hasOnlyCanonicalJSONNumberTypes)
    }
    if let array = value as? [Any] {
        return array.allSatisfy(hasOnlyCanonicalJSONNumberTypes)
    }
    if let number = value as? NSNumber {
        return CFGetTypeID(number) == CFBooleanGetTypeID()
            || !["f", "d"].contains(String(cString: number.objCType))
    }
    return value is String || value is NSNull
}

func sha256Hex(of data: Data) -> String? {
#if canImport(CryptoKit)
    return SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
#else
    return nil
#endif
}

func sha256Hex(of url: URL) -> String? {
#if canImport(CryptoKit)
    guard isNonSymbolicRegularFile(url),
          let handle = try? FileHandle(forReadingFrom: url) else {
        return nil
    }
    defer { try? handle.close() }
    var digest = SHA256()
    while true {
        let chunk: Data
        do {
            chunk = try handle.read(upToCount: 1_048_576) ?? Data()
        } catch {
            return nil
        }
        guard !chunk.isEmpty else { break }
        digest.update(data: chunk)
    }
    return digest.finalize().map { String(format: "%02x", $0) }.joined()
#else
    return nil
#endif
}

func nulDelimitedStringArraySHA256(_ values: [String]) -> String? {
    var payload = Data()
    for value in values {
        payload.append(contentsOf: value.utf8)
        payload.append(0)
    }
    return sha256Hex(of: payload)
}

func canonicalJSONSHA256(_ value: Any) -> String? {
    guard JSONSerialization.isValidJSONObject(value),
          let data = try? JSONSerialization.data(
              withJSONObject: value,
              options: [.sortedKeys]
          ) else {
        return nil
    }
    return sha256Hex(of: data)
}

func stringArrayMap(_ value: Any?) -> [String: [String]]? {
    guard let raw = value as? [String: Any] else { return nil }
    var result: [String: [String]] = [:]
    for (key, child) in raw {
        guard let values = child as? [String] else { return nil }
        result[key] = values
    }
    return result
}

func checkedInBridgeSlicePins() -> [String: String]? {
    let loaderURL = packageDirectory.appendingPathComponent(
        "Sources/IrohaSwift/NativeBridge.swift",
        isDirectory: false
    )
    let blockPattern = #"(?m)^    private static let expectedHashes: \[String: String\] = \[\n((?:[ \t]+\"(?:ios-arm64|ios-arm64_x86_64-simulator|macos-arm64_x86_64)\": \"[0-9a-f]{64}\",?\n){3})    \]$"#
    let linePattern = #"^[ \t]+\"(ios-arm64|ios-arm64_x86_64-simulator|macos-arm64_x86_64)\": \"([0-9a-f]{64})\"(,?)$"#
    guard isNonSymbolicRegularFile(loaderURL),
          let source = try? String(contentsOf: loaderURL, encoding: .utf8),
          let blockExpression = try? NSRegularExpression(pattern: blockPattern),
          let lineExpression = try? NSRegularExpression(pattern: linePattern) else {
        return nil
    }
    let range = NSRange(source.startIndex..<source.endIndex, in: source)
    let blocks = blockExpression.matches(in: source, range: range)
    guard blocks.count == 1,
          let bodyRange = Range(blocks[0].range(at: 1), in: source) else {
        return nil
    }
    let lines = source[bodyRange].split(separator: "\n")
    guard lines.count == requiredBridgeSliceIdentifiers.count else { return nil }
    var pins: [String: String] = [:]
    var suffixes: [String] = []
    for rawLine in lines {
        let line = String(rawLine)
        let lineRange = NSRange(line.startIndex..<line.endIndex, in: line)
        guard let match = lineExpression.firstMatch(in: line, range: lineRange),
              match.range == lineRange,
              let identifierRange = Range(match.range(at: 1), in: line),
              let digestRange = Range(match.range(at: 2), in: line),
              let suffixRange = Range(match.range(at: 3), in: line) else {
            return nil
        }
        let identifier = String(line[identifierRange])
        let digest = String(line[digestRange])
        suffixes.append(String(line[suffixRange]))
        guard pins.updateValue(digest, forKey: identifier) == nil else {
            return nil
        }
    }
    guard suffixes == [",", ",", ""],
          Set(pins.keys) == requiredBridgeSliceIdentifiers else {
        return nil
    }
    return pins
}

func canonicalPackageVersion() -> String? {
    let versionURL = packageDirectory.appendingPathComponent("VERSION")
    let semanticVersionPattern = #"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"#
    guard let data = boundedRegularFileData(
        at: versionURL,
        maximumByteCount: 1_024
    ),
          let contents = String(data: data, encoding: .utf8),
          contents.hasSuffix("\n") else {
        return nil
    }
    let version = String(contents.dropLast())
    guard Data((version + "\n").utf8) == data,
          matchesEntireString(version, pattern: semanticVersionPattern) else {
        return nil
    }
    return version
}

func validateReviewedBuildEnvironment(_ value: Any?) -> Bool {
    guard let environment = value as? [String: Any],
          Set(environment.keys) == requiredBridgeBuildEnvironmentFields,
          environment["schema"] as? String
            == "iroha.mobile-native-build-environment.v2",
          environment["hermetic_runner_schema"] as? String
            == "iroha.mobile-hermetic-command.v2",
          canonicalJSONInteger(environment["cargo_build_jobs"]) == 1,
          stringArrayMap(environment["environment_profiles"])
            == requiredPrivacyEnvironmentProfileAllowLists,
          canonicalJSONSHA256(requiredPrivacyEnvironmentProfileAllowLists)
            == requiredBridgePrivacyEnvironmentProfilesSha256,
          environment["rust_toolchain_channel"] as? String == "1.93.1",
          environment["cargo_release"] as? String == "1.93.1",
          environment["rustc_release"] as? String == "1.93.1",
          environment["rustdoc_release"] as? String == "1.93.1",
          environment["iphoneos_deployment_target"] as? String == "15.0",
          environment["iphonesimulator_deployment_target"] as? String == "15.0",
          environment["macosx_deployment_target"] as? String == "12.0",
          let rustcCommit = canonicalLowercaseHex(
              environment["rustc_commit_hash"],
              byteCount: 20
          ),
          environment["rustdoc_commit_hash"] as? String == rustcCommit,
          canonicalLowercaseHex(
              environment["cargo_commit_hash"],
              byteCount: 20
          ) != nil else {
        return false
    }
    for field in [
        "hermetic_runner_sha256",
        "cargo_binary_sha256",
        "rustc_binary_sha256",
        "rustdoc_binary_sha256",
        "python_binary_sha256",
        "git_binary_sha256",
        "rustup_binary_sha256",
    ] where canonicalLowercaseHex(environment[field], byteCount: 32) == nil {
        return false
    }
    guard matchesEntireString(
              environment["python_version"],
              pattern: #"3\.12\.[0-9]+"#
          ),
          matchesEntireString(
              environment["git_version"],
              pattern: #"[0-9]+(?:\.[0-9]+){1,3}"#
          ),
          matchesEntireString(
              environment["rustup_version"],
              pattern: #"[0-9]+(?:\.[0-9]+){1,2}"#
          ),
          matchesEntireString(
              environment["xcode_version"],
              pattern: #"[0-9]+(?:\.[0-9]+){0,2}"#
          ),
          matchesEntireString(
              environment["xcode_build_version"],
              pattern: #"[A-Za-z0-9.]+"#
          ) else {
        return false
    }
    for field in [
        "iphoneos_sdk_version",
        "iphonesimulator_sdk_version",
        "macosx_sdk_version",
    ] where !matchesEntireString(
        environment[field],
        pattern: #"[0-9]+(?:\.[0-9]+){1,2}"#
    ) {
        return false
    }
    let runner = repositoryDirectory.appendingPathComponent(
        "scripts/run_mobile_hermetic_command.py"
    )
    return sha256Hex(of: runner) == environment["hermetic_runner_sha256"] as? String
}

func reviewedSourceAbiIsExact(_ headerDigest: String) -> Bool {
    let header = repositoryDirectory.appendingPathComponent(
        "crates/connect_norito_bridge/include/connect_norito_bridge.h"
    )
    let bridge = repositoryDirectory.appendingPathComponent(
        "crates/connect_norito_bridge/src/lib.rs"
    )
    let protocolSource = repositoryDirectory.appendingPathComponent(
        "crates/iroha_data_model/src/privacy/protocol.rs"
    )
    return sha256Hex(of: header) == headerDigest
        && sourceHasExactlyOneMatch(
            at: header,
            pattern: #"(?m)^#define[ \t]+CONNECT_NORITO_BRIDGE_ABI_VERSION[ \t]+22[ \t]*$"#
        )
        && sourceHasExactlyOneMatch(
            at: bridge,
            pattern: #"(?m)^const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1;$"#
        )
        && sourceHasExactlyOneMatch(
            at: protocolSource,
            pattern: #"(?m)^pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 22;$"#
        )
}

func validateReviewedBridgeFilesystem(
    at artifactRoot: URL,
    metadata: [String: Any],
    libraries: [[String: Any]],
    manifestHashes: [String: String],
    headerDigest: String
) -> String? {
    guard Set(metadata.keys) == [
              "AvailableLibraries",
              "CFBundlePackageType",
              "XCFrameworkFormatVersion",
          ],
          metadata["CFBundlePackageType"] as? String == "XFWK",
          metadata["XCFrameworkFormatVersion"] as? String == "1.0" else {
        return "error: reviewed NoritoBridge metadata identity is not canonical."
    }
    guard exactDirectoryEntries(
        at: artifactRoot,
        expected: requiredBridgeTopLevelEntries
    ) else {
        return "error: reviewed NoritoBridge top-level inventory is not exact."
    }
    let privacyMarker = artifactRoot.appendingPathComponent(
        ".privacy-production-enabled"
    )
    guard isNonSymbolicRegularFile(privacyMarker),
          let markerAttributes = try? FileManager.default.attributesOfItem(
              atPath: privacyMarker.path
          ),
          (markerAttributes[.size] as? NSNumber)?.uint64Value == 0 else {
        return "error: reviewed NoritoBridge privacy-production marker is invalid."
    }
    let publicManifest = artifactRoot.deletingLastPathComponent()
        .appendingPathComponent("NoritoBridge.artifacts.json")
    let expectedLinkTarget =
        "NoritoBridge.xcframework/NoritoBridge.artifacts.json"
    guard filesystemType(at: publicManifest) == .typeSymbolicLink,
          (try? FileManager.default.destinationOfSymbolicLink(
              atPath: publicManifest.path
          )) == expectedLinkTarget,
          publicManifest.resolvingSymlinksInPath().standardizedFileURL
            == artifactRoot.appendingPathComponent(
                "NoritoBridge.artifacts.json"
            ).standardizedFileURL else {
        return "error: reviewed NoritoBridge public manifest link is not canonical."
    }

    let authoritativeHeaders: [String: URL] = [
        "NoritoBridge.h": repositoryDirectory.appendingPathComponent(
            "crates/connect_norito_bridge/include/NoritoBridge.h"
        ),
        "connect_norito_bridge.h": repositoryDirectory.appendingPathComponent(
            "crates/connect_norito_bridge/include/connect_norito_bridge.h"
        ),
        "module.modulemap": repositoryDirectory.appendingPathComponent(
            "crates/connect_norito_bridge/module.modulemap.template"
        ),
    ]
    var authoritativeContents: [String: Data] = [:]
    for (name, url) in authoritativeHeaders {
        guard isNonSymbolicRegularFile(url),
              let contents = try? Data(contentsOf: url) else {
            return "error: reviewed NoritoBridge authoritative header inventory is unavailable."
        }
        authoritativeContents[name] = contents
    }

    let binaryPathShapes = Set(libraries.map { $0["BinaryPath"] != nil })
    guard libraries.count == requiredBridgeSliceIdentifiers.count,
          binaryPathShapes.count == 1 else {
        return "error: reviewed NoritoBridge slice metadata shape is not canonical."
    }
    var observedIdentifiers = Set<String>()
    for library in libraries {
        guard let identifier = library["LibraryIdentifier"] as? String,
              observedIdentifiers.insert(identifier).inserted,
              let expectedHash = manifestHashes[identifier] else {
            return "error: reviewed NoritoBridge slice inventory is not canonical."
        }
        let expectedArchitectures: [String]
        let expectedPlatform: String
        let expectedVariant: String?
        switch identifier {
        case "ios-arm64":
            expectedArchitectures = ["arm64"]
            expectedPlatform = "ios"
            expectedVariant = nil
        case "ios-arm64_x86_64-simulator":
            expectedArchitectures = ["arm64", "x86_64"]
            expectedPlatform = "ios"
            expectedVariant = "simulator"
        case "macos-arm64_x86_64":
            expectedArchitectures = ["arm64", "x86_64"]
            expectedPlatform = "macos"
            expectedVariant = nil
        default:
            return "error: reviewed NoritoBridge slice inventory is not canonical."
        }
        var expectedFields: Set<String> = [
            "HeadersPath",
            "LibraryIdentifier",
            "LibraryPath",
            "SupportedArchitectures",
            "SupportedPlatform",
        ]
        if expectedVariant != nil { expectedFields.insert("SupportedPlatformVariant") }
        if library["BinaryPath"] != nil { expectedFields.insert("BinaryPath") }
        guard Set(library.keys) == expectedFields,
              library["LibraryPath"] as? String == "libNoritoBridge.a",
              library["HeadersPath"] as? String == "Headers",
              library["SupportedArchitectures"] as? [String]
                == expectedArchitectures,
              library["SupportedPlatform"] as? String == expectedPlatform,
              library["SupportedPlatformVariant"] as? String
                == expectedVariant,
              library["BinaryPath"] == nil
                || library["BinaryPath"] as? String == "libNoritoBridge.a" else {
            return "error: reviewed NoritoBridge slice metadata is not canonical: \(identifier)."
        }

        let sliceRoot = artifactRoot.appendingPathComponent(
            identifier,
            isDirectory: true
        )
        let headersRoot = sliceRoot.appendingPathComponent(
            "Headers",
            isDirectory: true
        )
        guard exactDirectoryEntries(
                  at: sliceRoot,
                  expected: ["Headers", "libNoritoBridge.a"]
              ),
              exactDirectoryEntries(
                  at: headersRoot,
                  expected: requiredBridgeHeaderEntries
              ) else {
            return "error: reviewed NoritoBridge slice file inventory is not exact: \(identifier)."
        }
        let binaryURL = sliceRoot.appendingPathComponent("libNoritoBridge.a")
        guard sha256Hex(of: binaryURL) == expectedHash else {
            return "error: reviewed NoritoBridge slice \(identifier) does not match its checked-in digest pin."
        }
        for (name, contents) in authoritativeContents {
            let candidate = headersRoot.appendingPathComponent(name)
            guard isNonSymbolicRegularFile(candidate),
                  (try? Data(contentsOf: candidate)) == contents else {
                return "error: reviewed NoritoBridge header inventory is substituted: \(identifier)/\(name)."
            }
        }
        guard sha256Hex(
            of: headersRoot.appendingPathComponent("connect_norito_bridge.h")
        ) == headerDigest else {
            return "error: reviewed NoritoBridge slice \(identifier) has a substituted bridge header."
        }
    }
    guard observedIdentifiers == requiredBridgeSliceIdentifiers else {
        return "error: reviewed NoritoBridge slice inventory is incomplete."
    }
    return nil
}

func validateReviewedReleaseBridgeArtifact(
    at artifactRoot: URL,
    manifestData: Data,
    manifest: [String: Any],
    metadata: [String: Any],
    libraries: [[String: Any]]
) -> String? {
    guard hasNoDuplicateJSONMembers(manifestData),
          hasOnlyCanonicalJSONNumberTypes(manifest),
          Set(manifest.keys) == requiredBridgeManifestFields else {
        return "error: reviewed NoritoBridge manifest field inventory is not exact."
    }
    guard let expectedVersion = canonicalPackageVersion(),
          manifest["version"] as? String == expectedVersion,
          canonicalJSONBoolean(manifest["privacy_production_enabled"]) == true,
          manifest["cargo_features"] as? [String] == ["privacy-production-enabled"],
          validateReviewedBuildEnvironment(manifest["build_environment"]),
          canonicalJSONBoolean(manifest["source_tree_dirty"]) == false,
          canonicalLowercaseHex(manifest["source_commit"], byteCount: 20) != nil,
          canonicalLowercaseHex(
              manifest["source_fingerprint_sha256"],
              byteCount: 32
          ) != nil,
          manifest["workspace_cargo_lock_sha256"] as? String
            == requiredWorkspaceCargoLockSha256,
          manifest["build_cargo_lock_sha256"] as? String
            == requiredPrivacyReleaseCargoLockSha256,
          manifest["build_cargo_lock_authority"] as? String
            == "privacy-sdk-release-v2",
          let headerDigest = canonicalLowercaseHex(
              manifest["bridge_header_sha256"],
              byteCount: 32
          ),
          let requiredSymbols = manifest["required_symbols"] as? [String],
          requiredSymbols.count == 102,
          nulDelimitedStringArraySHA256(requiredSymbols)
            == requiredBridgeRequiredSymbolsSha256,
          let forbiddenSymbols = manifest["forbidden_symbols"] as? [String],
          forbiddenSymbols.count == 11,
          nulDelimitedStringArraySHA256(forbiddenSymbols)
            == requiredBridgeForbiddenSymbolsSha256,
          let artifactRoles = manifest["kagemusha_mobile_artifact_roles"]
            as? [[String: Any]],
          artifactRoles.count == 14,
          canonicalJSONSHA256(artifactRoles)
            == requiredBridgeProductionRolesSha256,
          let manifestHashes = manifest["hashes"] as? [String: String],
          Set(manifestHashes.keys) == requiredBridgeSliceIdentifiers,
          manifestHashes.values.allSatisfy({
              canonicalLowercaseHex($0, byteCount: 32) != nil
          }),
          let checkedInPins = checkedInBridgeSlicePins(),
          manifestHashes == checkedInPins else {
        return "error: reviewed NoritoBridge manifest provenance or compatibility pins are invalid."
    }

    let workspaceLock = repositoryDirectory.appendingPathComponent("Cargo.lock")
    let privacyReleaseLock = repositoryDirectory.appendingPathComponent(
        "ci/privacy_sdk_release_lock_v2.toml"
    )
    guard sha256Hex(of: workspaceLock) == requiredWorkspaceCargoLockSha256,
          sha256Hex(of: privacyReleaseLock) == requiredPrivacyReleaseCargoLockSha256,
          reviewedSourceAbiIsExact(headerDigest) else {
        return "error: reviewed NoritoBridge source lock or header digest does not match the checkout."
    }
    return validateReviewedBridgeFilesystem(
        at: artifactRoot,
        metadata: metadata,
        libraries: libraries,
        manifestHashes: manifestHashes,
        headerDigest: headerDigest
    )
}

let bridgeAbsolutePath: URL
let bridgeTargetPath: String
if let configuredArtifactDirectory {
    guard configuredArtifactDirectory.hasPrefix("/") else {
        fatalError("error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be an absolute path.")
    }
    guard
        let canonicalArtifactDirectory =
            canonicalExistingFilesystemPath(configuredArtifactDirectory),
        canonicalArtifactDirectory == configuredArtifactDirectory
    else {
        fatalError(
            """
            error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be an existing canonical \
            path that does not traverse a symbolic link.
            """
        )
    }
    let resolvedURL = URL(
        fileURLWithPath: canonicalArtifactDirectory,
        isDirectory: true
    )
    guard
        isNonSymbolicDirectory(resolvedURL),
        resolvedURL.path != repositoryDirectory.path,
        !resolvedURL.path.hasPrefix(repositoryDirectory.path + "/")
    else {
        fatalError(
            "error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be outside the reviewed Iroha source tree."
        )
    }
    bridgeAbsolutePath = resolvedURL
        .appendingPathComponent("NoritoBridge.xcframework", isDirectory: true)
    bridgeTargetPath = relativePath(
        from: packageDirectory,
        to: bridgeAbsolutePath
    )
} else {
    bridgeAbsolutePath = packageDirectory
        .appendingPathComponent(bridgeRelativePath)
        .standardizedFileURL
    bridgeTargetPath = bridgeRelativePath
}

func validateBridgeArtifact(at artifactRoot: URL) -> String? {
    guard FileManager.default.fileExists(atPath: artifactRoot.path) else {
        return """
        error: NoritoBridge.xcframework is required at \(artifactRoot.path). \
        Set MOBILE_SDK_APPLE_ARTIFACT_DIR to the external directory containing \
        NoritoBridge.xcframework before building a reviewed source closure.
        """
    }

    let infoURL = artifactRoot.appendingPathComponent("Info.plist")
    let infoData: Data?
    if requireExternalArtifact {
        infoData = boundedRegularFileData(
            at: infoURL,
            maximumByteCount: 1_048_576
        )
    } else {
        infoData = try? Data(contentsOf: infoURL)
    }
    guard
        let data = infoData,
        let plist = try? PropertyListSerialization.propertyList(from: data, options: [], format: nil),
        let dictionary = plist as? [String: Any],
        let libraries = dictionary["AvailableLibraries"] as? [[String: Any]],
        !libraries.isEmpty
    else {
        return "error: NoritoBridge.xcframework at \(artifactRoot.path) has unreadable metadata."
    }

    for library in libraries {
        let identifier = library["LibraryIdentifier"] as? String ?? "<unknown>"
        let relativePaths = ["BinaryPath", "LibraryPath"].compactMap { library[$0] as? String }
        guard !relativePaths.isEmpty else {
            return "error: NoritoBridge.xcframework slice \(identifier) is missing BinaryPath/LibraryPath metadata."
        }

        for relativePath in relativePaths {
            let referencedURL = artifactRoot
                .appendingPathComponent(identifier, isDirectory: true)
                .appendingPathComponent(relativePath)
            guard FileManager.default.fileExists(atPath: referencedURL.path) else {
                return "error: NoritoBridge.xcframework slice \(identifier) is missing \(relativePath)."
            }
        }
    }

    let artifactManifestURL = artifactRoot.appendingPathComponent(
        "NoritoBridge.artifacts.json"
    )
    let loadedManifestData: Data?
    if requireExternalArtifact {
        loadedManifestData = boundedRegularFileData(
            at: artifactManifestURL,
            maximumByteCount: 4 * 1_024 * 1_024
        )
    } else {
        loadedManifestData = try? Data(contentsOf: artifactManifestURL)
    }
    guard
        let manifestData = loadedManifestData,
        !manifestData.isEmpty,
        manifestData.count <= 4 * 1_024 * 1_024,
        !requireExternalArtifact || hasNoDuplicateJSONMembers(manifestData),
        let manifest = try? JSONSerialization.jsonObject(with: manifestData)
            as? [String: Any],
        let bridgeAbiVersion = canonicalJSONInteger(
            manifest["native_bridge_abi_version"]
        )
    else {
        return "error: NoritoBridge.xcframework is missing readable ABI-bound artifact metadata."
    }
    guard bridgeAbiVersion == requiredBridgeAbiVersion else {
        return "error: NoritoBridge.xcframework requires exact native bridge ABI \(requiredBridgeAbiVersion); found \(bridgeAbiVersion)."
    }
    if requirePrivacyProductionArtifact,
       let releaseError = validateReviewedReleaseBridgeArtifact(
           at: artifactRoot,
           manifestData: manifestData,
           manifest: manifest,
           metadata: dictionary,
           libraries: libraries
       ) {
        return releaseError
    }

    return nil
}

if let bridgeArtifactError = validateBridgeArtifact(at: bridgeAbsolutePath) {
    fatalError(bridgeArtifactError)
}
var targets: [Target] = []
var irohaSwiftDependencies: [Target.Dependency] = []
var testDependencies: [Target.Dependency] = ["IrohaSwift"]
var irohaSwiftLinkerSettings: [LinkerSetting] = []

targets.append(
    .binaryTarget(
        name: "NoritoBridge",
        path: bridgeTargetPath
    )
)
let bridgeDependency: Target.Dependency = .target(name: "NoritoBridge", condition: .when(platforms: [.iOS, .macOS]))
irohaSwiftDependencies.append(bridgeDependency)
testDependencies.append(bridgeDependency)
// Ensure static bridge object files are retained so runtime dlsym lookups resolve.
irohaSwiftLinkerSettings.append(.unsafeFlags(["-Xlinker", "-all_load"], .when(platforms: [.iOS, .macOS])))

var swiftSettings: [SwiftSetting] = [
    .define("IROHA_SWIFT"),
    .define("IROHASWIFT_ENABLE_SECP256K1"),
    .define("IROHASWIFT_ENABLE_MLDSA"),
    .define("IROHASWIFT_ENABLE_BLS"),
    .define("IROHASWIFT_ENABLE_GOST"),
    .define("IROHASWIFT_ENABLE_SM"),
    .define("IROHASWIFT_BRIDGE_REQUIRED"),
    .define("IROHASWIFT_BRIDGE_PRESENT")
]

// Keep Google's Apple Nearby implementation deterministic for fresh Xcode
// checkouts. Nearby's transitive Abseil branch is additionally locked by the
// checked-in Package.resolved file.
let packageDependencies: [Package.Dependency] = [
    .package(
        url: "https://github.com/google/nearby.git",
        revision: "53568fe88281d4408e48e3ebec7d8560bed7077d"
    ),
    .package(
        url: "https://github.com/firebase/boringssl-SwiftPM.git",
        exact: "0.7.2"
    )
]
let mobileTransportDependencies: [Target.Dependency] = [
    "IrohaSwift",
    .product(
        name: "NearbyConnections",
        package: "nearby",
        condition: .when(platforms: [.iOS, .macOS])
    )
]
let mobileTransportTestDependencies: [Target.Dependency] =
    testDependencies + ["IrohaSwiftMobileTransports"]

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "IrohaSwift",
            targets: ["IrohaSwift"]),
        .library(
            name: "IrohaSwiftMobileTransports",
            targets: ["IrohaSwiftMobileTransports"]),
        .library(
            name: "IrohaSwiftTransferUI",
            targets: ["IrohaSwiftTransferUI"])
    ],
    dependencies: packageDependencies,
    targets: targets + [
        .target(
            name: "IrohaSwift",
            dependencies: irohaSwiftDependencies,
            path: "Sources/IrohaSwift",
            exclude: [],
            resources: [],
            swiftSettings: swiftSettings,
            linkerSettings: irohaSwiftLinkerSettings
        ),
        .target(
            name: "IrohaSwiftMobileTransports",
            dependencies: mobileTransportDependencies,
            path: "Sources/IrohaSwiftMobileTransports",
            swiftSettings: swiftSettings
        ),
        .target(
            name: "IrohaSwiftTransferUI",
            dependencies: ["IrohaSwift"],
            path: "Sources/IrohaSwiftTransferUI",
            swiftSettings: swiftSettings
        ),
        .testTarget(
            name: "IrohaSwiftTests",
            dependencies: testDependencies,
            path: "Tests/IrohaSwiftTests",
            resources: [
                .process("Fixtures")
            ],
            swiftSettings: swiftSettings
        ),
        .testTarget(
            name: "IrohaSwiftMobileTransportsTests",
            dependencies: mobileTransportTestDependencies,
            path: "Tests/IrohaSwiftMobileTransportsTests",
            swiftSettings: swiftSettings
        )
    ]
)
