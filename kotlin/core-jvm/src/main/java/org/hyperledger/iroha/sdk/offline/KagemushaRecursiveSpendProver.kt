package org.hyperledger.iroha.sdk.offline

/** Exact first-release ABI-18 Kagemusha recursive-spend bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        RECURSIVE_SPEND("recursive_spend_v2"),
    }

    companion object {
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 18
        const val ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        const val MODE: String = "recursive_spend_v2"
        const val PROOF_BACKEND: String = "halo2/ipa-pasta-cycle-v1"
        const val TRANSCRIPT_PROFILE: String = "kagemusha-pasta-cycle-poseidon-v1"
        const val TRANSITION_CIRCUIT_ID: String =
            "kagemusha-recursive-spend-transition-eq-v1"
        const val STATE_CIRCUIT_ID: String = "kagemusha-recursive-spend-state-ep-v1"
        const val MAX_PROOF_BYTES: Int = 4_096
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val nativeAvailable = loadExactNativeBridge()
        private val artifactIngestAvailable = probeArtifactIngest()
        private val proofBackendAvailable = probeProofBackend()

        /** True only when the loaded bridge ABI is exactly 18. */
        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        /** Exact ABI-18 streaming artifact-ingest surface. */
        @JvmStatic
        fun isArtifactIngestAvailable(): Boolean = artifactIngestAvailable

        /** Audited proof capability; false in the current bridge build. */
        @JvmStatic
        fun isProofBackendAvailable(): Boolean = proofBackendAvailable

        @JvmStatic
        fun preferredMode(): Mode? = preferredMode(proofBackendAvailable)

        @JvmStatic
        fun preferredMode(proofBackendAvailable: Boolean): Mode? =
            if (proofBackendAvailable) Mode.RECURSIVE_SPEND else null

        /** Begin one manifest-bound, bounded stream for a complete KRV3 package. */
        @JvmStatic
        fun beginArtifactIngest(
            manifestNorito: ByteArray?,
            manifestSha256: ByteArray?,
            artifactSha256: ByteArray?,
        ): ArtifactIngest {
            val manifest = requireManifest(manifestNorito)
            val manifestDigest = requireDigest(manifestSha256, "manifestSha256")
            val artifactDigest = requireDigest(artifactSha256, "artifactSha256")
            requireArtifactBridge()
            val handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest)
            check(handle > 0L) { "native Kagemusha artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        /** Begin one all-or-nothing six-artifact installation. */
        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray?,
            manifestSha256: ByteArray?,
        ): ArtifactInstallSession {
            val manifest = requireManifest(manifestNorito)
            val manifestDigest = requireDigest(manifestSha256, "manifestSha256")
            requireArtifactBridge()
            return ArtifactInstallSession(manifest, manifestDigest)
        }

        internal fun isExactBridgeAbi(abiVersion: Int): Boolean =
            abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        private fun requireManifest(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_MANIFEST_BYTES) {
                "manifestNorito must contain between 1 and $MAX_MANIFEST_BYTES bytes"
            }
            return value
        }

        private fun requireDigest(value: ByteArray?, name: String): ByteArray {
            require(value != null && value.size == 32) { "$name must be exactly 32 bytes" }
            require(value.any { it.toInt() != 0 }) { "$name must not be all zero" }
            return value
        }

        private fun requireArtifactBridge() {
            check(artifactIngestAvailable) {
                "$LIBRARY_NAME exact ABI-18 artifact ingest is not available in this runtime"
            }
        }

        private fun loadExactNativeBridge(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                isExactBridgeAbi(nativeBridgeAbiVersion())
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }

        private fun probeArtifactIngest(): Boolean {
            if (!nativeAvailable) return false
            return try {
                nativeArtifactBeginV3(byteArrayOf(0), byteArrayOf(1), byteArrayOf(1))
                false
            } catch (_: IllegalArgumentException) {
                true
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        private fun probeProofBackend(): Boolean {
            if (!nativeAvailable) return false
            return try {
                isExactBridgeAbi(nativeBridgeAbiVersion()) &&
                    nativePastaCycleV3BackendAvailable()
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativePastaCycleV3BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            artifactSha256: ByteArray,
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
            handles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeArtifactSetUninstallV3(manifestSha256: ByteArray)

        private fun ByteArray.toHexDigest(): String =
            joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }
    }

    /** Owns one native KRV3 spool until installation or close. */
    class ArtifactIngest internal constructor(initialHandle: Long) : AutoCloseable {
        private var handle = initialHandle
        private var finalized = false
        private var installClaimed = false

        @Synchronized
        fun write(chunk: ByteArray?) {
            requireMutable()
            require(chunk != null && chunk.isNotEmpty()) { "chunk must not be empty" }
            nativeArtifactWriteV3(handle, chunk)
        }

        @Synchronized
        fun finish() {
            requireMutable()
            nativeArtifactFinalizeV3(handle)
            finalized = true
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "Kagemusha artifact ingest is being installed" }
            val current = handle
            nativeArtifactCancelV3(current)
            handle = 0L
            finalized = false
        }

        @Synchronized
        internal fun claimFinalizedHandle(): Long {
            check(handle != 0L && finalized && !installClaimed) {
                "Kagemusha artifact ingest is not installable"
            }
            installClaimed = true
            return handle
        }

        @Synchronized
        internal fun releaseInstallClaim(expectedHandle: Long) {
            if (handle == expectedHandle && installClaimed) installClaimed = false
        }

        @Synchronized
        internal fun relinquishInstalledHandle(expectedHandle: Long) {
            check(handle == expectedHandle && finalized && installClaimed) {
                "Kagemusha artifact install ownership mismatch"
            }
            handle = 0L
            finalized = false
            installClaimed = false
        }

        private fun requireMutable() {
            check(handle != 0L) { "Kagemusha artifact ingest is closed" }
            check(!finalized) { "Kagemusha artifact ingest is already finalized" }
            check(!installClaimed) { "Kagemusha artifact ingest is being installed" }
        }
    }

    /** Coordinates one atomic six-artifact V3 generation install. */
    class ArtifactInstallSession internal constructor(
        manifestNorito: ByteArray,
        manifestSha256: ByteArray,
    ) : AutoCloseable {
        private val manifestNorito = manifestNorito.copyOf()
        private val manifestSha256 = manifestSha256.copyOf()
        private val artifacts = linkedMapOf<String, ArtifactIngest>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(expectedArtifactSha256: ByteArray?): ArtifactIngest {
            requirePending()
            check(artifacts.size < 6) { "artifact set already has six streams" }
            val digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val key = digest.toHexDigest()
            require(!artifacts.containsKey(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also { artifacts[key] = it }
        }

        /** Native failure consumes no handles and preserves the previous generation. */
        @Synchronized
        fun install() {
            requirePending()
            check(artifacts.size == 6) { "artifact set must contain exactly six streams" }
            val ordered = artifacts.values.toList()
            val handles = LongArray(6)
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
            ordered.forEachIndexed { index, artifact ->
                artifact.relinquishInstalledHandle(handles[index])
            }
            artifacts.clear()
            installed = true
        }

        @Synchronized
        fun isInstalled(): Boolean {
            if (closed && !installed) return false
            return nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256)
        }

        /** A digest guard prevents a stale session from removing a newer generation. */
        @Synchronized
        fun uninstall() {
            if (!installed || closed) return
            nativeArtifactSetUninstallV3(manifestSha256)
            installed = false
            closed = true
        }

        /** Cancels pending streams; an installed generation requires explicit uninstall. */
        @Synchronized
        override fun close() {
            if (closed || installed) return
            var firstFailure: RuntimeException? = null
            artifacts.values.forEach { artifact ->
                try {
                    artifact.close()
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
