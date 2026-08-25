import Foundation

private typealias OfflineCashArtifactBeginFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UInt8, UnsafeMutablePointer<UInt64>?
) -> Int32
private typealias OfflineCashArtifactWriteFn = @convention(c) (
    UInt64, UnsafePointer<UInt8>?, CUnsignedLong
) -> Int32
private typealias OfflineCashArtifactHandleFn = @convention(c) (UInt64) -> Int32
private typealias OfflineCashArtifactSetInstallFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt64>?, CUnsignedLong
) -> Int32
private typealias OfflineCashArtifactSetUninstallFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong
) -> Int32

extension NoritoNativeBridge {
    fileprivate func offlineCashArtifactBeginV1(manifest: Data, role: UInt8) throws -> UInt64 {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_artifact_begin_v1",
            as: OfflineCashArtifactBeginFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var handle: UInt64 = 0
        let status = manifest.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                role,
                &handle
            )
        }
        guard status == 0, handle != 0 else {
            throw OfflineCashV1Error.invalidStateTransition("artifact_begin_rejected")
        }
        return handle
    }

    fileprivate func offlineCashArtifactWriteV1(handle: UInt64, chunk: Data) throws {
        guard !chunk.isEmpty,
              let function = resolveKagemushaV2Symbol(
                "connect_norito_offline_cash_artifact_write_v1",
                as: OfflineCashArtifactWriteFn.self
              ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        let status = chunk.withUnsafeBytes { buffer in
            function(
                handle,
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count)
            )
        }
        guard status == 0 else {
            throw OfflineCashV1Error.invalidStateTransition("artifact_write_rejected")
        }
    }

    fileprivate func offlineCashArtifactFinalizeV1(handle: UInt64) throws {
        try offlineCashArtifactHandleV1(
            symbol: "connect_norito_offline_cash_artifact_finalize_v1",
            handle: handle,
            failure: "artifact_finalize_rejected"
        )
    }

    fileprivate func offlineCashArtifactCancelV1(handle: UInt64) throws {
        try offlineCashArtifactHandleV1(
            symbol: "connect_norito_offline_cash_artifact_cancel_v1",
            handle: handle,
            failure: "artifact_cancel_rejected"
        )
    }

    private func offlineCashArtifactHandleV1(
        symbol: String,
        handle: UInt64,
        failure: String
    ) throws {
        guard let function = resolveKagemushaV2Symbol(
            symbol,
            as: OfflineCashArtifactHandleFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        guard function(handle) == 0 else {
            throw OfflineCashV1Error.invalidStateTransition(failure)
        }
    }

    fileprivate func offlineCashArtifactSetInstallV1(
        manifest: Data,
        expectedManifestSHA256: Data,
        validationReceipt: Data,
        trustedPolicy: Data,
        releaseAttestation: Data,
        handles: [UInt64]
    ) throws {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_artifact_set_install_v1",
            as: OfflineCashArtifactSetInstallFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        let status = manifest.withUnsafeBytes { manifestBuffer in
            expectedManifestSHA256.withUnsafeBytes { digestBuffer in
                validationReceipt.withUnsafeBytes { receiptBuffer in
                    trustedPolicy.withUnsafeBytes { policyBuffer in
                        releaseAttestation.withUnsafeBytes { attestationBuffer in
                            handles.withUnsafeBufferPointer { handlesBuffer in
                                function(
                                    manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                    CUnsignedLong(manifestBuffer.count),
                                    digestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                    CUnsignedLong(digestBuffer.count),
                                    receiptBuffer.bindMemory(to: UInt8.self).baseAddress,
                                    CUnsignedLong(receiptBuffer.count),
                                    policyBuffer.bindMemory(to: UInt8.self).baseAddress,
                                    CUnsignedLong(policyBuffer.count),
                                    attestationBuffer.bindMemory(to: UInt8.self).baseAddress,
                                    CUnsignedLong(attestationBuffer.count),
                                    handlesBuffer.baseAddress,
                                    CUnsignedLong(handlesBuffer.count)
                                )
                            }
                        }
                    }
                }
            }
        }
        guard status == 0 else {
            throw OfflineCashV1Error.invalidStateTransition("artifact_set_install_rejected")
        }
    }

    fileprivate func offlineCashArtifactSetUninstallV1(
        expectedReleaseId: Data,
        expectedManifestSHA256: Data
    ) throws {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_artifact_set_uninstall_v1",
            as: OfflineCashArtifactSetUninstallFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        let status = expectedReleaseId.withUnsafeBytes { releaseBuffer in
            expectedManifestSHA256.withUnsafeBytes { manifestBuffer in
                function(
                    releaseBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(releaseBuffer.count),
                    manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(manifestBuffer.count)
                )
            }
        }
        guard status == 0 else {
            throw OfflineCashV1Error.invalidStateTransition("artifact_set_uninstall_rejected")
        }
    }
}

/// Canonical index of one file in the authenticated 34-role release inventory.
public enum OfflineCashArtifactRoleV1: UInt8, CaseIterable, Sendable {
    case paramsEq
    case paramsEp
    case statePkEq
    case stateVkEq
    case statePkEp
    case stateVkEp
    case guardUsePkEq
    case guardUseVkEq
    case guardUsePkEp
    case guardUseVkEp
    case platformBindPkEq
    case platformBindVkEq
    case platformBindPkEp
    case platformBindVkEp
    case androidKeyCertPkEq
    case androidKeyCertVkEq
    case androidKeyCertPkEp
    case androidKeyCertVkEp
    case guardBundlePkEq
    case guardBundleVkEq
    case guardBundlePkEp
    case guardBundleVkEp
    case p256V3PkEq
    case p256V3VkEq
    case p256V3PkEp
    case p256V3VkEp
    case stateLeafPkEq
    case stateLeafVkEq
    case stateLeafPkEp
    case stateLeafVkEp
    case guardBundleLeafPkEq
    case guardBundleLeafVkEq
    case guardBundleLeafPkEp
    case guardBundleLeafVkEp
}

/// Streams, authenticates, and atomically installs one complete Offline Cash V1 release.
public enum OfflineCashArtifactSetInstallerV1 {
    public static let requiredArtifactCount = 34
    public static let maximumChunkBytes = 1_048_576

    public static func install(
        manifest: Data,
        expectedManifestSHA256: Data,
        validationReceipt: Data,
        trustedPolicy: Data,
        releaseAttestation: Data,
        artifactFiles: [OfflineCashArtifactRoleV1: URL]
    ) throws {
        guard expectedManifestSHA256.count == 32,
              expectedManifestSHA256.contains(where: { $0 != 0 }),
              artifactFiles.count == requiredArtifactCount,
              OfflineCashArtifactRoleV1.allCases.allSatisfy({ artifactFiles[$0] != nil }) else {
            throw OfflineCashV1Error.invalidDigest("artifact_release_inventory")
        }

        var handles: [UInt64] = []
        var installed = false
        defer {
            if !installed {
                for handle in handles {
                    try? NoritoNativeBridge.shared.offlineCashArtifactCancelV1(handle: handle)
                }
            }
        }

        for role in OfflineCashArtifactRoleV1.allCases {
            guard let fileURL = artifactFiles[role], fileURL.isFileURL else {
                throw OfflineCashV1Error.invalidStateTransition("artifact_file_missing")
            }
            let handle = try NoritoNativeBridge.shared.offlineCashArtifactBeginV1(
                manifest: manifest,
                role: role.rawValue
            )
            handles.append(handle)
            do {
                let file = try FileHandle(forReadingFrom: fileURL)
                defer { try? file.close() }
                while let chunk = try file.read(upToCount: maximumChunkBytes), !chunk.isEmpty {
                    try NoritoNativeBridge.shared.offlineCashArtifactWriteV1(
                        handle: handle,
                        chunk: chunk
                    )
                }
            }
            try NoritoNativeBridge.shared.offlineCashArtifactFinalizeV1(handle: handle)
        }

        try NoritoNativeBridge.shared.offlineCashArtifactSetInstallV1(
            manifest: manifest,
            expectedManifestSHA256: expectedManifestSHA256,
            validationReceipt: validationReceipt,
            trustedPolicy: trustedPolicy,
            releaseAttestation: releaseAttestation,
            handles: handles
        )
        installed = true
    }

    public static func uninstall(
        expectedReleaseId: Data,
        expectedManifestSHA256: Data
    ) throws {
        guard expectedReleaseId.count == 32,
              expectedReleaseId.contains(where: { $0 != 0 }),
              expectedManifestSHA256.count == 32,
              expectedManifestSHA256.contains(where: { $0 != 0 }) else {
            throw OfflineCashV1Error.invalidDigest("artifact_release_identity")
        }
        try NoritoNativeBridge.shared.offlineCashArtifactSetUninstallV1(
            expectedReleaseId: expectedReleaseId,
            expectedManifestSHA256: expectedManifestSHA256
        )
    }
}
