package org.hyperledger.iroha.sdk.offline

import java.io.File

/** Canonical index of one file in the authenticated 34-role release inventory. */
enum class OfflineCashArtifactRoleV1 {
    PARAMS_EQ,
    PARAMS_EP,
    STATE_PK_EQ,
    STATE_VK_EQ,
    STATE_PK_EP,
    STATE_VK_EP,
    GUARD_USE_PK_EQ,
    GUARD_USE_VK_EQ,
    GUARD_USE_PK_EP,
    GUARD_USE_VK_EP,
    PLATFORM_BIND_PK_EQ,
    PLATFORM_BIND_VK_EQ,
    PLATFORM_BIND_PK_EP,
    PLATFORM_BIND_VK_EP,
    ANDROID_KEY_CERT_PK_EQ,
    ANDROID_KEY_CERT_VK_EQ,
    ANDROID_KEY_CERT_PK_EP,
    ANDROID_KEY_CERT_VK_EP,
    GUARD_BUNDLE_PK_EQ,
    GUARD_BUNDLE_VK_EQ,
    GUARD_BUNDLE_PK_EP,
    GUARD_BUNDLE_VK_EP,
    P256_V3_PK_EQ,
    P256_V3_VK_EQ,
    P256_V3_PK_EP,
    P256_V3_VK_EP,
    STATE_LEAF_PK_EQ,
    STATE_LEAF_VK_EQ,
    STATE_LEAF_PK_EP,
    STATE_LEAF_VK_EP,
    GUARD_BUNDLE_LEAF_PK_EQ,
    GUARD_BUNDLE_LEAF_VK_EQ,
    GUARD_BUNDLE_LEAF_PK_EP,
    GUARD_BUNDLE_LEAF_VK_EP,
}

/** Streams, authenticates, and atomically installs one complete Offline Cash V1 release. */
object OfflineCashArtifactSetInstallerV1 {
    const val REQUIRED_ARTIFACT_COUNT: Int = 34
    const val MAXIMUM_CHUNK_BYTES: Int = 1_048_576

    @JvmStatic
    fun install(
        manifest: ByteArray,
        expectedManifestSHA256: ByteArray,
        validationReceipt: ByteArray,
        trustedPolicy: ByteArray,
        releaseAttestation: ByteArray,
        artifactFiles: Map<OfflineCashArtifactRoleV1, File>,
    ) {
        require(expectedManifestSHA256.size == 32 && expectedManifestSHA256.any { it != 0.toByte() }) {
            "expectedManifestSHA256 must be a non-zero 32-byte digest"
        }
        require(
            artifactFiles.size == REQUIRED_ARTIFACT_COUNT &&
                OfflineCashArtifactRoleV1.entries.all(artifactFiles::containsKey),
        ) { "Offline Cash V1 install requires the exact canonical 34-role inventory" }

        val handles = ArrayList<Long>(REQUIRED_ARTIFACT_COUNT)
        var installed = false
        try {
            for (role in OfflineCashArtifactRoleV1.entries) {
                val file = checkNotNull(artifactFiles[role])
                require(file.isFile) { "Offline Cash V1 artifact is not a regular file: $role" }
                val handle = OfflineCashNativeV1.artifactBegin(manifest, role.ordinal)
                check(handle > 0) { "native Offline Cash V1 artifact begin returned no handle" }
                handles += handle
                file.inputStream().buffered(MAXIMUM_CHUNK_BYTES).use { input ->
                    val buffer = ByteArray(MAXIMUM_CHUNK_BYTES)
                    try {
                        while (true) {
                            val count = input.read(buffer)
                            if (count < 0) break
                            if (count == 0) continue
                            if (count == buffer.size) {
                                OfflineCashNativeV1.artifactWrite(handle, buffer)
                            } else {
                                val tail = buffer.copyOf(count)
                                try {
                                    OfflineCashNativeV1.artifactWrite(handle, tail)
                                } finally {
                                    tail.fill(0)
                                }
                            }
                        }
                    } finally {
                        buffer.fill(0)
                    }
                }
                OfflineCashNativeV1.artifactFinalize(handle)
            }
            OfflineCashNativeV1.artifactSetInstall(
                manifest,
                expectedManifestSHA256,
                validationReceipt,
                trustedPolicy,
                releaseAttestation,
                handles.toLongArray(),
            )
            installed = true
        } finally {
            if (!installed) {
                handles.forEach { handle -> runCatching { OfflineCashNativeV1.artifactCancel(handle) } }
            }
        }
    }

    @JvmStatic
    fun uninstall(expectedReleaseId: ByteArray, expectedManifestSHA256: ByteArray) {
        require(expectedReleaseId.size == 32 && expectedReleaseId.any { it != 0.toByte() }) {
            "expectedReleaseId must be a non-zero 32-byte digest"
        }
        require(expectedManifestSHA256.size == 32 && expectedManifestSHA256.any { it != 0.toByte() }) {
            "expectedManifestSHA256 must be a non-zero 32-byte digest"
        }
        OfflineCashNativeV1.artifactSetUninstall(expectedReleaseId, expectedManifestSHA256)
    }
}
