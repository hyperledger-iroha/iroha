package org.hyperledger.iroha.sdk.privacy

import java.util.Collections

/**
 * Canonical first-release privacy capability bridge.
 *
 * The bridge exposes only the authoritative typed Norito capability snapshot. Proof operations
 * live behind protocol-specific APIs and cannot be selected with free-form strings here.
 */
class PrivacyNativeBridge private constructor() {
    enum class ValidationStatusV1(val code: Int) {
        VALID(0),
        NULL_POINTER(1),
        EMPTY(2),
        ARCHIVE_TOO_LARGE(3),
        DECODE_RESOURCE_LIMIT(4),
        SCHEMA_MISMATCH(5),
        NON_CANONICAL(6),
        MALFORMED_ARCHIVE(7),
        INVALID_SNAPSHOT(8),
    }

    enum class ProtocolIdV1(val canonicalLabel: String) {
        ZK_ACE_PQ_AUTHORIZATION_V0("zk-ace-pq-authorization-v0"),
        ANONYMOUS_PGC_K_OUT_OF_N_V1("anonymous-pgc-k-out-of-n-v1"),
        VERANGE_TRANSPARENT_RANGE_V1("verange-transparent-range-v1"),
        IROHA_ZK_AMS_V1("iroha-zk-ams-v1"),
        VEGA_EXISTING_CREDENTIAL_ZK_V0("vega-existing-credential-zk-v0"),
        IROHA_ZK_X509_STARK_P256_V0("iroha-zk-x509-stark-p256-v0"),
        IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0("iroha-jindo-polynomial-commitment-v0"),
        IROHA_BOOTLE_LANTERN_ANONCRED_V1("iroha-bootle-lantern-anoncred-v1"),
        ORCHARD_HALO2_ACTIONS_V1("orchard-halo2-actions-v1"),
        MONERO_FCMP_PLUS_PLUS_V1("monero-fcmp-plus-plus-v1"),
        IROHA_IVM_PRIVATE_NOTE_STARK_V1("iroha-ivm-private-note-stark-v1"),
        PQ_MASP_STARK_V0("pq-masp-stark-v0"),
        ;

        companion object {
            /**
             * Parse one exact canonical label.
             *
             * Aliases, retired identifiers, whitespace, and case changes are rejected.
             */
            @JvmStatic
            fun fromCanonicalLabel(label: String): ProtocolIdV1 =
                values().firstOrNull { it.canonicalLabel == label }
                    ?: throw IllegalArgumentException("unknown canonical privacy protocol id")
        }
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 21
        const val PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: Int = 256 * 1024
        private const val LIBRARY_NAME: String = "connect_norito_bridge"
        private val PROTOCOLS: List<ProtocolIdV1> =
            Collections.unmodifiableList(ProtocolIdV1.values().toList())
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        /** All twelve protocol identities in exact wire order. */
        @JvmStatic
        fun protocolsV1(): List<ProtocolIdV1> = PROTOCOLS

        /** Returns the authoritative `PrivacyCapabilitySnapshotV1` Norito archive. */
        @JvmStatic
        fun capabilitiesArchiveV1(): ByteArray {
            check(nativeAvailable) { "native privacy capability bridge is unavailable" }
            val archive =
                try {
                    nativeCapabilities()
                } catch (error: RuntimeException) {
                    throw IllegalStateException("native privacy capability query failed", error)
                } catch (error: LinkageError) {
                    throw IllegalStateException("native privacy capability query failed", error)
                }
            return requireCapabilityArchive(archive)
        }

        internal fun requireCapabilityArchive(archive: ByteArray?): ByteArray {
            check(
                archive != null &&
                    archive.isNotEmpty() &&
                    archive.size <= PRIVACY_NATIVE_ARCHIVE_MAX_BYTES,
            ) {
                "invalid privacy capability archive length"
            }
            requireNotNull(archive)
            check(nativeValidateCapabilities(archive) == ValidationStatusV1.VALID.code) {
                "invalid typed privacy capability archive"
            }
            return archive.copyOf()
        }

        private fun loadLibrary(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION &&
                    requireCapabilityArchive(nativeCapabilities()).isNotEmpty()
            } catch (_: RuntimeException) {
                false
            } catch (_: LinkageError) {
                false
            }

        @JvmStatic private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic private external fun nativeCapabilities(): ByteArray?

        @JvmStatic private external fun nativeValidateCapabilities(archive: ByteArray): Int
    }
}
