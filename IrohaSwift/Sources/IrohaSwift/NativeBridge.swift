import Foundation
import CryptoKit
#if canImport(Darwin)
import Darwin
#if IROHASWIFT_BRIDGE_PRESENT
import NoritoBridge
#endif

enum BridgePolicyHint {
    private static let relativeBridgePath = "../dist/NoritoBridge.xcframework"

    static var message: String {
        "NoritoBridge.xcframework is required under \(relativeBridgePath) for native helpers."
    }

    static func unavailableMessage(_ prefix: String) -> String {
        "\(prefix) \(message)"
    }
}

struct SorafsLocalFetchOutput {
    let payload: Data
    let reportJSON: String
}

struct NativeSorafsOrderbookOrderRequestFields {
    let orderId: Data
    let side: UInt32
    let tier: UInt32
    let pricePerGibMicroXor: String
    let quantityGib: UInt64
    let remainingGib: UInt64
    let ownerAccount: Data
    let expiryUnix: UInt64
    let nonce: UInt64
    let makerFeeBps: UInt32
    let takerFeeBps: UInt32
}

struct NativeSorafsOrderbookOrderCancelFields {
    let orderId: Data
    let ownerAccount: Data
    let reason: UInt32
    let nonce: UInt64
}

struct NativeSorafsOrderbookSettlementReceiptFields {
    let receiptId: Data
    let channelId: Data
    let tradeId: Data
    let rangeStart: UInt64
    let rangeEnd: UInt64
    let chunkHash: Data
    let bytesDelivered: UInt64
    let xorDebitedMicroXor: String
    let providerCreditMicroXor: String
    let feeAmountMicroXor: String
    let issuedAtUnix: UInt64
}

enum NoritoBridgeLoader {
    enum ValidationStatus: Equatable {
        case valid(path: String, identifier: String)
        case pathDenied(path: String)
        case missing(path: String)
        case hashMismatch(path: String, expected: String, actual: String?)
        case versionMismatch(path: String, expected: String, actual: String?)
        case abiMismatch(path: String, expected: UInt32, actual: UInt32?)
    }

    static let expectedVersion = "0.1.0"
    static var expectedBridgeAbiVersion: UInt32 {
        expectedBridgeAbiVersion(for: currentIdentifier())
    }
    private static let expectedHashes: [String: String] = [
        "macos-arm64": "d1dc2069532ff760e03ebf66fdd17811d8d7fa520dcd9f9048b61bbcbcffc3e2",
        "ios-arm64": "26bb800e9dce021ef38306caef70dbba7928dd99c6612801fb1bbc520b52b7a9",
        "ios-arm64_x86_64-simulator": "d0f651e6dc837bff7e92b05c9bf1e3a2988fc6995cabee6e3aaa269a01ecd1b5"
    ]
    private static let requiredSymbols = [
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
        "connect_norito_encode_transfer_signed_transaction"
    ]

    private typealias BridgeAbiVersionFn = @convention(c) () -> UInt32

    private struct ArtifactManifest {
        let version: String
        let hashes: [String: String]
    }

    static func expectedBridgeAbiVersion(for identifier: String) -> UInt32 {
        return 10
    }

    static func isSupportedBridgeAbiVersion(_ actual: UInt32?, for identifier: String = currentIdentifier()) -> Bool {
        guard let actual else {
            return false
        }
        return actual >= expectedBridgeAbiVersion(for: identifier)
    }

    private static func packagedBinaryRelativePaths(for identifier: String = currentIdentifier()) -> [String] {
        return ["libNoritoBridge.a", "NoritoBridge.framework/NoritoBridge"]
    }

    static func openHandle() -> (UnsafeMutableRawPointer?, ValidationStatus) {
        // Xcode 26 debug-dylib: app code lives in <name>.debug.dylib, not the main executable.
        // dlopen(nil) returns a handle to the 57 KB launcher image. It may re-export a few symbols
        // (e.g. connect_norito_free) so the dlsym probe succeeds, but calling heavier functions
        // through that image crashes. Always prefer the debug dylib.
        if let execURL = Bundle.main.executableURL {
            let debugDylibURL = execURL.deletingLastPathComponent()
                .appendingPathComponent(execURL.lastPathComponent + ".debug.dylib")
            let exists = FileManager.default.fileExists(atPath: debugDylibURL.path)
            NSLog("[NoritoBridgeLoader] debug dylib path=%@ exists=%d", debugDylibURL.path, exists ? 1 : 0)
            if exists {
                let debugHandle = debugDylibURL.path.withCString { ptr in
                    dlopen(ptr, RTLD_NOW | RTLD_GLOBAL)
                }
                let abiVersion = bridgeAbiVersion(in: debugHandle)
                let hasCurrentAbi = hasRequiredSymbols(in: debugHandle)
                let hasTransfer = debugHandle.flatMap {
                    dlsym($0, "connect_norito_encode_transfer_signed_transaction")
                } != nil
                NSLog(
                    "[NoritoBridgeLoader] debug dylib handle=%@, hasCurrentAbi=%d, abi=%@, hasTransfer=%d",
                    debugHandle == nil ? "nil" : "ok",
                    hasCurrentAbi ? 1 : 0,
                    abiVersion.map(String.init) ?? "nil",
                    hasTransfer ? 1 : 0
                )
                if let debugHandle, hasCurrentAbi, hasTransfer {
                    return (debugHandle, .valid(path: "debug.dylib", identifier: currentIdentifier()))
                }
                if let debugHandle {
                    dlclose(debugHandle)
                }
            }
        } else {
            NSLog("[NoritoBridgeLoader] Bundle.main.executableURL is nil")
        }

        if let executableURL = Bundle.main.executableURL,
           let executableHandle = openImageIfSymbolsPresent(at: executableURL) {
            NSLog("[NoritoBridgeLoader] loaded bridge symbols from main executable %@", executableURL.path)
            return (executableHandle, .valid(path: executableURL.path, identifier: currentIdentifier()))
        }

        if let defaultHandle = rtldDefaultHandle(),
           hasRequiredSymbols(in: defaultHandle) {
            NSLog("[NoritoBridgeLoader] loaded bridge symbols from RTLD_DEFAULT")
            return (defaultHandle, .valid(path: "RTLD_DEFAULT", identifier: currentIdentifier()))
        }

        var lastFailure: ValidationStatus = .missing(path: defaultBridgeBinaryPath())
        for path in candidateLibraryPaths() {
            let status = validateBridge(at: path, allowUntrustedLocation: false)
            switch status {
            case .valid:
                let handle = path.withCString { pointer in
                    dlopen(pointer, RTLD_NOW | RTLD_GLOBAL)
                }
                if let handle,
                   hasRequiredSymbols(in: handle) {
                    return (handle, status)
                }
                let actualAbi = bridgeAbiVersion(in: handle)
                if let handle {
                    dlclose(handle)
                }
                lastFailure = .abiMismatch(
                    path: path,
                    expected: expectedBridgeAbiVersion,
                    actual: actualAbi
                )
            default:
                lastFailure = status
            }
        }

        if let defaultHandle = dlopen(nil, RTLD_NOW | RTLD_GLOBAL),
           hasRequiredSymbols(in: defaultHandle) {
            NSLog("[NoritoBridgeLoader] loaded bridge symbols from dlopen(nil)")
            return (defaultHandle, .valid(path: "RTLD_DEFAULT", identifier: currentIdentifier()))
        }

        return (nil, lastFailure)
    }

    private static func rtldDefaultHandle() -> UnsafeMutableRawPointer? {
        UnsafeMutableRawPointer(bitPattern: UInt(bitPattern: -2))
    }

    private static func hasRequiredSymbols(in handle: UnsafeMutableRawPointer?) -> Bool {
        guard let handle else { return false }
        for symbol in requiredSymbols where dlsym(handle, symbol) == nil {
            return false
        }
        return isSupportedBridgeAbiVersion(bridgeAbiVersion(in: handle))
    }

    private static func bridgeAbiVersion(in handle: UnsafeMutableRawPointer?) -> UInt32? {
        guard let handle,
              let symbol = dlsym(handle, "connect_norito_bridge_abi_version") else {
            return nil
        }
        let function = unsafeBitCast(symbol, to: BridgeAbiVersionFn.self)
        return function()
    }

    private static func openImageIfSymbolsPresent(at url: URL) -> UnsafeMutableRawPointer? {
        let handle = url.path.withCString { pointer in
            dlopen(pointer, RTLD_NOW | RTLD_GLOBAL)
        }
        guard hasRequiredSymbols(in: handle) else {
            if let handle {
                dlclose(handle)
            }
            return nil
        }
        return handle
    }

    static func validateForTests(at path: String, allowUntrustedLocation: Bool = true) -> ValidationStatus {
        validateBridge(at: path, allowUntrustedLocation: allowUntrustedLocation)
    }

    private static func validateBridge(at path: String, allowUntrustedLocation: Bool) -> ValidationStatus {
        let url = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: url.path) else {
            return .missing(path: path)
        }
        guard allowUntrustedLocation || isTrustedLocation(url) else {
            return .pathDenied(path: path)
        }
        guard let identifier = identifier(for: url) else {
            return .pathDenied(path: path)
        }

        let manifest = artifactManifest(near: url)
        if let version = manifest?.version, version != expectedVersion {
            return .versionMismatch(path: path, expected: expectedVersion, actual: version)
        }
        guard let expectedHash = manifest?.hashes[identifier] ?? expectedHashes[identifier] else {
            return .pathDenied(path: path)
        }

        let actualHash = sha256(url: url)
        if actualHash != expectedHash {
            return .hashMismatch(path: path, expected: expectedHash, actual: actualHash)
        }

        return .valid(path: path, identifier: identifier)
    }

    private static func artifactManifest(near binaryURL: URL) -> ArtifactManifest? {
        let candidates = candidateArtifactManifestURLs(near: binaryURL)
        for candidate in candidates {
            if let manifest = parseArtifactManifest(at: candidate) {
                return manifest
            }
        }
        return nil
    }

    private static func candidateArtifactManifestURLs(near binaryURL: URL) -> [URL] {
        var seen = Set<String>()
        var candidates: [URL] = []
        var cursor = binaryURL.deletingLastPathComponent()

        while true {
            if cursor.pathExtension == "xcframework" {
                let urls = [
                    cursor.appendingPathComponent("NoritoBridge.artifacts.json"),
                    cursor.deletingLastPathComponent().appendingPathComponent("NoritoBridge.artifacts.json")
                ]
                for url in urls where !seen.contains(url.path) {
                    seen.insert(url.path)
                    candidates.append(url)
                }
            }
            let parent = cursor.deletingLastPathComponent()
            if parent.path == cursor.path {
                break
            }
            cursor = parent
        }

        if candidates.isEmpty {
            let defaultCandidates = [
                binaryURL.deletingLastPathComponent().appendingPathComponent("NoritoBridge.artifacts.json"),
                binaryURL.deletingLastPathComponent().deletingLastPathComponent()
                    .appendingPathComponent("NoritoBridge.artifacts.json")
            ]
            for url in defaultCandidates where !seen.contains(url.path) {
                seen.insert(url.path)
                candidates.append(url)
            }
        }

        return candidates
    }

    private static func parseArtifactManifest(at url: URL) -> ArtifactManifest? {
        guard let data = try? Data(contentsOf: url),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let version = json["version"] as? String else {
            return nil
        }
        let hashes = json["hashes"] as? [String: String] ?? [:]
        return ArtifactManifest(version: version, hashes: hashes)
    }

    private static func sha256(url: URL) -> String? {
        guard let data = try? Data(contentsOf: url) else {
            return nil
        }
        let digest = SHA256.hash(data: data)
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func identifier(for url: URL) -> String? {
        for key in expectedHashes.keys {
            if url.path.contains("/\(key)/") {
                return key
            }
        }
        return currentIdentifier()
    }

    static func currentIdentifier() -> String {
        #if os(macOS)
        return "macos-arm64"
        #else
        #if targetEnvironment(simulator)
        return "ios-arm64_x86_64-simulator"
        #else
        return "ios-arm64"
        #endif
        #endif
    }

    private static func isTrustedLocation(_ url: URL) -> Bool {
        let allowedRoots = trustedSearchRoots()
        return allowedRoots.contains { url.path.hasPrefix($0.path) }
    }

    private static func candidateLibraryPaths() -> [String] {
        var seen = Set<String>()
        var paths: [String] = []

        func addIfExisting(_ url: URL) {
            let path = url.path
            guard FileManager.default.fileExists(atPath: path), !seen.contains(path) else {
                return
            }
            seen.insert(path)
            paths.append(path)
        }

        for framework in Bundle.allFrameworks
            where framework.bundleURL.lastPathComponent == "NoritoBridge.framework" {
            if let executableURL = framework.executableURL {
                addIfExisting(executableURL)
            }
        }

        for root in trustedSearchRoots() {
            for relativePath in packagedBinaryRelativePaths() {
                addIfExisting(root.appendingPathComponent(relativePath))
            }
        }

        return paths
    }

    private static func trustedSearchRoots() -> [URL] {
        var roots: [URL] = []
        let bundleRoots = Bundle.allBundles + Bundle.allFrameworks
        for bundle in bundleRoots {
            roots.append(bundle.bundleURL)
            roots.append(bundle.bundleURL.deletingLastPathComponent())
        }

        if let mainBundleURL = Bundle.main.bundleURL as URL? {
            roots.append(mainBundleURL.deletingLastPathComponent())
            roots.append(mainBundleURL.appendingPathComponent("Frameworks"))
            if let privateFrameworks = Bundle.main.privateFrameworksPath {
                roots.append(URL(fileURLWithPath: privateFrameworks))
            }
        }

        if let executableURL = Bundle.main.executableURL {
            roots.append(executableURL.deletingLastPathComponent()
                .appendingPathComponent("Frameworks"))
            roots.append(executableURL.deletingLastPathComponent().deletingLastPathComponent()
                .appendingPathComponent("Frameworks"))
        }

        // XCTest and simulator launches often expose the real build-products directory through
        // dyld environment variables even when the app bundle only embeds a thin framework shell.
        let processInfo = ProcessInfo.processInfo
        for key in ["BUILT_PRODUCTS_DIR", "DYLD_FRAMEWORK_PATH", "DYLD_LIBRARY_PATH"] {
            guard let rawValue = processInfo.environment[key]?
                .trimmingCharacters(in: .whitespacesAndNewlines),
                  !rawValue.isEmpty else {
                continue
            }
            for component in rawValue.split(separator: ":") {
                let path = String(component).trimmingCharacters(in: .whitespacesAndNewlines)
                guard !path.isEmpty else { continue }
                roots.append(URL(fileURLWithPath: path))
            }
        }

        let sourceFile = URL(fileURLWithPath: #filePath)
        var root = sourceFile
        for _ in 0..<4 {
            root.deleteLastPathComponent()
        }
        roots.append(root.appendingPathComponent("dist/NoritoBridge.xcframework/\(currentIdentifier())"))

        var deduped: [URL] = []
        var seen = Set<String>()
        for url in roots {
            let path = url.standardized.path
            if !seen.contains(path) {
                seen.insert(path)
                deduped.append(URL(fileURLWithPath: path))
            }
        }
        return deduped
    }

    private static func defaultBridgeBinaryPath() -> String {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            root.deleteLastPathComponent()
        }
        let sliceRoot = root
            .appendingPathComponent("dist/NoritoBridge.xcframework")
            .appendingPathComponent(currentIdentifier())
        for relativePath in packagedBinaryRelativePaths() {
            let candidate = sliceRoot.appendingPathComponent(relativePath)
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate.path
            }
        }
        return sliceRoot.appendingPathComponent(packagedBinaryRelativePaths().first ?? "libNoritoBridge.a").path
    }
}

private extension ConnectFrame {
    var ciphertextPayload: Data? {
        if case .ciphertext(let ciphertext) = kind {
            return ciphertext.payload
        }
        return nil
    }
}

struct ConnectNoritoAccelerationConfig {
    var enable_simd: UInt8
    var enable_metal: UInt8
    var enable_cuda: UInt8
    var max_gpus: UInt64
    var max_gpus_present: UInt8
    var merkle_min_leaves_gpu: UInt64
    var merkle_min_leaves_gpu_present: UInt8
    var merkle_min_leaves_metal: UInt64
    var merkle_min_leaves_metal_present: UInt8
    var merkle_min_leaves_cuda: UInt64
    var merkle_min_leaves_cuda_present: UInt8
    var prefer_cpu_sha2_max_leaves_aarch64: UInt64
    var prefer_cpu_sha2_max_leaves_aarch64_present: UInt8
    var prefer_cpu_sha2_max_leaves_x86: UInt64
    var prefer_cpu_sha2_max_leaves_x86_present: UInt8
}

struct ConnectNoritoAccelerationBackendStatus {
    var supported: UInt8
    var configured: UInt8
    var available: UInt8
    var parity_ok: UInt8
    var last_error_ptr: UnsafeMutablePointer<UInt8>?
    var last_error_len: UInt
}

struct ConnectNoritoAccelerationState {
    var config: ConnectNoritoAccelerationConfig
    var simd: ConnectNoritoAccelerationBackendStatus
    var metal: ConnectNoritoAccelerationBackendStatus
    var cuda: ConnectNoritoAccelerationBackendStatus
}
#endif

struct NativeSignedTransaction {
    let signedBytes: Data
    let hash: Data
}

struct NativeAccountAddressParseResult {
    let canonicalBytes: Data
    let networkPrefix: UInt16?
}

struct NativeAccountAddressRenderResult {
    let canonicalHex: String
    let i105: String
}

enum NativeBridgeError: Error, Equatable {
    case nullPointer
    case utf8
    case chainId
    case authority
    case assetDefinition
    case destination
    case quantity
    case invalidTtl
    case invalidNonce
    case privateKey
    case alloc
    case hashOutBuffer
    case invalidNoteCommitment
    case confidentialPayload
    case proofAttachment
    case invalidNullifiers
    case invalidRootHint
    case offlineReceiver
    case offlineAsset
    case offlineNonce
    case offlineSerialize
    case offlineCommitment
    case offlineBlinding
    case offlineNoteProve
    case kagemushaProve
    case kagemushaRecursiveCompactUnavailable
    case invalidKagemushaVerifierOutput
    case unsupportedAlgorithm
    case metadataTarget
    case metadataKey
    case metadataValue
    case governance
    case hex
    case accountList
    case multisigSpec
    case identifierReceipt
    case verifyingKeyId
    case zkAssetMode
    case secpParse
    case secpSign
    case secpVerify
    case invalidPrivacyRequest
    case invalidPrivacyOutput
    case unknown(Int32)

    static func fromStatus(_ status: Int32) -> NativeBridgeError? {
        if status == 0 { return nil }
        switch status {
        case -1: return .nullPointer
        case -2: return .utf8
        case -3: return .chainId
        case -4: return .authority
        case -5: return .assetDefinition
        case -6: return .destination
        case -7: return .quantity
        case -8: return .invalidTtl
        case -31: return .invalidNonce
        case -9: return .privateKey
        case -10: return .alloc
        case -11: return .hashOutBuffer
        case -14: return .invalidNoteCommitment
        case -15: return .confidentialPayload
        case -18: return .proofAttachment
        case -19: return .invalidNullifiers
        case -20: return .invalidRootHint
        case -21: return .unsupportedAlgorithm
        case -22: return .secpParse
        case -23: return .secpSign
        case -24: return .secpVerify
        case -25: return .metadataTarget
        case -26: return .metadataKey
        case -27: return .metadataValue
        case -28: return .governance
        case -29: return .hex
        case -30: return .accountList
        case -300: return .offlineReceiver
        case -301: return .offlineAsset
        case -303: return .offlineNonce
        case -304: return .offlineSerialize
        case -305: return .offlineCommitment
        case -306: return .offlineBlinding
        case -310: return .offlineNoteProve
        case -311: return .kagemushaProve
        case -312: return .kagemushaRecursiveCompactUnavailable
        case -402: return .multisigSpec
        case -406: return .identifierReceipt
        case -403: return .verifyingKeyId
        case -404: return .zkAssetMode
        default: return .unknown(status)
        }
    }
}

public final class NoritoNativeBridge: @unchecked Sendable {
    public static let shared = NoritoNativeBridge()
    private var bridgeStatus: NoritoBridgeLoader.ValidationStatus
    #if canImport(Darwin)
    private let chainDiscriminantLock = NSRecursiveLock()
    #endif

    private func throwOnStatus(_ status: Int32) throws {
        if let error = NativeBridgeError.fromStatus(status) {
            throw error
        }
    }

    static func normalizeKagemushaRecursiveCompactVerifierOutput(
        status: Int32,
        valid: UInt8
    ) throws -> Bool {
        if let error = NativeBridgeError.fromStatus(status) {
            throw error
        }
        switch valid {
        case 0:
            return false
        case 1:
            return true
        default:
            throw NativeBridgeError.invalidKagemushaVerifierOutput
        }
    }

    #if canImport(Darwin)
    static func copyKagemushaNativeArchiveOutput(
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong,
        free: (UnsafeMutablePointer<UInt8>?) -> Void
    ) throws -> Data {
        guard let pointer else {
            throw NativeBridgeError.nullPointer
        }
        defer {
            free(pointer)
        }
        guard length <= CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) else {
            throw NativeBridgeError.kagemushaProve
        }
        return Data(bytes: pointer, count: Int(length))
    }
    #endif

    static let privacyNativeArchiveMaxBytes = 64 * 1024 * 1024
    private static let privacyRequestTextFieldMaxBytes = 1024
    private static let privacyRequestPublicInputsMaxBytes = 1024 * 1024
    private static let privacyRequestWitnessMaxBytes = privacyNativeArchiveMaxBytes / 2
    private static let privacyRequestProofMaxBytes = privacyNativeArchiveMaxBytes / 2
    private static let privacyNoritoHeaderBytes = 40
    private static let privacyNoritoMaxHeaderPaddingBytes = 64
    private static let privacyNoritoSupportedFlagsMask: UInt8 = 0x27
    private static let privacyNoritoFieldBitsetFlag: UInt8 = 0x20
    private static let privacyNoritoFieldBitsetRequiredFlags: UInt8 = 0x06
    private static let privacyRequestSchemaByte: UInt8 = 0x52
    private static let privacyCapabilitiesResultSchemaByte: UInt8 = 0x50
    private static let privacyBuildProofResultSchemaByte: UInt8 = 0x42
    private static let privacyVerifyProofResultSchemaByte: UInt8 = 0x56
    private static let privacyCrc64ReflectedPoly: UInt64 = 0xC96C_5795_D787_0F42
    private static let privacyNoritoMagic: [UInt8] = [0x4E, 0x52, 0x54, 0x30]

    static func isValidPrivacyNoritoArchive(_ archive: Data) -> Bool {
        guard archive.count >= privacyNoritoHeaderBytes,
              archive.count <= privacyNativeArchiveMaxBytes else {
            return false
        }
        let bytes = [UInt8](archive)
        guard Array(bytes[0..<4]) == privacyNoritoMagic else {
            return false
        }
        guard bytes[4] == 0, bytes[5] == 0, bytes[22] == 0 else {
            return false
        }
        let flags = bytes[39]
        guard (flags & ~privacyNoritoSupportedFlagsMask) == 0 else {
            return false
        }
        if (flags & privacyNoritoFieldBitsetFlag) != 0,
           (flags & privacyNoritoFieldBitsetRequiredFlags) != privacyNoritoFieldBitsetRequiredFlags {
            return false
        }
        let payloadLength = readPrivacyUInt64LittleEndian(bytes, offset: 23)
        guard payloadLength <= UInt64(Int.max - privacyNoritoHeaderBytes) else {
            return false
        }
        let minimumLength = privacyNoritoHeaderBytes + Int(payloadLength)
        guard bytes.count >= minimumLength else {
            return false
        }
        let paddingLength = bytes.count - minimumLength
        guard paddingLength <= privacyNoritoMaxHeaderPaddingBytes else {
            return false
        }
        if paddingLength > 0 {
            let paddingStart = privacyNoritoHeaderBytes
            let paddingEnd = paddingStart + paddingLength
            guard bytes[paddingStart..<paddingEnd].allSatisfy({ $0 == 0 }) else {
                return false
            }
        }
        let payloadStart = privacyNoritoHeaderBytes + paddingLength
        let expectedCrc = readPrivacyUInt64LittleEndian(bytes, offset: 31)
        return privacyCrc64(bytes[payloadStart..<bytes.count]) == expectedCrc
    }

    static func hasNonEmptyPrivacyNoritoPayload(_ archive: Data) -> Bool {
        guard isValidPrivacyNoritoArchive(archive) else {
            return false
        }
        let bytes = [UInt8](archive)
        return readPrivacyUInt64LittleEndian(bytes, offset: 23) > 0
    }

    static func hasPrivacyNoritoSchema(_ archive: Data, expectedSchemaByte: UInt8) -> Bool {
        guard archive.count >= 22 else {
            return false
        }
        return archive[6..<22].allSatisfy { $0 == expectedSchemaByte }
    }

    private static func readPrivacyUInt64LittleEndian(_ bytes: [UInt8], offset: Int) -> UInt64 {
        var value: UInt64 = 0
        for index in 0..<8 {
            value |= UInt64(bytes[offset + index]) << (8 * index)
        }
        return value
    }

    private static func privacyCrc64(_ payload: ArraySlice<UInt8>) -> UInt64 {
        var crc = UInt64.max
        for byte in payload {
            crc ^= UInt64(byte)
            for _ in 0..<8 {
                crc = (crc & 1) != 0
                    ? (crc >> 1) ^ privacyCrc64ReflectedPoly
                    : crc >> 1
            }
        }
        return crc ^ UInt64.max
    }

    #if canImport(Darwin)
    private typealias EncodeTransferFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeTransferWithFeeSponsorFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeTransferWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeTransferWithFeeSponsorWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeMintFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeMintWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeShieldFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeShieldWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeUnshieldFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeRegisterZkAssetFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeRegisterZkAssetWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeUnshieldWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeZkTransferFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeZkTransferWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeMultisigRegisterFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeMultisigRegisterWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeClaimIdentifierFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeClaimIdentifierWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32

    private typealias EncodeBurnFn = EncodeMintFn
    private typealias EncodeBurnWithAlgFn = EncodeMintWithAlgFn
    private typealias EncodeSetKeyValueFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeSetKeyValueWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeRemoveKeyValueFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeRemoveKeyValueWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceProposeDeployFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64, UInt64, UInt8,
        UInt8, UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceProposeDeployWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64, UInt64, UInt8,
        UInt8, UInt8,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceCastPlainBallotFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceCastPlainBallotWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceCastZkBallotFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceCastZkBallotWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceEnactReferendumFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceEnactReferendumWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceFinalizeReferendumFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernanceFinalizeReferendumWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernancePersistCouncilFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt64,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeGovernancePersistCouncilWithAlgFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt64,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias DecodeSignedFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32
    private typealias DecodeReceiptFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32
    private typealias DecodeAssetIdFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias SetChainDiscriminantFn = @convention(c) (UInt16) -> UInt16
    private typealias SetAccelerationConfigFn = @convention(c) (UnsafeRawPointer?) -> Void
    private typealias GetAccelerationConfigFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32
    private typealias GetAccelerationStateFn = @convention(c) (UnsafeMutableRawPointer?) -> Int32

    private typealias EncodeCiphertextFrameFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlOpenFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlApproveFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<CChar>?,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlApproveWithAlgFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UnsafePointer<UInt8>?,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlRejectFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UInt16,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlCloseFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UInt8,
        UInt16,
        UnsafePointer<CChar>?, UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlPingFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt8, UInt64,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeControlPongFn = EncodeControlPingFn

    private typealias EncodeConfidentialPayloadFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias AccountAddressParseFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UInt16, UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt16>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias AccountAddressRenderFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UInt16,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias Sm2DefaultDistidFn = @convention(c) (
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias Sm2KeypairFromSeedFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?, UInt
    ) -> Int32

    private typealias Sm2SignFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?, UInt
    ) -> Int32

    private typealias Sm2VerifyFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt
    ) -> Int32

    private typealias Sm2PublicKeyStringFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias Sm2ComputeZaFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?, UInt
    ) -> Int32

    private typealias Secp256k1PublicKeyFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias Secp256k1SignFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias Secp256k1VerifyFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias MldsaParametersFn = @convention(c) (
        UInt32,
        UnsafeMutablePointer<UInt32>?,
        UnsafeMutablePointer<UInt32>?,
        UnsafeMutablePointer<UInt32>?
    ) -> Int32

    private typealias MldsaGenerateKeypairFn = @convention(c) (
        UInt32,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias MldsaSignFn = @convention(c) (
        UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias MldsaVerifyFn = @convention(c) (
        UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32


    private typealias ConnectGenerateKeypairFn = @convention(c) (
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias ConnectPublicFromPrivateFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias ConnectDeriveKeysFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UnsafePointer<UInt8>?,
        UnsafePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias ConnectEncryptEnvelopeFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UnsafePointer<UInt8>?,
        UInt8,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias ConnectDecryptCiphertextFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeSignRequestTxFn = @convention(c) (
        UInt64,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeSignRequestRawFn = @convention(c) (
        UInt64,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeSignResultOkFn = @convention(c) (
        UInt64,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeSignResultOkWithAlgFn = @convention(c) (
        UInt64,
        UnsafePointer<CChar>?,
        UInt,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeSignResultErrFn = @convention(c) (
        UInt64,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeControlCloseFn = @convention(c) (
        UInt64,
        UInt8,
        UInt16,
        UnsafePointer<UInt8>?,
        UInt,
        UInt8,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias EncodeEnvelopeControlRejectFn = @convention(c) (
        UInt64,
        UInt16,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeEnvelopeKindFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UInt64>?,
        UnsafeMutablePointer<UInt16>?
    ) -> Int32

    private typealias DecodeEnvelopeJSONFn = @convention(c) (
        UnsafePointer<UInt8>?,
        UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlKindFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt64>?,
        UnsafeMutablePointer<UInt16>?
    ) -> Int32

    private typealias DecodeCiphertextFrameFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt64>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlOpenPubFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias DecodeControlOpenChainIdFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlOpenAppMetadataFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlOpenPermissionsFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlApprovePubFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias DecodeControlApproveAccountFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlApprovePermissionsFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlApproveProofFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlApproveSigFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private typealias DecodeControlApproveSigAlgFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlCloseFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt16>?,
        UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlRejectFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt16>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?
    ) -> Int32

    private typealias DecodeControlPingFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt64>?
    ) -> Int32

    private typealias DecodeControlPongFn = DecodeControlPingFn

    private typealias DaProofSummaryFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        CUnsignedLong, UInt64,
        UnsafePointer<CUnsignedLong>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32

    private typealias SorafsLocalFetchFn = @convention(c) (
        UnsafePointer<CChar>?, CUnsignedLong,
        UnsafePointer<CChar>?, CUnsignedLong,
        UnsafePointer<CChar>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferencePayloadFn = @convention(c) (
        UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferenceOrderbookSignFn = @convention(c) (
        UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferenceOrderbookOrderRequestBuilderFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt32, UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64, UInt64,
        UInt32, UInt32,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferenceOrderbookCancelBuilderFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt32, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferenceOrderbookSettlementReceiptBuilderFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferencePdpPairFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SorafsReferencePdpBundleFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias Blake3HashFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaVerifyRecursiveCompactPaymentTokenFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UInt64,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias KagemushaRecursiveSpendArchiveFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias PrivacyCapabilitiesFn = @convention(c) (
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias PrivacyProofArchiveFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias PrivacyProofRequestFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaRecursiveSpendLineageWitnessFromInitResultFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KagemushaRecursiveSpendLineageWitnessAppendResultFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias EncodeOfflineNoteTxFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeOfflineNoteTxWithMetadataFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeDefundOfflineNoteTxFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UInt32,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias EncodeDefundOfflineNoteTxWithMetadataFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt64,
        UInt64,
        UInt8,
        UInt32,
        UInt8,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt32,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?,
        UInt
    ) -> Int32
    private typealias PublicKeyFromPrivateFn = @convention(c) (
        UInt8,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias KeypairFromSeedFn = @convention(c) (
        UInt8,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias SignDetachedFn = @convention(c) (
        UInt8,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias VerifyDetachedFn = @convention(c) (
        UInt8,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32

    private var bridgeHandle: UnsafeMutableRawPointer? = nil
    private var encodeTransferFn: EncodeTransferFn? = nil
    private var encodeTransferWithFeeSponsorFn: EncodeTransferWithFeeSponsorFn? = nil
    private var encodeTransferWithAlgFn: EncodeTransferWithAlgFn? = nil
    private var encodeTransferWithFeeSponsorWithAlgFn: EncodeTransferWithFeeSponsorWithAlgFn? = nil
    private var encodeMintFn: EncodeMintFn? = nil
    private var encodeMintWithAlgFn: EncodeMintWithAlgFn? = nil
    private var encodeShieldFn: EncodeShieldFn? = nil
    private var encodeShieldWithAlgFn: EncodeShieldWithAlgFn? = nil
    private var encodeUnshieldFn: EncodeUnshieldFn? = nil
    private var encodeUnshieldWithAlgFn: EncodeUnshieldWithAlgFn? = nil
    private var encodeZkTransferFn: EncodeZkTransferFn? = nil
    private var encodeZkTransferWithAlgFn: EncodeZkTransferWithAlgFn? = nil
    private var encodeRegisterZkAssetFn: EncodeRegisterZkAssetFn? = nil
    private var encodeRegisterZkAssetWithAlgFn: EncodeRegisterZkAssetWithAlgFn? = nil
    private var encodeMultisigRegisterFn: EncodeMultisigRegisterFn? = nil
    private var encodeMultisigRegisterWithAlgFn: EncodeMultisigRegisterWithAlgFn? = nil
    private var encodeClaimIdentifierFn: EncodeClaimIdentifierFn? = nil
    private var encodeClaimIdentifierWithAlgFn: EncodeClaimIdentifierWithAlgFn? = nil
    private var encodeBurnFn: EncodeBurnFn? = nil
    private var encodeBurnWithAlgFn: EncodeBurnWithAlgFn? = nil
    private var encodeSetKeyValueFn: EncodeSetKeyValueFn? = nil
    private var encodeSetKeyValueWithAlgFn: EncodeSetKeyValueWithAlgFn? = nil
    private var encodeRemoveKeyValueFn: EncodeRemoveKeyValueFn? = nil
    private var encodeRemoveKeyValueWithAlgFn: EncodeRemoveKeyValueWithAlgFn? = nil
    private var encodeGovernanceProposeDeployFn: EncodeGovernanceProposeDeployFn? = nil
    private var encodeGovernanceProposeDeployWithAlgFn: EncodeGovernanceProposeDeployWithAlgFn? = nil
    private var encodeGovernanceCastPlainBallotFn: EncodeGovernanceCastPlainBallotFn? = nil
    private var encodeGovernanceCastPlainBallotWithAlgFn: EncodeGovernanceCastPlainBallotWithAlgFn? = nil
    private var encodeGovernanceCastZkBallotFn: EncodeGovernanceCastZkBallotFn? = nil
    private var encodeGovernanceCastZkBallotWithAlgFn: EncodeGovernanceCastZkBallotWithAlgFn? = nil
    private var encodeGovernanceEnactReferendumFn: EncodeGovernanceEnactReferendumFn? = nil
    private var encodeGovernanceEnactReferendumWithAlgFn: EncodeGovernanceEnactReferendumWithAlgFn? = nil
    private var encodeGovernanceFinalizeReferendumFn: EncodeGovernanceFinalizeReferendumFn? = nil
    private var encodeGovernanceFinalizeReferendumWithAlgFn: EncodeGovernanceFinalizeReferendumWithAlgFn? = nil
    private var encodeGovernancePersistCouncilFn: EncodeGovernancePersistCouncilFn? = nil
    private var encodeGovernancePersistCouncilWithAlgFn: EncodeGovernancePersistCouncilWithAlgFn? = nil
    private var decodeSignedFn: DecodeSignedFn? = nil
    private var decodeReceiptFn: DecodeReceiptFn? = nil
    private var decodeAssetIdFn: DecodeAssetIdFn? = nil
    private var freeFn: FreeFn? = nil
    private var setChainDiscriminantFn: SetChainDiscriminantFn? = nil
    private var setAccelerationConfigFn: SetAccelerationConfigFn? = nil
    private var getAccelerationConfigFn: GetAccelerationConfigFn? = nil
    private var getAccelerationStateFn: GetAccelerationStateFn? = nil
    private var encodeCiphertextFrameFn: EncodeCiphertextFrameFn? = nil
    private var encodeControlOpenFn: EncodeControlOpenFn? = nil
    private var encodeControlApproveFn: EncodeControlApproveFn? = nil
    private var encodeControlApproveWithAlgFn: EncodeControlApproveWithAlgFn? = nil
    private var encodeControlRejectFn: EncodeControlRejectFn? = nil
    private var encodeControlCloseFn: EncodeControlCloseFn? = nil
    private var encodeControlPingFn: EncodeControlPingFn? = nil
    private var encodeControlPongFn: EncodeControlPongFn? = nil
    private var encodeConfidentialPayloadFn: EncodeConfidentialPayloadFn? = nil
    private var accountAddressParseFn: AccountAddressParseFn? = nil
    private var accountAddressRenderFn: AccountAddressRenderFn? = nil
    private var publicKeyFromPrivateFn: PublicKeyFromPrivateFn? = nil
    private var keypairFromSeedFn: KeypairFromSeedFn? = nil
    private var signDetachedFn: SignDetachedFn? = nil
    private var verifyDetachedFn: VerifyDetachedFn? = nil
    private var sm2DefaultDistidFn: Sm2DefaultDistidFn? = nil
    private var sm2KeypairFromSeedFn: Sm2KeypairFromSeedFn? = nil
    private var sm2SignFn: Sm2SignFn? = nil
    private var sm2VerifyFn: Sm2VerifyFn? = nil
    private var sm2PublicKeyPrefixedFn: Sm2PublicKeyStringFn? = nil
    private var sm2PublicKeyMultihashFn: Sm2PublicKeyStringFn? = nil
    private var sm2ComputeZaFn: Sm2ComputeZaFn? = nil
    private var secp256k1PublicKeyFn: Secp256k1PublicKeyFn? = nil
    private var secp256k1SignFn: Secp256k1SignFn? = nil
    private var secp256k1VerifyFn: Secp256k1VerifyFn? = nil
    private var mldsaParametersFn: MldsaParametersFn? = nil
    private var mldsaGenerateKeypairFn: MldsaGenerateKeypairFn? = nil
    private var mldsaSignFn: MldsaSignFn? = nil
    private var mldsaVerifyFn: MldsaVerifyFn? = nil
    private var connectGenerateKeypairFn: ConnectGenerateKeypairFn? = nil
    private var connectPublicFromPrivateFn: ConnectPublicFromPrivateFn? = nil
    private var connectDeriveKeysFn: ConnectDeriveKeysFn? = nil
    private var connectEncryptEnvelopeFn: ConnectEncryptEnvelopeFn? = nil
    private var connectDecryptCiphertextFn: ConnectDecryptCiphertextFn? = nil
    private var encodeEnvelopeSignRequestTxFn: EncodeEnvelopeSignRequestTxFn? = nil
    private var encodeEnvelopeSignRequestRawFn: EncodeEnvelopeSignRequestRawFn? = nil
    private var encodeEnvelopeSignResultOkFn: EncodeEnvelopeSignResultOkFn? = nil
    private var encodeEnvelopeSignResultOkWithAlgFn: EncodeEnvelopeSignResultOkWithAlgFn? = nil
    private var encodeEnvelopeSignResultErrFn: EncodeEnvelopeSignResultErrFn? = nil
    private var encodeEnvelopeControlCloseFn: EncodeEnvelopeControlCloseFn? = nil
    private var encodeEnvelopeControlRejectFn: EncodeEnvelopeControlRejectFn? = nil
    private var decodeEnvelopeKindFn: DecodeEnvelopeKindFn? = nil
    private var decodeEnvelopeJSONFn: DecodeEnvelopeJSONFn? = nil
    private var decodeControlKindFn: DecodeControlKindFn? = nil
    private var decodeCiphertextFrameFn: DecodeCiphertextFrameFn? = nil
    private var decodeControlOpenPubFn: DecodeControlOpenPubFn? = nil
    private var decodeControlOpenChainIdFn: DecodeControlOpenChainIdFn? = nil
    private var decodeControlOpenAppMetadataFn: DecodeControlOpenAppMetadataFn? = nil
    private var decodeControlOpenPermissionsFn: DecodeControlOpenPermissionsFn? = nil
    private var decodeControlApprovePubFn: DecodeControlApprovePubFn? = nil
    private var decodeControlApproveAccountFn: DecodeControlApproveAccountFn? = nil
    private var decodeControlApprovePermissionsFn: DecodeControlApprovePermissionsFn? = nil
    private var decodeControlApproveProofFn: DecodeControlApproveProofFn? = nil
    private var decodeControlApproveSigFn: DecodeControlApproveSigFn? = nil
    private var decodeControlApproveSigAlgFn: DecodeControlApproveSigAlgFn? = nil
    private var decodeControlCloseFn: DecodeControlCloseFn? = nil
    private var decodeControlRejectFn: DecodeControlRejectFn? = nil
    private var decodeControlPingFn: DecodeControlPingFn? = nil
    private var decodeControlPongFn: DecodeControlPongFn? = nil
    private var sorafsLocalFetchFn: SorafsLocalFetchFn? = nil
    private var sorafsReferenceValidateOrderbookFn: SorafsReferencePayloadFn? = nil
    private var sorafsReferenceSignOrderbookFn: SorafsReferenceOrderbookSignFn? = nil
    private var sorafsReferenceBuildOrderbookOrderRequestFn: SorafsReferenceOrderbookOrderRequestBuilderFn? = nil
    private var sorafsReferenceBuildOrderbookOrderCancelFn: SorafsReferenceOrderbookCancelBuilderFn? = nil
    private var sorafsReferenceBuildOrderbookSettlementReceiptFn: SorafsReferenceOrderbookSettlementReceiptBuilderFn? = nil
    private var sorafsReferenceValidatePdpPayloadFn: SorafsReferencePayloadFn? = nil
    private var sorafsReferenceValidatePdpCommitmentChallengeFn: SorafsReferencePdpPairFn? = nil
    private var sorafsReferenceValidatePdpChallengeProofFn: SorafsReferencePdpPairFn? = nil
    private var sorafsReferenceValidatePdpBundleFn: SorafsReferencePdpBundleFn? = nil
    private var daProofSummaryFn: DaProofSummaryFn? = nil
    private var blake3HashFn: Blake3HashFn? = nil
    private var kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn: KagemushaProveVerifiedCompactPaymentTokenWithRecordsFn? = nil
    private var kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn: KagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn? = nil
    private var kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn: KagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn? = nil
    private var kagemushaVerifyRecursiveCompactPaymentTokenFn: KagemushaVerifyRecursiveCompactPaymentTokenFn? = nil
    private var kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn? = nil
    private var kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn? = nil
    private var kagemushaBuildPallasOpenEnvelopesArchiveFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaBuildPreviousProofOpenEnvelopesArchiveFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendInitFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendAppendFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendTransitionProfileInitFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendTransitionProfileAppendFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendLineageAppendBoundaryFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendLineageWitnessFromInitResultFn: KagemushaRecursiveSpendLineageWitnessFromInitResultFn? = nil
    private var kagemushaRecursiveSpendLineageWitnessAppendResultFn: KagemushaRecursiveSpendLineageWitnessAppendResultFn? = nil
    private var kagemushaRecursiveSpendVerifyFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var kagemushaRecursiveSpendRedeemFn: KagemushaRecursiveSpendArchiveFn? = nil
    private var privacyCapabilitiesFn: PrivacyCapabilitiesFn? = nil
    private var privacyProofRequestFn: PrivacyProofRequestFn? = nil
    private var privacyBuildProofFn: PrivacyProofArchiveFn? = nil
    private var privacyVerifyProofFn: PrivacyProofArchiveFn? = nil
    private var privacyFreeFn: FreeFn? = nil
    private var privacyNativeProbeOk = false
    private var kagemushaCompactPaymentTokenNativeProbeOk = false
    private var kagemushaRecursiveAggregationNativeProbeOk = false
    private var kagemushaRecursiveCompactPaymentTokenNativeProbeOk = false
    private var kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk = false
    private var kagemushaRecursiveSpendCompactProjectionNativeProbeOk = false
    private var kagemushaRecursiveSpendCompactProjectionVerifierNativeProbeOk = false
    private var kagemushaRecursiveSpendNativeProbeOk = false
    private var encodeIssueOfflineNoteFn: EncodeOfflineNoteTxFn? = nil
    private var encodeRedeemOfflineNoteFn: EncodeOfflineNoteTxFn? = nil
    private var encodeAuditOfflineNoteFn: EncodeOfflineNoteTxFn? = nil
    private var encodeDefundOfflineNoteFn: EncodeDefundOfflineNoteTxFn? = nil
    private var encodeIssueOfflineNoteWithMetadataFn: EncodeOfflineNoteTxWithMetadataFn? = nil
    private var encodeRedeemOfflineNoteWithMetadataFn: EncodeOfflineNoteTxWithMetadataFn? = nil
    private var encodeAuditOfflineNoteWithMetadataFn: EncodeOfflineNoteTxWithMetadataFn? = nil
    private var encodeDefundOfflineNoteWithMetadataFn: EncodeDefundOfflineNoteTxWithMetadataFn? = nil
#else
    private let bridgeHandle: Any? = nil
    private let encodeTransferFn: Any? = nil
    private let encodeTransferWithAlgFn: Any? = nil
    private let encodeMintFn: Any? = nil
    private let encodeMintWithAlgFn: Any? = nil
    private let encodeShieldFn: Any? = nil
    private let encodeShieldWithAlgFn: Any? = nil
    private let encodeUnshieldFn: Any? = nil
    private let encodeUnshieldWithAlgFn: Any? = nil
    private let encodeZkTransferFn: Any? = nil
    private let encodeZkTransferWithAlgFn: Any? = nil
    private let encodeRegisterZkAssetFn: Any? = nil
    private let encodeRegisterZkAssetWithAlgFn: Any? = nil
    private let encodeMultisigRegisterFn: Any? = nil
    private let encodeMultisigRegisterWithAlgFn: Any? = nil
    private let encodeClaimIdentifierFn: Any? = nil
    private let encodeClaimIdentifierWithAlgFn: Any? = nil
    private let encodeBurnFn: Any? = nil
    private let encodeBurnWithAlgFn: Any? = nil
    private let encodeSetKeyValueFn: Any? = nil
    private let encodeSetKeyValueWithAlgFn: Any? = nil
    private let encodeRemoveKeyValueFn: Any? = nil
    private let encodeRemoveKeyValueWithAlgFn: Any? = nil
    private let encodeGovernanceProposeDeployFn: Any? = nil
    private let encodeGovernanceProposeDeployWithAlgFn: Any? = nil
    private let encodeGovernanceCastPlainBallotFn: Any? = nil
    private let encodeGovernanceCastPlainBallotWithAlgFn: Any? = nil
    private let encodeGovernanceCastZkBallotFn: Any? = nil
    private let encodeGovernanceCastZkBallotWithAlgFn: Any? = nil
    private let encodeGovernanceEnactReferendumFn: Any? = nil
    private let encodeGovernanceEnactReferendumWithAlgFn: Any? = nil
    private let encodeGovernanceFinalizeReferendumFn: Any? = nil
    private let encodeGovernanceFinalizeReferendumWithAlgFn: Any? = nil
    private let encodeGovernancePersistCouncilFn: Any? = nil
    private let encodeGovernancePersistCouncilWithAlgFn: Any? = nil
    private let decodeSignedFn: Any? = nil
    private let decodeAssetIdFn: Any? = nil
    private let freeFn: Any? = nil
    private let setChainDiscriminantFn: Any? = nil
    private let setAccelerationConfigFn: Any? = nil
    private let encodeCiphertextFrameFn: Any? = nil
    private let encodeControlOpenFn: Any? = nil
    private let encodeControlApproveFn: Any? = nil
    private let encodeControlApproveWithAlgFn: Any? = nil
    private let encodeControlRejectFn: Any? = nil
    private let encodeControlCloseFn: Any? = nil
    private let encodeControlPingFn: Any? = nil
    private let encodeControlPongFn: Any? = nil
    private let encodeConfidentialPayloadFn: Any? = nil
    private let accountAddressParseFn: Any? = nil
    private let accountAddressRenderFn: Any? = nil
    private let publicKeyFromPrivateFn: Any? = nil
    private let keypairFromSeedFn: Any? = nil
    private let signDetachedFn: Any? = nil
    private let verifyDetachedFn: Any? = nil
    private let sm2DefaultDistidFn: Any? = nil
    private let sm2KeypairFromSeedFn: Any? = nil
    private let sm2SignFn: Any? = nil
    private let sm2VerifyFn: Any? = nil
    private let sm2PublicKeyPrefixedFn: Any? = nil
    private let sm2PublicKeyMultihashFn: Any? = nil
    private let sm2ComputeZaFn: Any? = nil
    private let secp256k1PublicKeyFn: Any? = nil
    private let secp256k1SignFn: Any? = nil
    private let secp256k1VerifyFn: Any? = nil
    private let mldsaParametersFn: Any? = nil
    private let mldsaGenerateKeypairFn: Any? = nil
    private let mldsaSignFn: Any? = nil
    private let mldsaVerifyFn: Any? = nil
    private let connectGenerateKeypairFn: Any? = nil
    private let connectPublicFromPrivateFn: Any? = nil
    private let connectDeriveKeysFn: Any? = nil
    private let connectEncryptEnvelopeFn: Any? = nil
    private let connectDecryptCiphertextFn: Any? = nil
    private let encodeEnvelopeSignRequestTxFn: Any? = nil
    private let encodeEnvelopeSignRequestRawFn: Any? = nil
    private let encodeEnvelopeSignResultOkFn: Any? = nil
    private let encodeEnvelopeSignResultOkWithAlgFn: Any? = nil
    private let encodeEnvelopeSignResultErrFn: Any? = nil
    private let encodeEnvelopeControlCloseFn: Any? = nil
    private let encodeEnvelopeControlRejectFn: Any? = nil
    private let decodeEnvelopeKindFn: Any? = nil
    private let decodeEnvelopeJSONFn: Any? = nil
    private let decodeControlKindFn: Any? = nil
    private let decodeCiphertextFrameFn: Any? = nil
    private let decodeControlOpenPubFn: Any? = nil
    private let decodeControlOpenChainIdFn: Any? = nil
    private let decodeControlOpenAppMetadataFn: Any? = nil
    private let decodeControlOpenPermissionsFn: Any? = nil
    private let decodeControlApprovePubFn: Any? = nil
    private let decodeControlApproveAccountFn: Any? = nil
    private let decodeControlApprovePermissionsFn: Any? = nil
    private let decodeControlApproveProofFn: Any? = nil
    private let decodeControlApproveSigFn: Any? = nil
    private let decodeControlApproveSigAlgFn: Any? = nil
    private let decodeControlCloseFn: Any? = nil
    private let decodeControlRejectFn: Any? = nil
    private let decodeControlPingFn: Any? = nil
    private let decodeControlPongFn: Any? = nil
    private let sorafsLocalFetchFn: Any? = nil
    private let sorafsReferenceValidateOrderbookFn: Any? = nil
    private let sorafsReferenceSignOrderbookFn: Any? = nil
    private let sorafsReferenceBuildOrderbookOrderRequestFn: Any? = nil
    private let sorafsReferenceBuildOrderbookOrderCancelFn: Any? = nil
    private let sorafsReferenceBuildOrderbookSettlementReceiptFn: Any? = nil
    private let sorafsReferenceValidatePdpPayloadFn: Any? = nil
    private let sorafsReferenceValidatePdpCommitmentChallengeFn: Any? = nil
    private let sorafsReferenceValidatePdpChallengeProofFn: Any? = nil
    private let sorafsReferenceValidatePdpBundleFn: Any? = nil
    private let daProofSummaryFn: Any? = nil
    private let blake3HashFn: Any? = nil
    private let kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn: Any? = nil
    private let kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn: Any? = nil
    private let kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn: Any? = nil
    private let kagemushaVerifyRecursiveCompactPaymentTokenFn: Any? = nil
    private let kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn: Any? = nil
    private let kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn: Any? = nil
    private let kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn: Any? = nil
    private let kagemushaBuildPallasOpenEnvelopesArchiveFn: Any? = nil
    private let kagemushaBuildPreviousProofOpenEnvelopesArchiveFn: Any? = nil
    private let kagemushaRecursiveSpendInitFn: Any? = nil
    private let kagemushaRecursiveSpendAppendFn: Any? = nil
    private let kagemushaRecursiveSpendTransitionProfileInitFn: Any? = nil
    private let kagemushaRecursiveSpendTransitionProfileAppendFn: Any? = nil
    private let kagemushaRecursiveSpendLineageAppendBoundaryFn: Any? = nil
    private let kagemushaRecursiveSpendLineageWitnessFromInitResultFn: Any? = nil
    private let kagemushaRecursiveSpendLineageWitnessAppendResultFn: Any? = nil
    private let kagemushaRecursiveSpendVerifyFn: Any? = nil
    private let kagemushaRecursiveSpendRedeemFn: Any? = nil
    private let privacyCapabilitiesFn: Any? = nil
    private let privacyProofRequestFn: Any? = nil
    private let privacyBuildProofFn: Any? = nil
    private let privacyVerifyProofFn: Any? = nil
    private let privacyFreeFn: Any? = nil
    private let privacyNativeProbeOk = false
    private let kagemushaCompactPaymentTokenNativeProbeOk = false
    private let kagemushaRecursiveAggregationNativeProbeOk = false
    private let kagemushaRecursiveSpendCompactProjectionNativeProbeOk = false
    private let kagemushaRecursiveSpendCompactProjectionVerifierNativeProbeOk = false
    private let kagemushaRecursiveSpendNativeProbeOk = false
    private let encodeIssueOfflineNoteFn: Any? = nil
    private let encodeRedeemOfflineNoteFn: Any? = nil
    private let encodeAuditOfflineNoteFn: Any? = nil
    private let encodeDefundOfflineNoteFn: Any? = nil
#endif

#if canImport(Darwin)
#endif

    #if canImport(Darwin)
    private func loadPrivacySymbols(from handle: UnsafeMutableRawPointer?) {
        guard let handle else {
            self.privacyCapabilitiesFn = nil
            self.privacyProofRequestFn = nil
            self.privacyBuildProofFn = nil
            self.privacyVerifyProofFn = nil
            self.privacyFreeFn = nil
            return
        }
        if let capabilitiesSymbol = dlsym(handle, "iroha_privacy_capabilities_v1") {
            self.privacyCapabilitiesFn = unsafeBitCast(capabilitiesSymbol, to: PrivacyCapabilitiesFn.self)
        } else {
            self.privacyCapabilitiesFn = nil
        }
        if let proofRequestSymbol = dlsym(handle, "iroha_privacy_proof_request_v1") {
            self.privacyProofRequestFn = unsafeBitCast(proofRequestSymbol, to: PrivacyProofRequestFn.self)
        } else {
            self.privacyProofRequestFn = nil
        }
        if let buildProofSymbol = dlsym(handle, "iroha_privacy_build_proof_v1") {
            self.privacyBuildProofFn = unsafeBitCast(buildProofSymbol, to: PrivacyProofArchiveFn.self)
        } else {
            self.privacyBuildProofFn = nil
        }
        if let verifyProofSymbol = dlsym(handle, "iroha_privacy_verify_proof_v1") {
            self.privacyVerifyProofFn = unsafeBitCast(verifyProofSymbol, to: PrivacyProofArchiveFn.self)
        } else {
            self.privacyVerifyProofFn = nil
        }
        if let freeSymbol = dlsym(handle, "iroha_privacy_free_buffer") {
            self.privacyFreeFn = unsafeBitCast(freeSymbol, to: FreeFn.self)
        } else {
            self.privacyFreeFn = nil
        }
    }

    private func installStaticallyLinkedBridgeIfAvailable() {
        #if IROHASWIFT_BRIDGE_PRESENT
        let abiVersion = connect_norito_bridge_abi_version()
        guard NoritoBridgeLoader.isSupportedBridgeAbiVersion(abiVersion) else {
            NSLog(
                "[NoritoNativeBridge] statically linked transfer bridge ABI mismatch expected=%u actual=%u",
                NoritoBridgeLoader.expectedBridgeAbiVersion,
                abiVersion
            )
            return
        }

        self.encodeTransferFn = connect_norito_encode_transfer_signed_transaction
        self.encodeTransferWithFeeSponsorFn = connect_norito_encode_transfer_signed_transaction_with_fee_sponsor
        self.encodeTransferWithAlgFn = connect_norito_encode_transfer_signed_transaction_alg
        self.encodeTransferWithFeeSponsorWithAlgFn =
            connect_norito_encode_transfer_signed_transaction_with_fee_sponsor_alg
        self.encodeMintFn = connect_norito_encode_mint_signed_transaction
        self.encodeMintWithAlgFn = connect_norito_encode_mint_signed_transaction_alg
        let staticHandle = dlopen(nil, RTLD_NOW | RTLD_GLOBAL)
        loadPrivacySymbols(from: staticHandle)
        if let issueNoteSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_issue_offline_note_signed_transaction") }) {
            self.encodeIssueOfflineNoteFn = unsafeBitCast(issueNoteSymbol, to: EncodeOfflineNoteTxFn.self)
        }
        if let issueNoteMetadataSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_issue_offline_note_signed_transaction_with_metadata") }) {
            self.encodeIssueOfflineNoteWithMetadataFn = unsafeBitCast(issueNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
        }
        if let redeemNoteSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_redeem_offline_note_signed_transaction") }) {
            self.encodeRedeemOfflineNoteFn = unsafeBitCast(redeemNoteSymbol, to: EncodeOfflineNoteTxFn.self)
        }
        if let redeemNoteMetadataSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_redeem_offline_note_signed_transaction_with_metadata") }) {
            self.encodeRedeemOfflineNoteWithMetadataFn = unsafeBitCast(redeemNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
        }
        if let auditNoteSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_audit_offline_note_signed_transaction") }) {
            self.encodeAuditOfflineNoteFn = unsafeBitCast(auditNoteSymbol, to: EncodeOfflineNoteTxFn.self)
        }
        if let auditNoteMetadataSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_audit_offline_note_signed_transaction_with_metadata") }) {
            self.encodeAuditOfflineNoteWithMetadataFn = unsafeBitCast(auditNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
        }
        if let defundNoteSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_defund_offline_note_signed_transaction") }) {
            self.encodeDefundOfflineNoteFn = unsafeBitCast(defundNoteSymbol, to: EncodeDefundOfflineNoteTxFn.self)
        }
        if let defundNoteMetadataSymbol = staticHandle.flatMap({ dlsym($0, "connect_norito_encode_defund_offline_note_signed_transaction_with_metadata") }) {
            self.encodeDefundOfflineNoteWithMetadataFn = unsafeBitCast(defundNoteMetadataSymbol, to: EncodeDefundOfflineNoteTxWithMetadataFn.self)
        }
        if let kagemushaSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records")
        }) {
            self.kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = unsafeBitCast(
                kagemushaSymbol,
                to: KagemushaProveVerifiedCompactPaymentTokenWithRecordsFn.self
            )
        } else {
            self.kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = nil
        }
        if let kagemushaRecursiveSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes")
        }) {
            self.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = unsafeBitCast(
                kagemushaRecursiveSymbol,
                to: KagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn.self
            )
        } else {
            self.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = nil
        }
        if let kagemushaRecursiveCompactSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes")
        }) {
            self.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = unsafeBitCast(
                kagemushaRecursiveCompactSymbol,
                to: KagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn.self
            )
        } else {
            self.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = nil
        }
        if let kagemushaRecursiveCompactVerifySymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_verify_recursive_compact_payment_token")
        }) {
            self.kagemushaVerifyRecursiveCompactPaymentTokenFn = unsafeBitCast(
                kagemushaRecursiveCompactVerifySymbol,
                to: KagemushaVerifyRecursiveCompactPaymentTokenFn.self
            )
        } else {
            self.kagemushaVerifyRecursiveCompactPaymentTokenFn = nil
        }
        if let kagemushaRecursiveSpendCompactProjectionSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle")
        }) {
            self.kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn = unsafeBitCast(
                kagemushaRecursiveSpendCompactProjectionSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn = nil
        }
        if let kagemushaRecursiveSpendCompactProjectionVerifySymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection")
        }) {
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = unsafeBitCast(
                kagemushaRecursiveSpendCompactProjectionVerifySymbol,
                to: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn.self
            )
        } else {
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = nil
        }
        if let kagemushaRecursiveSpendCompactProjectionVerifyAtHeightSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height")
        }) {
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = unsafeBitCast(
                kagemushaRecursiveSpendCompactProjectionVerifyAtHeightSymbol,
                to: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn.self
            )
        } else {
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = nil
        }
        if let kagemushaBuildPallasOpenEnvelopesArchiveSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_build_pallas_open_envelopes_archive")
        }) {
            self.kagemushaBuildPallasOpenEnvelopesArchiveFn = unsafeBitCast(
                kagemushaBuildPallasOpenEnvelopesArchiveSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaBuildPallasOpenEnvelopesArchiveFn = nil
        }
        if let kagemushaBuildPreviousProofOpenEnvelopesArchiveSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive")
        }) {
            self.kagemushaBuildPreviousProofOpenEnvelopesArchiveFn = unsafeBitCast(
                kagemushaBuildPreviousProofOpenEnvelopesArchiveSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaBuildPreviousProofOpenEnvelopesArchiveFn = nil
        }
        if let kagemushaRecursiveSpendInitSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_init")
        }) {
            self.kagemushaRecursiveSpendInitFn = unsafeBitCast(
                kagemushaRecursiveSpendInitSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendInitFn = nil
        }
        if let kagemushaRecursiveSpendAppendSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_append")
        }) {
            self.kagemushaRecursiveSpendAppendFn = unsafeBitCast(
                kagemushaRecursiveSpendAppendSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendAppendFn = nil
        }
        if let kagemushaRecursiveSpendTransitionProfileInitSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_transition_profile_init")
        }) {
            self.kagemushaRecursiveSpendTransitionProfileInitFn = unsafeBitCast(
                kagemushaRecursiveSpendTransitionProfileInitSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendTransitionProfileInitFn = nil
        }
        if let kagemushaRecursiveSpendTransitionProfileAppendSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_transition_profile_append")
        }) {
            self.kagemushaRecursiveSpendTransitionProfileAppendFn = unsafeBitCast(
                kagemushaRecursiveSpendTransitionProfileAppendSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendTransitionProfileAppendFn = nil
        }
        if let kagemushaRecursiveSpendLineageAppendBoundarySymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_lineage_append_boundary")
        }) {
            self.kagemushaRecursiveSpendLineageAppendBoundaryFn = unsafeBitCast(
                kagemushaRecursiveSpendLineageAppendBoundarySymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendLineageAppendBoundaryFn = nil
        }
        if let kagemushaRecursiveSpendLineageWitnessFromInitResultSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result")
        }) {
            self.kagemushaRecursiveSpendLineageWitnessFromInitResultFn = unsafeBitCast(
                kagemushaRecursiveSpendLineageWitnessFromInitResultSymbol,
                to: KagemushaRecursiveSpendLineageWitnessFromInitResultFn.self
            )
        } else {
            self.kagemushaRecursiveSpendLineageWitnessFromInitResultFn = nil
        }
        if let kagemushaRecursiveSpendLineageWitnessAppendResultSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result")
        }) {
            self.kagemushaRecursiveSpendLineageWitnessAppendResultFn = unsafeBitCast(
                kagemushaRecursiveSpendLineageWitnessAppendResultSymbol,
                to: KagemushaRecursiveSpendLineageWitnessAppendResultFn.self
            )
        } else {
            self.kagemushaRecursiveSpendLineageWitnessAppendResultFn = nil
        }
        if let kagemushaRecursiveSpendVerifySymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_verify")
        }) {
            self.kagemushaRecursiveSpendVerifyFn = unsafeBitCast(
                kagemushaRecursiveSpendVerifySymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendVerifyFn = nil
        }
        if let kagemushaRecursiveSpendRedeemSymbol = staticHandle.flatMap({
            dlsym($0, "connect_norito_kagemusha_recursive_spend_redeem")
        }) {
            self.kagemushaRecursiveSpendRedeemFn = unsafeBitCast(
                kagemushaRecursiveSpendRedeemSymbol,
                to: KagemushaRecursiveSpendArchiveFn.self
            )
        } else {
            self.kagemushaRecursiveSpendRedeemFn = nil
        }
        self.freeFn = connect_norito_free
        self.setChainDiscriminantFn = connect_norito_set_chain_discriminant
        self.bridgeStatus = .valid(path: "static", identifier: NoritoBridgeLoader.currentIdentifier())
        NSLog("[NoritoNativeBridge] using statically linked Norito bridge")
        #endif
    }

    private func withSignedOutputs<R>(
        signedPtr: inout UnsafeMutablePointer<UInt8>?,
        signedLen: inout UInt,
        _ body: (UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
                 UnsafeMutablePointer<UInt>?) -> R
    ) -> R {
        return withUnsafeMutablePointer(to: &signedPtr) { signedPtrPtr in
            withUnsafeMutablePointer(to: &signedLen) { signedLenPtr in
                body(signedPtrPtr, signedLenPtr)
            }
        }
    }

    private func withOptionalCString<R>(
        _ value: String?,
        _ body: (UnsafePointer<CChar>?, UInt, UInt8) -> R
    ) -> R {
        guard let value else {
            return body(nil, 0, 0)
        }
        return value.withCString { pointer in
            body(pointer, UInt(value.utf8.count), 1)
        }
    }
#endif

    func withChainDiscriminant<R>(_ discriminant: UInt16?, _ body: () throws -> R) rethrows -> R {
        #if canImport(Darwin)
        guard let discriminant, let setChainDiscriminantFn else {
            return try body()
        }
        chainDiscriminantLock.lock()
        let previous = setChainDiscriminantFn(discriminant)
        defer {
            _ = setChainDiscriminantFn(previous)
            chainDiscriminantLock.unlock()
        }
        return try body()
        #else
        return try body()
        #endif
    }

    private func inferredAuthorityDiscriminant(_ authority: String) -> UInt16? {
        inferredAccountAddressDiscriminant(authority)
    }

    private func withAuthorityChainDiscriminant<R>(
        authority: String,
        _ body: () throws -> R
    ) throws -> R {
        try withChainDiscriminant(inferredAuthorityDiscriminant(authority), body)
    }

    private init() {
        #if canImport(Darwin)
        if Self.shouldDisableBridgeForHostedXCTestApp {
            self.bridgeStatus = .missing(path: "disabled for hosted XCTest app")
            NSLog("[NoritoNativeBridge] native bridge disabled for hosted XCTest app")
            return
        }
        let loadResult = NoritoBridgeLoader.openHandle()
        let handle = loadResult.0
        self.bridgeStatus = loadResult.1
        self.bridgeHandle = handle
        if let handle,
           let encodeSymbol = dlsym(handle, "connect_norito_encode_transfer_signed_transaction"),
           let freeSymbol = dlsym(handle, "connect_norito_free") {
            self.encodeTransferFn = unsafeBitCast(encodeSymbol, to: EncodeTransferFn.self)
            self.freeFn = unsafeBitCast(freeSymbol, to: FreeFn.self)
            loadPrivacySymbols(from: handle)
            if let encodeFeeSponsorSymbol = dlsym(handle, "connect_norito_encode_transfer_signed_transaction_with_fee_sponsor") {
                self.encodeTransferWithFeeSponsorFn = unsafeBitCast(
                    encodeFeeSponsorSymbol,
                    to: EncodeTransferWithFeeSponsorFn.self
                )
            } else {
                self.encodeTransferWithFeeSponsorFn = nil
            }
            if let chainSymbol = dlsym(handle, "connect_norito_set_chain_discriminant") {
                self.setChainDiscriminantFn = unsafeBitCast(chainSymbol, to: SetChainDiscriminantFn.self)
            } else {
                self.setChainDiscriminantFn = nil
            }
            if let encodeAlgSymbol = dlsym(handle, "connect_norito_encode_transfer_signed_transaction_alg") {
                self.encodeTransferWithAlgFn = unsafeBitCast(encodeAlgSymbol, to: EncodeTransferWithAlgFn.self)
            } else {
                self.encodeTransferWithAlgFn = nil
            }
            if let encodeFeeSponsorAlgSymbol = dlsym(handle, "connect_norito_encode_transfer_signed_transaction_with_fee_sponsor_alg") {
                self.encodeTransferWithFeeSponsorWithAlgFn = unsafeBitCast(
                    encodeFeeSponsorAlgSymbol,
                    to: EncodeTransferWithFeeSponsorWithAlgFn.self
                )
            } else {
                self.encodeTransferWithFeeSponsorWithAlgFn = nil
            }
            if let mintSymbol = dlsym(handle, "connect_norito_encode_mint_signed_transaction") {
                self.encodeMintFn = unsafeBitCast(mintSymbol, to: EncodeMintFn.self)
            } else {
                self.encodeMintFn = nil
            }
            if let mintAlgSymbol = dlsym(handle, "connect_norito_encode_mint_signed_transaction_alg") {
                self.encodeMintWithAlgFn = unsafeBitCast(mintAlgSymbol, to: EncodeMintWithAlgFn.self)
            } else {
                self.encodeMintWithAlgFn = nil
            }
            if let issueNoteSymbol = dlsym(handle, "connect_norito_encode_issue_offline_note_signed_transaction") {
                self.encodeIssueOfflineNoteFn = unsafeBitCast(issueNoteSymbol, to: EncodeOfflineNoteTxFn.self)
            } else {
                self.encodeIssueOfflineNoteFn = nil
            }
            if let issueNoteMetadataSymbol = dlsym(handle, "connect_norito_encode_issue_offline_note_signed_transaction_with_metadata") {
                self.encodeIssueOfflineNoteWithMetadataFn = unsafeBitCast(issueNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
            } else {
                self.encodeIssueOfflineNoteWithMetadataFn = nil
            }
            if let redeemNoteSymbol = dlsym(handle, "connect_norito_encode_redeem_offline_note_signed_transaction") {
                self.encodeRedeemOfflineNoteFn = unsafeBitCast(redeemNoteSymbol, to: EncodeOfflineNoteTxFn.self)
            } else {
                self.encodeRedeemOfflineNoteFn = nil
            }
            if let redeemNoteMetadataSymbol = dlsym(handle, "connect_norito_encode_redeem_offline_note_signed_transaction_with_metadata") {
                self.encodeRedeemOfflineNoteWithMetadataFn = unsafeBitCast(redeemNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
            } else {
                self.encodeRedeemOfflineNoteWithMetadataFn = nil
            }
            if let auditNoteSymbol = dlsym(handle, "connect_norito_encode_audit_offline_note_signed_transaction") {
                self.encodeAuditOfflineNoteFn = unsafeBitCast(auditNoteSymbol, to: EncodeOfflineNoteTxFn.self)
            } else {
                self.encodeAuditOfflineNoteFn = nil
            }
            if let auditNoteMetadataSymbol = dlsym(handle, "connect_norito_encode_audit_offline_note_signed_transaction_with_metadata") {
                self.encodeAuditOfflineNoteWithMetadataFn = unsafeBitCast(auditNoteMetadataSymbol, to: EncodeOfflineNoteTxWithMetadataFn.self)
            } else {
                self.encodeAuditOfflineNoteWithMetadataFn = nil
            }
            if let defundNoteSymbol = dlsym(handle, "connect_norito_encode_defund_offline_note_signed_transaction") {
                self.encodeDefundOfflineNoteFn = unsafeBitCast(defundNoteSymbol, to: EncodeDefundOfflineNoteTxFn.self)
            } else {
                self.encodeDefundOfflineNoteFn = nil
            }
            if let defundNoteMetadataSymbol = dlsym(handle, "connect_norito_encode_defund_offline_note_signed_transaction_with_metadata") {
                self.encodeDefundOfflineNoteWithMetadataFn = unsafeBitCast(defundNoteMetadataSymbol, to: EncodeDefundOfflineNoteTxWithMetadataFn.self)
            } else {
                self.encodeDefundOfflineNoteWithMetadataFn = nil
            }
            if let shieldSymbol = dlsym(handle, "connect_norito_encode_shield_signed_transaction") {
                self.encodeShieldFn = unsafeBitCast(shieldSymbol, to: EncodeShieldFn.self)
            } else {
                self.encodeShieldFn = nil
            }
            if let shieldAlgSymbol = dlsym(handle, "connect_norito_encode_shield_signed_transaction_alg") {
                self.encodeShieldWithAlgFn = unsafeBitCast(shieldAlgSymbol, to: EncodeShieldWithAlgFn.self)
            } else {
                self.encodeShieldWithAlgFn = nil
            }
            if let unshieldSymbol = dlsym(handle, "connect_norito_encode_unshield_signed_transaction") {
                self.encodeUnshieldFn = unsafeBitCast(unshieldSymbol, to: EncodeUnshieldFn.self)
            } else {
                self.encodeUnshieldFn = nil
            }
            if let unshieldAlgSymbol = dlsym(handle, "connect_norito_encode_unshield_signed_transaction_alg") {
                self.encodeUnshieldWithAlgFn = unsafeBitCast(unshieldAlgSymbol, to: EncodeUnshieldWithAlgFn.self)
            } else {
                self.encodeUnshieldWithAlgFn = nil
            }
            if let zkTransferSymbol = dlsym(handle, "connect_norito_encode_zk_transfer_signed_transaction") {
                self.encodeZkTransferFn = unsafeBitCast(zkTransferSymbol, to: EncodeZkTransferFn.self)
            } else {
                self.encodeZkTransferFn = nil
            }
            if let zkTransferAlgSymbol = dlsym(handle, "connect_norito_encode_zk_transfer_signed_transaction_alg") {
                self.encodeZkTransferWithAlgFn = unsafeBitCast(zkTransferAlgSymbol, to: EncodeZkTransferWithAlgFn.self)
            } else {
                self.encodeZkTransferWithAlgFn = nil
            }
            if let registerZkSymbol = dlsym(handle, "connect_norito_encode_register_zk_asset_signed_transaction") {
                self.encodeRegisterZkAssetFn = unsafeBitCast(registerZkSymbol, to: EncodeRegisterZkAssetFn.self)
            } else {
                self.encodeRegisterZkAssetFn = nil
            }
            if let registerZkAlgSymbol = dlsym(handle, "connect_norito_encode_register_zk_asset_signed_transaction_alg") {
                self.encodeRegisterZkAssetWithAlgFn = unsafeBitCast(registerZkAlgSymbol, to: EncodeRegisterZkAssetWithAlgFn.self)
            } else {
                self.encodeRegisterZkAssetWithAlgFn = nil
            }
            if let multisigRegisterSymbol = dlsym(handle, "connect_norito_encode_multisig_register_signed_transaction") {
                self.encodeMultisigRegisterFn = unsafeBitCast(multisigRegisterSymbol, to: EncodeMultisigRegisterFn.self)
            } else {
                self.encodeMultisigRegisterFn = nil
            }
            if let multisigRegisterAlgSymbol = dlsym(handle, "connect_norito_encode_multisig_register_signed_transaction_alg") {
                self.encodeMultisigRegisterWithAlgFn = unsafeBitCast(multisigRegisterAlgSymbol, to: EncodeMultisigRegisterWithAlgFn.self)
            } else {
                self.encodeMultisigRegisterWithAlgFn = nil
            }
            if let claimIdentifierSymbol = dlsym(handle, "connect_norito_encode_claim_identifier_signed_transaction") {
                self.encodeClaimIdentifierFn = unsafeBitCast(claimIdentifierSymbol, to: EncodeClaimIdentifierFn.self)
            } else {
                self.encodeClaimIdentifierFn = nil
            }
            if let claimIdentifierAlgSymbol = dlsym(handle, "connect_norito_encode_claim_identifier_signed_transaction_alg") {
                self.encodeClaimIdentifierWithAlgFn = unsafeBitCast(claimIdentifierAlgSymbol, to: EncodeClaimIdentifierWithAlgFn.self)
            } else {
                self.encodeClaimIdentifierWithAlgFn = nil
            }
            if let burnSymbol = dlsym(handle, "connect_norito_encode_burn_signed_transaction") {
                self.encodeBurnFn = unsafeBitCast(burnSymbol, to: EncodeBurnFn.self)
            } else {
                self.encodeBurnFn = nil
            }
            if let burnAlgSymbol = dlsym(handle, "connect_norito_encode_burn_signed_transaction_alg") {
                self.encodeBurnWithAlgFn = unsafeBitCast(burnAlgSymbol, to: EncodeBurnWithAlgFn.self)
            } else {
                self.encodeBurnWithAlgFn = nil
            }
            if let setMetadataSymbol = dlsym(handle, "connect_norito_encode_set_key_value_signed_transaction") {
                self.encodeSetKeyValueFn = unsafeBitCast(setMetadataSymbol, to: EncodeSetKeyValueFn.self)
            } else {
                self.encodeSetKeyValueFn = nil
            }
            if let setMetadataAlgSymbol = dlsym(handle, "connect_norito_encode_set_key_value_signed_transaction_alg") {
                self.encodeSetKeyValueWithAlgFn = unsafeBitCast(setMetadataAlgSymbol, to: EncodeSetKeyValueWithAlgFn.self)
            } else {
                self.encodeSetKeyValueWithAlgFn = nil
            }
            if let removeMetadataSymbol = dlsym(handle, "connect_norito_encode_remove_key_value_signed_transaction") {
                self.encodeRemoveKeyValueFn = unsafeBitCast(removeMetadataSymbol, to: EncodeRemoveKeyValueFn.self)
            } else {
                self.encodeRemoveKeyValueFn = nil
            }
            if let removeMetadataAlgSymbol = dlsym(handle, "connect_norito_encode_remove_key_value_signed_transaction_alg") {
                self.encodeRemoveKeyValueWithAlgFn = unsafeBitCast(removeMetadataAlgSymbol, to: EncodeRemoveKeyValueWithAlgFn.self)
            } else {
                self.encodeRemoveKeyValueWithAlgFn = nil
            }
            if let proposeDeploySymbol = dlsym(handle, "connect_norito_encode_governance_propose_deploy_signed_transaction") {
                self.encodeGovernanceProposeDeployFn = unsafeBitCast(proposeDeploySymbol, to: EncodeGovernanceProposeDeployFn.self)
            } else {
                self.encodeGovernanceProposeDeployFn = nil
            }
            if let proposeDeployAlgSymbol = dlsym(handle, "connect_norito_encode_governance_propose_deploy_signed_transaction_alg") {
                self.encodeGovernanceProposeDeployWithAlgFn = unsafeBitCast(proposeDeployAlgSymbol, to: EncodeGovernanceProposeDeployWithAlgFn.self)
            } else {
                self.encodeGovernanceProposeDeployWithAlgFn = nil
            }
            if let castPlainSymbol = dlsym(handle, "connect_norito_encode_governance_cast_plain_ballot_signed_transaction") {
                self.encodeGovernanceCastPlainBallotFn = unsafeBitCast(castPlainSymbol, to: EncodeGovernanceCastPlainBallotFn.self)
            } else {
                self.encodeGovernanceCastPlainBallotFn = nil
            }
            if let castPlainAlgSymbol = dlsym(handle, "connect_norito_encode_governance_cast_plain_ballot_signed_transaction_alg") {
                self.encodeGovernanceCastPlainBallotWithAlgFn = unsafeBitCast(castPlainAlgSymbol, to: EncodeGovernanceCastPlainBallotWithAlgFn.self)
            } else {
                self.encodeGovernanceCastPlainBallotWithAlgFn = nil
            }
            if let castZkSymbol = dlsym(handle, "connect_norito_encode_governance_cast_zk_ballot_signed_transaction") {
                self.encodeGovernanceCastZkBallotFn = unsafeBitCast(castZkSymbol, to: EncodeGovernanceCastZkBallotFn.self)
            } else {
                self.encodeGovernanceCastZkBallotFn = nil
            }
            if let castZkAlgSymbol = dlsym(handle, "connect_norito_encode_governance_cast_zk_ballot_signed_transaction_alg") {
                self.encodeGovernanceCastZkBallotWithAlgFn = unsafeBitCast(castZkAlgSymbol, to: EncodeGovernanceCastZkBallotWithAlgFn.self)
            } else {
                self.encodeGovernanceCastZkBallotWithAlgFn = nil
            }
            if let enactSymbol = dlsym(handle, "connect_norito_encode_governance_enact_referendum_signed_transaction") {
                self.encodeGovernanceEnactReferendumFn = unsafeBitCast(enactSymbol, to: EncodeGovernanceEnactReferendumFn.self)
            } else {
                self.encodeGovernanceEnactReferendumFn = nil
            }
            if let enactAlgSymbol = dlsym(handle, "connect_norito_encode_governance_enact_referendum_signed_transaction_alg") {
                self.encodeGovernanceEnactReferendumWithAlgFn = unsafeBitCast(enactAlgSymbol, to: EncodeGovernanceEnactReferendumWithAlgFn.self)
            } else {
                self.encodeGovernanceEnactReferendumWithAlgFn = nil
            }
            if let finalizeSymbol = dlsym(handle, "connect_norito_encode_governance_finalize_referendum_signed_transaction") {
                self.encodeGovernanceFinalizeReferendumFn = unsafeBitCast(finalizeSymbol, to: EncodeGovernanceFinalizeReferendumFn.self)
            } else {
                self.encodeGovernanceFinalizeReferendumFn = nil
            }
            if let finalizeAlgSymbol = dlsym(handle, "connect_norito_encode_governance_finalize_referendum_signed_transaction_alg") {
                self.encodeGovernanceFinalizeReferendumWithAlgFn = unsafeBitCast(finalizeAlgSymbol, to: EncodeGovernanceFinalizeReferendumWithAlgFn.self)
            } else {
                self.encodeGovernanceFinalizeReferendumWithAlgFn = nil
            }
            if let persistSymbol = dlsym(handle, "connect_norito_encode_governance_persist_council_signed_transaction") {
                self.encodeGovernancePersistCouncilFn = unsafeBitCast(persistSymbol, to: EncodeGovernancePersistCouncilFn.self)
            } else {
                self.encodeGovernancePersistCouncilFn = nil
            }
            if let persistAlgSymbol = dlsym(handle, "connect_norito_encode_governance_persist_council_signed_transaction_alg") {
                self.encodeGovernancePersistCouncilWithAlgFn = unsafeBitCast(persistAlgSymbol, to: EncodeGovernancePersistCouncilWithAlgFn.self)
            } else {
                self.encodeGovernancePersistCouncilWithAlgFn = nil
            }
            if let decodeSymbol = dlsym(handle, "connect_norito_decode_signed_transaction_json") {
                self.decodeSignedFn = unsafeBitCast(decodeSymbol, to: DecodeSignedFn.self)
            } else {
                self.decodeSignedFn = nil
            }
            if let decodeReceiptSymbol = dlsym(handle, "connect_norito_decode_transaction_receipt_json") {
                self.decodeReceiptFn = unsafeBitCast(decodeReceiptSymbol, to: DecodeReceiptFn.self)
            } else {
                self.decodeReceiptFn = nil
            }
            if let decodeAssetIdSymbol = dlsym(handle, "connect_norito_decode_asset_id_json") {
                self.decodeAssetIdFn = unsafeBitCast(decodeAssetIdSymbol, to: DecodeAssetIdFn.self)
            } else {
                self.decodeAssetIdFn = nil
            }
            if let publicKeyFromPrivateSymbol = dlsym(handle, "connect_norito_public_key_from_private") {
                self.publicKeyFromPrivateFn = unsafeBitCast(publicKeyFromPrivateSymbol, to: PublicKeyFromPrivateFn.self)
            } else {
                self.publicKeyFromPrivateFn = nil
            }
            if let keypairFromSeedSymbol = dlsym(handle, "connect_norito_keypair_from_seed") {
                self.keypairFromSeedFn = unsafeBitCast(keypairFromSeedSymbol, to: KeypairFromSeedFn.self)
            } else {
                self.keypairFromSeedFn = nil
            }
            if let signDetachedSymbol = dlsym(handle, "connect_norito_sign_detached") {
                self.signDetachedFn = unsafeBitCast(signDetachedSymbol, to: SignDetachedFn.self)
            } else {
                self.signDetachedFn = nil
            }
            if let verifyDetachedSymbol = dlsym(handle, "connect_norito_verify_detached") {
                self.verifyDetachedFn = unsafeBitCast(verifyDetachedSymbol, to: VerifyDetachedFn.self)
            } else {
                self.verifyDetachedFn = nil
            }
            if let accelSymbol = dlsym(handle, "connect_norito_set_acceleration_config") {
                self.setAccelerationConfigFn = unsafeBitCast(accelSymbol, to: SetAccelerationConfigFn.self)
            } else {
                self.setAccelerationConfigFn = nil
            }
            if let accelGetSymbol = dlsym(handle, "connect_norito_get_acceleration_config") {
                self.getAccelerationConfigFn = unsafeBitCast(accelGetSymbol, to: GetAccelerationConfigFn.self)
            } else {
                self.getAccelerationConfigFn = nil
            }
            if let accelStateSymbol = dlsym(handle, "connect_norito_get_acceleration_state") {
                self.getAccelerationStateFn = unsafeBitCast(accelStateSymbol, to: GetAccelerationStateFn.self)
            } else {
                self.getAccelerationStateFn = nil
            }
            if let encodeCiphertextSymbol = dlsym(handle, "connect_norito_encode_ciphertext_frame") {
                self.encodeCiphertextFrameFn = unsafeBitCast(encodeCiphertextSymbol, to: EncodeCiphertextFrameFn.self)
            } else {
                self.encodeCiphertextFrameFn = nil
            }
            if let encodeControlOpenSymbol = dlsym(handle, "connect_norito_encode_control_open_ext") {
                self.encodeControlOpenFn = unsafeBitCast(encodeControlOpenSymbol, to: EncodeControlOpenFn.self)
            } else {
                self.encodeControlOpenFn = nil
            }
            if let encodeControlApproveSymbol = dlsym(handle, "connect_norito_encode_control_approve_ext") {
                self.encodeControlApproveFn = unsafeBitCast(encodeControlApproveSymbol, to: EncodeControlApproveFn.self)
            } else {
                self.encodeControlApproveFn = nil
            }
            if let encodeControlApproveAlgSymbol = dlsym(handle, "connect_norito_encode_control_approve_ext_with_alg") {
                self.encodeControlApproveWithAlgFn = unsafeBitCast(encodeControlApproveAlgSymbol, to: EncodeControlApproveWithAlgFn.self)
            } else {
                self.encodeControlApproveWithAlgFn = nil
            }
            if let encodeControlRejectSymbol = dlsym(handle, "connect_norito_encode_control_reject") {
                self.encodeControlRejectFn = unsafeBitCast(encodeControlRejectSymbol, to: EncodeControlRejectFn.self)
            } else {
                self.encodeControlRejectFn = nil
            }
            if let encodeControlCloseSymbol = dlsym(handle, "connect_norito_encode_control_close") {
                self.encodeControlCloseFn = unsafeBitCast(encodeControlCloseSymbol, to: EncodeControlCloseFn.self)
            } else {
                self.encodeControlCloseFn = nil
            }
            if let encodeControlPingSymbol = dlsym(handle, "connect_norito_encode_control_ping") {
                self.encodeControlPingFn = unsafeBitCast(encodeControlPingSymbol, to: EncodeControlPingFn.self)
            } else {
                self.encodeControlPingFn = nil
            }
            if let encodeControlPongSymbol = dlsym(handle, "connect_norito_encode_control_pong") {
                self.encodeControlPongFn = unsafeBitCast(encodeControlPongSymbol, to: EncodeControlPongFn.self)
            } else {
                self.encodeControlPongFn = nil
            }
            if let encodeConfidentialSymbol = dlsym(handle, "connect_norito_encode_confidential_encrypted_payload") {
                self.encodeConfidentialPayloadFn = unsafeBitCast(encodeConfidentialSymbol, to: EncodeConfidentialPayloadFn.self)
            } else {
                self.encodeConfidentialPayloadFn = nil
            }
            if let accountAddressParseSymbol = dlsym(handle, "connect_norito_account_address_parse") {
                self.accountAddressParseFn = unsafeBitCast(accountAddressParseSymbol, to: AccountAddressParseFn.self)
            } else {
                self.accountAddressParseFn = nil
            }
            if let accountAddressRenderSymbol = dlsym(handle, "connect_norito_account_address_render") {
                self.accountAddressRenderFn = unsafeBitCast(accountAddressRenderSymbol, to: AccountAddressRenderFn.self)
            } else {
                self.accountAddressRenderFn = nil
            }
            if let connectGenerateKeypairSymbol = dlsym(handle, "connect_norito_connect_generate_keypair") {
                self.connectGenerateKeypairFn = unsafeBitCast(connectGenerateKeypairSymbol, to: ConnectGenerateKeypairFn.self)
            } else {
                self.connectGenerateKeypairFn = nil
            }
            if let connectPublicFromPrivateSymbol = dlsym(handle, "connect_norito_connect_public_from_private") {
                self.connectPublicFromPrivateFn = unsafeBitCast(connectPublicFromPrivateSymbol, to: ConnectPublicFromPrivateFn.self)
            } else {
                self.connectPublicFromPrivateFn = nil
            }
            if let connectDeriveKeysSymbol = dlsym(handle, "connect_norito_connect_derive_keys") {
                self.connectDeriveKeysFn = unsafeBitCast(connectDeriveKeysSymbol, to: ConnectDeriveKeysFn.self)
            } else {
                self.connectDeriveKeysFn = nil
            }
            if let connectEncryptEnvelopeSymbol = dlsym(handle, "connect_norito_connect_encrypt_envelope") {
                self.connectEncryptEnvelopeFn = unsafeBitCast(connectEncryptEnvelopeSymbol, to: ConnectEncryptEnvelopeFn.self)
            } else {
                self.connectEncryptEnvelopeFn = nil
            }
            if let connectDecryptCiphertextSymbol = dlsym(handle, "connect_norito_connect_decrypt_ciphertext") {
                self.connectDecryptCiphertextFn = unsafeBitCast(connectDecryptCiphertextSymbol, to: ConnectDecryptCiphertextFn.self)
            } else {
                self.connectDecryptCiphertextFn = nil
            }
            if let encodeEnvelopeSignRequestTxSymbol = dlsym(handle, "connect_norito_encode_envelope_sign_request_tx") {
                self.encodeEnvelopeSignRequestTxFn = unsafeBitCast(encodeEnvelopeSignRequestTxSymbol, to: EncodeEnvelopeSignRequestTxFn.self)
            } else {
                self.encodeEnvelopeSignRequestTxFn = nil
            }
            if let encodeEnvelopeSignRequestRawSymbol = dlsym(handle, "connect_norito_encode_envelope_sign_request_raw") {
                self.encodeEnvelopeSignRequestRawFn = unsafeBitCast(encodeEnvelopeSignRequestRawSymbol, to: EncodeEnvelopeSignRequestRawFn.self)
            } else {
                self.encodeEnvelopeSignRequestRawFn = nil
            }
            if let encodeEnvelopeSignResultOkSymbol = dlsym(handle, "connect_norito_encode_envelope_sign_result_ok") {
                self.encodeEnvelopeSignResultOkFn = unsafeBitCast(encodeEnvelopeSignResultOkSymbol, to: EncodeEnvelopeSignResultOkFn.self)
            } else {
                self.encodeEnvelopeSignResultOkFn = nil
            }
            if let encodeEnvelopeSignResultOkWithAlgSymbol = dlsym(handle, "connect_norito_encode_envelope_sign_result_ok_with_alg") {
                self.encodeEnvelopeSignResultOkWithAlgFn = unsafeBitCast(encodeEnvelopeSignResultOkWithAlgSymbol, to: EncodeEnvelopeSignResultOkWithAlgFn.self)
            } else {
                self.encodeEnvelopeSignResultOkWithAlgFn = nil
            }
            if let encodeEnvelopeSignResultErrSymbol = dlsym(handle, "connect_norito_encode_envelope_sign_result_err") {
                self.encodeEnvelopeSignResultErrFn = unsafeBitCast(encodeEnvelopeSignResultErrSymbol, to: EncodeEnvelopeSignResultErrFn.self)
            } else {
                self.encodeEnvelopeSignResultErrFn = nil
            }
            if let encodeEnvelopeControlCloseSymbol = dlsym(handle, "connect_norito_encode_envelope_control_close") {
                self.encodeEnvelopeControlCloseFn = unsafeBitCast(encodeEnvelopeControlCloseSymbol, to: EncodeEnvelopeControlCloseFn.self)
            } else {
                self.encodeEnvelopeControlCloseFn = nil
            }
            if let encodeEnvelopeControlRejectSymbol = dlsym(handle, "connect_norito_encode_envelope_control_reject") {
                self.encodeEnvelopeControlRejectFn = unsafeBitCast(encodeEnvelopeControlRejectSymbol, to: EncodeEnvelopeControlRejectFn.self)
            } else {
                self.encodeEnvelopeControlRejectFn = nil
            }
            if let decodeEnvelopeKindSymbol = dlsym(handle, "connect_norito_decode_envelope_kind") {
                self.decodeEnvelopeKindFn = unsafeBitCast(decodeEnvelopeKindSymbol, to: DecodeEnvelopeKindFn.self)
            } else {
                self.decodeEnvelopeKindFn = nil
            }
            if let decodeEnvelopeJsonSymbol = dlsym(handle, "connect_norito_decode_envelope_json") {
                self.decodeEnvelopeJSONFn = unsafeBitCast(decodeEnvelopeJsonSymbol, to: DecodeEnvelopeJSONFn.self)
            } else {
                self.decodeEnvelopeJSONFn = nil
            }
            if let decodeControlKindSymbol = dlsym(handle, "connect_norito_decode_control_kind") {
                self.decodeControlKindFn = unsafeBitCast(decodeControlKindSymbol, to: DecodeControlKindFn.self)
            } else {
                self.decodeControlKindFn = nil
            }
            if let decodeCiphertextSymbol = dlsym(handle, "connect_norito_decode_ciphertext_frame") {
                self.decodeCiphertextFrameFn = unsafeBitCast(decodeCiphertextSymbol, to: DecodeCiphertextFrameFn.self)
            } else {
                self.decodeCiphertextFrameFn = nil
            }
            if let decodeControlOpenPubSymbol = dlsym(handle, "connect_norito_decode_control_open_pub") {
                self.decodeControlOpenPubFn = unsafeBitCast(decodeControlOpenPubSymbol, to: DecodeControlOpenPubFn.self)
            } else {
                self.decodeControlOpenPubFn = nil
            }
            if let decodeControlOpenChainSymbol = dlsym(handle, "connect_norito_decode_control_open_chain_id") {
                self.decodeControlOpenChainIdFn = unsafeBitCast(decodeControlOpenChainSymbol, to: DecodeControlOpenChainIdFn.self)
            } else {
                self.decodeControlOpenChainIdFn = nil
            }
            if let decodeControlOpenMetadataSymbol = dlsym(handle, "connect_norito_decode_control_open_app_metadata_json") {
                self.decodeControlOpenAppMetadataFn = unsafeBitCast(decodeControlOpenMetadataSymbol, to: DecodeControlOpenAppMetadataFn.self)
            } else {
                self.decodeControlOpenAppMetadataFn = nil
            }
            if let decodeControlOpenPermsSymbol = dlsym(handle, "connect_norito_decode_control_open_permissions_json") {
                self.decodeControlOpenPermissionsFn = unsafeBitCast(decodeControlOpenPermsSymbol, to: DecodeControlOpenPermissionsFn.self)
            } else {
                self.decodeControlOpenPermissionsFn = nil
            }
            if let decodeControlApprovePubSymbol = dlsym(handle, "connect_norito_decode_control_approve_pub") {
                self.decodeControlApprovePubFn = unsafeBitCast(decodeControlApprovePubSymbol, to: DecodeControlApprovePubFn.self)
            } else {
                self.decodeControlApprovePubFn = nil
            }
            if let decodeControlApproveAccountSymbol = dlsym(handle, "connect_norito_decode_control_approve_account") {
                self.decodeControlApproveAccountFn = unsafeBitCast(decodeControlApproveAccountSymbol, to: DecodeControlApproveAccountFn.self)
            } else {
                self.decodeControlApproveAccountFn = nil
            }
            if let decodeControlApprovePermsSymbol = dlsym(handle, "connect_norito_decode_control_approve_permissions_json") {
                self.decodeControlApprovePermissionsFn = unsafeBitCast(decodeControlApprovePermsSymbol, to: DecodeControlApprovePermissionsFn.self)
            } else {
                self.decodeControlApprovePermissionsFn = nil
            }
            if let decodeControlApproveProofSymbol = dlsym(handle, "connect_norito_decode_control_approve_proof_json") {
                self.decodeControlApproveProofFn = unsafeBitCast(decodeControlApproveProofSymbol, to: DecodeControlApproveProofFn.self)
            } else {
                self.decodeControlApproveProofFn = nil
            }
            if let decodeControlApproveSigSymbol = dlsym(handle, "connect_norito_decode_control_approve_sig") {
                self.decodeControlApproveSigFn = unsafeBitCast(decodeControlApproveSigSymbol, to: DecodeControlApproveSigFn.self)
            } else {
                self.decodeControlApproveSigFn = nil
            }
            if let decodeControlApproveSigAlgSymbol = dlsym(handle, "connect_norito_decode_control_approve_sig_alg") {
                self.decodeControlApproveSigAlgFn = unsafeBitCast(decodeControlApproveSigAlgSymbol, to: DecodeControlApproveSigAlgFn.self)
            } else {
                self.decodeControlApproveSigAlgFn = nil
            }
            if let decodeControlCloseSymbol = dlsym(handle, "connect_norito_decode_control_close") {
                self.decodeControlCloseFn = unsafeBitCast(decodeControlCloseSymbol, to: DecodeControlCloseFn.self)
            } else {
                self.decodeControlCloseFn = nil
            }
            if let decodeControlRejectSymbol = dlsym(handle, "connect_norito_decode_control_reject") {
                self.decodeControlRejectFn = unsafeBitCast(decodeControlRejectSymbol, to: DecodeControlRejectFn.self)
            } else {
                self.decodeControlRejectFn = nil
            }
            if let decodeControlPingSymbol = dlsym(handle, "connect_norito_decode_control_ping") {
                self.decodeControlPingFn = unsafeBitCast(decodeControlPingSymbol, to: DecodeControlPingFn.self)
            } else {
                self.decodeControlPingFn = nil
            }
            if let decodeControlPongSymbol = dlsym(handle, "connect_norito_decode_control_pong") {
                self.decodeControlPongFn = unsafeBitCast(decodeControlPongSymbol, to: DecodeControlPongFn.self)
            } else {
                self.decodeControlPongFn = nil
            }
            if let sorafsLocalFetchSymbol = dlsym(handle, "connect_norito_sorafs_local_fetch") {
                self.sorafsLocalFetchFn = unsafeBitCast(sorafsLocalFetchSymbol, to: SorafsLocalFetchFn.self)
            } else {
                self.sorafsLocalFetchFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_validate_orderbook_json") {
                self.sorafsReferenceValidateOrderbookFn = unsafeBitCast(symbol, to: SorafsReferencePayloadFn.self)
            } else {
                self.sorafsReferenceValidateOrderbookFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_sign_orderbook_payload") {
                self.sorafsReferenceSignOrderbookFn = unsafeBitCast(symbol, to: SorafsReferenceOrderbookSignFn.self)
            } else {
                self.sorafsReferenceSignOrderbookFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_build_signed_orderbook_order_request") {
                self.sorafsReferenceBuildOrderbookOrderRequestFn = unsafeBitCast(
                    symbol,
                    to: SorafsReferenceOrderbookOrderRequestBuilderFn.self
                )
            } else {
                self.sorafsReferenceBuildOrderbookOrderRequestFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_build_signed_orderbook_order_cancel") {
                self.sorafsReferenceBuildOrderbookOrderCancelFn = unsafeBitCast(
                    symbol,
                    to: SorafsReferenceOrderbookCancelBuilderFn.self
                )
            } else {
                self.sorafsReferenceBuildOrderbookOrderCancelFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt") {
                self.sorafsReferenceBuildOrderbookSettlementReceiptFn = unsafeBitCast(
                    symbol,
                    to: SorafsReferenceOrderbookSettlementReceiptBuilderFn.self
                )
            } else {
                self.sorafsReferenceBuildOrderbookSettlementReceiptFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_validate_pdp_payload_json") {
                self.sorafsReferenceValidatePdpPayloadFn = unsafeBitCast(symbol, to: SorafsReferencePayloadFn.self)
            } else {
                self.sorafsReferenceValidatePdpPayloadFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json") {
                self.sorafsReferenceValidatePdpCommitmentChallengeFn = unsafeBitCast(symbol, to: SorafsReferencePdpPairFn.self)
            } else {
                self.sorafsReferenceValidatePdpCommitmentChallengeFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json") {
                self.sorafsReferenceValidatePdpChallengeProofFn = unsafeBitCast(symbol, to: SorafsReferencePdpPairFn.self)
            } else {
                self.sorafsReferenceValidatePdpChallengeProofFn = nil
            }
            if let symbol = dlsym(handle, "connect_norito_sorafs_reference_validate_pdp_bundle_json") {
                self.sorafsReferenceValidatePdpBundleFn = unsafeBitCast(symbol, to: SorafsReferencePdpBundleFn.self)
            } else {
                self.sorafsReferenceValidatePdpBundleFn = nil
            }
            if let daProofSummarySymbol = dlsym(handle, "connect_norito_da_proof_summary") {
                self.daProofSummaryFn = unsafeBitCast(daProofSummarySymbol, to: DaProofSummaryFn.self)
            } else {
                self.daProofSummaryFn = nil
            }
            if let blake3Symbol = dlsym(handle, "connect_norito_blake3_hash") {
                self.blake3HashFn = unsafeBitCast(blake3Symbol, to: Blake3HashFn.self)
            } else {
                self.blake3HashFn = nil
            }
            if let kagemushaSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records"
            ) {
                self.kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = unsafeBitCast(
                    kagemushaSymbol,
                    to: KagemushaProveVerifiedCompactPaymentTokenWithRecordsFn.self
                )
            } else {
                self.kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = nil
            }
            if let kagemushaRecursiveSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes"
            ) {
                self.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = unsafeBitCast(
                    kagemushaRecursiveSymbol,
                    to: KagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn.self
                )
            } else {
                self.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = nil
            }
            if let kagemushaRecursiveCompactSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes"
            ) {
                self.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = unsafeBitCast(
                    kagemushaRecursiveCompactSymbol,
                    to: KagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn.self
                )
            } else {
                self.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = nil
            }
            if let kagemushaRecursiveCompactVerifySymbol = dlsym(
                handle,
                "connect_norito_kagemusha_verify_recursive_compact_payment_token"
            ) {
                self.kagemushaVerifyRecursiveCompactPaymentTokenFn = unsafeBitCast(
                    kagemushaRecursiveCompactVerifySymbol,
                    to: KagemushaVerifyRecursiveCompactPaymentTokenFn.self
                )
            } else {
                self.kagemushaVerifyRecursiveCompactPaymentTokenFn = nil
            }
            if let kagemushaRecursiveSpendCompactProjectionSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle"
            ) {
                self.kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn = unsafeBitCast(
                    kagemushaRecursiveSpendCompactProjectionSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn = nil
            }
            if let kagemushaRecursiveSpendCompactProjectionVerifySymbol = dlsym(
                handle,
                "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection"
            ) {
                self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = unsafeBitCast(
                    kagemushaRecursiveSpendCompactProjectionVerifySymbol,
                    to: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn.self
                )
            } else {
                self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = nil
            }
            if let kagemushaRecursiveSpendCompactProjectionVerifyAtHeightSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
            ) {
                self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = unsafeBitCast(
                    kagemushaRecursiveSpendCompactProjectionVerifyAtHeightSymbol,
                    to: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn.self
                )
            } else {
                self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = nil
            }
            if let kagemushaBuildPallasOpenEnvelopesArchiveSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_build_pallas_open_envelopes_archive"
            ) {
                self.kagemushaBuildPallasOpenEnvelopesArchiveFn = unsafeBitCast(
                    kagemushaBuildPallasOpenEnvelopesArchiveSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaBuildPallasOpenEnvelopesArchiveFn = nil
            }
            if let kagemushaBuildPreviousProofOpenEnvelopesArchiveSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive"
            ) {
                self.kagemushaBuildPreviousProofOpenEnvelopesArchiveFn = unsafeBitCast(
                    kagemushaBuildPreviousProofOpenEnvelopesArchiveSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaBuildPreviousProofOpenEnvelopesArchiveFn = nil
            }
            if let kagemushaRecursiveSpendInitSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_init"
            ) {
                self.kagemushaRecursiveSpendInitFn = unsafeBitCast(
                    kagemushaRecursiveSpendInitSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendInitFn = nil
            }
            if let kagemushaRecursiveSpendAppendSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_append"
            ) {
                self.kagemushaRecursiveSpendAppendFn = unsafeBitCast(
                    kagemushaRecursiveSpendAppendSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendAppendFn = nil
            }
            if let kagemushaRecursiveSpendTransitionProfileInitSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_transition_profile_init"
            ) {
                self.kagemushaRecursiveSpendTransitionProfileInitFn = unsafeBitCast(
                    kagemushaRecursiveSpendTransitionProfileInitSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendTransitionProfileInitFn = nil
            }
            if let kagemushaRecursiveSpendTransitionProfileAppendSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_transition_profile_append"
            ) {
                self.kagemushaRecursiveSpendTransitionProfileAppendFn = unsafeBitCast(
                    kagemushaRecursiveSpendTransitionProfileAppendSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendTransitionProfileAppendFn = nil
            }
            if let kagemushaRecursiveSpendLineageAppendBoundarySymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_lineage_append_boundary"
            ) {
                self.kagemushaRecursiveSpendLineageAppendBoundaryFn = unsafeBitCast(
                    kagemushaRecursiveSpendLineageAppendBoundarySymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendLineageAppendBoundaryFn = nil
            }
            if let kagemushaRecursiveSpendLineageWitnessFromInitResultSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result"
            ) {
                self.kagemushaRecursiveSpendLineageWitnessFromInitResultFn = unsafeBitCast(
                    kagemushaRecursiveSpendLineageWitnessFromInitResultSymbol,
                    to: KagemushaRecursiveSpendLineageWitnessFromInitResultFn.self
                )
            } else {
                self.kagemushaRecursiveSpendLineageWitnessFromInitResultFn = nil
            }
            if let kagemushaRecursiveSpendLineageWitnessAppendResultSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result"
            ) {
                self.kagemushaRecursiveSpendLineageWitnessAppendResultFn = unsafeBitCast(
                    kagemushaRecursiveSpendLineageWitnessAppendResultSymbol,
                    to: KagemushaRecursiveSpendLineageWitnessAppendResultFn.self
                )
            } else {
                self.kagemushaRecursiveSpendLineageWitnessAppendResultFn = nil
            }
            if let kagemushaRecursiveSpendVerifySymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_verify"
            ) {
                self.kagemushaRecursiveSpendVerifyFn = unsafeBitCast(
                    kagemushaRecursiveSpendVerifySymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendVerifyFn = nil
            }
            if let kagemushaRecursiveSpendRedeemSymbol = dlsym(
                handle,
                "connect_norito_kagemusha_recursive_spend_redeem"
            ) {
                self.kagemushaRecursiveSpendRedeemFn = unsafeBitCast(
                    kagemushaRecursiveSpendRedeemSymbol,
                    to: KagemushaRecursiveSpendArchiveFn.self
                )
            } else {
                self.kagemushaRecursiveSpendRedeemFn = nil
            }
            if let sm2DefaultSymbol = dlsym(handle, "connect_norito_sm2_default_distid") {
                self.sm2DefaultDistidFn = unsafeBitCast(sm2DefaultSymbol, to: Sm2DefaultDistidFn.self)
            } else {
                self.sm2DefaultDistidFn = nil
            }
            if let sm2KeypairSymbol = dlsym(handle, "connect_norito_sm2_keypair_from_seed") {
                self.sm2KeypairFromSeedFn = unsafeBitCast(sm2KeypairSymbol, to: Sm2KeypairFromSeedFn.self)
            } else {
                self.sm2KeypairFromSeedFn = nil
            }
            if let sm2SignSymbol = dlsym(handle, "connect_norito_sm2_sign") {
                self.sm2SignFn = unsafeBitCast(sm2SignSymbol, to: Sm2SignFn.self)
            } else {
                self.sm2SignFn = nil
            }
            if let sm2VerifySymbol = dlsym(handle, "connect_norito_sm2_verify") {
                self.sm2VerifyFn = unsafeBitCast(sm2VerifySymbol, to: Sm2VerifyFn.self)
            } else {
                self.sm2VerifyFn = nil
            }
            if let sm2PrefixedSymbol = dlsym(handle, "connect_norito_sm2_public_key_prefixed") {
                self.sm2PublicKeyPrefixedFn = unsafeBitCast(sm2PrefixedSymbol, to: Sm2PublicKeyStringFn.self)
            } else {
                self.sm2PublicKeyPrefixedFn = nil
            }
            if let sm2MultihashSymbol = dlsym(handle, "connect_norito_sm2_public_key_multihash") {
                self.sm2PublicKeyMultihashFn = unsafeBitCast(sm2MultihashSymbol, to: Sm2PublicKeyStringFn.self)
            } else {
                self.sm2PublicKeyMultihashFn = nil
            }
            if let sm2ComputeZaSymbol = dlsym(handle, "connect_norito_sm2_compute_za") {
                self.sm2ComputeZaFn = unsafeBitCast(sm2ComputeZaSymbol, to: Sm2ComputeZaFn.self)
            } else {
                self.sm2ComputeZaFn = nil
            }
            if let secpPublicSymbol = dlsym(handle, "connect_norito_secp256k1_public_key") {
                self.secp256k1PublicKeyFn = unsafeBitCast(secpPublicSymbol, to: Secp256k1PublicKeyFn.self)
            } else {
                self.secp256k1PublicKeyFn = nil
            }
            if let secpSignSymbol = dlsym(handle, "connect_norito_secp256k1_sign") {
                self.secp256k1SignFn = unsafeBitCast(secpSignSymbol, to: Secp256k1SignFn.self)
            } else {
                self.secp256k1SignFn = nil
            }
            if let secpVerifySymbol = dlsym(handle, "connect_norito_secp256k1_verify") {
                self.secp256k1VerifyFn = unsafeBitCast(secpVerifySymbol, to: Secp256k1VerifyFn.self)
            } else {
                self.secp256k1VerifyFn = nil
            }
            if let mldsaParamsSymbol = dlsym(handle, "connect_norito_mldsa_parameters") ?? dlsym(handle, "soranet_mldsa_parameters") {
                self.mldsaParametersFn = unsafeBitCast(mldsaParamsSymbol, to: MldsaParametersFn.self)
            } else {
                self.mldsaParametersFn = nil
            }
            if let mldsaGenerateSymbol = dlsym(handle, "connect_norito_mldsa_generate_keypair") ?? dlsym(handle, "soranet_mldsa_generate_keypair") {
                self.mldsaGenerateKeypairFn = unsafeBitCast(mldsaGenerateSymbol, to: MldsaGenerateKeypairFn.self)
            } else {
                self.mldsaGenerateKeypairFn = nil
            }
            if let mldsaSignSymbol = dlsym(handle, "connect_norito_mldsa_sign") ?? dlsym(handle, "soranet_mldsa_sign") {
                self.mldsaSignFn = unsafeBitCast(mldsaSignSymbol, to: MldsaSignFn.self)
            } else {
                self.mldsaSignFn = nil
            }
            if let mldsaVerifySymbol = dlsym(handle, "connect_norito_mldsa_verify") ?? dlsym(handle, "soranet_mldsa_verify") {
                self.mldsaVerifyFn = unsafeBitCast(mldsaVerifySymbol, to: MldsaVerifyFn.self)
            } else {
                self.mldsaVerifyFn = nil
            }
        } else {
            self.encodeTransferFn = nil
            self.encodeTransferWithFeeSponsorFn = nil
            self.encodeTransferWithAlgFn = nil
            self.encodeTransferWithFeeSponsorWithAlgFn = nil
            self.encodeMintFn = nil
            self.encodeMintWithAlgFn = nil
            self.encodeIssueOfflineNoteFn = nil
            self.encodeRedeemOfflineNoteFn = nil
            self.encodeAuditOfflineNoteFn = nil
            self.encodeDefundOfflineNoteFn = nil
            self.encodeIssueOfflineNoteWithMetadataFn = nil
            self.encodeRedeemOfflineNoteWithMetadataFn = nil
            self.encodeAuditOfflineNoteWithMetadataFn = nil
            self.encodeDefundOfflineNoteWithMetadataFn = nil
            self.encodeShieldFn = nil
            self.encodeShieldWithAlgFn = nil
            self.encodeUnshieldFn = nil
            self.encodeUnshieldWithAlgFn = nil
            self.encodeZkTransferFn = nil
            self.encodeZkTransferWithAlgFn = nil
            self.encodeRegisterZkAssetFn = nil
            self.encodeRegisterZkAssetWithAlgFn = nil
            self.encodeMultisigRegisterFn = nil
            self.encodeMultisigRegisterWithAlgFn = nil
            self.encodeClaimIdentifierFn = nil
            self.encodeClaimIdentifierWithAlgFn = nil
            self.encodeBurnFn = nil
            self.encodeBurnWithAlgFn = nil
            self.encodeSetKeyValueFn = nil
            self.encodeSetKeyValueWithAlgFn = nil
            self.encodeRemoveKeyValueFn = nil
            self.encodeRemoveKeyValueWithAlgFn = nil
            self.encodeGovernanceProposeDeployFn = nil
            self.encodeGovernanceProposeDeployWithAlgFn = nil
            self.encodeGovernanceCastPlainBallotFn = nil
            self.encodeGovernanceCastPlainBallotWithAlgFn = nil
            self.encodeGovernanceCastZkBallotFn = nil
            self.encodeGovernanceCastZkBallotWithAlgFn = nil
            self.encodeGovernanceEnactReferendumFn = nil
            self.encodeGovernanceEnactReferendumWithAlgFn = nil
            self.encodeGovernanceFinalizeReferendumFn = nil
            self.encodeGovernanceFinalizeReferendumWithAlgFn = nil
            self.encodeGovernancePersistCouncilFn = nil
            self.encodeGovernancePersistCouncilWithAlgFn = nil
            self.decodeSignedFn = nil
            self.decodeReceiptFn = nil
            self.decodeAssetIdFn = nil
            self.freeFn = nil
            self.setChainDiscriminantFn = nil
            self.setAccelerationConfigFn = nil
            self.getAccelerationConfigFn = nil
            self.getAccelerationStateFn = nil
            self.encodeCiphertextFrameFn = nil
            self.encodeControlOpenFn = nil
            self.encodeControlApproveFn = nil
            self.encodeControlApproveWithAlgFn = nil
            self.encodeControlRejectFn = nil
            self.encodeControlCloseFn = nil
            self.encodeControlPingFn = nil
            self.encodeControlPongFn = nil
            self.sorafsLocalFetchFn = nil
            self.sorafsReferenceValidateOrderbookFn = nil
            self.sorafsReferenceSignOrderbookFn = nil
            self.sorafsReferenceBuildOrderbookOrderRequestFn = nil
            self.sorafsReferenceBuildOrderbookOrderCancelFn = nil
            self.sorafsReferenceBuildOrderbookSettlementReceiptFn = nil
            self.sorafsReferenceValidatePdpPayloadFn = nil
            self.sorafsReferenceValidatePdpCommitmentChallengeFn = nil
            self.sorafsReferenceValidatePdpChallengeProofFn = nil
            self.sorafsReferenceValidatePdpBundleFn = nil
            self.daProofSummaryFn = nil
            self.blake3HashFn = nil
            self.kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn = nil
            self.kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn = nil
            self.kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn = nil
            self.kagemushaVerifyRecursiveCompactPaymentTokenFn = nil
            self.kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn = nil
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn = nil
            self.kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn = nil
            self.kagemushaBuildPallasOpenEnvelopesArchiveFn = nil
            self.kagemushaBuildPreviousProofOpenEnvelopesArchiveFn = nil
            self.kagemushaRecursiveSpendInitFn = nil
            self.kagemushaRecursiveSpendAppendFn = nil
            self.kagemushaRecursiveSpendTransitionProfileInitFn = nil
            self.kagemushaRecursiveSpendTransitionProfileAppendFn = nil
            self.kagemushaRecursiveSpendLineageAppendBoundaryFn = nil
            self.kagemushaRecursiveSpendLineageWitnessFromInitResultFn = nil
            self.kagemushaRecursiveSpendLineageWitnessAppendResultFn = nil
            self.kagemushaRecursiveSpendVerifyFn = nil
            self.kagemushaRecursiveSpendRedeemFn = nil
            self.privacyCapabilitiesFn = nil
            self.privacyProofRequestFn = nil
            self.privacyBuildProofFn = nil
            self.privacyVerifyProofFn = nil
            self.privacyFreeFn = nil
            self.encodeConfidentialPayloadFn = nil
            self.accountAddressParseFn = nil
            self.accountAddressRenderFn = nil
            self.sm2DefaultDistidFn = nil
            self.sm2KeypairFromSeedFn = nil
            self.sm2SignFn = nil
            self.sm2VerifyFn = nil
            self.sm2PublicKeyPrefixedFn = nil
            self.sm2PublicKeyMultihashFn = nil
            self.sm2ComputeZaFn = nil
            self.secp256k1PublicKeyFn = nil
            self.secp256k1SignFn = nil
            self.secp256k1VerifyFn = nil
            self.mldsaParametersFn = nil
            self.mldsaGenerateKeypairFn = nil
            self.mldsaSignFn = nil
            self.mldsaVerifyFn = nil
            self.connectGenerateKeypairFn = nil
            self.connectPublicFromPrivateFn = nil
            self.connectDeriveKeysFn = nil
            self.connectEncryptEnvelopeFn = nil
            self.connectDecryptCiphertextFn = nil
            self.encodeEnvelopeSignRequestTxFn = nil
            self.encodeEnvelopeSignRequestRawFn = nil
            self.encodeEnvelopeSignResultOkFn = nil
            self.encodeEnvelopeSignResultOkWithAlgFn = nil
            self.encodeEnvelopeSignResultErrFn = nil
            self.encodeEnvelopeControlCloseFn = nil
            self.encodeEnvelopeControlRejectFn = nil
            self.decodeEnvelopeKindFn = nil
            self.decodeEnvelopeJSONFn = nil
            self.decodeControlKindFn = nil
            self.decodeCiphertextFrameFn = nil
            self.decodeControlOpenPubFn = nil
            self.decodeControlOpenChainIdFn = nil
            self.decodeControlOpenAppMetadataFn = nil
            self.decodeControlOpenPermissionsFn = nil
            self.decodeControlApprovePubFn = nil
            self.decodeControlApproveAccountFn = nil
            self.decodeControlApprovePermissionsFn = nil
            self.decodeControlApproveProofFn = nil
            self.decodeControlApproveSigFn = nil
            self.decodeControlApproveSigAlgFn = nil
            self.decodeControlCloseFn = nil
            self.decodeControlRejectFn = nil
            self.decodeControlPingFn = nil
            self.decodeControlPongFn = nil
        }

        if self.encodeTransferFn == nil || self.freeFn == nil {
            installStaticallyLinkedBridgeIfAvailable()
        }
        probeKagemushaNativeAvailability()
        probePrivacyNativeAvailability()

        if let setAccelerationConfigFn {
            var defaults = ConnectNoritoAccelerationConfig(
                enable_simd: 1,
                enable_metal: 1,
                enable_cuda: 0,
                max_gpus: 0,
                max_gpus_present: 0,
                merkle_min_leaves_gpu: 0,
                merkle_min_leaves_gpu_present: 0,
                merkle_min_leaves_metal: 0,
                merkle_min_leaves_metal_present: 0,
                merkle_min_leaves_cuda: 0,
                merkle_min_leaves_cuda_present: 0,
                prefer_cpu_sha2_max_leaves_aarch64: 0,
                prefer_cpu_sha2_max_leaves_aarch64_present: 0,
                prefer_cpu_sha2_max_leaves_x86: 0,
                prefer_cpu_sha2_max_leaves_x86_present: 0
            )
            withUnsafePointer(to: &defaults) { ptr in
                setAccelerationConfigFn(UnsafeRawPointer(ptr))
            }
        }
        NSLog("[NoritoNativeBridge] init done — status=%@, handle=%@, free=%@",
              "\(self.bridgeStatus)",
              self.bridgeHandle == nil ? "nil" : "ok",
              self.freeFn == nil ? "nil" : "ok")
        #else
        self.bridgeStatus = .missing(path: "unsupported platform")
        #endif
    }

    #if canImport(Darwin)
    static let expectedKagemushaProbeStatus: Int32 = -311
    static let privacyNativeAvailabilityProbeArchive: Data = {
        var archive = Data(repeating: 0, count: 40)
        archive[0] = 0x4E
        archive[1] = 0x52
        archive[2] = 0x54
        archive[3] = 0x30
        for index in 6..<22 {
            archive[index] = NoritoNativeBridge.privacyRequestSchemaByte
        }
        return archive
    }()

    static func isExpectedKagemushaMalformedProbeResult(
        status: Int32,
        outPtr: UnsafeMutablePointer<UInt8>?,
        outLen: CUnsignedLong
    ) -> Bool {
        status == expectedKagemushaProbeStatus && outPtr == nil && outLen == 0
    }

    static func isValidPrivacyNativeProbeResult(
        status: Int32,
        outPtr: UnsafeMutablePointer<UInt8>?,
        outLen: CUnsignedLong,
        expectedSchemaByte: UInt8
    ) -> Bool {
        guard status == 0,
              let outPtr,
              outLen > 0,
              outLen <= CUnsignedLong(Self.privacyNativeArchiveMaxBytes) else {
            return false
        }
        var archive = Data(bytes: outPtr, count: Int(outLen))
        defer {
            archive.resetBytes(in: 0..<archive.count)
        }
        return Self.isValidPrivacyNoritoArchive(archive)
            && Self.hasNonEmptyPrivacyNoritoPayload(archive)
            && Self.hasPrivacyNoritoSchema(archive, expectedSchemaByte: expectedSchemaByte)
    }

    private func probeKagemushaNativeAvailability() {
        kagemushaCompactPaymentTokenNativeProbeOk =
            probeKagemushaArchiveFunction(kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn)
        kagemushaRecursiveAggregationNativeProbeOk =
            probeKagemushaRecursiveAggregationFunction(
                kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn
            )
        kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk =
            probeKagemushaRecursiveCompactPaymentTokenVerifierFunction(
                kagemushaVerifyRecursiveCompactPaymentTokenFn
            )
        kagemushaRecursiveCompactPaymentTokenNativeProbeOk =
            probeKagemushaRecursiveCompactPaymentTokenFunction(
                kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn
            )
            && kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk
        kagemushaRecursiveSpendCompactProjectionNativeProbeOk =
            probeKagemushaArchiveFunction(kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn)
        kagemushaRecursiveSpendCompactProjectionVerifierNativeProbeOk =
            probeKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierFunction(
                kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn,
                kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn
            )

        var recursiveSpendOk = true
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaBuildPallasOpenEnvelopesArchiveFn) && recursiveSpendOk
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaBuildPreviousProofOpenEnvelopesArchiveFn) && recursiveSpendOk
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaRecursiveSpendInitFn) && recursiveSpendOk
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaRecursiveSpendAppendFn) && recursiveSpendOk
        recursiveSpendOk =
            probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileInitFn)
            && recursiveSpendOk
        recursiveSpendOk =
            probeKagemushaArchiveFunction(kagemushaRecursiveSpendTransitionProfileAppendFn)
            && recursiveSpendOk
        recursiveSpendOk =
            probeKagemushaArchiveFunction(kagemushaRecursiveSpendLineageAppendBoundaryFn)
            && recursiveSpendOk
        recursiveSpendOk =
            probeKagemushaLineageWitnessFromInitResultFunction(
                kagemushaRecursiveSpendLineageWitnessFromInitResultFn
            ) && recursiveSpendOk
        recursiveSpendOk =
            probeKagemushaLineageWitnessAppendResultFunction(
                kagemushaRecursiveSpendLineageWitnessAppendResultFn
            ) && recursiveSpendOk
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaRecursiveSpendVerifyFn) && recursiveSpendOk
        recursiveSpendOk = probeKagemushaArchiveFunction(kagemushaRecursiveSpendRedeemFn) && recursiveSpendOk
        kagemushaRecursiveSpendNativeProbeOk = recursiveSpendOk
    }

    private func probeKagemushaArchiveFunction(_ function: KagemushaRecursiveSpendArchiveFn?) -> Bool {
        guard let function, freeFn != nil else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { buffer -> Int32 in
            let baseAddress = buffer.bindMemory(to: UInt8.self).baseAddress
            return function(
                baseAddress,
                CUnsignedLong(buffer.count),
                &outPtr,
                &outLen
            )
        }
        return consumeKagemushaProbeResult(status: status, outPtr: outPtr, outLen: outLen)
    }

    private func probeKagemushaRecursiveAggregationFunction(
        _ function: KagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn?
    ) -> Bool {
        guard let function, freeFn != nil else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { recordBuffer -> Int32 in
            let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { envelopeBuffer -> Int32 in
                let envelopeBaseAddress = envelopeBuffer.bindMemory(to: UInt8.self).baseAddress
                return function(
                    recordBaseAddress,
                    CUnsignedLong(recordBuffer.count),
                    envelopeBaseAddress,
                    CUnsignedLong(envelopeBuffer.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        return consumeKagemushaProbeResult(status: status, outPtr: outPtr, outLen: outLen)
    }

    private func probeKagemushaRecursiveCompactPaymentTokenFunction(
        _ function: KagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn?
    ) -> Bool {
        guard let function, freeFn != nil else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { recordBuffer -> Int32 in
            let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { envelopeBuffer -> Int32 in
                let envelopeBaseAddress = envelopeBuffer.bindMemory(to: UInt8.self).baseAddress
                return probeArchive.withUnsafeBytes { keyBuffer -> Int32 in
                    let keyBaseAddress = keyBuffer.bindMemory(to: UInt8.self).baseAddress
                    return function(
                        recordBaseAddress,
                        CUnsignedLong(recordBuffer.count),
                        envelopeBaseAddress,
                        CUnsignedLong(envelopeBuffer.count),
                        keyBaseAddress,
                        CUnsignedLong(keyBuffer.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        return consumeKagemushaProbeResult(status: status, outPtr: outPtr, outLen: outLen)
    }

    private func probeKagemushaRecursiveCompactPaymentTokenVerifierFunction(
        _ function: KagemushaVerifyRecursiveCompactPaymentTokenFn?
    ) -> Bool {
        guard let function else {
            return false
        }
        var valid: UInt8 = 0xFF
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { tokenBuffer -> Int32 in
            let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { keyBuffer -> Int32 in
                let keyBaseAddress = keyBuffer.bindMemory(to: UInt8.self).baseAddress
                return function(
                    tokenBaseAddress,
                    CUnsignedLong(tokenBuffer.count),
                    keyBaseAddress,
                    CUnsignedLong(keyBuffer.count),
                    &valid
                )
            }
        }
        return status == Self.expectedKagemushaProbeStatus && valid == 0
    }

    private func probeKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierFunction(
        _ function: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn?,
        _ atHeightFunction: KagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn?
    ) -> Bool {
        guard let function, let atHeightFunction else {
            return false
        }
        let probeArchive = Data([0x00])

        var noHeightValid: UInt8 = 0xFF
        let noHeightStatus = probeArchive.withUnsafeBytes { tokenBuffer -> Int32 in
            let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { recordBuffer -> Int32 in
                let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
                return function(
                    tokenBaseAddress,
                    CUnsignedLong(tokenBuffer.count),
                    recordBaseAddress,
                    CUnsignedLong(recordBuffer.count),
                    &noHeightValid
                )
            }
        }

        var heightValid: UInt8 = 0xFF
        let heightStatus = probeArchive.withUnsafeBytes { tokenBuffer -> Int32 in
            let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { recordBuffer -> Int32 in
                let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
                return atHeightFunction(
                    tokenBaseAddress,
                    CUnsignedLong(tokenBuffer.count),
                    recordBaseAddress,
                    CUnsignedLong(recordBuffer.count),
                    0,
                    &heightValid
                )
            }
        }

        return noHeightStatus == Self.expectedKagemushaProbeStatus
            && noHeightValid == 0
            && heightStatus == Self.expectedKagemushaProbeStatus
            && heightValid == 0
    }

    private func probeKagemushaLineageWitnessFromInitResultFunction(
        _ function: KagemushaRecursiveSpendLineageWitnessFromInitResultFn?
    ) -> Bool {
        guard let function, freeFn != nil else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { requestBuffer -> Int32 in
            let requestBaseAddress = requestBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { bundleBuffer -> Int32 in
                let bundleBaseAddress = bundleBuffer.bindMemory(to: UInt8.self).baseAddress
                return function(
                    requestBaseAddress,
                    CUnsignedLong(requestBuffer.count),
                    bundleBaseAddress,
                    CUnsignedLong(bundleBuffer.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        return consumeKagemushaProbeResult(status: status, outPtr: outPtr, outLen: outLen)
    }

    private func probeKagemushaLineageWitnessAppendResultFunction(
        _ function: KagemushaRecursiveSpendLineageWitnessAppendResultFn?
    ) -> Bool {
        guard let function, freeFn != nil else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Data([0x00])
        let status = probeArchive.withUnsafeBytes { witnessBuffer -> Int32 in
            let witnessBaseAddress = witnessBuffer.bindMemory(to: UInt8.self).baseAddress
            return probeArchive.withUnsafeBytes { requestBuffer -> Int32 in
                let requestBaseAddress = requestBuffer.bindMemory(to: UInt8.self).baseAddress
                return probeArchive.withUnsafeBytes { bundleBuffer -> Int32 in
                    let bundleBaseAddress = bundleBuffer.bindMemory(to: UInt8.self).baseAddress
                    return function(
                        witnessBaseAddress,
                        CUnsignedLong(witnessBuffer.count),
                        requestBaseAddress,
                        CUnsignedLong(requestBuffer.count),
                        bundleBaseAddress,
                        CUnsignedLong(bundleBuffer.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        return consumeKagemushaProbeResult(status: status, outPtr: outPtr, outLen: outLen)
    }

    private func consumeKagemushaProbeResult(
        status: Int32,
        outPtr: UnsafeMutablePointer<UInt8>?,
        outLen: CUnsignedLong
    ) -> Bool {
        let expected = Self.isExpectedKagemushaMalformedProbeResult(
            status: status,
            outPtr: outPtr,
            outLen: outLen
        )
        if let outPtr {
            freeFn?(outPtr)
        }
        return expected
    }

    private func probePrivacyNativeAvailability() {
        var available = true
        available = probePrivacyCapabilitiesFunction(privacyCapabilitiesFn) && available
        available = probePrivacyProofRequestFunction(
            privacyProofRequestFn,
            expectedSchemaByte: Self.privacyRequestSchemaByte
        ) && available
        available = probePrivacyProofFunction(
            privacyBuildProofFn,
            expectedSchemaByte: Self.privacyBuildProofResultSchemaByte
        ) && available
        available = probePrivacyProofFunction(
            privacyVerifyProofFn,
            expectedSchemaByte: Self.privacyVerifyProofResultSchemaByte
        ) && available
        privacyNativeProbeOk = available
    }

    private func probePrivacyCapabilitiesFunction(_ function: PrivacyCapabilitiesFn?) -> Bool {
        guard let function, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = function(&outPtr, &outLen)
        return consumePrivacyNativeProbeResult(
            status: status,
            outPtr: outPtr,
            outLen: outLen,
            expectedSchemaByte: Self.privacyCapabilitiesResultSchemaByte,
            free: freePrivacyFn
        )
    }

    private func probePrivacyProofRequestFunction(
        _ function: PrivacyProofRequestFn?,
        expectedSchemaByte: UInt8
    ) -> Bool {
        guard let function, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        var algorithmId = Array("zk-ace-pq-authorization-v0".utf8)
        var entrypoint = Array("buildZkAceAuthorizationProofV1".utf8)
        var vkRef = Array("stark-fri:zk_ace_pq_authorization_v0".utf8)
        var publicInputs = Array("public-inputs".utf8)
        defer {
            Self.clearTemporaryPrivacyRequestArchive(&algorithmId)
            Self.clearTemporaryPrivacyRequestArchive(&entrypoint)
            Self.clearTemporaryPrivacyRequestArchive(&vkRef)
            Self.clearTemporaryPrivacyRequestArchive(&publicInputs)
        }
        let status = algorithmId.withUnsafeBufferPointer { algorithmBuffer in
            entrypoint.withUnsafeBufferPointer { entrypointBuffer in
                vkRef.withUnsafeBufferPointer { vkRefBuffer in
                    publicInputs.withUnsafeBufferPointer { publicInputsBuffer in
                        function(
                            algorithmBuffer.baseAddress,
                            CUnsignedLong(algorithmBuffer.count),
                            entrypointBuffer.baseAddress,
                            CUnsignedLong(entrypointBuffer.count),
                            vkRefBuffer.baseAddress,
                            CUnsignedLong(vkRefBuffer.count),
                            publicInputsBuffer.baseAddress,
                            CUnsignedLong(publicInputsBuffer.count),
                            nil,
                            0,
                            nil,
                            0,
                            &outPtr,
                            &outLen
                        )
                    }
                }
            }
        }
        return consumePrivacyNativeProbeResult(
            status: status,
            outPtr: outPtr,
            outLen: outLen,
            expectedSchemaByte: expectedSchemaByte,
            free: freePrivacyFn
        )
    }

    private func probePrivacyProofFunction(
        _ function: PrivacyProofArchiveFn?,
        expectedSchemaByte: UInt8
    ) -> Bool {
        guard let function, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return false
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let probeArchive = Self.privacyNativeAvailabilityProbeArchive
        let status = probeArchive.withUnsafeBytes { buffer -> Int32 in
            guard let baseAddress = buffer.bindMemory(to: UInt8.self).baseAddress else {
                return -1
            }
            return function(
                baseAddress,
                CUnsignedLong(buffer.count),
                &outPtr,
                &outLen
            )
        }
        return consumePrivacyNativeProbeResult(
            status: status,
            outPtr: outPtr,
            outLen: outLen,
            expectedSchemaByte: expectedSchemaByte,
            free: freePrivacyFn
        )
    }

    private func consumePrivacyNativeProbeResult(
        status: Int32,
        outPtr: UnsafeMutablePointer<UInt8>?,
        outLen: CUnsignedLong,
        expectedSchemaByte: UInt8,
        free: FreeFn
    ) -> Bool {
        let expected = Self.isValidPrivacyNativeProbeResult(
            status: status,
            outPtr: outPtr,
            outLen: outLen,
            expectedSchemaByte: expectedSchemaByte
        )
        if let outPtr {
            Self.clearPrivacyNativeBuffer(outPtr, length: outLen)
            free(outPtr)
        }
        return expected
    }
    #endif

    public var isAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return encodeTransferFn != nil && freeFn != nil
        #else
        return false
        #endif
    }

    public var isSorafsReferenceValidationAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return sorafsReferenceValidateOrderbookFn != nil
            && sorafsReferenceValidatePdpPayloadFn != nil
            && sorafsReferenceValidatePdpCommitmentChallengeFn != nil
            && sorafsReferenceValidatePdpChallengeProofFn != nil
            && sorafsReferenceValidatePdpBundleFn != nil
            && freeFn != nil
        #else
        return false
        #endif
    }

    public var isSorafsReferenceOrderbookSigningAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return sorafsReferenceSignOrderbookFn != nil && freeFn != nil
        #else
        return false
        #endif
    }

    public var isSorafsReferenceOrderbookFieldBuilderAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return sorafsReferenceBuildOrderbookOrderRequestFn != nil
            && sorafsReferenceBuildOrderbookOrderCancelFn != nil
            && sorafsReferenceBuildOrderbookSettlementReceiptFn != nil
            && freeFn != nil
        #else
        return false
        #endif
    }

    public var isKagemushaCompactPaymentTokenProverAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn != nil
            && freeFn != nil
            && kagemushaCompactPaymentTokenNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveAggregationProofBundleProverAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn != nil
            && freeFn != nil
            && kagemushaRecursiveAggregationNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveCompactPaymentTokenProverAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn != nil
            && kagemushaVerifyRecursiveCompactPaymentTokenFn != nil
            && freeFn != nil
            && kagemushaRecursiveCompactPaymentTokenNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveCompactPaymentTokenVerifierAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaVerifyRecursiveCompactPaymentTokenFn != nil
            && kagemushaRecursiveCompactPaymentTokenVerifierNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveSpendCompactPaymentTokenProjectionAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn != nil
            && freeFn != nil
            && kagemushaRecursiveSpendCompactProjectionNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn != nil
            && kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn != nil
            && kagemushaRecursiveSpendCompactProjectionVerifierNativeProbeOk
        #else
        return false
        #endif
    }

    public var isKagemushaRecursiveSpendAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return kagemushaRecursiveSpendInitFn != nil
            && kagemushaBuildPallasOpenEnvelopesArchiveFn != nil
            && kagemushaBuildPreviousProofOpenEnvelopesArchiveFn != nil
            && kagemushaRecursiveSpendAppendFn != nil
            && kagemushaRecursiveSpendTransitionProfileInitFn != nil
            && kagemushaRecursiveSpendTransitionProfileAppendFn != nil
            && kagemushaRecursiveSpendLineageAppendBoundaryFn != nil
            && kagemushaRecursiveSpendLineageWitnessFromInitResultFn != nil
            && kagemushaRecursiveSpendLineageWitnessAppendResultFn != nil
            && kagemushaRecursiveSpendVerifyFn != nil
            && kagemushaRecursiveSpendRedeemFn != nil
            && freeFn != nil
            && kagemushaRecursiveSpendNativeProbeOk
        #else
        return false
        #endif
    }

    public var isPrivacyNativeAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return privacyCapabilitiesFn != nil
            && privacyProofRequestFn != nil
            && privacyBuildProofFn != nil
            && privacyVerifyProofFn != nil
            && (privacyFreeFn != nil || freeFn != nil)
            && privacyNativeProbeOk
        #else
        return false
        #endif
    }

    public var bridgeStatusDescription: String {
        #if canImport(Darwin)
        return "\(bridgeStatus)"
        #else
        return "non-Darwin"
        #endif
    }

    public var bridgeLoadIssue: String? {
        #if canImport(Darwin)
        switch bridgeStatus {
        case .valid:
            return nil
        case .pathDenied(let path):
            return BridgePolicyHint.unavailableMessage("NoritoBridge load denied for path \(path).")
        case .missing(let path):
            return BridgePolicyHint.unavailableMessage("NoritoBridge missing at \(path).")
        case .hashMismatch(let path, let expected, let actual):
            return BridgePolicyHint.unavailableMessage(
                "NoritoBridge hash mismatch for \(path) (expected \(expected), actual \(actual ?? "nil"))."
            )
        case .versionMismatch(let path, let expected, let actual):
            return BridgePolicyHint.unavailableMessage(
                "NoritoBridge version mismatch for \(path) (expected \(expected), actual \(actual ?? "nil"))."
            )
        case .abiMismatch(let path, let expected, let actual):
            return BridgePolicyHint.unavailableMessage(
                "NoritoBridge ABI mismatch for \(path) (expected \(expected), actual \(actual.map(String.init) ?? "nil"))."
            )
        }
        #else
        return BridgePolicyHint.unavailableMessage("NoritoBridge unsupported on this platform.")
        #endif
    }

    var isConnectCryptoAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return canUseConnectCrypto
        #else
        return false
        #endif
    }

    var isSm2Available: Bool {
        #if canImport(Darwin)
        return sm2DefaultDistidFn != nil
            && sm2KeypairFromSeedFn != nil
            && sm2SignFn != nil
            && sm2VerifyFn != nil
            && sm2PublicKeyPrefixedFn != nil
            && sm2PublicKeyMultihashFn != nil
            && sm2ComputeZaFn != nil
            && freeFn != nil
        #else
        return false
        #endif
    }

    public func supportsTransactions(using algorithm: SigningAlgorithm) -> Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }

        let hasAlgorithmEncoders =
            self.encodeTransferWithAlgFn != nil
                && self.encodeMintWithAlgFn != nil
                && self.encodeBurnWithAlgFn != nil
                && self.encodeShieldWithAlgFn != nil
                && self.encodeUnshieldWithAlgFn != nil
                && self.encodeZkTransferWithAlgFn != nil
                && self.encodeRegisterZkAssetWithAlgFn != nil

        switch algorithm {
        case .ed25519:
            return encodeTransferFn != nil
                && encodeMintFn != nil
                && encodeBurnFn != nil
                && encodeShieldFn != nil
                && encodeUnshieldFn != nil
                && encodeZkTransferFn != nil
                && encodeRegisterZkAssetFn != nil
        case .sm2:
            return isSm2Available && hasAlgorithmEncoders && canParseSm2TransactionAuthority()
        case .secp256k1:
            return secp256k1Supported && hasAlgorithmEncoders
        case .mlDsa:
            return mldsaSupported && hasAlgorithmEncoders
        case .blsNormal, .blsSmall,
             .gost2012_256A, .gost2012_256B, .gost2012_256C,
             .gost2012_512A, .gost2012_512B:
            return publicKeyFromPrivateFn != nil
                && signDetachedFn != nil
                && verifyDetachedFn != nil
                && hasAlgorithmEncoders
        }
        #else
        return false
        #endif
    }

    private func canParseSm2TransactionAuthority() -> Bool {
        #if canImport(Darwin) && IROHASWIFT_ENABLE_SM
        guard isAccountAddressCodecAvailable else { return false }
        let distid = Sm2Keypair.defaultDistid()
        let seed = Data(repeating: 0xA5, count: Sm2Keypair.privateKeyLength)
        guard let pair = sm2KeypairFromSeed(distid: distid, seed: seed),
              let authority = try? AccountId.makeI105(
                publicKey: pair.publicKey,
                algorithm: "sm2",
                distid: distid
              ) else {
            return false
        }
        return (try? parseAccountAddress(
            literal: authority,
            expectedPrefix: AccountId.defaultNetworkPrefix
        )) != nil
        #else
        return false
        #endif
    }

    var isAccountAddressCodecAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        return accountAddressParseFn != nil
            && accountAddressRenderFn != nil
            && freeFn != nil
        #else
        return false
        #endif
    }

    var isConnectCodecAvailable: Bool {
        #if canImport(Darwin)
        guard bridgeEnabledForRuntime else { return false }
        if connectCodecAvailabilityOverride == false {
            return false
        }
        return canUseConnectCodec
        #else
        return false
        #endif
    }

    func overrideConnectCodecAvailabilityForTests(_ override: Bool?) {
        #if canImport(Darwin)
        connectCodecAvailabilityOverride = override
        #endif
    }

    func overrideBridgeAvailabilityForTests(_ override: Bool?) {
        #if canImport(Darwin)
        bridgeAvailabilityOverride = override
        #endif
    }

    #if canImport(Darwin)
    private static var shouldDisableBridgeForHostedXCTestApp: Bool {
        if ProcessInfo.processInfo.environment["IROHA_SWIFT_ENABLE_BRIDGE_IN_HOSTED_XCTEST"] == "1" {
            return false
        }
        guard ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil else {
            return false
        }
        return (Bundle.main.object(forInfoDictionaryKey: "CFBundlePackageType") as? String) == "APPL"
    }

    private var bridgeEnabledForRuntime: Bool {
        if let override = bridgeAvailabilityOverride {
            return override
        }
        return true
    }

    private var bridgeAvailabilityOverride: Bool?
    private var connectCodecAvailabilityOverride: Bool?
    #endif

    private var canUseConnectCodec: Bool {
        #if canImport(Darwin)
        let hasEncode =
            encodeCiphertextFrameFn != nil
            && encodeControlOpenFn != nil
            && (encodeControlApproveFn != nil || encodeControlApproveWithAlgFn != nil)
            && encodeControlRejectFn != nil
            && encodeControlCloseFn != nil
            && encodeControlPingFn != nil
            && encodeControlPongFn != nil
        let hasDecode =
            decodeControlKindFn != nil
            && decodeCiphertextFrameFn != nil
            && decodeControlOpenPubFn != nil
            && decodeControlOpenChainIdFn != nil
            && decodeControlOpenAppMetadataFn != nil
            && decodeControlOpenPermissionsFn != nil
            && decodeControlApprovePubFn != nil
            && decodeControlApproveAccountFn != nil
            && decodeControlApprovePermissionsFn != nil
            && decodeControlApproveProofFn != nil
            && decodeControlApproveSigFn != nil
            && decodeControlApproveSigAlgFn != nil
            && decodeControlCloseFn != nil
            && decodeControlRejectFn != nil
            && decodeControlPingFn != nil
            && decodeControlPongFn != nil
        return hasEncode && hasDecode && freeFn != nil
        #else
        return false
        #endif
    }

    #if canImport(Darwin)
    private func withOptionalBytes<R>(_ data: Data?, _ body: (UnsafePointer<UInt8>?, UInt) -> R) -> R {
        if let data {
            return data.withUnsafeBytes { buffer in
                let base = buffer.bindMemory(to: UInt8.self).baseAddress
                return body(base, UInt(data.count))
            }
        } else {
            return body(nil, 0)
        }
    }

    private func withOptionalCStringData<R>(_ data: Data?, _ body: (UnsafePointer<CChar>?, UInt) -> R) -> R {
        if let data {
            return data.withUnsafeBytes { buffer in
                let base = buffer.bindMemory(to: CChar.self).baseAddress
                return body(base, UInt(data.count))
            }
        } else {
            return body(nil, 0)
        }
    }

    private func withOptionalCString<R>(_ string: String?, _ body: (UnsafePointer<CChar>?, UInt) -> R) -> R {
        if let string {
            return string.withCString { cString in
                body(cString, UInt(string.utf8.count))
            }
        } else {
            return body(nil, 0)
        }
    }

    private func withDataPointer<R>(_ data: Data, _ body: (UnsafePointer<UInt8>?, CUnsignedLong) -> R) -> R {
        data.withUnsafeBytes { buffer in
            body(buffer.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(buffer.count))
        }
    }

    private func takeData(pointer: UnsafeMutablePointer<UInt8>?, length: UInt) -> Data? {
        guard let pointer else { return nil }
        let data = Data(bytes: pointer, count: Int(length))
        if let freeFn {
            freeFn(pointer)
        } else {
            Darwin.free(pointer)
        }
        return data
    }

    private func takeString(pointer: UnsafeMutablePointer<UInt8>?, length: UInt) -> String? {
        guard let data = takeData(pointer: pointer, length: length) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private func takeCString(pointer: UnsafeMutablePointer<CChar>?, length: UInt) -> String? {
        guard let pointer else { return nil }
        let data = Data(bytes: pointer, count: Int(length))
        Darwin.free(pointer)
        return String(data: data, encoding: .utf8)
    }

    private func consumeAccountAddressError(pointer: UnsafeMutablePointer<UInt8>?, length: UInt) -> AccountAddressError? {
        guard let data = takeData(pointer: pointer, length: length) else { return nil }
        guard let payload = AccountAddressError.bridgePayload(from: data) else { return nil }
        return AccountAddressError.fromBridgePayload(payload)
    }
    #endif

    private func inferredAccountAddressDiscriminant(_ literal: String) -> UInt16? {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("sora") {
            return 753
        }
        if trimmed.hasPrefix("test") {
            return 0x0171
        }
        if trimmed.hasPrefix("dev") {
            return 0
        }
        guard trimmed.hasPrefix("n") else {
            return nil
        }
        let digits = trimmed.dropFirst().prefix { $0.isASCII && $0.isNumber }
        guard !digits.isEmpty else {
            return nil
        }
        return UInt16(String(digits))
    }

    func parseAccountAddress(
        literal: String,
        expectedPrefix: UInt16?
    ) throws -> NativeAccountAddressParseResult? {
        #if canImport(Darwin)
        guard isAccountAddressCodecAvailable,
              let accountAddressParseFn else {
            return nil
        }

        var canonicalPtr: UnsafeMutablePointer<UInt8>? = nil
        var canonicalLen: UInt = 0
        var networkPrefix: UInt16 = 0
        var errorPtr: UnsafeMutablePointer<UInt8>? = nil
        var errorLen: UInt = 0
        let prefixFlag: UInt8 = expectedPrefix == nil ? 0 : 1
        let prefixValue = expectedPrefix ?? 0

        let status = literal.withCString { cString in
            accountAddressParseFn(
                cString,
                UInt(literal.utf8.count),
                prefixValue,
                prefixFlag,
                &canonicalPtr,
                &canonicalLen,
                &networkPrefix,
                &errorPtr,
                &errorLen
            )
        }

        if status == 0 {
            guard let canonicalPtr,
                  let canonical = takeData(pointer: canonicalPtr, length: canonicalLen)
            else {
                return nil
            }
            return NativeAccountAddressParseResult(
                canonicalBytes: canonical,
                networkPrefix: networkPrefix
            )
        }

        if let error = consumeAccountAddressError(pointer: errorPtr, length: errorLen) {
            throw error
        }
        if let canonicalPtr {
            freeFn?(canonicalPtr)
        }
        return nil
        #else
        return nil
        #endif
    }

    func renderAccountAddress(
        canonicalBytes: Data,
        networkPrefix: UInt16
    ) throws -> NativeAccountAddressRenderResult? {
        #if canImport(Darwin)
        guard isAccountAddressCodecAvailable,
              let accountAddressRenderFn else {
            return nil
        }

        var hexPtr: UnsafeMutablePointer<UInt8>? = nil
        var hexLen: UInt = 0
        var i105Ptr: UnsafeMutablePointer<UInt8>? = nil
        var i105Len: UInt = 0
        var errorPtr: UnsafeMutablePointer<UInt8>? = nil
        var errorLen: UInt = 0

        let status = canonicalBytes.withUnsafeBytes { buffer in
            accountAddressRenderFn(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                UInt(canonicalBytes.count),
                networkPrefix,
                &hexPtr,
                &hexLen,
                &i105Ptr,
                &i105Len,
                &errorPtr,
                &errorLen
            )
        }

        if status == 0 {
            guard
                let canonicalHex = takeString(pointer: hexPtr, length: hexLen),
                let i105 = takeString(pointer: i105Ptr, length: i105Len)
            else {
                return nil
            }
            return NativeAccountAddressRenderResult(
                canonicalHex: canonicalHex,
                i105: i105
            )
        }

        if let error = consumeAccountAddressError(pointer: errorPtr, length: errorLen) {
            if let hexPtr { freeFn?(hexPtr) }
            if let i105Ptr { freeFn?(i105Ptr) }
            throw error
        }

        if let hexPtr { freeFn?(hexPtr) }
        if let i105Ptr { freeFn?(i105Ptr) }
        return nil
        #else
        return nil
        #endif
    }

    func blake3Hash(data: Data) -> Data? {
        #if canImport(Darwin)
        guard let blake3HashFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            let baseAddress = buffer.bindMemory(to: UInt8.self).baseAddress
            return blake3HashFn(baseAddress, CUnsignedLong(buffer.count), &outPtr, &outLen)
        }
        guard status == 0 else {
            if let outPtr {
                freeFn?(outPtr)
            }
            return nil
        }
        return takeData(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    private enum OfflineNoteTxKind {
        case issue
        case redeem
        case audit
    }

    private func encodeOfflineNoteTransaction(
        kind: OfflineNoteTxKind,
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32?,
        modelBytes: Data,
        privateKey: Data,
        metadataJSON: Data? = nil
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let metadataBytes = metadataJSON ?? Data()
        let encodeFn: EncodeOfflineNoteTxFn?
        let encodeWithMetadataFn: EncodeOfflineNoteTxWithMetadataFn?
        switch kind {
        case .issue:
            encodeFn = encodeIssueOfflineNoteFn
            encodeWithMetadataFn = encodeIssueOfflineNoteWithMetadataFn
        case .redeem:
            encodeFn = encodeRedeemOfflineNoteFn
            encodeWithMetadataFn = encodeRedeemOfflineNoteWithMetadataFn
        case .audit:
            encodeFn = encodeAuditOfflineNoteFn
            encodeWithMetadataFn = encodeAuditOfflineNoteWithMetadataFn
        }
        if metadataBytes.isEmpty {
            guard encodeFn != nil else { return nil }
        } else {
            guard encodeWithMetadataFn != nil else { return nil }
        }

        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let nonceValue = nonce ?? 0
        let nonceFlag: UInt8 = nonce == nil ? 0 : 1

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)

        let status: Int32
        if metadataBytes.isEmpty {
            guard let encodeFn else { return nil }
            status = try withAuthorityChainDiscriminant(authority: authority) {
                chainId.withCString { chainPtr in
                authority.withCString { authorityPtr in
                    modelBytes.withUnsafeBytes { modelBuffer -> Int32 in
                        privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                            hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                guard let hashPtr = hashBuffer.baseAddress else { return -1 }
                                return self.withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                    encodeFn(
                                        chainPtr, UInt(chainId.utf8.count),
                                        authorityPtr, UInt(authority.utf8.count),
                                        creationTimeMs,
                                        ttlValue,
                                        ttlFlag,
                                        nonceValue,
                                        nonceFlag,
                                        modelBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(modelBytes.count),
                                        keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                        signedPtrPtr,
                                        signedLenPtr,
                                        hashPtr,
                                        hashLength
                                    )
                                }
                            }
                        }
                    }
                }
                }
            }
        } else {
            guard let encodeWithMetadataFn else { return nil }
            status = try withAuthorityChainDiscriminant(authority: authority) {
                chainId.withCString { chainPtr in
                authority.withCString { authorityPtr in
                    metadataBytes.withUnsafeBytes { metadataBuffer -> Int32 in
                        modelBytes.withUnsafeBytes { modelBuffer -> Int32 in
                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else { return -1 }
                                    return self.withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        encodeWithMetadataFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            nonceValue,
                                            nonceFlag,
                                            metadataBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(metadataBytes.count),
                                            modelBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(modelBytes.count),
                                            keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
                }
            }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeIssueOfflineNote(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        issueModel: Data,
        privateKey: Data,
        metadataJSON: Data? = nil
    ) throws -> NativeSignedTransaction? {
        try encodeOfflineNoteTransaction(
            kind: .issue,
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            modelBytes: issueModel,
            privateKey: privateKey,
            metadataJSON: metadataJSON
        )
    }

    func encodeRedeemOfflineNote(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        redemptionModel: Data,
        privateKey: Data,
        metadataJSON: Data? = nil
    ) throws -> NativeSignedTransaction? {
        try encodeOfflineNoteTransaction(
            kind: .redeem,
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            modelBytes: redemptionModel,
            privateKey: privateKey,
            metadataJSON: metadataJSON
        )
    }

    func encodeAuditOfflineNote(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        auditModel: Data,
        privateKey: Data,
        metadataJSON: Data? = nil
    ) throws -> NativeSignedTransaction? {
        try encodeOfflineNoteTransaction(
            kind: .audit,
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            modelBytes: auditModel,
            privateKey: privateKey,
            metadataJSON: metadataJSON
        )
    }

    func encodeDefundOfflineNote(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        bearerAuditTrail: [Data],
        redemptionModel: Data,
        privateKey: Data,
        metadataJSON: Data? = nil
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let metadataBytes = metadataJSON ?? Data()
        if metadataBytes.isEmpty {
            guard encodeDefundOfflineNoteFn != nil else { return nil }
        } else {
            guard encodeDefundOfflineNoteWithMetadataFn != nil else { return nil }
        }

        var trail = Data()
        for audit in bearerAuditTrail {
            var len = UInt64(audit.count).littleEndian
            withUnsafeBytes(of: &len) { trail.append(contentsOf: $0) }
            trail.append(audit)
        }
        let auditCount = UInt32(bearerAuditTrail.count)

        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let nonceValue = nonce ?? 0
        let nonceFlag: UInt8 = nonce == nil ? 0 : 1

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)

        let status: Int32
        if metadataBytes.isEmpty {
            guard let encodeFn = encodeDefundOfflineNoteFn else { return nil }
            status = try withAuthorityChainDiscriminant(authority: authority) {
                chainId.withCString { chainPtr in
                authority.withCString { authorityPtr in
                    trail.withUnsafeBytes { trailBuffer -> Int32 in
                        redemptionModel.withUnsafeBytes { redeemBuffer -> Int32 in
                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else { return -1 }
                                    return self.withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        encodeFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            nonceValue,
                                            nonceFlag,
                                            trailBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(trail.count), auditCount,
                                            redeemBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(redemptionModel.count),
                                            keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
                }
            }
        } else {
            guard let encodeFn = encodeDefundOfflineNoteWithMetadataFn else { return nil }
            status = try withAuthorityChainDiscriminant(authority: authority) {
                chainId.withCString { chainPtr in
                authority.withCString { authorityPtr in
                    metadataBytes.withUnsafeBytes { metadataBuffer -> Int32 in
                        trail.withUnsafeBytes { trailBuffer -> Int32 in
                            redemptionModel.withUnsafeBytes { redeemBuffer -> Int32 in
                                privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                    hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                        guard let hashPtr = hashBuffer.baseAddress else { return -1 }
                                        return self.withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                            encodeFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                nonceValue,
                                                nonceFlag,
                                                metadataBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(metadataBytes.count),
                                                trailBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(trail.count), auditCount,
                                                redeemBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(redemptionModel.count),
                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                }
            }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeTransfer(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        assetDefinitionId: String,
        quantity: String,
        destination: String,
        feeSponsor: String? = nil,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let nonceValue = nonce ?? 0
        let nonceFlag: UInt8 = nonce == nil ? 0 : 1
        let normalizedFeeSponsor: String?
        if let rawFeeSponsor = feeSponsor?.trimmingCharacters(in: .whitespacesAndNewlines),
           !rawFeeSponsor.isEmpty
        {
            normalizedFeeSponsor = rawFeeSponsor
        } else {
            normalizedFeeSponsor = nil
        }
        let requiresFeeSponsorBridge = normalizedFeeSponsor != nil
        let useAlg = algorithm != .ed25519
        if useAlg {
            guard requiresFeeSponsorBridge
                ? encodeTransferWithFeeSponsorWithAlgFn != nil
                : encodeTransferWithAlgFn != nil
            else {
                return nil
            }
        } else {
            guard requiresFeeSponsorBridge
                ? encodeTransferWithFeeSponsorFn != nil
                : encodeTransferFn != nil
            else {
                return nil
            }
        }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    quantity.withCString { quantityPtr in
                        destination.withCString { destinationPtr in
                            let encodeTransferCall: (UnsafePointer<CChar>?, UInt) -> Int32 = { [self] feeSponsorPtr, feeSponsorLen in
                                privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                    hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                        guard let hashPtr = hashBuffer.baseAddress else {
                                            return -1
                                        }
                                        return self.withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                            if useAlg,
                                               let feeSponsorPtr,
                                               let encodeTransferWithFeeSponsorWithAlgFn = self.encodeTransferWithFeeSponsorWithAlgFn
                                            {
                                                return encodeTransferWithFeeSponsorWithAlgFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    nonceValue,
                                                    nonceFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    quantityPtr, UInt(quantity.utf8.count),
                                                    destinationPtr, UInt(destination.utf8.count),
                                                    feeSponsorPtr, feeSponsorLen,
                                                    keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                    algorithmRaw,
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else if useAlg, let encodeTransferWithAlgFn = self.encodeTransferWithAlgFn {
                                                return encodeTransferWithAlgFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    nonceValue,
                                                    nonceFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    quantityPtr, UInt(quantity.utf8.count),
                                                    destinationPtr, UInt(destination.utf8.count),
                                                    keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                    algorithmRaw,
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else if let feeSponsorPtr,
                                                      let encodeTransferWithFeeSponsorFn = self.encodeTransferWithFeeSponsorFn
                                            {
                                                return encodeTransferWithFeeSponsorFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    nonceValue,
                                                    nonceFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    quantityPtr, UInt(quantity.utf8.count),
                                                    destinationPtr, UInt(destination.utf8.count),
                                                    feeSponsorPtr, feeSponsorLen,
                                                    keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else if let encodeTransferFn = self.encodeTransferFn {
                                                return encodeTransferFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    nonceValue,
                                                    nonceFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    quantityPtr, UInt(quantity.utf8.count),
                                                    destinationPtr, UInt(destination.utf8.count),
                                                    keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else {
                                                return -1
                                            }
                                        }
                                    }
                                }
                            }
                            if let normalizedFeeSponsor {
                                return normalizedFeeSponsor.withCString { feeSponsorPtr in
                                    encodeTransferCall(feeSponsorPtr, UInt(normalizedFeeSponsor.utf8.count))
                                }
                            }
                            return encodeTransferCall(nil, 0)
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeRegisterZkAsset(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        assetDefinitionId: String,
        modeCode: UInt8,
        allowShield: Bool,
        allowUnshield: Bool,
        transferVerifyingKey: String?,
        unshieldVerifyingKey: String?,
        shieldVerifyingKey: String?,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let allowShieldFlag: UInt8 = allowShield ? 1 : 0
        let allowUnshieldFlag: UInt8 = allowUnshield ? 1 : 0
        let useAlg = algorithm != .ed25519 && encodeRegisterZkAssetWithAlgFn != nil
        guard useAlg || encodeRegisterZkAssetFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                        guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                            return -1
                        }
                        return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                            guard let hashPtr = hashBuffer.baseAddress else {
                                return -1
                            }
                            return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                withOptionalCString(transferVerifyingKey) { transferPtr, transferLen, transferFlag in
                                    withOptionalCString(unshieldVerifyingKey) { unshieldPtr, unshieldLen, unshieldFlag in
                                        withOptionalCString(shieldVerifyingKey) { shieldPtr, shieldLen, shieldFlag in
                                            if useAlg, let encodeRegisterZkAssetWithAlgFn {
                                                return encodeRegisterZkAssetWithAlgFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    modeCode,
                                                    allowShieldFlag,
                                                    allowUnshieldFlag,
                                                    transferPtr, transferLen,
                                                    transferFlag,
                                                    unshieldPtr, unshieldLen,
                                                    unshieldFlag,
                                                    shieldPtr, shieldLen,
                                                    shieldFlag,
                                                    keyBase, UInt(privateKey.count),
                                                    algorithmRaw,
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else if let encodeRegisterZkAssetFn {
                                                return encodeRegisterZkAssetFn(
                                                    chainPtr, UInt(chainId.utf8.count),
                                                    authorityPtr, UInt(authority.utf8.count),
                                                    creationTimeMs,
                                                    ttlValue,
                                                    ttlFlag,
                                                    assetPtr, UInt(assetDefinitionId.utf8.count),
                                                    modeCode,
                                                    allowShieldFlag,
                                                    allowUnshieldFlag,
                                                    transferPtr, transferLen,
                                                    transferFlag,
                                                    unshieldPtr, unshieldLen,
                                                    unshieldFlag,
                                                    shieldPtr, shieldLen,
                                                    shieldFlag,
                                                    keyBase, UInt(privateKey.count),
                                                    signedPtrPtr,
                                                    signedLenPtr,
                                                    hashPtr,
                                                    hashLength
                                                )
                                            } else {
                                                return -1
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr {
                freeFn(signedPtr)
            }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeMint(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        assetDefinitionId: String,
        quantity: String,
        destination: String,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let nonceValue = nonce ?? 0
        let nonceFlag: UInt8 = nonce == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeMintWithAlgFn != nil
        guard useAlg || encodeMintFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    quantity.withCString { quantityPtr in
                        destination.withCString { destinationPtr in
                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeMintWithAlgFn {
                                            return encodeMintWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                nonceValue,
                                                nonceFlag,
                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                quantityPtr, UInt(quantity.utf8.count),
                                                destinationPtr, UInt(destination.utf8.count),
                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                algorithmRaw,
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeMintFn {
                                            return encodeMintFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                nonceValue,
                                                nonceFlag,
                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                quantityPtr, UInt(quantity.utf8.count),
                                                destinationPtr, UInt(destination.utf8.count),
                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeShield(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        assetDefinitionId: String,
        fromAccountId: String,
        amount: String,
        noteCommitment: Data,
        payloadEphemeral: Data,
        payloadNonce: Data,
        payloadCiphertext: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        guard noteCommitment.count == 32,
              payloadEphemeral.count == 32,
              payloadNonce.count == 24 else {
            return nil
        }
        if payloadCiphertext.count > UInt32.max {
            return nil
        }
        let useAlg = algorithm != .ed25519 && encodeShieldWithAlgFn != nil
        guard useAlg || encodeShieldFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    fromAccountId.withCString { fromPtr in
                        amount.withCString { amountPtr in
                            noteCommitment.withUnsafeBytes { noteBuffer in
                                payloadEphemeral.withUnsafeBytes { ephBuffer in
                                    payloadNonce.withUnsafeBytes { nonceBuffer in
                                        payloadCiphertext.withUnsafeBytes { cipherBuffer in
                                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                                hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                                    guard let hashPtr = hashBuffer.baseAddress else {
                                                        return -1
                                                    }
                                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                                        let ttlValue = ttlMs ?? 0
                                                        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
                                                        if useAlg, let encodeShieldWithAlgFn {
                                                            return encodeShieldWithAlgFn(
                                                                chainPtr, UInt(chainId.utf8.count),
                                                                authorityPtr, UInt(authority.utf8.count),
                                                                creationTimeMs,
                                                                ttlValue,
                                                                ttlFlag,
                                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                                fromPtr, UInt(fromAccountId.utf8.count),
                                                                amountPtr, UInt(amount.utf8.count),
                                                                noteBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(noteCommitment.count),
                                                                ephBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadEphemeral.count),
                                                                nonceBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadNonce.count),
                                                                cipherBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadCiphertext.count),
                                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                                algorithmRaw,
                                                                signedPtrPtr,
                                                                signedLenPtr,
                                                                hashPtr,
                                                                hashLength
                                                            )
                                                        } else if let encodeShieldFn {
                                                            return encodeShieldFn(
                                                                chainPtr, UInt(chainId.utf8.count),
                                                                authorityPtr, UInt(authority.utf8.count),
                                                                creationTimeMs,
                                                                ttlValue,
                                                                ttlFlag,
                                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                                fromPtr, UInt(fromAccountId.utf8.count),
                                                                amountPtr, UInt(amount.utf8.count),
                                                                noteBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(noteCommitment.count),
                                                                ephBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadEphemeral.count),
                                                                nonceBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadNonce.count),
                                                                cipherBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(payloadCiphertext.count),
                                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                                signedPtrPtr,
                                                                signedLenPtr,
                                                                hashPtr,
                                                                hashLength
                                                            )
                                                        } else {
                                                            return -1
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeUnshield(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        assetDefinitionId: String,
        destinationAccountId: String,
        amount: String,
        inputs: Data,
        proofJSON: Data,
        rootHint: Data?,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        guard !inputs.isEmpty, !proofJSON.isEmpty else { return nil }
        let useAlg = algorithm != .ed25519 && encodeUnshieldWithAlgFn != nil
        guard useAlg || encodeUnshieldFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    destinationAccountId.withCString { destinationPtr in
                        amount.withCString { amountPtr in
                            inputs.withUnsafeBytes { inputBuffer -> Int32 in
                                guard let inputBase = inputBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                    return -1
                                }
                                return proofJSON.withUnsafeBytes { proofBuffer -> Int32 in
                                    guard let proofBase = proofBuffer.bindMemory(to: CChar.self).baseAddress else {
                                        return -1
                                    }
                                    return withOptionalBytes(rootHint) { rootPtr, rootLen in
                                        privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                            guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                                return -1
                                            }
                                            return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                                guard let hashPtr = hashBuffer.baseAddress else {
                                                    return -1
                                                }
                                                return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                                    let ttlValue = ttlMs ?? 0
                                                    let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
                                                    if useAlg, let encodeUnshieldWithAlgFn {
                                                        return encodeUnshieldWithAlgFn(
                                                            chainPtr, UInt(chainId.utf8.count),
                                                            authorityPtr, UInt(authority.utf8.count),
                                                            creationTimeMs,
                                                            ttlValue,
                                                            ttlFlag,
                                                            assetPtr, UInt(assetDefinitionId.utf8.count),
                                                            destinationPtr, UInt(destinationAccountId.utf8.count),
                                                            amountPtr, UInt(amount.utf8.count),
                                                            inputBase, UInt(inputs.count),
                                                            proofBase, UInt(proofJSON.count),
                                                            rootPtr, rootLen,
                                                            keyBase, UInt(privateKey.count),
                                                            algorithmRaw,
                                                            signedPtrPtr,
                                                            signedLenPtr,
                                                            hashPtr,
                                                            hashLength
                                                        )
                                                    } else if let encodeUnshieldFn {
                                                        return encodeUnshieldFn(
                                                            chainPtr, UInt(chainId.utf8.count),
                                                            authorityPtr, UInt(authority.utf8.count),
                                                            creationTimeMs,
                                                            ttlValue,
                                                            ttlFlag,
                                                            assetPtr, UInt(assetDefinitionId.utf8.count),
                                                            destinationPtr, UInt(destinationAccountId.utf8.count),
                                                            amountPtr, UInt(amount.utf8.count),
                                                            inputBase, UInt(inputs.count),
                                                            proofBase, UInt(proofJSON.count),
                                                            rootPtr, rootLen,
                                                            keyBase, UInt(privateKey.count),
                                                            signedPtrPtr,
                                                            signedLenPtr,
                                                            hashPtr,
                                                            hashLength
                                                        )
                                                    } else {
                                                        return -1
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeZkTransfer(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        assetDefinitionId: String,
        inputs: Data,
        outputs: Data,
        proofJSON: Data,
        rootHint: Data?,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        guard !inputs.isEmpty, !outputs.isEmpty, !proofJSON.isEmpty else { return nil }
        let useAlg = algorithm != .ed25519 && encodeZkTransferWithAlgFn != nil
        guard useAlg || encodeZkTransferFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    inputs.withUnsafeBytes { inputBuffer -> Int32 in
                        guard let inputBase = inputBuffer.bindMemory(to: UInt8.self).baseAddress else {
                            return -1
                        }
                        return outputs.withUnsafeBytes { outputBuffer -> Int32 in
                            guard let outputBase = outputBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return proofJSON.withUnsafeBytes { proofBuffer -> Int32 in
                                guard let proofBase = proofBuffer.bindMemory(to: CChar.self).baseAddress else {
                                    return -1
                                }
                                return withOptionalBytes(rootHint) { rootPtr, rootLen in
                                    privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                        guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                            return -1
                                        }
                                        return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                            guard let hashPtr = hashBuffer.baseAddress else {
                                                return -1
                                            }
                                            return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                                let ttlValue = ttlMs ?? 0
                                                let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
                                                if useAlg, let encodeZkTransferWithAlgFn {
                                                    return encodeZkTransferWithAlgFn(
                                                        chainPtr, UInt(chainId.utf8.count),
                                                        authorityPtr, UInt(authority.utf8.count),
                                                        creationTimeMs,
                                                        ttlValue,
                                                        ttlFlag,
                                                        assetPtr, UInt(assetDefinitionId.utf8.count),
                                                        inputBase, UInt(inputs.count),
                                                        outputBase, UInt(outputs.count),
                                                        proofBase, UInt(proofJSON.count),
                                                        rootPtr, rootLen,
                                                        keyBase, UInt(privateKey.count),
                                                        algorithmRaw,
                                                        signedPtrPtr,
                                                        signedLenPtr,
                                                        hashPtr,
                                                        hashLength
                                                    )
                                                } else if let encodeZkTransferFn {
                                                    return encodeZkTransferFn(
                                                        chainPtr, UInt(chainId.utf8.count),
                                                        authorityPtr, UInt(authority.utf8.count),
                                                        creationTimeMs,
                                                        ttlValue,
                                                        ttlFlag,
                                                        assetPtr, UInt(assetDefinitionId.utf8.count),
                                                        inputBase, UInt(inputs.count),
                                                        outputBase, UInt(outputs.count),
                                                        proofBase, UInt(proofJSON.count),
                                                        rootPtr, rootLen,
                                                        keyBase, UInt(privateKey.count),
                                                        signedPtrPtr,
                                                        signedLenPtr,
                                                        hashPtr,
                                                        hashLength
                                                    )
                                                } else {
                                                    return -1
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeMultisigRegister(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        accountId: String,
        specJSON: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let useAlg = algorithm != .ed25519 && encodeMultisigRegisterWithAlgFn != nil
        guard useAlg || encodeMultisigRegisterFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr -> Int32 in
                return authority.withCString { authorityPtr -> Int32 in
                return accountId.withCString { accountPtr -> Int32 in
                    return specJSON.withUnsafeBytes { specBuffer -> Int32 in
                        guard let specPtr = specBuffer.bindMemory(to: CChar.self).baseAddress else {
                            return -1
                        }
                        return privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                            guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                guard let hashPtr = hashBuffer.baseAddress else {
                                    return -1
                                }
                                return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                    let specLen = UInt(specJSON.count)
                                    let keyLen = UInt(privateKey.count)
                                    let accountLen = UInt(accountId.utf8.count)
                                    if useAlg, let encodeMultisigRegisterWithAlgFn {
                                        return encodeMultisigRegisterWithAlgFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            specPtr, specLen,
                                            accountPtr, accountLen,
                                            keyPtr, keyLen,
                                            algorithmRaw,
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else if let encodeMultisigRegisterFn {
                                        return encodeMultisigRegisterFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            specPtr, specLen,
                                            accountPtr, accountLen,
                                            keyPtr, keyLen,
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else {
                                        return -1
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeClaimIdentifier(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        accountId: String,
        receiptJSON: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let useAlg = algorithm != .ed25519 && encodeClaimIdentifierWithAlgFn != nil
        guard useAlg || encodeClaimIdentifierFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr -> Int32 in
                authority.withCString { authorityPtr -> Int32 in
                    accountId.withCString { accountPtr -> Int32 in
                        receiptJSON.withUnsafeBytes { receiptBuffer -> Int32 in
                            guard let receiptPtr = receiptBuffer.bindMemory(to: CChar.self).baseAddress else {
                                return -1
                            }
                            return privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                    return -1
                                }
                                return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeClaimIdentifierWithAlgFn {
                                            return encodeClaimIdentifierWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                accountPtr, UInt(accountId.utf8.count),
                                                receiptPtr, UInt(receiptJSON.count),
                                                keyPtr, UInt(privateKey.count),
                                                algorithmRaw,
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeClaimIdentifierFn {
                                            return encodeClaimIdentifierFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                accountPtr, UInt(accountId.utf8.count),
                                                receiptPtr, UInt(receiptJSON.count),
                                                keyPtr, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeBurn(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32? = nil,
        assetDefinitionId: String,
        quantity: String,
        destination: String,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let nonceValue = nonce ?? 0
        let nonceFlag: UInt8 = nonce == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeBurnWithAlgFn != nil
        guard useAlg || encodeBurnFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                assetDefinitionId.withCString { assetPtr in
                    quantity.withCString { quantityPtr in
                        destination.withCString { destinationPtr in
                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeBurnWithAlgFn {
                                            return encodeBurnWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                nonceValue,
                                                nonceFlag,
                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                quantityPtr, UInt(quantity.utf8.count),
                                                destinationPtr, UInt(destination.utf8.count),
                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                algorithmRaw,
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeBurnFn {
                                            return encodeBurnFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                nonceValue,
                                                nonceFlag,
                                                assetPtr, UInt(assetDefinitionId.utf8.count),
                                                quantityPtr, UInt(quantity.utf8.count),
                                                destinationPtr, UInt(destination.utf8.count),
                                                keyBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeSetKeyValue(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        targetKind: UInt8,
        objectId: String,
        key: String,
        valueJson: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeSetKeyValueWithAlgFn != nil
        guard useAlg || encodeSetKeyValueFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                objectId.withCString { objectPtr in
                    key.withCString { keyCStrPtr in
                        valueJson.withUnsafeBytes { valueBuffer -> Int32 in
                            guard let valuePtr = valueBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                guard let privateKeyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                    return -1
                                }
                                return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeSetKeyValueWithAlgFn {
                                            return encodeSetKeyValueWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                targetKind,
                                                objectPtr, UInt(objectId.utf8.count),
                                                keyCStrPtr, UInt(key.utf8.count),
                                                valuePtr, UInt(valueJson.count),
                                                privateKeyPtr, UInt(privateKey.count),
                                                algorithmRaw,
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeSetKeyValueFn {
                                            return encodeSetKeyValueFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                targetKind,
                                                objectPtr, UInt(objectId.utf8.count),
                                                keyCStrPtr, UInt(key.utf8.count),
                                                valuePtr, UInt(valueJson.count),
                                                privateKeyPtr, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeRemoveKeyValue(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        targetKind: UInt8,
        objectId: String,
        key: String,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeRemoveKeyValueWithAlgFn != nil
        guard useAlg || encodeRemoveKeyValueFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                objectId.withCString { objectPtr in
                    key.withCString { keyPtr in
                        privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                            guard let keyPtrBytes = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                guard let hashPtr = hashBuffer.baseAddress else {
                                    return -1
                                }
                                return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                    if useAlg, let encodeRemoveKeyValueWithAlgFn {
                                        return encodeRemoveKeyValueWithAlgFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            targetKind,
                                            objectPtr, UInt(objectId.utf8.count),
                                            keyPtr, UInt(key.utf8.count),
                                            keyPtrBytes, UInt(privateKey.count),
                                            algorithmRaw,
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else if let encodeRemoveKeyValueFn {
                                        return encodeRemoveKeyValueFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            targetKind,
                                            objectPtr, UInt(objectId.utf8.count),
                                            keyPtr, UInt(key.utf8.count),
                                            keyPtrBytes, UInt(privateKey.count),
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else {
                                        return -1
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernanceProposeDeploy(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        contractAddress: String,
        codeHashHex: String,
        abiHashHex: String,
        abiVersion: String,
        window: (UInt64, UInt64)?,
        modeCode: UInt8?,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernanceProposeDeployWithAlgFn != nil
        guard useAlg || encodeGovernanceProposeDeployFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                contractAddress.withCString { contractAddressPtr in
                        codeHashHex.withCString { codeHashPtr in
                            abiHashHex.withCString { abiHashPtr in
                                abiVersion.withCString { abiVersionPtr in
                                    privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                        guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                            return -1
                                        }
                                        return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                            guard let hashPtr = hashBuffer.baseAddress else {
                                                return -1
                                            }
                                            return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                                let windowLower = window?.0 ?? 0
                                                let windowUpper = window?.1 ?? 0
                                                let windowFlag: UInt8 = window == nil ? 0 : 1
                                                let modeFlag: UInt8 = modeCode == nil ? 0 : 1
                                                if useAlg, let encodeGovernanceProposeDeployWithAlgFn {
                                                    return encodeGovernanceProposeDeployWithAlgFn(
                                                        chainPtr, UInt(chainId.utf8.count),
                                                        authorityPtr, UInt(authority.utf8.count),
                                                        creationTimeMs,
                                                        ttlValue,
                                                        ttlFlag,
                                                        contractAddressPtr, UInt(contractAddress.utf8.count),
                                                        codeHashPtr, UInt(codeHashHex.utf8.count),
                                                        abiHashPtr, UInt(abiHashHex.utf8.count),
                                                        abiVersionPtr, UInt(abiVersion.utf8.count),
                                                        windowLower,
                                                        windowUpper,
                                                        windowFlag,
                                                        modeCode ?? 0,
                                                        modeFlag,
                                                        keyPtr, UInt(privateKey.count),
                                                        algorithmRaw,
                                                        signedPtrPtr,
                                                        signedLenPtr,
                                                        hashPtr,
                                                        hashLength
                                                    )
                                                } else if let encodeGovernanceProposeDeployFn {
                                                    return encodeGovernanceProposeDeployFn(
                                                        chainPtr, UInt(chainId.utf8.count),
                                                        authorityPtr, UInt(authority.utf8.count),
                                                        creationTimeMs,
                                                        ttlValue,
                                                        ttlFlag,
                                                        contractAddressPtr, UInt(contractAddress.utf8.count),
                                                        codeHashPtr, UInt(codeHashHex.utf8.count),
                                                        abiHashPtr, UInt(abiHashHex.utf8.count),
                                                        abiVersionPtr, UInt(abiVersion.utf8.count),
                                                        windowLower,
                                                        windowUpper,
                                                        windowFlag,
                                                        modeCode ?? 0,
                                                        modeFlag,
                                                        keyPtr, UInt(privateKey.count),
                                                        signedPtrPtr,
                                                        signedLenPtr,
                                                        hashPtr,
                                                        hashLength
                                                    )
                                                } else {
                                                    return -1
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernanceCastPlainBallot(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        referendumId: String,
        owner: String,
        amount: String,
        durationBlocks: UInt64,
        direction: UInt8,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernanceCastPlainBallotWithAlgFn != nil
        guard useAlg || encodeGovernanceCastPlainBallotFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                referendumId.withCString { referendumPtr in
                    owner.withCString { ownerPtr in
                        amount.withCString { amountPtr in
                            privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                    return -1
                                }
                                return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeGovernanceCastPlainBallotWithAlgFn {
                                            return encodeGovernanceCastPlainBallotWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                referendumPtr, UInt(referendumId.utf8.count),
                                                ownerPtr, UInt(owner.utf8.count),
                                                amountPtr, UInt(amount.utf8.count),
                                                durationBlocks,
                                                direction,
                                                keyPtr, UInt(privateKey.count),
                                                algorithmRaw,
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeGovernanceCastPlainBallotFn {
                                            return encodeGovernanceCastPlainBallotFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                referendumPtr, UInt(referendumId.utf8.count),
                                                ownerPtr, UInt(owner.utf8.count),
                                                amountPtr, UInt(amount.utf8.count),
                                                durationBlocks,
                                                direction,
                                                keyPtr, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernanceCastZkBallot(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        electionId: String,
        proofB64: String,
        publicInputs: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernanceCastZkBallotWithAlgFn != nil
        guard useAlg || encodeGovernanceCastZkBallotFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                electionId.withCString { electionPtr in
                    proofB64.withCString { proofPtr in
                        publicInputs.withUnsafeBytes { inputsBuffer -> Int32 in
                            guard let inputsPtr = inputsBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                                guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                    return -1
                                }
                                return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                    guard let hashPtr = hashBuffer.baseAddress else {
                                        return -1
                                    }
                                    return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                        if useAlg, let encodeGovernanceCastZkBallotWithAlgFn {
                                            return encodeGovernanceCastZkBallotWithAlgFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                electionPtr, UInt(electionId.utf8.count),
                                                proofPtr, UInt(proofB64.utf8.count),
                                                inputsPtr, UInt(publicInputs.count),
                                                keyPtr, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else if let encodeGovernanceCastZkBallotFn {
                                            return encodeGovernanceCastZkBallotFn(
                                                chainPtr, UInt(chainId.utf8.count),
                                                authorityPtr, UInt(authority.utf8.count),
                                                creationTimeMs,
                                                ttlValue,
                                                ttlFlag,
                                                electionPtr, UInt(electionId.utf8.count),
                                                proofPtr, UInt(proofB64.utf8.count),
                                                inputsPtr, UInt(publicInputs.count),
                                                keyPtr, UInt(privateKey.count),
                                                signedPtrPtr,
                                                signedLenPtr,
                                                hashPtr,
                                                hashLength
                                            )
                                        } else {
                                            return -1
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernanceEnactReferendum(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        referendumIdHex: String,
        preimageHashHex: String,
        windowLower: UInt64,
        windowUpper: UInt64,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernanceEnactReferendumWithAlgFn != nil
        guard useAlg || encodeGovernanceEnactReferendumFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                referendumIdHex.withCString { referendumPtr in
                    preimageHashHex.withCString { preimagePtr in
                        privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                            guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                guard let hashPtr = hashBuffer.baseAddress else {
                                    return -1
                                }
                                return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                    if useAlg, let encodeGovernanceEnactReferendumWithAlgFn {
                                        return encodeGovernanceEnactReferendumWithAlgFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            referendumPtr, UInt(referendumIdHex.utf8.count),
                                            preimagePtr, UInt(preimageHashHex.utf8.count),
                                            windowLower,
                                            windowUpper,
                                            keyPtr, UInt(privateKey.count),
                                            algorithmRaw,
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else if let encodeGovernanceEnactReferendumFn {
                                        return encodeGovernanceEnactReferendumFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            referendumPtr, UInt(referendumIdHex.utf8.count),
                                            preimagePtr, UInt(preimageHashHex.utf8.count),
                                            windowLower,
                                            windowUpper,
                                            keyPtr, UInt(privateKey.count),
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else {
                                        return -1
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernanceFinalizeReferendum(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        referendumId: String,
        proposalIdHex: String,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernanceFinalizeReferendumWithAlgFn != nil
        guard useAlg || encodeGovernanceFinalizeReferendumFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                referendumId.withCString { referendumPtr in
                    proposalIdHex.withCString { proposalPtr in
                        privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                            guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                                return -1
                            }
                            return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                                guard let hashPtr = hashBuffer.baseAddress else {
                                    return -1
                                }
                                return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                    if useAlg, let encodeGovernanceFinalizeReferendumWithAlgFn {
                                        return encodeGovernanceFinalizeReferendumWithAlgFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            referendumPtr, UInt(referendumId.utf8.count),
                                            proposalPtr, UInt(proposalIdHex.utf8.count),
                                            keyPtr, UInt(privateKey.count),
                                            algorithmRaw,
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else if let encodeGovernanceFinalizeReferendumFn {
                                        return encodeGovernanceFinalizeReferendumFn(
                                            chainPtr, UInt(chainId.utf8.count),
                                            authorityPtr, UInt(authority.utf8.count),
                                            creationTimeMs,
                                            ttlValue,
                                            ttlFlag,
                                            referendumPtr, UInt(referendumId.utf8.count),
                                            proposalPtr, UInt(proposalIdHex.utf8.count),
                                            keyPtr, UInt(privateKey.count),
                                            signedPtrPtr,
                                            signedLenPtr,
                                            hashPtr,
                                            hashLength
                                        )
                                    } else {
                                        return -1
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func encodeGovernancePersistCouncil(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        epoch: UInt64,
        candidatesCount: UInt32,
        derivedBy: UInt8,
        membersJson: Data,
        privateKey: Data,
        algorithm: SigningAlgorithm = .ed25519
    ) throws -> NativeSignedTransaction? {
        #if canImport(Darwin)
        guard let freeFn else { return nil }
        let ttlValue = ttlMs ?? 0
        let ttlFlag: UInt8 = ttlMs == nil ? 0 : 1
        let useAlg = algorithm != .ed25519 && encodeGovernancePersistCouncilWithAlgFn != nil
        guard useAlg || encodeGovernancePersistCouncilFn != nil else { return nil }

        var signedPtr: UnsafeMutablePointer<UInt8>? = nil
        var signedLen: UInt = 0
        var hashBytes = [UInt8](repeating: 0, count: 32)
        let hashLength = UInt(hashBytes.count)
        let algorithmRaw = algorithm.noritoDiscriminant

        let status = try withAuthorityChainDiscriminant(authority: authority) {
            chainId.withCString { chainPtr in
            authority.withCString { authorityPtr in
                membersJson.withUnsafeBytes { membersBuffer -> Int32 in
                    guard let membersPtr = membersBuffer.bindMemory(to: UInt8.self).baseAddress else {
                        return -1
                    }
                    return privateKey.withUnsafeBytes { keyBuffer -> Int32 in
                        guard let keyPtr = keyBuffer.bindMemory(to: UInt8.self).baseAddress else {
                            return -1
                        }
                        return hashBytes.withUnsafeMutableBufferPointer { hashBuffer -> Int32 in
                            guard let hashPtr = hashBuffer.baseAddress else {
                                return -1
                            }
                            return withSignedOutputs(signedPtr: &signedPtr, signedLen: &signedLen) { signedPtrPtr, signedLenPtr in
                                if useAlg, let encodeGovernancePersistCouncilWithAlgFn {
                                    return encodeGovernancePersistCouncilWithAlgFn(
                                        chainPtr, UInt(chainId.utf8.count),
                                        authorityPtr, UInt(authority.utf8.count),
                                        creationTimeMs,
                                        ttlValue,
                                        ttlFlag,
                                        epoch,
                                        candidatesCount,
                                        derivedBy,
                                        membersPtr, UInt(membersJson.count),
                                        keyPtr, UInt(privateKey.count),
                                        algorithmRaw,
                                        signedPtrPtr,
                                        signedLenPtr,
                                        hashPtr,
                                        hashLength
                                    )
                                } else if let encodeGovernancePersistCouncilFn {
                                    return encodeGovernancePersistCouncilFn(
                                        chainPtr, UInt(chainId.utf8.count),
                                        authorityPtr, UInt(authority.utf8.count),
                                        creationTimeMs,
                                        ttlValue,
                                        ttlFlag,
                                        epoch,
                                        candidatesCount,
                                        derivedBy,
                                        membersPtr, UInt(membersJson.count),
                                        keyPtr, UInt(privateKey.count),
                                        signedPtrPtr,
                                        signedLenPtr,
                                        hashPtr,
                                        hashLength
                                    )
                                } else {
                                    return -1
                                }
                            }
                        }
                    }
                }
            }
        }
        }

        if status != 0 {
            if let signedPtr { freeFn(signedPtr) }
            try throwOnStatus(status)
            return nil
        }
        guard let signedPtr else { return nil }

        let signedData = Data(bytes: signedPtr, count: Int(signedLen))
        freeFn(signedPtr)
        let hashData = Data(hashBytes)
        return NativeSignedTransaction(signedBytes: signedData, hash: hashData)
        #else
        return nil
        #endif
    }

    func applyAccelerationSettings(_ settings: AccelerationSettings) {
        #if canImport(Darwin)
        guard isAvailable else {
            return
        }
        guard let setAccelerationConfigFn else {
            return
        }

        func encodeOptional(_ value: Int?) -> (UInt64, UInt8) {
            if let value, value >= 0 {
                return (UInt64(value), 1)
            }
            return (0, 0)
        }

        let (maxGPUsValue, maxGPUsPresent) = encodeOptional(settings.maxGPUs)
        let (gpuLeavesValue, gpuLeavesPresent) = encodeOptional(settings.merkleMinLeavesGPU)
        let (metalLeavesValue, metalLeavesPresent) = encodeOptional(settings.merkleMinLeavesMetal)
        let (cudaLeavesValue, cudaLeavesPresent) = encodeOptional(settings.merkleMinLeavesCUDA)
        let (preferAarch64Value, preferAarch64Present) = encodeOptional(settings.preferCpuSha2MaxLeavesAarch64)
        let (preferX86Value, preferX86Present) = encodeOptional(settings.preferCpuSha2MaxLeavesX86)

        var config = ConnectNoritoAccelerationConfig(
            enable_simd: settings.enableSIMD ? 1 : 0,
            enable_metal: settings.enableMetal ? 1 : 0,
            enable_cuda: settings.enableCUDA ? 1 : 0,
            max_gpus: maxGPUsValue,
            max_gpus_present: maxGPUsPresent,
            merkle_min_leaves_gpu: gpuLeavesValue,
            merkle_min_leaves_gpu_present: gpuLeavesPresent,
            merkle_min_leaves_metal: metalLeavesValue,
            merkle_min_leaves_metal_present: metalLeavesPresent,
            merkle_min_leaves_cuda: cudaLeavesValue,
            merkle_min_leaves_cuda_present: cudaLeavesPresent,
            prefer_cpu_sha2_max_leaves_aarch64: preferAarch64Value,
            prefer_cpu_sha2_max_leaves_aarch64_present: preferAarch64Present,
            prefer_cpu_sha2_max_leaves_x86: preferX86Value,
            prefer_cpu_sha2_max_leaves_x86_present: preferX86Present
        )

        withUnsafePointer(to: &config) { ptr in
            setAccelerationConfigFn(UnsafeRawPointer(ptr))
        }
        #endif
    }

    func currentAccelerationSettings() -> AccelerationSettings? {
        #if canImport(Darwin)
        guard isAvailable else {
            return nil
        }
        guard let getAccelerationConfigFn else {
            return nil
        }

        var native = ConnectNoritoAccelerationConfig(
            enable_simd: 0,
            enable_metal: 0,
            enable_cuda: 0,
            max_gpus: 0,
            max_gpus_present: 0,
            merkle_min_leaves_gpu: 0,
            merkle_min_leaves_gpu_present: 0,
            merkle_min_leaves_metal: 0,
            merkle_min_leaves_metal_present: 0,
            merkle_min_leaves_cuda: 0,
            merkle_min_leaves_cuda_present: 0,
            prefer_cpu_sha2_max_leaves_aarch64: 0,
            prefer_cpu_sha2_max_leaves_aarch64_present: 0,
            prefer_cpu_sha2_max_leaves_x86: 0,
            prefer_cpu_sha2_max_leaves_x86_present: 0
        )

        let status = withUnsafeMutablePointer(to: &native) { pointer in
            getAccelerationConfigFn(UnsafeMutableRawPointer(pointer))
        }
        guard status == 0 else { return nil }
        return AccelerationSettings(nativeConfig: native)
        #else
        return nil
        #endif
    }

    func currentAccelerationState() -> AccelerationState? {
        #if canImport(Darwin)
        guard isAvailable else {
            return nil
        }
        guard let getAccelerationStateFn else {
            return nil
        }

        var native = ConnectNoritoAccelerationState(
            config: ConnectNoritoAccelerationConfig(
                enable_simd: 0,
                enable_metal: 0,
                enable_cuda: 0,
                max_gpus: 0,
                max_gpus_present: 0,
                merkle_min_leaves_gpu: 0,
                merkle_min_leaves_gpu_present: 0,
                merkle_min_leaves_metal: 0,
                merkle_min_leaves_metal_present: 0,
                merkle_min_leaves_cuda: 0,
                merkle_min_leaves_cuda_present: 0,
                prefer_cpu_sha2_max_leaves_aarch64: 0,
                prefer_cpu_sha2_max_leaves_aarch64_present: 0,
                prefer_cpu_sha2_max_leaves_x86: 0,
                prefer_cpu_sha2_max_leaves_x86_present: 0
            ),
            simd: ConnectNoritoAccelerationBackendStatus(supported: 0, configured: 0, available: 0, parity_ok: 0, last_error_ptr: nil, last_error_len: 0),
            metal: ConnectNoritoAccelerationBackendStatus(supported: 0, configured: 0, available: 0, parity_ok: 0, last_error_ptr: nil, last_error_len: 0),
            cuda: ConnectNoritoAccelerationBackendStatus(supported: 0, configured: 0, available: 0, parity_ok: 0, last_error_ptr: nil, last_error_len: 0)
        )

        let status = withUnsafeMutablePointer(to: &native) { pointer in
            getAccelerationStateFn(UnsafeMutableRawPointer(pointer))
        }
        guard status == 0 else { return nil }
        let decoded = AccelerationState(nativeState: native)
        if let freeFn {
            freeFn(native.simd.last_error_ptr)
            freeFn(native.metal.last_error_ptr)
            freeFn(native.cuda.last_error_ptr)
        }
        return decoded
        #else
        return nil
        #endif
    }

    func decodeSignedTransaction(_ data: Data) -> String? {
        #if canImport(Darwin)
        guard let decodeSignedFn, let freeFn else { return nil }

        var jsonPtr: UnsafeMutablePointer<UInt8>? = nil
        var jsonLen: UInt = 0

        let status = data.withUnsafeBytes { buffer -> Int32 in
            decodeSignedFn(buffer.bindMemory(to: UInt8.self).baseAddress, UInt(data.count), &jsonPtr, &jsonLen)
        }

        guard status == 0, let jsonPtr else {
            if let jsonPtr { freeFn(jsonPtr) }
            return nil
        }

        let jsonData = Data(bytes: jsonPtr, count: Int(jsonLen))
        freeFn(jsonPtr)
        return String(data: jsonData, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func decodeTransactionReceipt(_ data: Data) -> String? {
        #if canImport(Darwin)
        guard let decodeReceiptFn, let freeFn else { return nil }

        var jsonPtr: UnsafeMutablePointer<UInt8>? = nil
        var jsonLen: UInt = 0

        let status = data.withUnsafeBytes { buffer -> Int32 in
            decodeReceiptFn(buffer.bindMemory(to: UInt8.self).baseAddress, UInt(data.count), &jsonPtr, &jsonLen)
        }

        guard status == 0, let jsonPtr else {
            if let jsonPtr { freeFn(jsonPtr) }
            return nil
        }

        let jsonData = Data(bytes: jsonPtr, count: Int(jsonLen))
        freeFn(jsonPtr)
        return String(data: jsonData, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func decodeAssetId(_ literal: String) -> String? {
        #if canImport(Darwin)
        guard let decodeAssetIdFn, let freeFn else { return nil }
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }

        var jsonPtr: UnsafeMutablePointer<UInt8>? = nil
        var jsonLen: UInt = 0

        let status = trimmed.withCString { cString -> Int32 in
            decodeAssetIdFn(cString, UInt(trimmed.utf8.count), &jsonPtr, &jsonLen)
        }

        guard status == 0, let jsonPtr else {
            if let jsonPtr { freeFn(jsonPtr) }
            return nil
        }

        let jsonData = Data(bytes: jsonPtr, count: Int(jsonLen))
        freeFn(jsonPtr)
        return String(data: jsonData, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func proveKagemushaVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: Data) throws -> Data? {
        #if canImport(Darwin)
        guard let kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = recordBundleArchive.withUnsafeBytes { buffer -> Int32 in
            let baseAddress = buffer.bindMemory(to: UInt8.self).baseAddress
            return kagemushaProveVerifiedCompactPaymentTokenWithRecordsFn(
                baseAddress,
                CUnsignedLong(buffer.count),
                &outPtr,
                &outLen
            )
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    func proveKagemushaVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = recordBundleArchive.withUnsafeBytes { recordBuffer -> Int32 in
            let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
            return pallasOpenEnvelopesArchive.withUnsafeBytes { envelopeBuffer -> Int32 in
                let envelopeBaseAddress = envelopeBuffer.bindMemory(to: UInt8.self).baseAddress
                return kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopesFn(
                    recordBaseAddress,
                    CUnsignedLong(recordBuffer.count),
                    envelopeBaseAddress,
                    CUnsignedLong(envelopeBuffer.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    func proveKagemushaVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        recordBundleArchive: Data,
        pallasOpenEnvelopesArchive: Data,
        recursiveCompactKeyArtifactsArchive: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = recordBundleArchive.withUnsafeBytes { recordBuffer -> Int32 in
            let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
            return pallasOpenEnvelopesArchive.withUnsafeBytes { envelopeBuffer -> Int32 in
                let envelopeBaseAddress = envelopeBuffer.bindMemory(to: UInt8.self).baseAddress
                return recursiveCompactKeyArtifactsArchive.withUnsafeBytes { keyBuffer -> Int32 in
                    let keyBaseAddress = keyBuffer.bindMemory(to: UInt8.self).baseAddress
                    return kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopesFn(
                        recordBaseAddress,
                        CUnsignedLong(recordBuffer.count),
                        envelopeBaseAddress,
                        CUnsignedLong(envelopeBuffer.count),
                        keyBaseAddress,
                        CUnsignedLong(keyBuffer.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    func verifyKagemushaRecursiveCompactPaymentToken(
        compactTokenArchive: Data,
        recursiveCompactVerifierKeysArchive: Data
    ) throws -> Bool? {
        #if canImport(Darwin)
        guard let kagemushaVerifyRecursiveCompactPaymentTokenFn else {
            return nil
        }
        var valid: UInt8 = 0
        let status = compactTokenArchive.withUnsafeBytes { tokenBuffer -> Int32 in
            let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
            return recursiveCompactVerifierKeysArchive.withUnsafeBytes { keyBuffer -> Int32 in
                let keyBaseAddress = keyBuffer.bindMemory(to: UInt8.self).baseAddress
                return kagemushaVerifyRecursiveCompactPaymentTokenFn(
                    tokenBaseAddress,
                    CUnsignedLong(tokenBuffer.count),
                    keyBaseAddress,
                    CUnsignedLong(keyBuffer.count),
                    &valid
                )
            }
        }
        return try Self.normalizeKagemushaRecursiveCompactVerifierOutput(
            status: status,
            valid: valid
        )
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendCompactPaymentTokenFromBundle(
        bundleArchive: Data
    ) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: bundleArchive,
            function: kagemushaRecursiveSpendCompactPaymentTokenFromBundleFn
        )
    }

    func verifyKagemushaRecursiveSpendCompactPaymentTokenProjection(
        compactTokenArchive: Data,
        verifierRecordArchive: Data,
        blockHeight: UInt64?
    ) throws -> Bool? {
        #if canImport(Darwin)
        var valid: UInt8 = 0
        let status: Int32
        if let blockHeight {
            guard let function = kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeightFn else {
                return nil
            }
            status = compactTokenArchive.withUnsafeBytes { tokenBuffer -> Int32 in
                let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
                return verifierRecordArchive.withUnsafeBytes { recordBuffer -> Int32 in
                    let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
                    return function(
                        tokenBaseAddress,
                        CUnsignedLong(tokenBuffer.count),
                        recordBaseAddress,
                        CUnsignedLong(recordBuffer.count),
                        blockHeight,
                        &valid
                    )
                }
            }
        } else {
            guard let function = kagemushaVerifyRecursiveSpendCompactPaymentTokenProjectionFn else {
                return nil
            }
            status = compactTokenArchive.withUnsafeBytes { tokenBuffer -> Int32 in
                let tokenBaseAddress = tokenBuffer.bindMemory(to: UInt8.self).baseAddress
                return verifierRecordArchive.withUnsafeBytes { recordBuffer -> Int32 in
                    let recordBaseAddress = recordBuffer.bindMemory(to: UInt8.self).baseAddress
                    return function(
                        tokenBaseAddress,
                        CUnsignedLong(tokenBuffer.count),
                        recordBaseAddress,
                        CUnsignedLong(recordBuffer.count),
                        &valid
                    )
                }
            }
        }
        return try Self.normalizeKagemushaRecursiveCompactVerifierOutput(
            status: status,
            valid: valid
        )
        #else
        return nil
        #endif
    }

    private func callKagemushaRecursiveSpend(
        requestArchive: Data,
        function: KagemushaRecursiveSpendArchiveFn?
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = requestArchive.withUnsafeBytes { buffer -> Int32 in
            let baseAddress = buffer.bindMemory(to: UInt8.self).baseAddress
            return function(
                baseAddress,
                CUnsignedLong(buffer.count),
                &outPtr,
                &outLen
            )
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    private func callKagemushaRecursiveSpendLineageWitnessFromInitResult(
        requestArchive: Data,
        bundleArchive: Data,
        function: KagemushaRecursiveSpendLineageWitnessFromInitResultFn?
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = requestArchive.withUnsafeBytes { requestBuffer -> Int32 in
            let requestBaseAddress = requestBuffer.bindMemory(to: UInt8.self).baseAddress
            return bundleArchive.withUnsafeBytes { bundleBuffer -> Int32 in
                let bundleBaseAddress = bundleBuffer.bindMemory(to: UInt8.self).baseAddress
                return function(
                    requestBaseAddress,
                    CUnsignedLong(requestBuffer.count),
                    bundleBaseAddress,
                    CUnsignedLong(bundleBuffer.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    private func callKagemushaRecursiveSpendLineageWitnessAppendResult(
        previousWitnessArchive: Data,
        requestArchive: Data,
        bundleArchive: Data,
        function: KagemushaRecursiveSpendLineageWitnessAppendResultFn?
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function, let freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = previousWitnessArchive.withUnsafeBytes { witnessBuffer -> Int32 in
            let witnessBaseAddress = witnessBuffer.bindMemory(to: UInt8.self).baseAddress
            return requestArchive.withUnsafeBytes { requestBuffer -> Int32 in
                let requestBaseAddress = requestBuffer.bindMemory(to: UInt8.self).baseAddress
                return bundleArchive.withUnsafeBytes { bundleBuffer -> Int32 in
                    let bundleBaseAddress = bundleBuffer.bindMemory(to: UInt8.self).baseAddress
                    return function(
                        witnessBaseAddress,
                        CUnsignedLong(witnessBuffer.count),
                        requestBaseAddress,
                        CUnsignedLong(requestBuffer.count),
                        bundleBaseAddress,
                        CUnsignedLong(bundleBuffer.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freeFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: outPtr,
            length: outLen,
            free: freeFn
        )
        #else
        return nil
        #endif
    }

    func kagemushaBuildPallasOpenEnvelopesArchive(recordBundleArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: recordBundleArchive,
            function: kagemushaBuildPallasOpenEnvelopesArchiveFn
        )
    }

    func kagemushaBuildPreviousProofOpenEnvelopesArchive(previousBundleArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: previousBundleArchive,
            function: kagemushaBuildPreviousProofOpenEnvelopesArchiveFn
        )
    }

    func kagemushaRecursiveSpendInit(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendInitFn
        )
    }

    func kagemushaRecursiveSpendAppend(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendAppendFn
        )
    }

    func kagemushaRecursiveSpendTransitionProfileInit(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendTransitionProfileInitFn
        )
    }

    func kagemushaRecursiveSpendTransitionProfileAppend(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendTransitionProfileAppendFn
        )
    }

    func kagemushaRecursiveSpendLineageAppendBoundary(profileArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: profileArchive,
            function: kagemushaRecursiveSpendLineageAppendBoundaryFn
        )
    }

    func kagemushaRecursiveSpendLineageWitnessFromInitResult(
        requestArchive: Data,
        bundleArchive: Data
    ) throws -> Data? {
        try callKagemushaRecursiveSpendLineageWitnessFromInitResult(
            requestArchive: requestArchive,
            bundleArchive: bundleArchive,
            function: kagemushaRecursiveSpendLineageWitnessFromInitResultFn
        )
    }

    func kagemushaRecursiveSpendLineageWitnessAppendResult(
        previousWitnessArchive: Data,
        requestArchive: Data,
        bundleArchive: Data
    ) throws -> Data? {
        try callKagemushaRecursiveSpendLineageWitnessAppendResult(
            previousWitnessArchive: previousWitnessArchive,
            requestArchive: requestArchive,
            bundleArchive: bundleArchive,
            function: kagemushaRecursiveSpendLineageWitnessAppendResultFn
        )
    }

    func kagemushaRecursiveSpendVerify(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendVerifyFn
        )
    }

    func kagemushaRecursiveSpendRedeem(requestArchive: Data) throws -> Data? {
        try callKagemushaRecursiveSpend(
            requestArchive: requestArchive,
            function: kagemushaRecursiveSpendRedeemFn
        )
    }

    func privacyCapabilitiesV1() throws -> Data? {
        #if canImport(Darwin)
        guard let privacyCapabilitiesFn, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = privacyCapabilitiesFn(&outPtr, &outLen)
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freePrivacyFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.readPrivacyNativeOutput(
            pointer: outPtr,
            length: outLen,
            expectedSchemaByte: Self.privacyCapabilitiesResultSchemaByte
        ) { pointer in
            freePrivacyFn(pointer)
        }
        #else
        return nil
        #endif
    }

    public func privacyProofRequestV1(
        algorithmId: String,
        entrypoint: String,
        vkRef: String,
        publicInputs: Data,
        witness: Data,
        proof: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let privacyProofRequestFn, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return nil
        }
        var algorithmIdBytes = try Self.privacyRequestTextBytes(algorithmId)
        var entrypointBytes = try Self.privacyRequestTextBytes(entrypoint)
        var vkRefBytes = try Self.privacyRequestTextBytes(vkRef)
        var publicInputBytes = try Self.privacyRequestComponentBytes(
            publicInputs,
            maxBytes: Self.privacyRequestPublicInputsMaxBytes,
            allowEmpty: false
        )
        var witnessBytes = try Self.privacyRequestComponentBytes(
            witness,
            maxBytes: Self.privacyRequestWitnessMaxBytes,
            allowEmpty: true
        )
        var proofBytes = try Self.privacyRequestComponentBytes(
            proof,
            maxBytes: Self.privacyRequestProofMaxBytes,
            allowEmpty: true
        )
        defer {
            Self.clearTemporaryPrivacyRequestArchive(&algorithmIdBytes)
            Self.clearTemporaryPrivacyRequestArchive(&entrypointBytes)
            Self.clearTemporaryPrivacyRequestArchive(&vkRefBytes)
            Self.clearTemporaryPrivacyRequestArchive(&publicInputBytes)
            Self.clearTemporaryPrivacyRequestArchive(&witnessBytes)
            Self.clearTemporaryPrivacyRequestArchive(&proofBytes)
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = algorithmIdBytes.withUnsafeBufferPointer { algorithmBuffer in
            entrypointBytes.withUnsafeBufferPointer { entrypointBuffer in
                vkRefBytes.withUnsafeBufferPointer { vkRefBuffer in
                    publicInputBytes.withUnsafeBufferPointer { publicInputBuffer in
                        witnessBytes.withUnsafeBufferPointer { witnessBuffer in
                            proofBytes.withUnsafeBufferPointer { proofBuffer in
                                privacyProofRequestFn(
                                    algorithmBuffer.baseAddress,
                                    CUnsignedLong(algorithmBuffer.count),
                                    entrypointBuffer.baseAddress,
                                    CUnsignedLong(entrypointBuffer.count),
                                    vkRefBuffer.baseAddress,
                                    CUnsignedLong(vkRefBuffer.count),
                                    publicInputBuffer.baseAddress,
                                    CUnsignedLong(publicInputBuffer.count),
                                    witnessBuffer.baseAddress,
                                    CUnsignedLong(witnessBuffer.count),
                                    proofBuffer.baseAddress,
                                    CUnsignedLong(proofBuffer.count),
                                    &outPtr,
                                    &outLen
                                )
                            }
                        }
                    }
                }
            }
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freePrivacyFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.readPrivacyNativeOutput(
            pointer: outPtr,
            length: outLen,
            expectedSchemaByte: Self.privacyRequestSchemaByte
        ) { pointer in
            freePrivacyFn(pointer)
        }
        #else
        return nil
        #endif
    }

    private func callPrivacyProof(
        requestArchive: Data,
        function: PrivacyProofArchiveFn?,
        expectedSchemaByte: UInt8
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function, let freePrivacyFn = privacyFreeFn ?? freeFn else {
            return nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = try Self.withTemporaryPrivacyRequestArchive(requestArchive: requestArchive) { buffer -> Int32 in
            let baseAddress = buffer.baseAddress
            return function(
                baseAddress,
                CUnsignedLong(buffer.count),
                &outPtr,
                &outLen
            )
        }
        if let error = NativeBridgeError.fromStatus(status) {
            if let outPtr {
                freePrivacyFn(outPtr)
            }
            throw error
        }
        guard let outPtr else {
            throw NativeBridgeError.nullPointer
        }
        return try Self.readPrivacyNativeOutput(
            pointer: outPtr,
            length: outLen,
            expectedSchemaByte: expectedSchemaByte
        ) { pointer in
            freePrivacyFn(pointer)
        }
        #else
        return nil
        #endif
    }

    static func privacyRequestTextBytes(_ value: String) throws -> [UInt8] {
        let bytes = Array(value.utf8)
        guard bytes.count <= Self.privacyRequestTextFieldMaxBytes else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        return bytes
    }

    static func privacyRequestComponentBytes(
        _ value: Data,
        maxBytes: Int,
        allowEmpty: Bool
    ) throws -> [UInt8] {
        guard allowEmpty || !value.isEmpty else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        guard value.count <= maxBytes else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        return Array(value)
    }

    static func withTemporaryPrivacyRequestArchive<T>(
        requestArchive: Data,
        didClearForTesting: (([UInt8]) -> Void)? = nil,
        _ body: (UnsafeBufferPointer<UInt8>) throws -> T
    ) throws -> T {
        guard !requestArchive.isEmpty, requestArchive.count <= Self.privacyNativeArchiveMaxBytes else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        guard Self.isValidPrivacyNoritoArchive(requestArchive) else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        guard Self.hasPrivacyNoritoSchema(
            requestArchive,
            expectedSchemaByte: Self.privacyRequestSchemaByte
        ) else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        guard Self.hasNonEmptyPrivacyNoritoPayload(requestArchive) else {
            throw NativeBridgeError.invalidPrivacyRequest
        }
        var request = [UInt8](requestArchive)
        defer {
            Self.clearTemporaryPrivacyRequestArchive(&request)
            didClearForTesting?(request)
        }
        return try request.withUnsafeBufferPointer { buffer in
            try body(buffer)
        }
    }

    static func clearTemporaryPrivacyRequestArchive(_ requestArchive: inout [UInt8]) {
        for index in requestArchive.indices {
            requestArchive[index] = 0
        }
    }

    func privacyBuildProofV1(requestArchive: Data) throws -> Data? {
        try callPrivacyProof(
            requestArchive: requestArchive,
            function: privacyBuildProofFn,
            expectedSchemaByte: Self.privacyBuildProofResultSchemaByte
        )
    }

    func privacyVerifyProofV1(requestArchive: Data) throws -> Data? {
        try callPrivacyProof(
            requestArchive: requestArchive,
            function: privacyVerifyProofFn,
            expectedSchemaByte: Self.privacyVerifyProofResultSchemaByte
        )
    }

    static func readPrivacyNativeOutput(
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong,
        expectedSchemaByte: UInt8,
        free: (UnsafeMutablePointer<UInt8>?) -> Void
    ) throws -> Data {
        guard let pointer else {
            throw NativeBridgeError.nullPointer
        }
        defer {
            Self.clearPrivacyNativeBuffer(pointer, length: length)
            free(pointer)
        }
        guard length > 0, length <= CUnsignedLong(Self.privacyNativeArchiveMaxBytes) else {
            throw NativeBridgeError.invalidPrivacyOutput
        }
        let archive = Data(bytes: pointer, count: Int(length))
        guard Self.isValidPrivacyNoritoArchive(archive) else {
            throw NativeBridgeError.invalidPrivacyOutput
        }
        guard Self.hasNonEmptyPrivacyNoritoPayload(archive) else {
            throw NativeBridgeError.invalidPrivacyOutput
        }
        guard Self.hasPrivacyNoritoSchema(archive, expectedSchemaByte: expectedSchemaByte) else {
            throw NativeBridgeError.invalidPrivacyOutput
        }
        return archive
    }

    private static func clearPrivacyNativeBuffer(
        _ pointer: UnsafeMutablePointer<UInt8>,
        length: CUnsignedLong
    ) {
        guard
            length > 0,
            length <= CUnsignedLong(Self.privacyNativeArchiveMaxBytes),
            let count = Int(exactly: length)
        else {
            return
        }
        pointer.update(repeating: 0, count: count)
    }

    var canUseConnectCrypto: Bool {
        #if canImport(Darwin)
        return connectGenerateKeypairFn != nil
            && connectDeriveKeysFn != nil
            && connectEncryptEnvelopeFn != nil
            && connectDecryptCiphertextFn != nil
        #else
        return false
        #endif
    }

    func publicKeyFromPrivate(algorithm: SigningAlgorithm, privateKey: Data) -> Data? {
        #if canImport(Darwin)
        guard let publicKeyFromPrivateFn, let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = privateKey.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return publicKeyFromPrivateFn(
                algorithm.noritoDiscriminant,
                base,
                CUnsignedLong(privateKey.count),
                &outPtr,
                &outLen
            )
        }
        guard status == 0, let ptr = outPtr else {
            if let outPtr { freeFn(outPtr) }
            return nil
        }
        let publicKey = Data(bytes: ptr, count: Int(outLen))
        freeFn(ptr)
        return publicKey
        #else
        return nil
        #endif
    }

    func keypairFromSeed(algorithm: SigningAlgorithm, seed: Data) -> (privateKey: Data, publicKey: Data)? {
        #if canImport(Darwin)
        guard let keypairFromSeedFn, let freeFn else { return nil }
        var outPrivatePtr: UnsafeMutablePointer<UInt8>? = nil
        var outPrivateLen: CUnsignedLong = 0
        var outPublicPtr: UnsafeMutablePointer<UInt8>? = nil
        var outPublicLen: CUnsignedLong = 0
        let status = seed.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return keypairFromSeedFn(
                algorithm.noritoDiscriminant,
                base,
                CUnsignedLong(seed.count),
                &outPrivatePtr,
                &outPrivateLen,
                &outPublicPtr,
                &outPublicLen
            )
        }
        guard status == 0, let privatePtr = outPrivatePtr, let publicPtr = outPublicPtr else {
            if let outPrivatePtr { freeFn(outPrivatePtr) }
            if let outPublicPtr { freeFn(outPublicPtr) }
            return nil
        }
        let privateKey = Data(bytes: privatePtr, count: Int(outPrivateLen))
        let publicKey = Data(bytes: publicPtr, count: Int(outPublicLen))
        freeFn(privatePtr)
        freeFn(publicPtr)
        return (privateKey, publicKey)
        #else
        return nil
        #endif
    }

    func signDetached(algorithm: SigningAlgorithm, privateKey: Data, message: Data) -> Data? {
        #if canImport(Darwin)
        guard let signDetachedFn, let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = privateKey.withUnsafeBytes { keyBuffer -> Int32 in
            guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signDetachedFn(
                    algorithm.noritoDiscriminant,
                    keyBase,
                    CUnsignedLong(privateKey.count),
                    msgBase,
                    CUnsignedLong(message.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let ptr = outPtr else {
            if let outPtr { freeFn(outPtr) }
            return nil
        }
        let signature = Data(bytes: ptr, count: Int(outLen))
        freeFn(ptr)
        return signature
        #else
        return nil
        #endif
    }

    func verifyDetached(
        algorithm: SigningAlgorithm,
        publicKey: Data,
        message: Data,
        signature: Data
    ) -> Bool? {
        #if canImport(Darwin)
        guard let verifyDetachedFn else { return nil }
        var valid: UInt8 = 0
        let status = publicKey.withUnsafeBytes { pubBuffer -> Int32 in
            guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signature.withUnsafeBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return verifyDetachedFn(
                        algorithm.noritoDiscriminant,
                        pubBase,
                        CUnsignedLong(publicKey.count),
                        msgBase,
                        CUnsignedLong(message.count),
                        sigBase,
                        CUnsignedLong(signature.count),
                        &valid
                    )
                }
            }
        }
        guard status == 0 else { return nil }
        return valid != 0
        #else
        return nil
        #endif
    }

    func sm2DefaultDistid() -> String? {
        #if canImport(Darwin)
        guard let sm2DefaultDistidFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = sm2DefaultDistidFn(&outPtr, &outLen)
        guard status == 0, let ptr = outPtr else { return nil }
        let data = Data(bytes: ptr, count: Int(outLen))
        freeFn(ptr)
        return String(data: data, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func sm2KeypairFromSeed(distid: String?, seed: Data) -> (privateKey: Data, publicKey: Data)? {
        #if canImport(Darwin)
        guard let sm2KeypairFromSeedFn else { return nil }
        var privateKey = [UInt8](repeating: 0, count: 32)
        var publicKey = [UInt8](repeating: 0, count: 65)
        let distData = distid?.data(using: .utf8)
        let seedCount = seed.count
        let privateCapacity = privateKey.count
        let publicCapacity = publicKey.count
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            seed.withUnsafeBytes { seedBuffer -> Int32 in
                guard let seedBase = seedBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return privateKey.withUnsafeMutableBytes { privBuffer -> Int32 in
                    guard let privBase = privBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return publicKey.withUnsafeMutableBytes { pubBuffer -> Int32 in
                        guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                        return sm2KeypairFromSeedFn(
                            distPtr,
                            distLen,
                            seedBase,
                            UInt(seedCount),
                            privBase,
                            UInt(privateCapacity),
                            pubBase,
                            UInt(publicCapacity)
                        )
                    }
                }
            }
        }
        guard status == 0 else { return nil }
        return (Data(privateKey), Data(publicKey))
        #else
        return nil
        #endif
    }

    func sm2Sign(distid: String?, privateKey: Data, message: Data) -> Data? {
        #if canImport(Darwin)
        guard let sm2SignFn,
              privateKey.count == 32 else { return nil }
        var signature = [UInt8](repeating: 0, count: 64)
        let distData = distid?.data(using: .utf8)
        let privateCapacity = privateKey.count
        let messageCount = message.count
        let signatureCapacity = signature.count
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            privateKey.withUnsafeBytes { privBuffer -> Int32 in
                guard let privBase = privBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return message.withUnsafeBytes { msgBuffer -> Int32 in
                    guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return signature.withUnsafeMutableBytes { sigBuffer -> Int32 in
                        guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                        return sm2SignFn(
                            distPtr,
                            distLen,
                            privBase,
                            UInt(privateCapacity),
                            msgBase,
                            UInt(messageCount),
                            sigBase,
                            UInt(signatureCapacity)
                        )
                    }
                }
            }
        }
        guard status == 0 else { return nil }
        return Data(signature)
        #else
        return nil
        #endif
    }

    func sm2Verify(distid: String?, publicKey: Data, message: Data, signature: Data) -> Bool? {
        #if canImport(Darwin)
        guard let sm2VerifyFn,
              publicKey.count == 65,
              signature.count == 64 else { return nil }
        let distData = distid?.data(using: .utf8)
        let publicCapacity = publicKey.count
        let messageCount = message.count
        let signatureCount = signature.count
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            publicKey.withUnsafeBytes { pubBuffer -> Int32 in
                guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return message.withUnsafeBytes { msgBuffer -> Int32 in
                    guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return signature.withUnsafeBytes { sigBuffer -> Int32 in
                        guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                        return sm2VerifyFn(
                            distPtr,
                            distLen,
                            pubBase,
                            UInt(publicCapacity),
                            msgBase,
                            UInt(messageCount),
                            sigBase,
                            UInt(signatureCount)
                        )
                    }
                }
            }
        }
        if status < 0 {
            return nil
        }
        return status == 1
        #else
        return nil
        #endif
    }

    func sm2PublicKeyPrefixed(distid: String?, publicKey: Data) -> String? {
        #if canImport(Darwin)
        guard let sm2PublicKeyPrefixedFn,
              let freeFn,
              publicKey.count == 65 else { return nil }
        let distData = distid?.data(using: .utf8)
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            publicKey.withUnsafeBytes { pubBuffer -> Int32 in
                guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return sm2PublicKeyPrefixedFn(
                    distPtr,
                    distLen,
                    pubBase,
                    UInt(publicKey.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let ptr = outPtr else { return nil }
        let data = Data(bytes: ptr, count: Int(outLen))
        freeFn(ptr)
        return String(data: data, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func sm2PublicKeyMultihash(distid: String?, publicKey: Data) -> String? {
        #if canImport(Darwin)
        guard let sm2PublicKeyMultihashFn,
              let freeFn,
              publicKey.count == 65 else { return nil }
        let distData = distid?.data(using: .utf8)
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            publicKey.withUnsafeBytes { pubBuffer -> Int32 in
                guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return sm2PublicKeyMultihashFn(
                    distPtr,
                    distLen,
                    pubBase,
                    UInt(publicKey.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let ptr = outPtr else { return nil }
        let data = Data(bytes: ptr, count: Int(outLen))
        freeFn(ptr)
        return String(data: data, encoding: .utf8)
        #else
        return nil
        #endif
    }

    func sm2ComputeZa(distid: String?, publicKey: Data) -> Data? {
        #if canImport(Darwin)
        guard let sm2ComputeZaFn,
              publicKey.count == 65 else { return nil }
        var za = [UInt8](repeating: 0, count: 32)
        let distData = distid?.data(using: .utf8)
        let publicCapacity = publicKey.count
        let zaCapacity = za.count
        let status = withOptionalCStringData(distData) { distPtr, distLen in
            publicKey.withUnsafeBytes { pubBuffer -> Int32 in
                guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return za.withUnsafeMutableBytes { zaBuffer -> Int32 in
                    guard let zaBase = zaBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return sm2ComputeZaFn(
                        distPtr,
                        distLen,
                        pubBase,
                        UInt(publicCapacity),
                        zaBase,
                        UInt(zaCapacity)
                    )
                }
            }
        }
        guard status == 0 else { return nil }
        return Data(za)
        #else
        return nil
        #endif
    }

    func secp256k1PublicKey(privateKey: Data) -> Data? {
        #if canImport(Darwin)
        guard let secp256k1PublicKeyFn,
              privateKey.count == 32 else { return nil }
        var publicKey = [UInt8](repeating: 0, count: 33)
        let privateCapacity = privateKey.count
        let publicCount = publicKey.count
        let status = privateKey.withUnsafeBytes { privBuffer -> Int32 in
            guard let privBase = privBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return publicKey.withUnsafeMutableBytes { pubBuffer -> Int32 in
                guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return secp256k1PublicKeyFn(
                    privBase,
                    UInt(privateCapacity),
                    pubBase,
                    UInt(publicCount)
                )
            }
        }
        guard status == 0 else { return nil }
        return Data(publicKey)
        #else
        return nil
        #endif
    }

    func secp256k1Sign(privateKey: Data, message: Data) -> Data? {
        #if canImport(Darwin)
        guard let secp256k1SignFn,
              privateKey.count == 32 else { return nil }
        var signature = [UInt8](repeating: 0, count: 64)
        let privateCapacity = privateKey.count
        let messageCount = message.count
        let signatureCount = signature.count
        let status = privateKey.withUnsafeBytes { privBuffer -> Int32 in
            guard let privBase = privBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signature.withUnsafeMutableBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return secp256k1SignFn(
                        privBase,
                        UInt(privateCapacity),
                        msgBase,
                        UInt(messageCount),
                        sigBase,
                        UInt(signatureCount)
                    )
                }
            }
        }
        guard status == 0 else { return nil }
        return Data(signature)
        #else
        return nil
        #endif
    }

    func secp256k1Verify(publicKey: Data, message: Data, signature: Data) -> Bool? {
        #if canImport(Darwin)
        guard let secp256k1VerifyFn,
              publicKey.count == 33,
              signature.count == 64 else { return nil }
        let publicCapacity = publicKey.count
        let messageCount = message.count
        let signatureCount = signature.count
        let status = publicKey.withUnsafeBytes { pubBuffer -> Int32 in
            guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signature.withUnsafeBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return secp256k1VerifyFn(
                        pubBase,
                        UInt(publicCapacity),
                        msgBase,
                        UInt(messageCount),
                        sigBase,
                        UInt(signatureCount),
                        nil,
                        0
                    )
                }
            }
        }
        if status < 0 {
            return nil
        }
        return status == 1
        #else
        return nil
        #endif
    }

    var secp256k1Supported: Bool {
        #if canImport(Darwin)
        let generic = publicKeyFromPrivateFn != nil
            && signDetachedFn != nil
            && verifyDetachedFn != nil
        let dedicated = secp256k1PublicKeyFn != nil
            && secp256k1SignFn != nil
            && secp256k1VerifyFn != nil
        return generic || dedicated
        #else
        return false
        #endif
    }

    var mldsaSupported: Bool {
        #if canImport(Darwin)
        return mldsaParametersFn != nil
            && mldsaGenerateKeypairFn != nil
            && mldsaSignFn != nil
            && mldsaVerifyFn != nil
        #else
        return false
        #endif
    }

    func mldsaParameters(suiteId: UInt8) -> (publicKeyLength: Int, secretKeyLength: Int, signatureLength: Int)? {
        #if canImport(Darwin)
        guard let mldsaParametersFn else { return nil }
        var publicLen: UInt32 = 0
        var secretLen: UInt32 = 0
        var signatureLen: UInt32 = 0
        let status = mldsaParametersFn(UInt32(suiteId), &publicLen, &secretLen, &signatureLen)
        guard status == 0 else { return nil }
        return (Int(publicLen), Int(secretLen), Int(signatureLen))
        #else
        return nil
        #endif
    }

    func mldsaGenerateKeypair(suiteId: UInt8, publicKeyLength: Int, secretKeyLength: Int) -> (publicKey: Data, secretKey: Data)? {
        #if canImport(Darwin)
        guard let mldsaGenerateKeypairFn else { return nil }
        var publicKey = Data(repeating: 0, count: publicKeyLength)
        var secretKey = Data(repeating: 0, count: secretKeyLength)
        let status = publicKey.withUnsafeMutableBytes { pubBuffer -> Int32 in
            guard let pubBase = pubBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return secretKey.withUnsafeMutableBytes { secBuffer -> Int32 in
                guard let secBase = secBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return mldsaGenerateKeypairFn(
                    UInt32(suiteId),
                    pubBase,
                    CUnsignedLong(publicKeyLength),
                    secBase,
                    CUnsignedLong(secretKeyLength)
                )
            }
        }
        guard status == 0 else { return nil }
        return (publicKey, secretKey)
        #else
        return nil
        #endif
    }

    func mldsaSign(suiteId: UInt8, secretKey: Data, message: Data, signatureLength: Int) -> Data? {
        #if canImport(Darwin)
        guard let mldsaSignFn else { return nil }
        var signature = Data(repeating: 0, count: signatureLength)
        let status = secretKey.withUnsafeBytes { skBuffer -> Int32 in
            guard let skBase = skBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signature.withUnsafeMutableBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return mldsaSignFn(
                        UInt32(suiteId),
                        skBase,
                        CUnsignedLong(secretKey.count),
                        msgBase,
                        CUnsignedLong(message.count),
                        sigBase,
                        CUnsignedLong(signatureLength)
                    )
                }
            }
        }
        guard status == 0 else { return nil }
        return signature
        #else
        return nil
        #endif
    }

    func mldsaVerify(suiteId: UInt8, publicKey: Data, message: Data, signature: Data) -> Bool? {
        #if canImport(Darwin)
        guard let mldsaVerifyFn else { return nil }
        let status = publicKey.withUnsafeBytes { pkBuffer -> Int32 in
            guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return message.withUnsafeBytes { msgBuffer -> Int32 in
                guard let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return signature.withUnsafeBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return mldsaVerifyFn(
                        UInt32(suiteId),
                        pkBase,
                        CUnsignedLong(publicKey.count),
                        msgBase,
                        CUnsignedLong(message.count),
                        sigBase,
                        CUnsignedLong(signature.count)
                    )
                }
            }
        }
        if status < 0 {
            return nil
        }
        return status == 0
        #else
        return nil
        #endif
    }

    func connectGenerateKeypair() -> (publicKey: Data, privateKey: Data)? {
        #if canImport(Darwin)
        guard let connectGenerateKeypairFn else { return nil }
        var publicKey = [UInt8](repeating: 0, count: 32)
        var privateKey = [UInt8](repeating: 0, count: 32)
        let status = publicKey.withUnsafeMutableBytes { pkBuffer -> Int32 in
            guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return privateKey.withUnsafeMutableBytes { skBuffer -> Int32 in
                guard let skBase = skBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return connectGenerateKeypairFn(pkBase, skBase)
            }
        }
        guard status == 0 else { return nil }
        return (Data(publicKey), Data(privateKey))
        #else
        return nil
        #endif
    }

    func connectPublicFromPrivate(_ privateKey: Data) -> Data? {
        #if canImport(Darwin)
        guard let connectPublicFromPrivateFn,
              privateKey.count == 32 else { return nil }
        var publicKey = [UInt8](repeating: 0, count: 32)
        let status = publicKey.withUnsafeMutableBytes { pkBuffer -> Int32 in
            guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return privateKey.withUnsafeBytes { skBuffer -> Int32 in
                guard let skBase = skBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return connectPublicFromPrivateFn(skBase, pkBase)
            }
        }
        guard status == 0 else { return nil }
        return Data(publicKey)
        #else
        return nil
        #endif
    }

    func connectDeriveKeys(privateKey: Data, peerPublicKey: Data, sessionID: Data) -> (appKey: Data, walletKey: Data)? {
        #if canImport(Darwin)
        guard let connectDeriveKeysFn,
              privateKey.count == 32,
              peerPublicKey.count == 32,
              sessionID.count == 32 else { return nil }
        var appKey = [UInt8](repeating: 0, count: 32)
        var walletKey = [UInt8](repeating: 0, count: 32)
        let status = privateKey.withUnsafeBytes { skBuffer -> Int32 in
            guard let skBase = skBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return peerPublicKey.withUnsafeBytes { pkBuffer -> Int32 in
                guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return sessionID.withUnsafeBytes { sidBuffer -> Int32 in
                    guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return appKey.withUnsafeMutableBytes { appBuffer -> Int32 in
                        guard let appBase = appBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                        return walletKey.withUnsafeMutableBytes { walBuffer -> Int32 in
                            guard let walBase = walBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                            return connectDeriveKeysFn(skBase, pkBase, sidBase, appBase, walBase)
                        }
                    }
                }
            }
        }
        guard status == 0 else { return nil }
        return (Data(appKey), Data(walletKey))
        #else
        return nil
        #endif
    }

    func connectEncryptEnvelope(key: Data, sessionID: Data, direction: ConnectDirection, envelope: Data) -> Data? {
        #if canImport(Darwin)
        guard let connectEncryptEnvelopeFn,
              let freeFn,
              key.count == 32,
              sessionID.count == 32 else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let dirRaw: UInt8 = direction == .appToWallet ? 0 : 1
        let status = key.withUnsafeBytes { keyBuffer -> Int32 in
            guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return sessionID.withUnsafeBytes { sidBuffer -> Int32 in
                guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return envelope.withUnsafeBytes { envBuffer -> Int32 in
                    let envBase = envBuffer.bindMemory(to: UInt8.self).baseAddress
                    return connectEncryptEnvelopeFn(
                        keyBase,
                        sidBase,
                        dirRaw,
                        envBase,
                        UInt(envelope.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func connectDecryptCiphertext(key: Data, frame: Data) -> Data? {
        #if canImport(Darwin)
        guard let connectDecryptCiphertextFn,
              let freeFn,
              key.count == 32 else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = key.withUnsafeBytes { keyBuffer -> Int32 in
            guard let keyBase = keyBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return frame.withUnsafeBytes { frameBuffer -> Int32 in
                guard let frameBase = frameBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return connectDecryptCiphertextFn(
                    keyBase,
                    frameBase,
                    UInt(frame.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }


    func encodeEnvelopeSignRequestTx(sequence: UInt64, txBytes: Data) -> Data? {
        #if canImport(Darwin)
        guard let encodeEnvelopeSignRequestTxFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = txBytes.withUnsafeBytes { txBuffer -> Int32 in
            let txBase = txBuffer.bindMemory(to: UInt8.self).baseAddress
            return encodeEnvelopeSignRequestTxFn(
                sequence,
                txBase,
                UInt(txBytes.count),
                &outPtr,
                &outLen
            )
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeEnvelopeSignRequestRaw(sequence: UInt64, domainTag: String, bytes: Data) -> Data? {
        #if canImport(Darwin)
        guard let encodeEnvelopeSignRequestRawFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let tagData = Data(domainTag.utf8)
        let status = tagData.withUnsafeBytes { tagBuffer -> Int32 in
            let tagBase = tagBuffer.bindMemory(to: UInt8.self).baseAddress
            return bytes.withUnsafeBytes { bytesBuffer -> Int32 in
                let bytesBase = bytesBuffer.bindMemory(to: UInt8.self).baseAddress
                return encodeEnvelopeSignRequestRawFn(
                    sequence,
                    tagBase,
                    UInt(tagData.count),
                    bytesBase,
                    UInt(bytes.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeEnvelopeSignResultOk(sequence: UInt64, algorithm: String?, signature: Data) -> Data? {
        #if canImport(Darwin)
        guard signature.count > 0 else { return nil }
        guard let freeFn else { return nil }
        let normalizedAlgorithm: String?
        if let algorithm {
            guard let normalized = ConnectWalletSignatureAlgorithm.normalize(algorithm) else { return nil }
            normalizedAlgorithm = normalized
        } else {
            normalizedAlgorithm = nil
        }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status: Int32
        if let normalizedAlgorithm,
           let encodeEnvelopeSignResultOkWithAlgFn {
            status = normalizedAlgorithm.withCString { algPtr in
                signature.withUnsafeBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                    return encodeEnvelopeSignResultOkWithAlgFn(
                        sequence,
                        algPtr,
                        UInt(normalizedAlgorithm.utf8.count),
                        sigBase,
                        UInt(signature.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        } else if let encodeEnvelopeSignResultOkFn {
            status = signature.withUnsafeBytes { sigBuffer -> Int32 in
                guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return encodeEnvelopeSignResultOkFn(
                    sequence,
                    sigBase,
                    UInt(signature.count),
                    &outPtr,
                    &outLen
                )
            }
        } else {
            return nil
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeEnvelopeSignResultErr(sequence: UInt64, code: String, message: String) -> Data? {
        #if canImport(Darwin)
        guard let encodeEnvelopeSignResultErrFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let codeData = Data(code.utf8)
        let messageData = Data(message.utf8)
        let status = codeData.withUnsafeBytes { codeBuffer -> Int32 in
            let codeBase = codeBuffer.bindMemory(to: UInt8.self).baseAddress
            return messageData.withUnsafeBytes { msgBuffer -> Int32 in
                let msgBase = msgBuffer.bindMemory(to: UInt8.self).baseAddress
                return encodeEnvelopeSignResultErrFn(
                    sequence,
                    codeBase,
                    UInt(codeData.count),
                    msgBase,
                    UInt(messageData.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeEnvelopeControlClose(sequence: UInt64,
                                    who: ConnectRole,
                                    code: UInt16,
                                    reason: String?,
                                    retryable: Bool) -> Data? {
        #if canImport(Darwin)
        guard let encodeEnvelopeControlCloseFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let whoRaw: UInt8 = who == .app ? 0 : 1
        let retryRaw: UInt8 = retryable ? 1 : 0
        let status: Int32
        if let reason {
            let reasonData = Data(reason.utf8)
            status = reasonData.withUnsafeBytes { reasonBuffer -> Int32 in
                let reasonBase = reasonBuffer.bindMemory(to: UInt8.self).baseAddress
                return encodeEnvelopeControlCloseFn(
                    sequence,
                    whoRaw,
                    code,
                    reasonBase,
                    UInt(reasonData.count),
                    retryRaw,
                    &outPtr,
                    &outLen
                )
            }
        } else {
            status = encodeEnvelopeControlCloseFn(
                sequence,
                whoRaw,
                code,
                nil,
                0,
                retryRaw,
                &outPtr,
                &outLen
            )
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeEnvelopeControlReject(sequence: UInt64,
                                     code: UInt16,
                                     codeID: String,
                                     reason: String) -> Data? {
        #if canImport(Darwin)
        guard let encodeEnvelopeControlRejectFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let codeData = Data(codeID.utf8)
        let reasonData = Data(reason.utf8)
        let status = codeData.withUnsafeBytes { codeBuffer -> Int32 in
            let codeBase = codeBuffer.bindMemory(to: UInt8.self).baseAddress
            return reasonData.withUnsafeBytes { reasonBuffer -> Int32 in
                let reasonBase = reasonBuffer.bindMemory(to: UInt8.self).baseAddress
                return encodeEnvelopeControlRejectFn(
                    sequence,
                    code,
                    codeBase,
                    UInt(codeData.count),
                    reasonBase,
                    UInt(reasonData.count),
                    &outPtr,
                    &outLen
                )
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func encodeConfidentialPayload(ephemeralPublicKey: Data,
                                   nonce: Data,
                                   ciphertext: Data) -> Data? {
        #if canImport(Darwin)
        guard let encodeConfidentialPayloadFn, let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = ephemeralPublicKey.withUnsafeBytes { ep in
            nonce.withUnsafeBytes { np in
                ciphertext.withUnsafeBytes { cp in
                    encodeConfidentialPayloadFn(
                        ep.bindMemory(to: UInt8.self).baseAddress, UInt(ep.count),
                        np.bindMemory(to: UInt8.self).baseAddress, UInt(nonce.count),
                        cp.bindMemory(to: UInt8.self).baseAddress, UInt(ciphertext.count),
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func decodeEnvelopeKind(_ data: Data) -> (sequence: UInt64, kind: UInt16)? {
        #if canImport(Darwin)
        guard let decodeEnvelopeKindFn else { return nil }
        var sequence: UInt64 = 0
        var kind: UInt16 = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeEnvelopeKindFn(base, UInt(data.count), &sequence, &kind)
        }
        guard status == 0 else { return nil }
        return (sequence, kind)
        #else
        return nil
        #endif
    }

    func decodeEnvelopeJSON(_ data: Data) -> Data? {
        #if canImport(Darwin)
        guard let decodeEnvelopeJSONFn,
              let freeFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeEnvelopeJSONFn(base, UInt(data.count), &outPtr, &outLen)
        }
        guard status == 0, let outPtr else {
            if status == 0, let outPtr { freeFn(outPtr) }
            return nil
        }
        return takeData(pointer: outPtr, length: outLen)
        #else
        return nil
        #endif
    }

    func sorafsLocalFetch(planJSON: String,
                          providersJSON: String,
                          optionsJSON: String?) -> SorafsLocalFetchOutput? {
        #if canImport(Darwin)
        guard let sorafsLocalFetchFn else { return nil }

        guard let planData = planJSON.data(using: .utf8),
              let providersData = providersJSON.data(using: .utf8) else {
            return nil
        }
        let optionsData = optionsJSON?.data(using: .utf8) ?? Data()

        var outPayloadPtr: UnsafeMutablePointer<UInt8>? = nil
        var outPayloadLen: CUnsignedLong = 0
        var outReportPtr: UnsafeMutablePointer<UInt8>? = nil
        var outReportLen: CUnsignedLong = 0

        var status: Int32 = 0
        planData.withUnsafeBytes { planBuffer in
            providersData.withUnsafeBytes { providerBuffer in
                optionsData.withUnsafeBytes { optionsBuffer in
                    status = sorafsLocalFetchFn(
                        planBuffer.bindMemory(to: CChar.self).baseAddress,
                        CUnsignedLong(planBuffer.count),
                        providerBuffer.bindMemory(to: CChar.self).baseAddress,
                        CUnsignedLong(providerBuffer.count),
                        optionsBuffer.bindMemory(to: CChar.self).baseAddress,
                        CUnsignedLong(optionsBuffer.count),
                        &outPayloadPtr,
                        &outPayloadLen,
                        &outReportPtr,
                        &outReportLen
                    )
                }
            }
        }

        if status != 0 {
            if let ptr = outPayloadPtr {
                if let freeFn {
                    freeFn(ptr)
                } else {
                    Darwin.free(ptr)
                }
            }
            if let ptr = outReportPtr {
                if let freeFn {
                    freeFn(ptr)
                } else {
                    Darwin.free(ptr)
                }
            }
            return nil
        }

        guard let payload = takeData(pointer: outPayloadPtr, length: UInt(outPayloadLen)),
              let report = takeString(pointer: outReportPtr, length: UInt(outReportLen)) else {
            return nil
        }

        return SorafsLocalFetchOutput(payload: payload, reportJSON: report)
        #else
        return nil
        #endif
    }

    func sorafsReferenceValidateOrderbook(kind: UInt32,
                                          payload: Data,
                                          label: String,
                                          generatedAtUnix: UInt64) -> String? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceValidateOrderbookFn,
              let labelData = label.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(payload) { payloadPtr, payloadLen in
            withDataPointer(labelData) { labelPtr, labelLen in
                function(kind, payloadPtr, payloadLen, labelPtr, labelLen, generatedAtUnix, &outPtr, &outLen)
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeString(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceSignOrderbook(kind: UInt32,
                                      payload: Data,
                                      privateKey: Data) -> Data? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceSignOrderbookFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(payload) { payloadPtr, payloadLen in
            withDataPointer(privateKey) { keyPtr, keyLen in
                function(kind, payloadPtr, payloadLen, keyPtr, keyLen, &outPtr, &outLen)
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeData(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceBuildSignedOrderbookOrderRequest(
        fields: NativeSorafsOrderbookOrderRequestFields,
        privateKey: Data
    ) -> Data? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceBuildOrderbookOrderRequestFn else { return nil }
        guard let priceData = fields.pricePerGibMicroXor.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(fields.orderId) { orderIdPtr, orderIdLen in
            withDataPointer(priceData) { pricePtr, priceLen in
                withDataPointer(fields.ownerAccount) { ownerPtr, ownerLen in
                    withDataPointer(privateKey) { keyPtr, keyLen in
                        function(
                            orderIdPtr,
                            orderIdLen,
                            fields.side,
                            fields.tier,
                            pricePtr,
                            priceLen,
                            fields.quantityGib,
                            fields.remainingGib,
                            ownerPtr,
                            ownerLen,
                            fields.expiryUnix,
                            fields.nonce,
                            fields.makerFeeBps,
                            fields.takerFeeBps,
                            keyPtr,
                            keyLen,
                            &outPtr,
                            &outLen
                        )
                    }
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeData(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceBuildSignedOrderbookOrderCancel(
        fields: NativeSorafsOrderbookOrderCancelFields,
        privateKey: Data
    ) -> Data? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceBuildOrderbookOrderCancelFn else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(fields.orderId) { orderIdPtr, orderIdLen in
            withDataPointer(fields.ownerAccount) { ownerPtr, ownerLen in
                withDataPointer(privateKey) { keyPtr, keyLen in
                    function(
                        orderIdPtr,
                        orderIdLen,
                        ownerPtr,
                        ownerLen,
                        fields.reason,
                        fields.nonce,
                        keyPtr,
                        keyLen,
                        &outPtr,
                        &outLen
                    )
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeData(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceBuildSignedOrderbookSettlementReceipt(
        fields: NativeSorafsOrderbookSettlementReceiptFields,
        privateKey: Data
    ) -> Data? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceBuildOrderbookSettlementReceiptFn else { return nil }
        guard let debitData = fields.xorDebitedMicroXor.data(using: .utf8),
              let creditData = fields.providerCreditMicroXor.data(using: .utf8),
              let feeData = fields.feeAmountMicroXor.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(fields.receiptId) { receiptPtr, receiptLen in
            withDataPointer(fields.channelId) { channelPtr, channelLen in
                withDataPointer(fields.tradeId) { tradePtr, tradeLen in
                    withDataPointer(fields.chunkHash) { chunkPtr, chunkLen in
                        withDataPointer(debitData) { debitPtr, debitLen in
                            withDataPointer(creditData) { creditPtr, creditLen in
                                withDataPointer(feeData) { feePtr, feeLen in
                                    withDataPointer(privateKey) { keyPtr, keyLen in
                                        function(
                                            receiptPtr,
                                            receiptLen,
                                            channelPtr,
                                            channelLen,
                                            tradePtr,
                                            tradeLen,
                                            fields.rangeStart,
                                            fields.rangeEnd,
                                            chunkPtr,
                                            chunkLen,
                                            fields.bytesDelivered,
                                            debitPtr,
                                            debitLen,
                                            creditPtr,
                                            creditLen,
                                            feePtr,
                                            feeLen,
                                            fields.issuedAtUnix,
                                            keyPtr,
                                            keyLen,
                                            &outPtr,
                                            &outLen
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeData(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceValidatePdpPayload(kind: UInt32,
                                           payload: Data,
                                           label: String,
                                           generatedAtUnix: UInt64) -> String? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceValidatePdpPayloadFn,
              let labelData = label.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(payload) { payloadPtr, payloadLen in
            withDataPointer(labelData) { labelPtr, labelLen in
                function(kind, payloadPtr, payloadLen, labelPtr, labelLen, generatedAtUnix, &outPtr, &outLen)
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeString(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceValidatePdpCommitmentChallenge(commitment: Data,
                                                       commitmentLabel: String,
                                                       challenge: Data,
                                                       challengeLabel: String,
                                                       generatedAtUnix: UInt64) -> String? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceValidatePdpCommitmentChallengeFn,
              let commitmentLabelData = commitmentLabel.data(using: .utf8),
              let challengeLabelData = challengeLabel.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(commitment) { commitmentPtr, commitmentLen in
            withDataPointer(commitmentLabelData) { commitmentLabelPtr, commitmentLabelLen in
                withDataPointer(challenge) { challengePtr, challengeLen in
                    withDataPointer(challengeLabelData) { challengeLabelPtr, challengeLabelLen in
                        function(
                            commitmentPtr, commitmentLen,
                            commitmentLabelPtr, commitmentLabelLen,
                            challengePtr, challengeLen,
                            challengeLabelPtr, challengeLabelLen,
                            generatedAtUnix,
                            &outPtr, &outLen
                        )
                    }
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeString(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceValidatePdpChallengeProof(challenge: Data,
                                                  challengeLabel: String,
                                                  proof: Data,
                                                  proofLabel: String,
                                                  generatedAtUnix: UInt64) -> String? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceValidatePdpChallengeProofFn,
              let challengeLabelData = challengeLabel.data(using: .utf8),
              let proofLabelData = proofLabel.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(challenge) { challengePtr, challengeLen in
            withDataPointer(challengeLabelData) { challengeLabelPtr, challengeLabelLen in
                withDataPointer(proof) { proofPtr, proofLen in
                    withDataPointer(proofLabelData) { proofLabelPtr, proofLabelLen in
                        function(
                            challengePtr, challengeLen,
                            challengeLabelPtr, challengeLabelLen,
                            proofPtr, proofLen,
                            proofLabelPtr, proofLabelLen,
                            generatedAtUnix,
                            &outPtr, &outLen
                        )
                    }
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeString(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func sorafsReferenceValidatePdpBundle(commitment: Data,
                                          commitmentLabel: String,
                                          challenge: Data,
                                          challengeLabel: String,
                                          proof: Data,
                                          proofLabel: String,
                                          generatedAtUnix: UInt64) -> String? {
        #if canImport(Darwin)
        guard let function = sorafsReferenceValidatePdpBundleFn,
              let commitmentLabelData = commitmentLabel.data(using: .utf8),
              let challengeLabelData = challengeLabel.data(using: .utf8),
              let proofLabelData = proofLabel.data(using: .utf8) else { return nil }
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: CUnsignedLong = 0
        let status = withDataPointer(commitment) { commitmentPtr, commitmentLen in
            withDataPointer(commitmentLabelData) { commitmentLabelPtr, commitmentLabelLen in
                withDataPointer(challenge) { challengePtr, challengeLen in
                    withDataPointer(challengeLabelData) { challengeLabelPtr, challengeLabelLen in
                        withDataPointer(proof) { proofPtr, proofLen in
                            withDataPointer(proofLabelData) { proofLabelPtr, proofLabelLen in
                                function(
                                    commitmentPtr, commitmentLen,
                                    commitmentLabelPtr, commitmentLabelLen,
                                    challengePtr, challengeLen,
                                    challengeLabelPtr, challengeLabelLen,
                                    proofPtr, proofLen,
                                    proofLabelPtr, proofLabelLen,
                                    generatedAtUnix,
                                    &outPtr, &outLen
                                )
                            }
                        }
                    }
                }
            }
        }
        guard status == 0 else {
            if let outPtr {
                if let freeFn { freeFn(outPtr) } else { Darwin.free(outPtr) }
            }
            return nil
        }
        return takeString(pointer: outPtr, length: UInt(outLen))
        #else
        return nil
        #endif
    }

    func encodeConnectFrame(_ frame: ConnectFrame) -> Data? {
        #if canImport(Darwin)
        guard isConnectCodecAvailable else { return nil }
        switch frame.kind {
        case .control(let control):
            switch control {
            case .open(let open):
                return encodeControlOpenFrame(frame: frame, open: open)
            case .approve(let approve):
                return encodeControlApproveFrame(frame: frame, approve: approve)
            case .reject(let reject):
                return encodeControlRejectFrame(frame: frame, reject: reject)
            case .close(let close):
                return encodeControlCloseFrame(frame: frame, close: close)
            case .ping(let ping):
                return encodeControlPingFrame(frame: frame, ping: ping)
            case .pong(let pong):
                return encodeControlPongFrame(frame: frame, pong: pong)
            case .serverEvent:
                return nil
            }
        case .ciphertext(let ciphertext):
            return encodeCiphertextFrame(frame: frame, ciphertext: ciphertext)
        }
        #else
        return nil
        #endif
    }

    func decodeConnectFrame(_ data: Data) -> ConnectFrame? {
        #if canImport(Darwin)
        guard isConnectCodecAvailable else { return nil }
        var sessionBytes = [UInt8](repeating: 0, count: 32)
        var dirRaw: UInt8 = 0
        var sequence: UInt64 = 0
        var kind: UInt16 = 0
        guard let decodeControlKindFn else { return nil }
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlKindFn(base, UInt(data.count), &sessionBytes, &dirRaw, &sequence, &kind)
        }
        guard status == 0 else { return nil }
        let sessionID = Data(sessionBytes)
        let direction: ConnectDirection = dirRaw == 0 ? .appToWallet : .walletToApp
        switch kind {
        case 1:
            return decodeControlOpenFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 2:
            return decodeControlApproveFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 100:
            return decodeCiphertextFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 3:
            return decodeControlRejectFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 4:
            return decodeControlCloseFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 5:
            return decodeControlPingFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        case 6:
            return decodeControlPongFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence)
        default:
            return nil
        }
        #else
        return nil
        #endif
    }

    #if canImport(Darwin)
    private func encodeControlOpenFrame(frame: ConnectFrame, open: ConnectOpen) -> Data? {
        guard let encodeControlOpenFn,
              let freeFn,
              frame.sessionID.count == 32,
              open.appPublicKey.count == 32
        else { return nil }

        let permissionsData = ConnectCodec.encodePermissionsJSON(open.permissions)
        let appMetadataData = ConnectCodec.encodeAppMetadataJSON(open.appMetadata)
        var result: Data?
        let status = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return open.appPublicKey.withUnsafeBytes { pkBuffer -> Int32 in
                guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -2 }
                return withOptionalBytes(appMetadataData) { metaPtr, metaLen in
                    withOptionalBytes(permissionsData) { permsPtr, permsLen in
                        open.constraints.chainID.withCString { chainPtr in
                            var outPtr: UnsafeMutablePointer<UInt8>? = nil
                            var outLen: UInt = 0
                            let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
                            let status = encodeControlOpenFn(
                                sidBase,
                                dirRaw,
                                frame.sequence,
                                pkBase,
                                UInt(open.appPublicKey.count),
                                metaPtr,
                                metaLen,
                                chainPtr,
                                permsPtr,
                                permsLen,
                                &outPtr,
                                &outLen
                            )
                            if status == 0, let outPtr {
                                result = Data(bytes: outPtr, count: Int(outLen))
                                freeFn(outPtr)
                            }
                            return status
                        }
                    }
                }
            }
        }
        if status != 0 {
            result = nil
        }
        return result
    }

    private func encodeControlApproveFrame(frame: ConnectFrame, approve: ConnectApprove) -> Data? {
        guard let freeFn,
              frame.sessionID.count == 32,
              approve.walletPublicKey.count == 32 else { return nil }
        let permissionsData = ConnectCodec.encodePermissionsJSON(approve.permissions)
        let proofData = ConnectCodec.encodeProofJSON(approve.proof)
        let signature = approve.walletSignature.signature
        let normalizedAlgorithm = ConnectWalletSignatureAlgorithm.normalize(approve.walletSignature.algorithm)
        guard !signature.isEmpty else { return nil }

        var result: Data?
        let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
        let signatureStatus = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return approve.walletPublicKey.withUnsafeBytes { pkBuffer -> Int32 in
                guard let pkBase = pkBuffer.bindMemory(to: UInt8.self).baseAddress else { return -2 }
                return signature.withUnsafeBytes { sigBuffer -> Int32 in
                    guard let sigBase = sigBuffer.bindMemory(to: UInt8.self).baseAddress else { return -3 }
                    if let encodeControlApproveWithAlgFn,
                       let normalizedAlgorithm,
                       let algorithmData = normalizedAlgorithm.data(using: .utf8) {
                        let status = withOptionalBytes(permissionsData) { permsRaw, permsLen in
                            withOptionalBytes(proofData) { proofRaw, proofLen in
                                algorithmData.withUnsafeBytes { algBuffer -> Int32 in
                                    guard let algRaw = algBuffer.bindMemory(to: UInt8.self).baseAddress else { return -5 }
                                    var outPtr: UnsafeMutablePointer<UInt8>? = nil
                                    var outLen: UInt = 0
                                    let permsPtr = permsRaw.map { UnsafeRawPointer($0).assumingMemoryBound(to: CChar.self) }
                                    let proofPtr = proofRaw.map { UnsafeRawPointer($0).assumingMemoryBound(to: CChar.self) }
                                    let algPtr = UnsafeRawPointer(algRaw).assumingMemoryBound(to: CChar.self)
                                    let status = approve.accountID.withCString { accountPtr in
                                        encodeControlApproveWithAlgFn(
                                            sidBase,
                                            dirRaw,
                                            frame.sequence,
                                            pkBase,
                                            accountPtr,
                                            UInt(approve.accountID.utf8.count),
                                            permsPtr,
                                            permsLen,
                                            proofPtr,
                                            proofLen,
                                            algPtr,
                                            UInt(algorithmData.count),
                                            sigBase,
                                            UInt(signature.count),
                                            &outPtr,
                                            &outLen
                                        )
                                    }
                                    if status == 0, let outPtr {
                                        result = Data(bytes: outPtr, count: Int(outLen))
                                        freeFn(outPtr)
                                    }
                                    return status
                                }
                            }
                        }
                        return status
                    } else {
                        guard let encodeControlApproveFn,
                              normalizedAlgorithm == "ed25519",
                              signature.count == 64 else { return -4 }
                        let status = withOptionalBytes(permissionsData) { permsPtr, permsLen in
                            withOptionalBytes(proofData) { proofPtr, proofLen in
                                var outPtr: UnsafeMutablePointer<UInt8>? = nil
                                var outLen: UInt = 0
                                let status = approve.accountID.withCString { accountPtr in
                                    encodeControlApproveFn(
                                        sidBase,
                                        dirRaw,
                                        frame.sequence,
                                        pkBase,
                                        UInt(approve.walletPublicKey.count),
                                        accountPtr,
                                        permsPtr,
                                        permsLen,
                                        proofPtr,
                                        proofLen,
                                        sigBase,
                                        UInt(signature.count),
                                        &outPtr,
                                        &outLen
                                    )
                                }
                                if status == 0, let outPtr {
                                    result = Data(bytes: outPtr, count: Int(outLen))
                                    freeFn(outPtr)
                                }
                                return status
                            }
                        }
                        return status
                    }
                }
            }
        }
        if signatureStatus != 0 {
            result = nil
        }
        return result
    }

    private func encodeControlRejectFrame(frame: ConnectFrame, reject: ConnectReject) -> Data? {
        guard let encodeControlRejectFn,
              let freeFn,
              frame.sessionID.count == 32 else { return nil }

        var result: Data?
        let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
        let status = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return reject.codeID.withCString { codeIdPtr in
                withOptionalCString(reject.reason.isEmpty ? nil : reject.reason) { reasonPtr, reasonLen in
                    var outPtr: UnsafeMutablePointer<UInt8>? = nil
                    var outLen: UInt = 0
                    let status = encodeControlRejectFn(
                        sidBase,
                        dirRaw,
                        frame.sequence,
                        reject.code,
                        codeIdPtr,
                        UInt(reject.codeID.utf8.count),
                        reasonPtr,
                        reasonLen,
                        &outPtr,
                        &outLen
                    )
                    if status == 0, let outPtr {
                        result = Data(bytes: outPtr, count: Int(outLen))
                        freeFn(outPtr)
                    }
                    return status
                }
            }
        }
        if status != 0 {
            result = nil
        }
        return result
    }

    private func encodeControlCloseFrame(frame: ConnectFrame, close: ConnectClose) -> Data? {
        guard let encodeControlCloseFn,
              let freeFn,
              frame.sessionID.count == 32 else { return nil }

        var result: Data?
        let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
        let whoRaw: UInt8 = close.role == .app ? 0 : 1
        let status = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return withOptionalCString(close.reason) { reasonPtr, reasonLen in
                var outPtr: UnsafeMutablePointer<UInt8>? = nil
                var outLen: UInt = 0
                let status = encodeControlCloseFn(
                    sidBase,
                    dirRaw,
                    frame.sequence,
                    whoRaw,
                    close.code,
                    reasonPtr,
                    reasonLen,
                    close.retryable ? 1 : 0,
                    &outPtr,
                    &outLen
                )
                if status == 0, let outPtr {
                    result = Data(bytes: outPtr, count: Int(outLen))
                    freeFn(outPtr)
                }
                return status
            }
        }
        if status != 0 {
            result = nil
        }
        return result
    }

    private func encodeControlPingFrame(frame: ConnectFrame, nonce: UInt64, fn pointer: EncodeControlPingFn?) -> Data? {
        guard let pointer,
              let freeFn,
              frame.sessionID.count == 32 else { return nil }
        var result: Data?
        let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
        let status = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            var outPtr: UnsafeMutablePointer<UInt8>? = nil
            var outLen: UInt = 0
            let status = pointer(
                sidBase,
                dirRaw,
                frame.sequence,
                nonce,
                &outPtr,
                &outLen
            )
            if status == 0, let outPtr {
                result = Data(bytes: outPtr, count: Int(outLen))
                freeFn(outPtr)
            }
            return status
        }
        if status != 0 {
            result = nil
        }
        return result
    }

    private func encodeControlPingFrame(frame: ConnectFrame, ping: ConnectPing) -> Data? {
        encodeControlPingFrame(frame: frame, nonce: ping.nonce, fn: encodeControlPingFn)
    }

    private func encodeControlPongFrame(frame: ConnectFrame, pong: ConnectPong) -> Data? {
        encodeControlPingFrame(frame: frame, nonce: pong.nonce, fn: encodeControlPongFn)
    }

    private func encodeCiphertextFrame(frame: ConnectFrame, ciphertext: ConnectCiphertext) -> Data? {
        guard let encodeCiphertextFrameFn,
              let freeFn,
              frame.sessionID.count == 32 else { return nil }
        var result: Data?
        let dirRaw: UInt8 = frame.direction == .appToWallet ? 0 : 1
        let status = frame.sessionID.withUnsafeBytes { sidBuffer -> Int32 in
            guard let sidBase = sidBuffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return ciphertext.payload.withUnsafeBytes { payloadBuffer -> Int32 in
                let payloadBase = payloadBuffer.bindMemory(to: UInt8.self).baseAddress
                var outPtr: UnsafeMutablePointer<UInt8>? = nil
                var outLen: UInt = 0
                let status = encodeCiphertextFrameFn(
                    sidBase,
                    dirRaw,
                    frame.sequence,
                    payloadBase,
                    UInt(ciphertext.payload.count),
                    &outPtr,
                    &outLen
                )
                if status == 0, let outPtr {
                    result = Data(bytes: outPtr, count: Int(outLen))
                    freeFn(outPtr)
                }
                return status
            }
        }
        if status != 0 {
            result = nil
        }
        return result
    }

    private func decodeControlOpenFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        guard let decodeControlOpenPubFn,
              let decodeControlOpenChainIdFn,
              let decodeControlOpenPermissionsFn else { return nil }
        var publicKeyBytes = [UInt8](repeating: 0, count: 32)
        let pubStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlOpenPubFn(base, UInt(data.count), &publicKeyBytes)
        }
        guard pubStatus == 0 else { return nil }

        var chainPtr: UnsafeMutablePointer<UInt8>? = nil
        var chainLen: UInt = 0
        let chainStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlOpenChainIdFn(base, UInt(data.count), &chainPtr, &chainLen)
        }
        guard chainStatus == 0, let chainID = takeString(pointer: chainPtr, length: chainLen) else { return nil }

        var permissionsPtr: UnsafeMutablePointer<UInt8>? = nil
        var permissionsLen: UInt = 0
        let permsStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlOpenPermissionsFn(base, UInt(data.count), &permissionsPtr, &permissionsLen)
        }
        guard permsStatus == 0 else { return nil }
        let permissionsData = takeData(pointer: permissionsPtr, length: permissionsLen)
        let permissions: ConnectPermissions?
        if let permissionsData {
            do {
                permissions = try ConnectCodec.decodePermissionsJSON(permissionsData)
            } catch {
                return nil
            }
        } else {
            permissions = nil
        }

        var appMetadata: ConnectAppMetadata?
        if let decodeControlOpenAppMetadataFn {
            var metadataPtr: UnsafeMutablePointer<UInt8>? = nil
            var metadataLen: UInt = 0
            let metadataStatus = data.withUnsafeBytes { buffer -> Int32 in
                guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
                return decodeControlOpenAppMetadataFn(base, UInt(data.count), &metadataPtr, &metadataLen)
            }
            if metadataStatus == 0 {
                let metadataData = takeData(pointer: metadataPtr, length: metadataLen)
                if let metadataData {
                    do {
                        appMetadata = try ConnectCodec.decodeAppMetadataJSON(metadataData)
                    } catch {
                        return nil
                    }
                } else {
                    appMetadata = nil
                }
            }
        }

        let open = ConnectOpen(
            appPublicKey: Data(publicKeyBytes),
            appMetadata: appMetadata,
            constraints: ConnectConstraints(chainID: chainID),
            permissions: permissions
        )
        return ConnectFrame(sessionID: sessionID, direction: direction, sequence: sequence, kind: .control(.open(open)))
    }

    private func decodeControlApproveFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        guard let decodeControlApprovePubFn,
              let decodeControlApproveAccountFn,
              let decodeControlApprovePermissionsFn,
              let decodeControlApproveProofFn,
              let decodeControlApproveSigFn,
              let decodeControlApproveSigAlgFn else { return nil }

        var walletPkBytes = [UInt8](repeating: 0, count: 32)
        let pubStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApprovePubFn(base, UInt(data.count), &walletPkBytes)
        }
        guard pubStatus == 0 else { return nil }

        var accountPtr: UnsafeMutablePointer<UInt8>? = nil
        var accountLen: UInt = 0
        let accountStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApproveAccountFn(base, UInt(data.count), &accountPtr, &accountLen)
        }
        guard accountStatus == 0, let accountID = takeString(pointer: accountPtr, length: accountLen) else { return nil }

        var permissionsPtr: UnsafeMutablePointer<UInt8>? = nil
        var permissionsLen: UInt = 0
        let permsStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApprovePermissionsFn(base, UInt(data.count), &permissionsPtr, &permissionsLen)
        }
        guard permsStatus == 0 else { return nil }
        let permissionsData = takeData(pointer: permissionsPtr, length: permissionsLen)
        let permissions: ConnectPermissions?
        if let permissionsData {
            do {
                permissions = try ConnectCodec.decodePermissionsJSON(permissionsData)
            } catch {
                return nil
            }
        } else {
            permissions = nil
        }

        var proofPtr: UnsafeMutablePointer<UInt8>? = nil
        var proofLen: UInt = 0
        let proofStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApproveProofFn(base, UInt(data.count), &proofPtr, &proofLen)
        }
        guard proofStatus == 0 else { return nil }
        let proofData = takeData(pointer: proofPtr, length: proofLen)
        let proof: ConnectSignInProof?
        if let proofData {
            do {
                proof = try ConnectCodec.decodeProofJSON(proofData)
            } catch {
                return nil
            }
        } else {
            proof = nil
        }

        var signatureBytes = [UInt8](repeating: 0, count: 64)
        let sigStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApproveSigFn(base, UInt(data.count), &signatureBytes)
        }
        guard sigStatus == 0 else { return nil }

        var algPtr: UnsafeMutablePointer<CChar>? = nil
        var algLen: UInt = 0
        let algStatus = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlApproveSigAlgFn(base, UInt(data.count), &algPtr, &algLen)
        }
        guard algStatus == 0, let algorithm = takeCString(pointer: algPtr, length: algLen) else { return nil }

        let signature = ConnectWalletSignature(algorithm: algorithm, signature: Data(signatureBytes))
        let approve = ConnectApprove(walletPublicKey: Data(walletPkBytes),
                                     accountID: accountID,
                                     permissions: permissions,
                                     proof: proof,
                                     walletSignature: signature,
                                     walletMetadata: nil)
        return ConnectFrame(sessionID: sessionID, direction: direction, sequence: sequence, kind: .control(.approve(approve)))
    }

    private func decodeCiphertextFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        guard let decodeCiphertextFrameFn else { return nil }
        var outSession = [UInt8](repeating: 0, count: 32)
        var outDir: UInt8 = 0
        var outSeq: UInt64 = 0
        var payloadPtr: UnsafeMutablePointer<UInt8>? = nil
        var payloadLen: UInt = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeCiphertextFrameFn(base, UInt(data.count), &outSession, &outDir, &outSeq, &payloadPtr, &payloadLen)
        }
        guard status == 0 else { return nil }
        let payload = takeData(pointer: payloadPtr, length: payloadLen) ?? Data()
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: direction,
                                 sequence: sequence,
                                 kind: .ciphertext(ConnectCiphertext(payload: payload)))
        return frame
    }

    private func decodeControlRejectFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        guard let decodeControlRejectFn else { return nil }
        var code: UInt16 = 0
        var codeIdPtr: UnsafeMutablePointer<UInt8>? = nil
        var codeIdLen: UInt = 0
        var reasonPtr: UnsafeMutablePointer<UInt8>? = nil
        var reasonLen: UInt = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlRejectFn(base, UInt(data.count), &code, &codeIdPtr, &codeIdLen, &reasonPtr, &reasonLen)
        }
        guard status == 0,
              let codeID = takeString(pointer: codeIdPtr, length: codeIdLen) else { return nil }
        let reason = takeString(pointer: reasonPtr, length: reasonLen) ?? ""
        let reject = ConnectReject(code: code, codeID: codeID, reason: reason)
        return ConnectFrame(sessionID: sessionID,
                             direction: direction,
                             sequence: sequence,
                             kind: .control(.reject(reject)))
    }

    private func decodeControlCloseFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        guard let decodeControlCloseFn else { return nil }
        var roleRaw: UInt8 = 0
        var code: UInt16 = 0
        var retryRaw: UInt8 = 0
        var reasonPtr: UnsafeMutablePointer<UInt8>? = nil
        var reasonLen: UInt = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return decodeControlCloseFn(base, UInt(data.count), &roleRaw, &code, &retryRaw, &reasonPtr, &reasonLen)
        }
        guard status == 0 else { return nil }
        let reasonString = takeString(pointer: reasonPtr, length: reasonLen)
        let role: ConnectRole = roleRaw == 0 ? .app : .wallet
        let close = ConnectClose(role: role,
                                 code: code,
                                 reason: (reasonString?.isEmpty ?? true) ? nil : reasonString,
                                 retryable: retryRaw != 0)
        return ConnectFrame(sessionID: sessionID,
                             direction: direction,
                             sequence: sequence,
                             kind: .control(.close(close)))
    }

    private func decodeControlPingFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64, fn pointer: DecodeControlPingFn?, builder: (UInt64) -> ConnectControl) -> ConnectFrame? {
        guard let pointer else { return nil }
        var nonce: UInt64 = 0
        let status = data.withUnsafeBytes { buffer -> Int32 in
            guard let base = buffer.bindMemory(to: UInt8.self).baseAddress else { return -1 }
            return pointer(base, UInt(data.count), &nonce)
        }
        guard status == 0 else { return nil }
        return ConnectFrame(sessionID: sessionID,
                             direction: direction,
                             sequence: sequence,
                             kind: .control(builder(nonce)))
    }

    private func decodeControlPingFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        decodeControlPingFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence, fn: decodeControlPingFn) {
            .ping(ConnectPing(nonce: $0))
        }
    }

    private func decodeControlPongFrame(data: Data, sessionID: Data, direction: ConnectDirection, sequence: UInt64) -> ConnectFrame? {
        decodeControlPingFrame(data: data, sessionID: sessionID, direction: direction, sequence: sequence, fn: decodeControlPongFn) {
            .pong(ConnectPong(nonce: $0))
        }
    }
    #endif
}

extension NoritoNativeBridge {
    func daProofSummary(
        manifest: Data,
        payload: Data,
        options: ToriiDaProofSummaryOptions
    ) -> Data? {
        #if canImport(Darwin)
        guard let daProofSummaryFn = daProofSummaryFn,
              let freeFn = freeFn else {
            return nil
        }
        guard !manifest.isEmpty, !payload.isEmpty else {
            return nil
        }

        var normalizedLeafIndexes = [CUnsignedLong]()
        normalizedLeafIndexes.reserveCapacity(options.leafIndexes.count)
        for index in options.leafIndexes {
            guard index >= 0 else { return nil }
            normalizedLeafIndexes.append(CUnsignedLong(index))
        }

        var outputPtr: UnsafeMutablePointer<UInt8>? = nil
        var outputLen: CUnsignedLong = 0
        let status = manifest.withUnsafeBytes { manifestBuffer -> Int32 in
            guard let manifestPtr = manifestBuffer.bindMemory(to: UInt8.self).baseAddress else {
                return -1
            }
            return payload.withUnsafeBytes { payloadBuffer -> Int32 in
                guard let payloadPtr = payloadBuffer.bindMemory(to: UInt8.self).baseAddress else {
                    return -1
                }
                return normalizedLeafIndexes.withUnsafeBufferPointer { indexesBuffer -> Int32 in
                    let indexesPtr = indexesBuffer.baseAddress
                    let indexesLen = CUnsignedLong(indexesBuffer.count)
                    return daProofSummaryFn(
                        manifestPtr,
                        CUnsignedLong(manifest.count),
                        payloadPtr,
                        CUnsignedLong(payload.count),
                        CUnsignedLong(max(options.sampleCount, 0)),
                        options.sampleSeed,
                        indexesPtr,
                        indexesLen,
                        &outputPtr,
                        &outputLen
                    )
                }
            }
        }

        guard status == 0, let summaryPtr = outputPtr else {
            if let outputPtr {
                freeFn(outputPtr)
            }
            return nil
        }
        let data = Data(bytes: summaryPtr, count: Int(outputLen))
        freeFn(summaryPtr)
        return data
        #else
        return nil
        #endif
    }

}

extension NoritoNativeBridge {
    static var bridgeRequirementHint: String {
        BridgePolicyHint.message
    }

    static func bridgeUnavailableMessage(_ prefix: String) -> String {
        BridgePolicyHint.unavailableMessage(prefix)
    }
}
