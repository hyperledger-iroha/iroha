package org.hyperledger.iroha.sdk.offline

/** Native recursive Kagemusha spend ABI-6 bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        RECURSIVE_COMPACT_V1("recursive_compact_v1"),
        RECURSIVE_SPEND_V1("recursive_spend_v1"),
        CHECKED_PREFOLD_V1("checked_prefold_v1"),
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 6
        const val RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION: Int = 7
        const val RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-aggregation-v1"
        const val RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
            "kagemusha-recursive-compact-v1"
        const val RECURSIVE_AGGREGATION_PROOF_BACKEND =
            "halo2/ipa"
        const val RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-v1"
        const val RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-onehop-v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-append-v1"
        const val COMPACT_TOKEN_MAX_HOPS: Int = 64
        const val RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: Int = 64
        const val RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: Boolean = true
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: Int = 1
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: Int = 8 * 1024 * 1024
        const val RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: Int = 128
        const val NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile"
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val MALFORMED_NATIVE_PROBE_ARCHIVE = byteArrayOf(0x00)
        private val nativeAvailable: Boolean = loadLibrary()
        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun preferredMode(): Mode =
            preferredMode(
                recursiveCompactAvailable = KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable(),
                recursiveSpendAvailable = nativeAvailable,
            )

        @JvmStatic
        fun preferredMode(recursiveSpendAvailable: Boolean): Mode =
            preferredMode(
                recursiveCompactAvailable = false,
                recursiveSpendAvailable = recursiveSpendAvailable,
            )

        @JvmStatic
        @Suppress("UNUSED_PARAMETER")
        fun preferredMode(
            recursiveCompactAvailable: Boolean,
            recursiveSpendAvailable: Boolean,
        ): Mode =
            if (recursiveSpendAvailable) {
                Mode.RECURSIVE_SPEND_V1
            } else {
                Mode.CHECKED_PREFOLD_V1
            }

        @JvmStatic
        fun canRedeemWitnessless(circuitId: String?, hopCount: Int): Boolean {
            val hopCountSupported =
                hopCount >= 1 &&
                    hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
            val canonicalLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            val oneHopLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            val appendLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            return RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
                (canonicalLineage || oneHopLineage || appendLineage)
        }

        @JvmStatic
        fun isLineageProofCircuitId(circuitId: String?): Boolean =
            circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
                circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        @JvmStatic
        fun isLineageAppendOutputCircuitId(outputCircuitId: String?): Boolean =
            outputCircuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
                outputCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        @JvmStatic
        fun isSupportedLineageKeyArtifactOpeningLen(verifierOpeningLen: Int): Boolean =
            when (verifierOpeningLen) {
                2, 4, 8, 16, 32, 64, 128 -> true
                else -> false
            }

        @JvmStatic
        fun lineageKeyArtifactsForInit(
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: ByteArray,
            lineageProvingKeyArchive: ByteArray,
        ): LineageKeyArtifacts =
            lineageKeyArtifacts(
                RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                verifierOpeningLen,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )

        @JvmStatic
        fun lineageKeyArtifactsForAppend(
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: ByteArray,
            lineageProvingKeyArchive: ByteArray,
        ): LineageKeyArtifacts =
            lineageKeyArtifacts(
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                verifierOpeningLen,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )

        @JvmStatic
        fun validateLineageKeyArtifacts(artifacts: LineageKeyArtifacts): LineageKeyArtifacts {
            require(
                artifacts.proofCircuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
                    artifacts.proofCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ) {
                "proof_circuit_id"
            }
            require(isSupportedLineageKeyArtifactOpeningLen(artifacts.verifierOpeningLen)) {
                "verifier_opening_len"
            }
            require(artifacts.lineageVerifierKeyBackend == RECURSIVE_AGGREGATION_PROOF_BACKEND) {
                "lineage_verifier_key"
            }
            require(artifacts.lineageVerifierKey().isNotEmpty()) { "lineage_verifier_key" }
            require(artifacts.lineageProvingKeyArchive().isNotEmpty()) {
                "lineage_proving_key_archive"
            }
            return artifacts
        }

        @JvmStatic
        fun lineageKeyArtifacts(
            proofCircuitId: String,
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: ByteArray,
            lineageProvingKeyArchive: ByteArray,
        ): LineageKeyArtifacts =
            validateLineageKeyArtifacts(
                LineageKeyArtifacts(
                    proofCircuitId,
                    verifierOpeningLen,
                    lineageVerifierKeyBackend,
                    lineageVerifierKey,
                    lineageProvingKeyArchive,
                ),
            )

        @JvmStatic
        fun requiresLineageKeyArtifactsForInit(): Boolean = true

        @JvmStatic
        fun requiresLineageWitnessForRedeem(circuitId: String?, hopCount: Int): Boolean =
            !canRedeemWitnessless(circuitId, hopCount)

        @JvmStatic
        fun canAppendWitnesslessLineage(previousHopCount: Int): Boolean =
            RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
                previousHopCount >= 1 &&
                previousHopCount < RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1

        @JvmStatic
        fun normalizeAppendOutputCircuitId(outputCircuitId: String?): String =
            if (outputCircuitId.isNullOrEmpty()) {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            } else if (outputCircuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1) {
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            } else {
                outputCircuitId
            }

        @JvmStatic
        fun isSupportedAppendOutputCircuitId(outputCircuitId: String?): Boolean =
            when (normalizeAppendOutputCircuitId(outputCircuitId)) {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                -> true
                else -> false
            }

        @JvmStatic
        fun requiresLineageKeyArtifactsForAppendOutput(outputCircuitId: String?): Boolean =
            isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId))

        @JvmStatic
        fun isSupportedPreviousProofCircuitId(previousProofCircuitId: String?): Boolean =
            previousProofCircuitId == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                isLineageProofCircuitId(previousProofCircuitId)

        @JvmStatic
        fun requiresPreviousLineageVerifierRecordForAppend(previousProofCircuitId: String?): Boolean =
            isLineageProofCircuitId(previousProofCircuitId)

        @JvmStatic
        fun isSupportedAppendProofTransition(
            previousProofCircuitId: String?,
            outputCircuitId: String?,
        ): Boolean {
            val normalizedOutput = normalizeAppendOutputCircuitId(outputCircuitId)
            return previousProofCircuitId == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 &&
                normalizedOutput == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                isLineageProofCircuitId(previousProofCircuitId) &&
                    (
                        normalizedOutput == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                            normalizedOutput == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                    )
        }

        @JvmStatic
        fun preferredAppendOutputCircuitId(previousHopCount: Int): String =
            if (canAppendWitnesslessLineage(previousHopCount)) {
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            } else {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            }

        @JvmStatic
        fun canProveAppendOutputCircuitId(outputCircuitId: String?, previousHopCount: Int): Boolean {
            if (previousHopCount < 1) {
                return false
            }
            return when (normalizeAppendOutputCircuitId(outputCircuitId)) {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ->
                    previousHopCount < COMPACT_TOKEN_MAX_HOPS
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 ->
                    canAppendWitnesslessLineage(previousHopCount)
                else -> false
            }
        }

        @JvmStatic
        fun canSelectAppendOutputCircuitId(
            previousProofCircuitId: String?,
            outputCircuitId: String?,
            previousHopCount: Int,
        ): Boolean {
            if (!canProveAppendOutputCircuitId(outputCircuitId, previousHopCount)) {
                return false
            }
            if (!isSupportedPreviousProofCircuitId(previousProofCircuitId)) {
                return false
            }
            return isSupportedAppendProofTransition(previousProofCircuitId, outputCircuitId)
        }

        @JvmStatic
        fun requiresPreviousProofOpenEnvelopesForAppend(
            outputCircuitId: String?,
            previousHopCount: Int,
        ): Boolean =
            isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId)) &&
                previousHopCount >= 1

        @JvmStatic
        fun initSpend(requestArchive: ByteArray): ByteArray =
            call("init", requestArchive, ::nativeInitSpend)

        @JvmStatic
        fun appendSpend(requestArchive: ByteArray): ByteArray =
            call("append", requestArchive, ::nativeAppendSpend)

        @JvmStatic
        fun transitionProfileInit(requestArchive: ByteArray): ByteArray =
            call("transition profile init", requestArchive, ::nativeTransitionProfileInit)

        @JvmStatic
        fun transitionProfileAppend(requestArchive: ByteArray): ByteArray =
            call("transition profile append", requestArchive, ::nativeTransitionProfileAppend)

        @JvmStatic
        fun lineageAppendBoundary(profileArchive: ByteArray): ByteArray =
            callArchive(
                "lineage append boundary",
                "profileArchive",
                profileArchive,
                ::nativeLineageAppendBoundary,
            )

        @JvmStatic
        fun lineageWitnessFromInitResult(
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray =
            call(
                "lineage witness from init result",
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessFromInitResult,
            )

        @JvmStatic
        fun lineageWitnessAppendResult(
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray =
            call(
                "lineage witness append result",
                previousWitnessArchive,
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessAppendResult,
            )

        @JvmStatic
        fun verifySpend(requestArchive: ByteArray): ByteArray =
            call("verify", requestArchive, ::nativeVerifySpend)

        @JvmStatic
        fun redeemSpend(requestArchive: ByteArray): ByteArray =
            call("redeem", requestArchive, ::nativeRedeemSpend)

        private fun call(
            label: String,
            requestArchive: ByteArray,
            nativeCall: (ByteArray) -> ByteArray?,
        ): ByteArray =
            callArchive(label, "requestArchive", requestArchive, nativeCall)

        private fun callArchive(
            label: String,
            archiveName: String,
            archive: ByteArray,
            nativeCall: (ByteArray) -> ByteArray?,
        ): ByteArray {
            requireNativeInput(archive, archiveName)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(archive)
            return requireRecursiveSpendOutput(output, label)
        }

        private fun call(
            label: String,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
            nativeCall: (ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            requireNativeInput(requestArchive, "requestArchive")
            requireNativeInput(bundleArchive, "bundleArchive")
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(requestArchive, bundleArchive)
            return requireRecursiveSpendOutput(output, label)
        }

        private fun call(
            label: String,
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
            nativeCall: (ByteArray, ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            requireNativeInput(previousWitnessArchive, "previousWitnessArchive")
            requireNativeInput(requestArchive, "requestArchive")
            requireNativeInput(bundleArchive, "bundleArchive")
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(previousWitnessArchive, requestArchive, bundleArchive)
            return requireRecursiveSpendOutput(output, label)
        }

        internal fun requireRecursiveSpendOutput(output: ByteArray?, label: String): ByteArray =
            requireNativeOutput(output, "native $label")

        private fun requireNativeInput(archive: ByteArray, archiveName: String) {
            require(archive.isNotEmpty()) { "$archiveName must not be empty" }
            require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
        }

        private fun requireNativeOutput(output: ByteArray?, label: String): ByteArray {
            check(output != null) { "$label returned no output" }
            check(output.isNotEmpty()) { "$label returned empty output" }
            check(output.size <= NATIVE_ARCHIVE_MAX_BYTES) { "$label returned oversized output" }
            check(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(output)) {
                "$label returned invalid Norito archive"
            }
            check(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(output)) {
                "$label returned empty Norito payload"
            }
            return output
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = { probeRequiredNativeSymbols() },
            )

        private fun probeRequiredNativeSymbols(): Boolean {
            val probe = MALFORMED_NATIVE_PROBE_ARCHIVE
            var available = true
            available = expectIllegalArgumentProbe { nativeInitSpend(probe) } && available
            available = expectIllegalArgumentProbe { nativeAppendSpend(probe) } && available
            available =
                expectIllegalArgumentProbe { nativeTransitionProfileInit(ByteArray(0)) } && available
            available =
                expectIllegalArgumentProbe { nativeTransitionProfileAppend(ByteArray(0)) } && available
            available =
                expectIllegalArgumentProbe { nativeLineageAppendBoundary(ByteArray(0)) } && available
            available = expectIllegalArgumentProbe { nativeVerifySpend(probe) } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessFromInitResult(probe, probe)
            } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessAppendResult(probe, probe, probe)
            } && available
            available = expectIllegalArgumentProbe { nativeRedeemSpend(probe) } && available
            return available
        }

        internal fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean =
            try {
                probe()
                false
            } catch (_: IllegalArgumentException) {
                true
            }

        internal fun detectNativeAvailability(
            loadLibrary: () -> Unit,
            bridgeAbiVersion: () -> Int,
            probeSymbol: () -> Boolean,
            requiredBridgeAbiVersion: Int = REQUIRED_BRIDGE_ABI_VERSION,
        ): Boolean {
            try {
                loadLibrary()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            val abiVersion = try {
                bridgeAbiVersion()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            if (abiVersion < requiredBridgeAbiVersion) {
                return false
            }
            return try {
                probeSymbol()
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeInitSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeTransitionProfileInit(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeTransitionProfileAppend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeLineageAppendBoundary(profileArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessFromInitResult(
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessAppendResult(
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeRedeemSpend(requestArchive: ByteArray): ByteArray?
    }

    /** Portable Reserved-lineage verifier/proving key artifact package. */
    class LineageKeyArtifacts internal constructor(
        val proofCircuitId: String,
        val verifierOpeningLen: Int,
        val lineageVerifierKeyBackend: String,
        lineageVerifierKey: ByteArray,
        lineageProvingKeyArchive: ByteArray,
    ) {
        private val lineageVerifierKeyBytes = lineageVerifierKey.copyOf()
        private val lineageProvingKeyArchiveBytes = lineageProvingKeyArchive.copyOf()

        fun isInitArtifact(): Boolean =
            proofCircuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1

        fun isAppendArtifact(): Boolean =
            proofCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        fun lineageVerifierKey(): ByteArray = lineageVerifierKeyBytes.copyOf()

        fun lineageProvingKeyArchive(): ByteArray = lineageProvingKeyArchiveBytes.copyOf()
    }
}
