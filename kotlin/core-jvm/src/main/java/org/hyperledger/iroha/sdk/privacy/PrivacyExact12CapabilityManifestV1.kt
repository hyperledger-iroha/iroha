// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.core.model.NetworkId
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonParser

/** Closed public operation schema carried by one Exact12 capability row. */
enum class PrivacyOperationSchemaV1(val canonicalLabel: String) {
    ZK_ACE_AUTHORIZATION_ACTION_V1("zk_ace_authorization_action_v1"),
    ANONYMOUS_PGC_PAYMENT_ACTION_V1("anonymous_pgc_payment_action_v1"),
    VERANGE_RANGE_PROOF_V1("verange_range_proof_v1"),
    ZK_AMS_ADMISSION_AND_PROVISIONING_V1("zk_ams_admission_and_provisioning_v1"),
    VEGA_CREDENTIAL_PRESENTATION_V1("vega_credential_presentation_v1"),
    ZK_X509_IDENTITY_PRESENTATION_V1("zk_x509_identity_presentation_v1"),
    JINDO_POLYNOMIAL_EVALUATION_V1("jindo_polynomial_evaluation_v1"),
    BOOTLE_LANTERN_CREDENTIAL_PRESENTATION_V1(
        "bootle_lantern_credential_presentation_v1",
    ),
    ORCHARD_NOTE_ACTION_V1("orchard_note_action_v1"),
    FCMP_MEMBERSHIP_PAYMENT_V1("fcmp_membership_payment_v1"),
    IVM_PRIVATE_NOTE_ACTION_V1("ivm_private_note_action_v1"),
    PQ_MASP_NOTE_ACTION_V1("pq_masp_note_action_v1"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyOperationSchemaV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown Exact12 operation schema")
    }
}

/** Closed execution classification carried by one Exact12 capability row. */
enum class PrivacyExecutionModeV1(val canonicalLabel: String) {
    AUTHORIZATION_ACTION("authorization_action"),
    PAYMENT_ACTION("payment_action"),
    COMPONENT("component"),
    ADMISSION_ACTION("admission_action"),
    PRESENTATION_ACTION("presentation_action"),
    NOTE_ACTION("note_action"),
    ;

    companion object {
        @JvmStatic
        fun fromCanonicalLabel(label: String): PrivacyExecutionModeV1 =
            values().firstOrNull { it.canonicalLabel == label }
                ?: throw IllegalArgumentException("unknown Exact12 execution mode")
    }
}

/** Current compiled or committed-policy reason that one protocol is unavailable. */
sealed class PrivacyCapabilityUnavailableReasonV1 {
    class CompiledProfile(
        @JvmField val failure: PrivacyCompiledProfileResultV1.Unavailable,
    ) : PrivacyCapabilityUnavailableReasonV1() {
        override fun equals(other: Any?): Boolean =
            other is CompiledProfile && failure == other.failure

        override fun hashCode(): Int = failure.hashCode()
    }


}

/** Runtime availability projected from the supported compiled profile and committed policy. */
sealed class PrivacyCapabilityReadinessV1 {
    object Available : PrivacyCapabilityReadinessV1()

    class Unavailable(
        @JvmField val reason: PrivacyCapabilityUnavailableReasonV1,
    ) : PrivacyCapabilityReadinessV1() {
        override fun equals(other: Any?): Boolean =
            other is Unavailable && reason == other.reason

        override fun hashCode(): Int = reason.hashCode()
    }
}

/** One canonical committed Exact12 capability row. */
class PrivacyExact12CapabilityRowV1 internal constructor(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val operationSchema: PrivacyOperationSchemaV1,
    @JvmField val executionMode: PrivacyExecutionModeV1,
    @JvmField val privacyFeatureMask: Int,
    @JvmField val compiledProfile: PrivacyCompiledProfileResultV1,
    @JvmField val readiness: PrivacyCapabilityReadinessV1,
    @JvmField val policy: PrivacyProtocolPolicyV1?,
    /** True only when native Rust compared the canonical profile tuple with this binary. */
    @JvmField val localCompiledTupleMatches: Boolean,
) {
    init {
        require(operationSchema == expectedOperationSchema(protocolId)) {
            "Exact12 operation schema does not match its protocol"
        }
        require(executionMode == expectedExecutionMode(protocolId)) {
            "Exact12 execution mode does not match its protocol"
        }
        require(privacyFeatureMask == expectedFeatureMask(protocolId)) {
            "Exact12 privacy feature mask does not match its protocol"
        }
        val available = compiledProfile as? PrivacyCompiledProfileResultV1.Available
        require(available == null || available.profile.protocolId == protocolId) {
            "Exact12 compiled profile does not match its protocol"
        }
        policy?.let { governed ->
            require(governed.protocolId == protocolId) {
                "Exact12 policy does not match its protocol"
            }
        }
        require(
            readiness == projectedReadinessV1(
                compiledProfile,
            ),
        ) {
            "Exact12 readiness was not derived from the compiled implementation"
        }
    }

    /** Available exactly when the canonical compiled implementation is executable. */
    fun isNetworkAvailable(): Boolean = readiness is PrivacyCapabilityReadinessV1.Available

    /** Current limits for an executable implementation, with canonical limits when no policy exists. */
    fun effectiveProtocolLimits(): PrivacyProtocolLimitsV1? =
        (compiledProfile as? PrivacyCompiledProfileResultV1.Available)?.let {
            policy?.currentLimits ?: it.profile.protocolLimits
        }

    override fun equals(other: Any?): Boolean =
        other is PrivacyExact12CapabilityRowV1 &&
            protocolId == other.protocolId &&
            operationSchema == other.operationSchema &&
            executionMode == other.executionMode &&
            privacyFeatureMask == other.privacyFeatureMask &&
            compiledProfile == other.compiledProfile &&
            readiness == other.readiness &&
            policy == other.policy &&
            localCompiledTupleMatches == other.localCompiledTupleMatches

    override fun hashCode(): Int {
        var result = protocolId.hashCode()
        result = 31 * result + operationSchema.hashCode()
        result = 31 * result + executionMode.hashCode()
        result = 31 * result + privacyFeatureMask
        result = 31 * result + compiledProfile.hashCode()
        result = 31 * result + readiness.hashCode()
        result = 31 * result + (policy?.hashCode() ?: 0)
        return 31 * result + localCompiledTupleMatches.hashCode()
    }
}

/**
 * Native-validated canonical Torii Exact12 capability manifest.
 *
 * Public decoding provides inspection only. Admission also requires the private origin binding
 * issued by the configured Torii transport after an authenticated response for its exact network.
 */
class PrivacyExact12CapabilityManifestV1 private constructor(
    @JvmField val version: Int,
    @JvmField val networkId: NetworkId,
    @JvmField val committedHeight: BigInteger,
    @JvmField val consensusPolicy: PrivacyConsensusPolicyV1,
    protocols: List<PrivacyExact12CapabilityRowV1>,
    @JvmField val manifestDigest: PrivacyFixed32V1,
    canonicalArchive: ByteArray,
    private val authenticatedNetworkId: NetworkId? = null,
) {
    @JvmField
    val protocols: List<PrivacyExact12CapabilityRowV1>
    private val archive = canonicalArchive.copyOf()

    init {
        require(version == VERSION) { "Exact12 capability manifest version must be $VERSION" }
        require(committedHeight.signum() >= 0 && committedHeight.bitLength() <= 64) {
            "Exact12 committed height must fit u64"
        }
        requirePrivacyConsensusPolicyAtHeightV1(consensusPolicy, committedHeight)
        val expected = PrivacyProtocolIdV1.values()
        require(protocols.size == expected.size) {
            "Exact12 capability manifest must contain exactly ${expected.size} rows"
        }
        protocols.forEachIndexed { index, row ->
            require(row.protocolId == expected[index]) {
                "Exact12 capability row $index is out of canonical order"
            }
            row.policy?.pendingUpdate?.let { pending ->
                require(pending.scheduledAtHeight <= committedHeight && pending.effectiveAtHeight > committedHeight) {
                    "Exact12 policy update has an invalid committed-height schedule"
                }
            }
            require(
                row.readiness == projectedReadinessV1(
                    row.compiledProfile,
                ),
            ) {
                "Exact12 readiness differs from the supported compiled profile and committed policy"
            }
        }
        require(archive.isNotEmpty() && archive.size <= MAX_ARCHIVE_BYTES) {
            "Exact12 canonical archive length is outside its bound"
        }
        this.protocols = Collections.unmodifiableList(protocols.toList())
    }

    /** Return a defensive copy of the exact Torii Norito response bytes. */
    fun canonicalBytes(): ByteArray = archive.copyOf()

    /** Return the canonical row for [protocolId]. */
    fun rowFor(protocolId: PrivacyProtocolIdV1): PrivacyExact12CapabilityRowV1 =
        protocols[protocolId.ordinal].also {
            check(it.protocolId == protocolId) { "Exact12 protocol registry order drifted" }
        }

    internal fun requireNetwork(expectedNetworkId: NetworkId) {
        require(networkId == expectedNetworkId) { "Exact12 capability manifest belongs to a different network" }
    }

    internal fun requireAuthenticatedNetwork(): NetworkId =
        requireNotNull(authenticatedNetworkId) {
            "Exact12 admission requires an authenticated Torii response; decoded archives are inspection-only"
        }.also(::requireNetwork)

    override fun equals(other: Any?): Boolean =
        other is PrivacyExact12CapabilityManifestV1 && archive.contentEquals(other.archive)

    override fun hashCode(): Int = archive.contentHashCode()

    companion object {
        const val VERSION: Int = 1
        const val MAX_ARCHIVE_BYTES: Int = 256 * 1024

        @JvmSynthetic
        internal fun inspection(
            version: Int,
            networkId: NetworkId,
            committedHeight: BigInteger,
            consensusPolicy: PrivacyConsensusPolicyV1,
            protocols: List<PrivacyExact12CapabilityRowV1>,
            manifestDigest: PrivacyFixed32V1,
            canonicalArchive: ByteArray,
        ): PrivacyExact12CapabilityManifestV1 = PrivacyExact12CapabilityManifestV1(
            version, networkId, committedHeight, consensusPolicy, protocols,
            manifestDigest, canonicalArchive,
        )

        /** Only the SDK's authenticated Torii transport may issue this origin binding. */
        @JvmSynthetic
        internal fun fromAuthenticatedTorii(
            archive: ByteArray,
            expectedNetworkId: NetworkId,
        ): PrivacyExact12CapabilityManifestV1 {
            val decoded = PrivacyNativeBridge.requireExact12CapabilityManifest(
                archive, expectedNetworkId,
            )
            decoded.requireNetwork(expectedNetworkId)
            return PrivacyExact12CapabilityManifestV1(
                decoded.version, decoded.networkId, decoded.committedHeight, decoded.consensusPolicy,
                decoded.protocols, decoded.manifestDigest,
                decoded.archive, expectedNetworkId,
            )
        }
    }
}

/** Opaque admission token issued only after committed/native tuple agreement. */
class PrivacyExact12CapabilityTupleAdmissionV1 private constructor(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val committedHeight: BigInteger,
    @JvmField val manifestDigest: PrivacyFixed32V1,
    @JvmField val operationSchema: PrivacyOperationSchemaV1,
    canonicalManifestArchive: ByteArray,
    private val expectedNetworkId: NetworkId,
    private val admittedProfile: PrivacyCompiledProfileV1,
    private val seal: Any,
) {
    private val manifestArchive = canonicalManifestArchive.copyOf()

    internal fun requireNetwork(networkId: NetworkId) {
        require(expectedNetworkId == networkId) {
            "Exact12 capability admission belongs to a different transaction network"
        }
    }

    companion object {
        private val SEAL = Any()

        internal fun issue(
            manifest: PrivacyExact12CapabilityManifestV1,
            row: PrivacyExact12CapabilityRowV1,
        ): PrivacyExact12CapabilityTupleAdmissionV1 {
            val archive = manifest.canonicalBytes()
            val networkId = manifest.requireAuthenticatedNetwork()
            PrivacyNativeBridge.requireExact12CapabilityTuple(archive, row.protocolId, networkId)
            return PrivacyExact12CapabilityTupleAdmissionV1(
                row.protocolId,
                manifest.committedHeight,
                manifest.manifestDigest,
                row.operationSchema,
                archive,
                networkId,
                (row.compiledProfile as PrivacyCompiledProfileResultV1.Available).profile,
                SEAL,
            )
        }

        internal fun requireAuthentic(
            admission: PrivacyExact12CapabilityTupleAdmissionV1,
            protocolId: PrivacyProtocolIdV1,
        ) {
            require(admission.seal === SEAL && admission.protocolId == protocolId) {
                "Exact12 capability admission token is absent, invalid, or protocol-substituted"
            }
            PrivacyNativeBridge.requireExact12CapabilityTuple(
                admission.manifestArchive,
                protocolId,
                admission.expectedNetworkId,
            )
        }

        internal fun requireSubmitProofConstruction(
            admission: PrivacyExact12CapabilityTupleAdmissionV1,
            protocolId: PrivacyProtocolIdV1,
            instructionArchive: ByteArray,
        ) {
            requireAuthentic(admission, protocolId)
            if (protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1) {
                SubmitPrivacyZkX509CertificateV1.fromCanonicalRequestV1(instructionArchive)
                    .requireCompiledContextV1(admission.expectedNetworkId, admission.admittedProfile)
            }
            PrivacyNativeBridge.requireExact12SubmitProofConstruction(
                admission.manifestArchive,
                protocolId,
                instructionArchive,
                admission.expectedNetworkId,
            )
        }
    }
}

/** Fail-closed bridge from a committed manifest to retained privacy construction. */
object PrivacyExact12CapabilityAdmissionV1 {
    /**
     * Require supported implementation availability and an exact native compiled-profile tuple match.
     *
     * A local catalog is used only for equality. It never creates network availability.
     */
    @JvmStatic
    fun requireExact12CapabilityTupleV1(
        manifest: PrivacyExact12CapabilityManifestV1,
        protocolId: PrivacyProtocolIdV1,
    ): PrivacyExact12CapabilityTupleAdmissionV1 {
        manifest.requireAuthenticatedNetwork()
        val row = manifest.rowFor(protocolId)
        require(row.isNetworkAvailable()) {
            "Exact12 protocol ${protocolId.canonicalLabel} has no supported executable implementation"
        }
        require(row.localCompiledTupleMatches) {
            "Exact12 committed profile tuple differs from compiledProfileCatalogTypedV1"
        }
        return PrivacyExact12CapabilityTupleAdmissionV1.issue(manifest, row)
    }

    /** Verify a token immediately before constructing a retained privacy action. */
    @JvmStatic
    fun requireForConstruction(
        admission: PrivacyExact12CapabilityTupleAdmissionV1,
        protocolId: PrivacyProtocolIdV1,
    ) {
        PrivacyExact12CapabilityTupleAdmissionV1.requireAuthentic(admission, protocolId)
    }

    /** Bind the protocol-specific canonical request to its current network and five compiled tuple fields.
     * X509 requires its proof-free attached request; the other eleven require SubmitPrivacyProofV1.
     */
    @JvmStatic
    fun requireForConstruction(
        admission: PrivacyExact12CapabilityTupleAdmissionV1,
        protocolId: PrivacyProtocolIdV1,
        instructionArchive: ByteArray,
    ) {
        PrivacyExact12CapabilityTupleAdmissionV1.requireSubmitProofConstruction(
            admission,
            protocolId,
            instructionArchive,
        )
    }
}

internal object PrivacyExact12CapabilityManifestInspectionV1 {
    private const val MAX_INSPECTION_BYTES = 1024 * 1024
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    fun parse(
        canonicalArchive: ByteArray,
        nativeInspection: ByteArray,
    ): PrivacyExact12CapabilityManifestV1 {
        require(nativeInspection.isNotEmpty() && nativeInspection.size <= MAX_INSPECTION_BYTES) {
            "native Exact12 capability inspection length is outside its bound"
        }
        val decoded = try {
            JsonParser.parse(String(nativeInspection, StandardCharsets.UTF_8))
        } catch (error: RuntimeException) {
            throw IllegalArgumentException("native Exact12 capability inspection is malformed", error)
        }
        val wrapper = exactObject(
            decoded,
            setOf("manifest", "local_compiled_tuple_matches"),
            "Exact12 native inspection",
        )
        val matches = list(
            wrapper["local_compiled_tuple_matches"],
            "Exact12 native inspection.local_compiled_tuple_matches",
        )
        val expected = PrivacyProtocolIdV1.values()
        require(matches.size == expected.size) {
            "native Exact12 tuple comparison must contain exactly ${expected.size} rows"
        }
        val manifest = exactObject(
            wrapper["manifest"],
            setOf(
                "version",
                "network_id",
                "committed_height",
                "consensus_policy",
                "protocols",
                "manifest_digest",
            ),
            "Exact12 capability manifest",
        )
        val version = uint32(manifest["version"], "Exact12 capability manifest.version")
        require(version == PrivacyExact12CapabilityManifestV1.VERSION) {
            "Exact12 capability manifest version must be 1"
        }
        val networkId = NetworkId.parse(text(manifest["network_id"], "Exact12 capability manifest.network_id"))
        val committedHeight = uint64(
            manifest["committed_height"],
            "Exact12 capability manifest.committed_height",
        )
        val consensusPolicy = parseConsensusPolicy(
            manifest["consensus_policy"],
            committedHeight,
            "Exact12 capability manifest.consensus_policy",
        )
        val rawRows = list(manifest["protocols"], "Exact12 capability manifest.protocols")
        require(rawRows.size == expected.size) {
            "Exact12 capability manifest must contain exactly ${expected.size} rows"
        }
        val rows = rawRows.mapIndexed { index, value ->
            val tupleMatch = matches[index] as? Boolean
                ?: throw IllegalArgumentException(
                    "Exact12 native tuple match $index must be boolean",
                )
            parseRow(
                value,
                expected[index],
                tupleMatch,
                committedHeight,
                "Exact12 capability manifest.protocols[$index]",
            )
        }
        val manifestDigest = fixed32(
            manifest["manifest_digest"],
            "Exact12 capability manifest.manifest_digest",
        )
        return PrivacyExact12CapabilityManifestV1.inspection(
            version,
            networkId,
            committedHeight,
            consensusPolicy,
            rows,
            manifestDigest,
            canonicalArchive,
        )
    }

    private fun parseRow(
        value: Any?,
        expected: PrivacyProtocolIdV1,
        tupleMatch: Boolean,
        committedHeight: BigInteger,
        path: String,
    ): PrivacyExact12CapabilityRowV1 {
        val row = exactObject(
            value,
            setOf(
                "protocol_id",
                "operation_schema",
                "execution_mode",
                "privacy_feature_mask",
                "compiled_profile",
                "readiness",
                "policy",
            ),
            path,
        )
        val protocol = protocolTag(row["protocol_id"], "$path.protocol_id")
        require(protocol == expected) { "$path is out of canonical protocol order" }
        val operation = PrivacyOperationSchemaV1.fromCanonicalLabel(
            taggedUnit(
                row["operation_schema"],
                "operation_schema",
                "value",
                "$path.operation_schema",
            ),
        )
        val execution = PrivacyExecutionModeV1.fromCanonicalLabel(
            taggedUnit(
                row["execution_mode"],
                "execution_mode",
                "value",
                "$path.execution_mode",
            ),
        )
        val featureMask = uint32(row["privacy_feature_mask"], "$path.privacy_feature_mask")
        require(featureMask <= 0xff) { "$path.privacy_feature_mask must fit uint8" }
        val compiled = parseCompiledProfile(row["compiled_profile"], protocol, "$path.compiled_profile")
        val readiness = parseReadiness(row["readiness"], "$path.readiness")
        val policy = row["policy"]?.let {
            parseProtocolPolicy(it, protocol, committedHeight, "$path.policy")
        }
        return PrivacyExact12CapabilityRowV1(
            protocol,
            operation,
            execution,
            featureMask,
            compiled,
            readiness,
            policy,
            tupleMatch,
        )
    }

    private fun parseCompiledProfile(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        path: String,
    ): PrivacyCompiledProfileResultV1 {
        val result = exactObject(value, setOf("status", "value"), path)
        return when (text(result["status"], "$path.status")) {
            "available" -> PrivacyCompiledProfileResultV1.Available(
                parseProfile(result["value"], protocol, "$path.value"),
            )
            "unavailable" -> parseUnavailable(result["value"], "$path.value")
            else -> throw IllegalArgumentException("$path.status is not a closed V1 status")
        }
    }

    private fun parseProfile(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        path: String,
    ): PrivacyCompiledProfileV1 {
        val profile = exactObject(value, PROFILE_KEYS, path)
        return parseProfileBindings(
            profile,
            protocol,
            parseProtocolLimits(profile["protocol_limits"], protocol, "$path.protocol_limits"),
            path,
        )
    }

    private fun parseProfileBindings(
        profile: Map<String, Any?>,
        protocol: PrivacyProtocolIdV1,
        protocolLimits: PrivacyProtocolLimitsV1,
        path: String,
    ): PrivacyCompiledProfileV1 {
        require(protocolTag(profile["protocol_id"], "$path.protocol_id") == protocol) {
            "$path protocol binding differs from its row"
        }
        val proof = PrivacyProofSystemIdV1.fromCanonicalLabel(
            taggedUnit(profile["proof_system_id"], "proof_system", "value", "$path.proof_system_id"),
        )
        val engine = PrivacyEngineIdV1.fromCanonicalLabel(
            taggedUnit(profile["engine_id"], "engine", "value", "$path.engine_id"),
        )
        return PrivacyCompiledProfileV1(
            protocol,
            proof,
            engine,
            fixed32(profile["parameter_id"], "$path.parameter_id"),
            fixed32(profile["parameter_digest"], "$path.parameter_digest"),
            fixed32(profile["verifier_digest"], "$path.verifier_digest"),
            fixed32(profile["statement_schema_digest"], "$path.statement_schema_digest"),
            fixed32(profile["engine_manifest_digest"], "$path.engine_manifest_digest"),
            protocolLimits,
        )
    }

    private fun parseProtocolLimits(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        path: String,
    ): PrivacyProtocolLimitsV1 {
        val tagged = exactObject(value, setOf("protocol", "limits"), path)
        val encodedProtocol = PrivacyProtocolIdV1.fromCanonicalLabel(
            text(tagged["protocol"], "$path.protocol"),
        )
        require(encodedProtocol == protocol) { "$path protocol tag differs from its row" }
        val rules = privacyProtocolLimitRulesV1(protocol)
        if (rules.isEmpty()) {
            require(tagged["limits"] == null) { "$path fixed limits must be null" }
            return PrivacyProtocolLimitsV1(protocol, null)
        }
        val limits = exactObject(tagged["limits"], rules.map { it.name }.toSet(), "$path.limits")
        val values = LinkedHashMap<String, Int>()
        for (rule in rules) {
            values[rule.name] = uint32(limits[rule.name], "$path.limits.${rule.name}")
        }
        return PrivacyProtocolLimitsV1(protocol, values)
    }

    private fun parseUnavailable(
        value: Any?,
        path: String,
    ): PrivacyCompiledProfileResultV1.Unavailable {
        val unavailable = exactObject(value, setOf("reason", "detail"), path)
        return when (text(unavailable["reason"], "$path.reason")) {
            "engine-unavailable" -> {
                require(unavailable["detail"] == null) { "$path.detail must be null" }
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
                    null,
                )
            }
            "profile-initialization-failed" -> {
                require(unavailable["detail"] == null) { "$path.detail must be null" }
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.PROFILE_INITIALIZATION_FAILED,
                    null,
                )
            }
            "joint-security-reduction-incomplete" -> {
                require(unavailable["detail"] == null) { "$path.detail must be null" }
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.JOINT_SECURITY_REDUCTION_INCOMPLETE,
                    null,
                )
            }
            "statement-schema-invalid" -> {
                val detail = taggedUnit(
                    unavailable["detail"],
                    "schema_error",
                    "detail",
                    "$path.detail",
                )
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID,
                    when (detail) {
                        "conflicting-stable-type-id" ->
                            PrivacyCompiledStatementSchemaErrorV1.CONFLICTING_STABLE_TYPE_ID
                        "missing-type-reference" ->
                            PrivacyCompiledStatementSchemaErrorV1.MISSING_TYPE_REFERENCE
                        else -> throw IllegalArgumentException("$path.detail is not a closed schema error")
                    },
                )
            }
            else -> throw IllegalArgumentException("$path.reason is not a closed unavailable reason")
        }
    }

    private fun parseReadiness(value: Any?, path: String): PrivacyCapabilityReadinessV1 {
        val readiness = exactObject(value, setOf("readiness", "detail"), path)
        return when (text(readiness["readiness"], "$path.readiness")) {
            "available" -> {
                require(readiness["detail"] == null) { "$path.detail must be null" }
                PrivacyCapabilityReadinessV1.Available
            }
            "unavailable" -> PrivacyCapabilityReadinessV1.Unavailable(
                parseCapabilityUnavailableReason(readiness["detail"], "$path.detail"),
            )
            else -> throw IllegalArgumentException("$path.readiness is not a closed readiness")
        }
    }

    private fun parseCapabilityUnavailableReason(
        value: Any?,
        path: String,
    ): PrivacyCapabilityUnavailableReasonV1 {
        val unavailable = exactObject(value, setOf("reason", "detail"), path)
        return when (text(unavailable["reason"], "$path.reason")) {
            "compiled-profile" -> PrivacyCapabilityUnavailableReasonV1.CompiledProfile(
                parseUnavailable(unavailable["detail"], "$path.detail"),
            )
            else -> throw IllegalArgumentException("$path.reason is not a closed unavailable reason")
        }
    }

    private fun parseConsensusPolicy(
        value: Any?,
        committedHeight: BigInteger,
        path: String,
    ): PrivacyConsensusPolicyV1 {
        val policy = exactObject(value, setOf("current_limits", "pending_update"), path)
        val current = parseConsensusLimits(policy["current_limits"], "$path.current_limits")
        val pending = policy["pending_update"]?.let { candidate ->
            val pendingPath = "$path.pending_update"
            val tightening = exactObject(
                candidate,
                setOf("scheduled_at_height", "effective_at_height", "next_limits"),
                pendingPath,
            )
            val scheduled = integer(
                tightening["scheduled_at_height"],
                "$pendingPath.scheduled_at_height",
            )
            val effective = integer(
                tightening["effective_at_height"],
                "$pendingPath.effective_at_height",
            )
            requireCommittedSchedule(scheduled, effective, committedHeight, pendingPath)
            PrivacyConsensusPolicyUpdateV1(
                scheduled,
                effective,
                parseConsensusLimits(tightening["next_limits"], "$pendingPath.next_limits"),
            )
        }
        return PrivacyConsensusPolicyV1(current, pending)
    }

    private fun parseConsensusLimits(value: Any?, path: String): PrivacyConsensusLimitsV1 {
        val limits = exactObject(value, CONSENSUS_LIMIT_KEYS, path)
        fun field(name: String): Int = uint32(limits[name], "$path.$name")
        return PrivacyConsensusLimitsV1(
            field("max_actions_per_transaction"),
            field("max_actions_per_block"),
            field("max_proof_bytes_per_action"),
            field("max_action_bytes"),
            field("max_privacy_bytes_per_transaction"),
            field("max_privacy_bytes_per_block"),
            field("max_statement_and_encrypted_output_bytes_per_transaction"),
            field("max_nullifiers_per_action"),
            field("max_commitments_per_action"),
            field("retained_root_count"),
        )
    }

    private fun parseProtocolPolicy(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        committedHeight: BigInteger,
        path: String,
    ): PrivacyProtocolPolicyV1 {
        val policy = exactObject(value, setOf("protocol_id", "current_limits", "pending_update"), path)
        require(protocolTag(policy["protocol_id"], "$path.protocol_id") == protocol) {
            "$path protocol binding differs from its row"
        }
        val current = parseProtocolLimits(policy["current_limits"], protocol, "$path.current_limits")
        val pending = policy["pending_update"]?.let { candidate ->
            val pendingPath = "$path.pending_update"
            val update = exactObject(
                candidate, setOf("scheduled_at_height", "effective_at_height", "next_limits"), pendingPath,
            )
            val scheduled = integer(update["scheduled_at_height"], "$pendingPath.scheduled_at_height")
            val effective = integer(update["effective_at_height"], "$pendingPath.effective_at_height")
            requireCommittedSchedule(scheduled, effective, committedHeight, pendingPath)
            PrivacyProtocolLimitsUpdateV1(
                scheduled, effective,
                parseProtocolLimits(update["next_limits"], protocol, "$pendingPath.next_limits"),
            )
        }
        return PrivacyProtocolPolicyV1(protocol, current, pending)
    }

    private fun requireCommittedSchedule(
        scheduled: BigInteger,
        effective: BigInteger,
        committedHeight: BigInteger,
        path: String,
    ) {
        requireValidPrivacyPolicyScheduleV1(scheduled, effective, path)
        require(
            scheduled <= committedHeight &&
                effective > committedHeight,
        ) { "$path has an invalid committed-height schedule" }
    }

    private fun protocolTag(value: Any?, path: String): PrivacyProtocolIdV1 =
        PrivacyProtocolIdV1.fromCanonicalLabel(taggedUnit(value, "protocol", "value", path))

    private fun taggedUnit(value: Any?, tag: String, content: String, path: String): String {
        val tagged = exactObject(value, setOf(tag, content), path)
        require(tagged[content] == null) { "$path.$content must be null" }
        return text(tagged[tag], "$path.$tag")
    }

    private fun fixed32(value: Any?, path: String): PrivacyFixed32V1 {
        return PrivacyFixed32V1(fixedBytes(value, 32, path))
    }

    private fun fixedBytes(value: Any?, size: Int, path: String): ByteArray {
        val bytes = list(value, path)
        require(bytes.size == size) { "$path must contain exactly $size bytes" }
        return ByteArray(size) { index ->
            val byte = uint32(bytes[index], "$path[$index]")
            require(byte <= 255) { "$path[$index] must fit uint8" }
            byte.toByte()
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun exactObject(value: Any?, keys: Set<String>, path: String): Map<String, Any?> {
        val objectValue = value as? Map<*, *>
            ?: throw IllegalArgumentException("$path must be an object")
        require(objectValue.keys.all { it is String }) { "$path contains a non-string field" }
        val actual = objectValue.keys.map { it as String }.toSet()
        require(actual == keys && objectValue.size == keys.size) {
            "$path must contain exactly ${keys.sorted().joinToString()}"
        }
        return objectValue as Map<String, Any?>
    }

    private fun list(value: Any?, path: String): List<Any?> =
        value as? List<Any?> ?: throw IllegalArgumentException("$path must be an array")

    private fun text(value: Any?, path: String): String =
        value as? String ?: throw IllegalArgumentException("$path must be a string")

    private fun uint32(value: Any?, path: String): Int {
        val number = integer(value, path)
        require(number.signum() >= 0 && number.bitLength() <= 31) {
            "$path must fit the supported uint32 range"
        }
        return number.intValueExact()
    }

    private fun uint64(value: Any?, path: String): BigInteger {
        val number = integer(value, path)
        require(number.signum() >= 0 && number <= U64_MAX) { "$path must fit uint64" }
        return number
    }

    private fun positiveUint64(value: Any?, path: String): BigInteger =
        uint64(value, path).also { require(it.signum() > 0) { "$path must be non-zero" } }

    private fun integer(value: Any?, path: String): BigInteger = when (value) {
        is Long -> BigInteger.valueOf(value)
        is BigInteger -> value
        else -> throw IllegalArgumentException("$path must be an integer")
    }

    private val PROFILE_KEYS = setOf(
        "protocol_id",
        "proof_system_id",
        "engine_id",
        "parameter_id",
        "parameter_digest",
        "verifier_digest",
        "statement_schema_digest",
        "engine_manifest_digest",
        "protocol_limits",
    )
    private val CONSENSUS_LIMIT_KEYS = setOf(
        "max_actions_per_transaction",
        "max_actions_per_block",
        "max_proof_bytes_per_action",
        "max_action_bytes",
        "max_privacy_bytes_per_transaction",
        "max_privacy_bytes_per_block",
        "max_statement_and_encrypted_output_bytes_per_transaction",
        "max_nullifiers_per_action",
        "max_commitments_per_action",
        "retained_root_count",
    )

}

private fun projectedReadinessV1(
    compiledProfile: PrivacyCompiledProfileResultV1,
): PrivacyCapabilityReadinessV1 = when (compiledProfile) {
    is PrivacyCompiledProfileResultV1.Unavailable -> PrivacyCapabilityReadinessV1.Unavailable(
        PrivacyCapabilityUnavailableReasonV1.CompiledProfile(compiledProfile),
    )
    is PrivacyCompiledProfileResultV1.Available -> PrivacyCapabilityReadinessV1.Available
}

internal fun expectedOperationSchema(protocol: PrivacyProtocolIdV1): PrivacyOperationSchemaV1 =
    when (protocol) {
        PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V1 ->
            PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1
        PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 ->
            PrivacyOperationSchemaV1.ANONYMOUS_PGC_PAYMENT_ACTION_V1
        PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 ->
            PrivacyOperationSchemaV1.VERANGE_RANGE_PROOF_V1
        PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 ->
            PrivacyOperationSchemaV1.ZK_AMS_ADMISSION_AND_PROVISIONING_V1
        PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V1 ->
            PrivacyOperationSchemaV1.VEGA_CREDENTIAL_PRESENTATION_V1
        PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1 ->
            PrivacyOperationSchemaV1.ZK_X509_IDENTITY_PRESENTATION_V1
        PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1 ->
            PrivacyOperationSchemaV1.JINDO_POLYNOMIAL_EVALUATION_V1
        PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1 ->
            PrivacyOperationSchemaV1.BOOTLE_LANTERN_CREDENTIAL_PRESENTATION_V1
        PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1 ->
            PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1
        PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1 ->
            PrivacyOperationSchemaV1.FCMP_MEMBERSHIP_PAYMENT_V1
        PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1 ->
            PrivacyOperationSchemaV1.IVM_PRIVATE_NOTE_ACTION_V1
        PrivacyProtocolIdV1.PQ_MASP_STARK_V1 -> PrivacyOperationSchemaV1.PQ_MASP_NOTE_ACTION_V1
    }

internal fun expectedExecutionMode(protocol: PrivacyProtocolIdV1): PrivacyExecutionModeV1 =
    when (protocol) {
        PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V1 ->
            PrivacyExecutionModeV1.AUTHORIZATION_ACTION
        PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
        PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
        -> PrivacyExecutionModeV1.PAYMENT_ACTION
        PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
        PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1,
        -> PrivacyExecutionModeV1.COMPONENT
        PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 -> PrivacyExecutionModeV1.ADMISSION_ACTION
        PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V1,
        PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1,
        PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1,
        -> PrivacyExecutionModeV1.PRESENTATION_ACTION
        PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
        PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
        PrivacyProtocolIdV1.PQ_MASP_STARK_V1,
        -> PrivacyExecutionModeV1.NOTE_ACTION
    }

internal fun expectedFeatureMask(protocol: PrivacyProtocolIdV1): Int = when (protocol) {
    PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V1,
    PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V1,
    -> 0
    PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 -> (1 shl 1) or (1 shl 2)
    PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 -> 1
    PrivacyProtocolIdV1.IROHA_ZK_AMS_V1,
    PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V1,
    PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1,
    PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1,
    PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
    -> 1 shl 1
    PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
    PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
    -> 1 or (1 shl 1) or (1 shl 2)
    PrivacyProtocolIdV1.PQ_MASP_STARK_V1 ->
        1 or (1 shl 1) or (1 shl 2) or (1 shl 3) or (1 shl 4)
}
