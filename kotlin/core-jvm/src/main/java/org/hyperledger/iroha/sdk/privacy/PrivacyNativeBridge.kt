package org.hyperledger.iroha.sdk.privacy

import java.util.Collections

/**
 * Canonical first-release local privacy build-metadata bridge.
 *
 * The bridge exposes this binary's typed Norito compiled-profile catalog and the Rust-derived
 * byte-complete exact-12 fixture bundle. The catalog never establishes network activation or
 * readiness; callers must fetch a fresh authoritative PrivacyCapabilitySnapshotV1 from live
 * Torii before submitting a privacy proof.
 */
class PrivacyNativeBridge private constructor() {
    enum class CompiledProfileCatalogValidationStatusV1(val code: Int) {
        VALID(0),
        NULL_POINTER(1),
        EMPTY(2),
        ARCHIVE_TOO_LARGE(3),
        DECODE_RESOURCE_LIMIT(4),
        SCHEMA_MISMATCH(5),
        NON_CANONICAL(6),
        MALFORMED_ARCHIVE(7),
        INVALID_CATALOG(8),
    }

    /** Stable ABI-21 result of validating the Rust-derived exact-12 fixture bundle. */
    enum class Exact12FixtureValidationStatusV1(val code: Int) {
        VALID(0),
        NULL_POINTER(1),
        EMPTY(2),
        ARCHIVE_TOO_LARGE(3),
        DECODE_RESOURCE_LIMIT(4),
        SCHEMA_MISMATCH(5),
        NON_CANONICAL(6),
        MALFORMED_ARCHIVE(7),
        INVALID_BUNDLE(8),
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 22
        const val COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES: Int = 256 * 1024
        const val EXACT12_FIXTURE_BUNDLE_MAX_BYTES: Int = 2 * 1024 * 1024
        private const val LIBRARY_NAME: String = "connect_norito_bridge"
        private val PROTOCOLS: List<PrivacyProtocolIdV1> =
            Collections.unmodifiableList(PrivacyProtocolIdV1.values().toList())
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        /** All twelve protocol identities in exact wire order. */
        @JvmStatic
        fun protocolsV1(): List<PrivacyProtocolIdV1> = PROTOCOLS

        /** Returns this binary's canonical `PrivacyCompiledProfileCatalogV1` Norito archive. */
        @JvmStatic
        fun compiledProfileCatalogV1(): ByteArray {
            check(nativeAvailable) { "native privacy compiled-profile catalog is unavailable" }
            val archive =
                try {
                    nativeCompiledProfileCatalog()
                } catch (error: RuntimeException) {
                    throw IllegalStateException(
                        "native privacy compiled-profile catalog query failed",
                        error,
                    )
                } catch (error: LinkageError) {
                    throw IllegalStateException(
                        "native privacy compiled-profile catalog query failed",
                        error,
                    )
                }
            return requireCompiledProfileCatalog(archive)
        }

        /** Returns this binary's local catalog as the closed typed first-release model. */
        @JvmStatic
        fun compiledProfileCatalogTypedV1(): PrivacyCompiledProfileCatalogV1 =
            PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(compiledProfileCatalogV1())

        /** Validates bytes as the exact compiled-profile catalog of the loaded binary. */
        @JvmStatic
        fun validateCompiledProfileCatalogV1(
            archive: ByteArray?,
        ): CompiledProfileCatalogValidationStatusV1 {
            if (archive == null) return CompiledProfileCatalogValidationStatusV1.NULL_POINTER
            if (archive.isEmpty()) return CompiledProfileCatalogValidationStatusV1.EMPTY
            if (archive.size > COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES) {
                return CompiledProfileCatalogValidationStatusV1.ARCHIVE_TOO_LARGE
            }
            check(nativeAvailable) { "native privacy compiled-profile catalog is unavailable" }
            val code =
                try {
                    nativeValidateCompiledProfileCatalog(archive)
                } catch (error: RuntimeException) {
                    throw IllegalStateException(
                        "native privacy compiled-profile catalog validation failed",
                        error,
                    )
                } catch (error: LinkageError) {
                    throw IllegalStateException(
                        "native privacy compiled-profile catalog validation failed",
                        error,
                    )
                }
            return CompiledProfileCatalogValidationStatusV1.values().firstOrNull {
                it.code == code
            } ?: throw IllegalStateException(
                "native privacy compiled-profile catalog validation returned an unknown status",
            )
        }

        /** Returns canonical Rust-derived exact-12 bytes through signed-transaction and hash layers. */
        @JvmStatic
        fun exact12FixtureBundleV1(): ByteArray {
            check(nativeAvailable) { "native exact-12 privacy fixture bridge is unavailable" }
            val archive =
                try {
                    nativeExact12FixtureBundle()
                } catch (error: RuntimeException) {
                    throw IllegalStateException(
                        "native exact-12 privacy fixture query failed",
                        error,
                    )
                } catch (error: LinkageError) {
                    throw IllegalStateException(
                        "native exact-12 privacy fixture query failed",
                        error,
                    )
                }
            return requireExact12FixtureBundle(archive)
        }

        /**
         * Validates untrusted bytes against the canonical Rust-derived exact-12 fixture bundle.
         *
         * Null, empty, and oversized inputs are rejected before JNI copies any bytes.
         */
        @JvmStatic
        fun validateExact12FixtureBundleV1(
            archive: ByteArray?,
        ): Exact12FixtureValidationStatusV1 {
            if (archive == null) return Exact12FixtureValidationStatusV1.NULL_POINTER
            if (archive.isEmpty()) return Exact12FixtureValidationStatusV1.EMPTY
            if (archive.size > EXACT12_FIXTURE_BUNDLE_MAX_BYTES) {
                return Exact12FixtureValidationStatusV1.ARCHIVE_TOO_LARGE
            }
            check(nativeAvailable) { "native exact-12 privacy fixture bridge is unavailable" }
            val code =
                try {
                    nativeValidateExact12FixtureBundle(archive)
                } catch (error: RuntimeException) {
                    throw IllegalStateException(
                        "native exact-12 privacy fixture validation failed",
                        error,
                    )
                } catch (error: LinkageError) {
                    throw IllegalStateException(
                        "native exact-12 privacy fixture validation failed",
                        error,
                    )
                }
            return Exact12FixtureValidationStatusV1.values().firstOrNull { it.code == code }
                ?: throw IllegalStateException(
                    "native exact-12 privacy fixture validation returned an unknown status",
                )
        }

        internal fun requireCompiledProfileCatalog(archive: ByteArray?): ByteArray {
            check(
                archive != null &&
                    archive.isNotEmpty() &&
                    archive.size <= COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES,
            ) {
                "invalid privacy compiled-profile catalog length"
            }
            requireNotNull(archive)
            check(
                nativeValidateCompiledProfileCatalog(archive) ==
                    CompiledProfileCatalogValidationStatusV1.VALID.code,
            ) {
                "invalid typed privacy compiled-profile catalog"
            }
            val snapshot = archive.copyOf()
            PrivacyCompiledProfileCatalogCodecV1.decodeCanonical(snapshot)
            return snapshot
        }

        internal fun requireExact12FixtureBundle(archive: ByteArray?): ByteArray {
            check(
                archive != null &&
                    archive.isNotEmpty() &&
                    archive.size <= EXACT12_FIXTURE_BUNDLE_MAX_BYTES,
            ) {
                "invalid exact-12 privacy fixture bundle length"
            }
            requireNotNull(archive)
            check(
                nativeValidateExact12FixtureBundle(archive) ==
                    Exact12FixtureValidationStatusV1.VALID.code,
            ) {
                "invalid exact-12 privacy fixture bundle"
            }
            return archive.copyOf()
        }

        private fun loadLibrary(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION &&
                    requireCompiledProfileCatalog(nativeCompiledProfileCatalog()).isNotEmpty() &&
                    requireExact12FixtureBundle(nativeExact12FixtureBundle()).isNotEmpty()
            } catch (_: RuntimeException) {
                false
            } catch (_: LinkageError) {
                false
            }

        @JvmStatic private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic private external fun nativeCompiledProfileCatalog(): ByteArray?

        @JvmStatic private external fun nativeValidateCompiledProfileCatalog(
            archive: ByteArray,
        ): Int

        @JvmStatic private external fun nativeExact12FixtureBundle(): ByteArray?

        @JvmStatic private external fun nativeValidateExact12FixtureBundle(
            archive: ByteArray?,
        ): Int
    }
}
