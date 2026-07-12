package org.hyperledger.iroha.sdk.offline

/**
 * ABI-19 Kagemusha V3 artifact streaming and capability bridge.
 *
 * The first-release JVM SDK intentionally exposes no offline-spend lifecycle. Swift owns the
 * supported wallet lifecycle; this surface only installs the opaque six-file proof artifact set.
 */
class KagemushaRecursiveSpendProver private constructor() {
    companion object {
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 19
        const val ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        val ARTIFACT_FILES: List<String> = listOf(
            "transition-eq.parameters.krv3",
            "transition-eq.proving-key.krv3",
            "transition-eq.verifying-key.krv3",
            "state-ep.parameters.krv3",
            "state-ep.proving-key.krv3",
            "state-ep.verifying-key.krv3",
        )
        const val ARTIFACT_COUNT: Int = 6
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val artifactBridgeAvailable = loadArtifactBridge()
        private val proofBackendAvailable = loadProofBackendCapability()

        @JvmStatic
        fun isArtifactStreamingAvailable(): Boolean = artifactBridgeAvailable

        @JvmStatic
        fun isProofBackendAvailable(): Boolean = proofBackendAvailable

        @JvmStatic
        fun beginArtifactIngest(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): ArtifactIngest {
            requireArtifactBridge()
            val manifest = requireManifest(manifestNorito)
            val manifestDigest = requireDigest(manifestSha256, "manifestSha256")
            val artifactDigest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest)
            check(handle > 0) { "native Kagemusha artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): ArtifactInstallSession {
            requireArtifactBridge()
            return ArtifactInstallSession(
                requireManifest(manifestNorito),
                requireDigest(manifestSha256, "manifestSha256"),
            )
        }

        internal fun isExactBridgeAbi(abiVersion: Int): Boolean =
            abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        internal fun detectExactNativeAvailability(
            loadLibrary: () -> Unit,
            abiVersion: () -> Int,
            symbolProbe: () -> Boolean,
        ): Boolean = try {
            loadLibrary()
            isExactBridgeAbi(abiVersion()) && symbolProbe()
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        } catch (_: RuntimeException) {
            false
        }

        private fun loadArtifactBridge(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                symbolProbe = {
                    expectIllegalArgumentProbe {
                        nativeArtifactBeginV3(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    }
                },
            )

        private fun loadProofBackendCapability(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                symbolProbe = { nativePastaCycleV3BackendAvailable() },
            )

        private fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            true
        }

        private fun requireArtifactBridge() {
            check(artifactBridgeAvailable) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION artifact streaming is unavailable"
            }
        }

        private fun requireManifest(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_MANIFEST_BYTES) {
                "manifestNorito must contain 1..$MAX_MANIFEST_BYTES bytes"
            }
            return value.copyOf()
        }

        private fun requireDigest(value: ByteArray?, name: String): ByteArray {
            require(value != null && value.size == 32) { "$name must contain exactly 32 bytes" }
            require(value.any { it.toInt() != 0 }) { "$name must be non-zero" }
            return value.copyOf()
        }

        private fun requireChunk(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty()) { "chunk must not be empty" }
            return value.copyOf()
        }

        private fun hex(digest: ByteArray): String = buildString(64) {
            for (octet in digest) append("%02x".format(octet.toInt() and 0xff))
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativePastaCycleV3BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): Long

        @JvmStatic
        private external fun nativeArtifactWriteV3(handle: Long, chunk: ByteArray)

        @JvmStatic
        private external fun nativeArtifactFinalizeV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactCancelV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactSetInstallV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            artifactHandles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeArtifactSetUninstallV3(manifestSha256: ByteArray)
    }

    /** Owns one native artifact spool until installation or cancellation. */
    class ArtifactIngest internal constructor(initialHandle: Long) : AutoCloseable {
        private var handle = initialHandle
        private var finalized = false
        private var installClaimed = false

        @Synchronized
        fun write(chunk: ByteArray) {
            requireOpen(allowFinalized = false)
            nativeArtifactWriteV3(handle, requireChunk(chunk))
        }

        @Synchronized
        fun finish() {
            requireOpen(allowFinalized = false)
            nativeArtifactFinalizeV3(handle)
            finalized = true
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "artifact ingest is being installed" }
            nativeArtifactCancelV3(handle)
            handle = 0
            finalized = false
        }

        @Synchronized
        internal fun claimFinalizedHandle(): Long {
            check(handle != 0L && finalized && !installClaimed) {
                "artifact ingest is not installable"
            }
            installClaimed = true
            return handle
        }

        @Synchronized
        internal fun releaseInstallClaim(expectedHandle: Long) {
            if (handle == expectedHandle) installClaimed = false
        }

        @Synchronized
        internal fun relinquishInstalledHandle(expectedHandle: Long) {
            check(handle == expectedHandle && finalized && installClaimed) {
                "artifact install ownership mismatch"
            }
            handle = 0
            finalized = false
            installClaimed = false
        }

        private fun requireOpen(allowFinalized: Boolean) {
            check(handle != 0L) { "artifact ingest is closed" }
            check(allowFinalized || !finalized) { "artifact ingest is already finalized" }
            check(!installClaimed) { "artifact ingest is being installed" }
        }
    }

    /** Coordinates one atomic six-artifact generation install. */
    class ArtifactInstallSession internal constructor(
        manifest: ByteArray,
        manifestDigest: ByteArray,
    ) : AutoCloseable {
        private val manifestNorito = manifest.copyOf()
        private val manifestSha256 = manifestDigest.copyOf()
        private val artifacts = linkedMapOf<String, ArtifactIngest>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(expectedArtifactSha256: ByteArray): ArtifactIngest {
            requirePending()
            check(artifacts.size < ARTIFACT_COUNT) { "artifact set already has six streams" }
            val digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val key = hex(digest)
            require(!artifacts.containsKey(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also { artifacts[key] = it }
        }

        @Synchronized
        fun install() {
            requirePending()
            check(artifacts.size == ARTIFACT_COUNT) {
                "artifact set must contain exactly six streams"
            }
            val ordered = artifacts.values.toList()
            val handles = LongArray(ARTIFACT_COUNT)
            var claimed = 0
            try {
                while (claimed < ordered.size) {
                    handles[claimed] = ordered[claimed].claimFinalizedHandle()
                    claimed += 1
                }
                nativeArtifactSetInstallV3(manifestNorito, manifestSha256, handles)
            } catch (failure: Throwable) {
                repeat(claimed) { index ->
                    ordered[index].releaseInstallClaim(handles[index])
                }
                throw failure
            }
            ordered.forEachIndexed { index, ingest ->
                ingest.relinquishInstalledHandle(handles[index])
            }
            artifacts.clear()
            installed = true
        }

        @Synchronized
        fun isInstalled(): Boolean =
            !closed && nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256)

        @Synchronized
        fun uninstall() {
            if (!installed || closed) return
            nativeArtifactSetUninstallV3(manifestSha256)
            installed = false
            closed = true
        }

        @Synchronized
        override fun close() {
            if (closed || installed) return
            var firstFailure: RuntimeException? = null
            artifacts.values.forEach { ingest ->
                try {
                    ingest.close()
                } catch (failure: RuntimeException) {
                    if (firstFailure == null) firstFailure = failure
                }
            }
            artifacts.clear()
            closed = true
            firstFailure?.let { throw it }
        }

        private fun requirePending() {
            check(!closed && !installed) { "artifact install session is not pending" }
        }
    }
}
