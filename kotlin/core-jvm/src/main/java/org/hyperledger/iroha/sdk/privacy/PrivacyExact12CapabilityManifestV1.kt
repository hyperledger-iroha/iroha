// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonParser

/** Closed public operation schema carried by one Exact12 capability row. */
enum class PrivacyOperationSchemaV1(val canonicalLabel: String) {
    ZK_ACE_AUTHORIZATION_ACTION_V1("zk_ace_authorization_action_v1"),
    ANONYMOUS_PGC_PAYMENT_ACTION_V1("anonymous_pgc_payment_action_v1"),
    VERANGE_RANGE_PROOF_V1("verange_range_proof_v1"),
    ZK_AMS_BATCH_ADMISSION_ACTION_V1("zk_ams_batch_admission_action_v1"),
    ZK_AMS_PROVISION_ACCOUNT_ACTION_V1("zk_ams_provision_account_action_v1"),
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

/** Evidence-derived readiness projected by committed state. */
enum class PrivacyCapabilityReadinessStateV1 {
    AVAILABLE,
    AVAILABLE_EXPERIMENTAL,
    UNAVAILABLE,
}

/** Governance activation projection carried by one committed row. */
enum class PrivacyCapabilityActivationStateV1 {
    NOT_REGISTERED,
    PROPOSED,
    ACTIVE,
    SUSPENDED,
    RETIRED,
}

/** Explicit retained limitation. Revised Jindo is the sole V1 member. */
enum class PrivacyCapabilityLimitationV1 {
    MISSING_DISTRIBUTION_WIDE_KNOWLEDGE_SOUNDNESS_EVIDENCE,
}

/** Typed evidence-derived readiness including the exact unavailable reason. */
class PrivacyCapabilityReadinessV1 internal constructor(
    @JvmField val state: PrivacyCapabilityReadinessStateV1,
    @JvmField val unavailable: PrivacyCompiledProfileResultV1.Unavailable?,
) {
    init {
        require(
            (state == PrivacyCapabilityReadinessStateV1.UNAVAILABLE) == (unavailable != null),
        ) { "Exact12 unavailable readiness must carry exactly one typed reason" }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyCapabilityReadinessV1 &&
            state == other.state &&
            unavailable == other.unavailable

    override fun hashCode(): Int = 31 * state.hashCode() + (unavailable?.hashCode() ?: 0)
}

/** One canonical committed Exact12 capability row. */
class PrivacyExact12CapabilityRowV1 internal constructor(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    operationSchemas: List<PrivacyOperationSchemaV1>,
    @JvmField val executionMode: PrivacyExecutionModeV1,
    @JvmField val privacyFeatureMask: Int,
    @JvmField val compiledProfile: PrivacyCompiledProfileResultV1,
    @JvmField val readiness: PrivacyCapabilityReadinessV1,
    @JvmField val activationState: PrivacyCapabilityActivationStateV1,
    @JvmField val limitation: PrivacyCapabilityLimitationV1?,
    /** True only when native Rust compared the complete committed tuple with this binary. */
    @JvmField val localCompiledTupleMatches: Boolean,
) {
    /** One canonical action schema, except for ZK-AMS's ordered pair. */
    @JvmField
    val operationSchemas: List<PrivacyOperationSchemaV1> =
        Collections.unmodifiableList(operationSchemas.toList())

    init {
        require(this.operationSchemas == expectedOperationSchemas(protocolId)) {
            "Exact12 operation schemas do not match their protocol"
        }
        require(executionMode == expectedExecutionMode(protocolId)) {
            "Exact12 execution mode does not match its protocol"
        }
        require(privacyFeatureMask == expectedFeatureMask(protocolId)) {
            "Exact12 privacy feature mask does not match its protocol"
        }
        val available = compiledProfile is PrivacyCompiledProfileResultV1.Available
        val expectedReadiness = when {
            !available -> PrivacyCapabilityReadinessStateV1.UNAVAILABLE
            protocolId == PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0 ->
                PrivacyCapabilityReadinessStateV1.AVAILABLE_EXPERIMENTAL
            else -> PrivacyCapabilityReadinessStateV1.AVAILABLE
        }
        require(readiness.state == expectedReadiness) {
            "Exact12 readiness was not derived from the committed compiled profile"
        }
        if (compiledProfile is PrivacyCompiledProfileResultV1.Unavailable) {
            require(compiledProfile == readiness.unavailable) {
                "Exact12 readiness unavailable reason differs from the compiled profile"
            }
        }
        val expectedLimitation =
            if (protocolId == PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0) {
                PrivacyCapabilityLimitationV1
                    .MISSING_DISTRIBUTION_WIDE_KNOWLEDGE_SOUNDNESS_EVIDENCE
            } else {
                null
            }
        require(limitation == expectedLimitation) {
            "Exact12 capability limitation does not match its protocol"
        }
    }

    /** Network availability is committed readiness plus an active governance state. */
    fun isNetworkAvailable(): Boolean =
        readiness.state != PrivacyCapabilityReadinessStateV1.UNAVAILABLE &&
            activationState == PrivacyCapabilityActivationStateV1.ACTIVE

    override fun equals(other: Any?): Boolean =
        other is PrivacyExact12CapabilityRowV1 &&
            protocolId == other.protocolId &&
            operationSchemas == other.operationSchemas &&
            executionMode == other.executionMode &&
            privacyFeatureMask == other.privacyFeatureMask &&
            compiledProfile == other.compiledProfile &&
            readiness == other.readiness &&
            activationState == other.activationState &&
            limitation == other.limitation &&
            localCompiledTupleMatches == other.localCompiledTupleMatches

    override fun hashCode(): Int {
        var result = protocolId.hashCode()
        result = 31 * result + operationSchemas.hashCode()
        result = 31 * result + executionMode.hashCode()
        result = 31 * result + privacyFeatureMask
        result = 31 * result + compiledProfile.hashCode()
        result = 31 * result + readiness.hashCode()
        result = 31 * result + activationState.hashCode()
        result = 31 * result + (limitation?.hashCode() ?: 0)
        return 31 * result + localCompiledTupleMatches.hashCode()
    }
}

/**
 * Native-validated canonical Torii Exact12 capability manifest.
 *
 * [canonicalBytes] are the exact immutable Norito bytes received from Torii. The manifest digest
 * identifies content but is not an authentication mechanism; transport/candidate authentication
 * remains the caller's responsibility. Instances can only be issued through [PrivacyNativeBridge].
 */
class PrivacyExact12CapabilityManifestV1 internal constructor(
    @JvmField val version: Int,
    @JvmField val committedHeight: BigInteger,
    protocols: List<PrivacyExact12CapabilityRowV1>,
    @JvmField val manifestDigest: PrivacyFixed32V1,
    canonicalArchive: ByteArray,
) {
    @JvmField
    val protocols: List<PrivacyExact12CapabilityRowV1>
    private val archive = canonicalArchive.copyOf()

    init {
        require(version == VERSION) { "Exact12 capability manifest version must be $VERSION" }
        require(committedHeight.signum() >= 0 && committedHeight.bitLength() <= 64) {
            "Exact12 committed height must fit u64"
        }
        val expected = PrivacyProtocolIdV1.values()
        require(protocols.size == expected.size) {
            "Exact12 capability manifest must contain exactly ${expected.size} rows"
        }
        protocols.forEachIndexed { index, row ->
            require(row.protocolId == expected[index]) {
                "Exact12 capability row $index is out of canonical order"
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

    override fun equals(other: Any?): Boolean =
        other is PrivacyExact12CapabilityManifestV1 && archive.contentEquals(other.archive)

    override fun hashCode(): Int = archive.contentHashCode()

    companion object {
        const val VERSION: Int = 1
        const val MAX_ARCHIVE_BYTES: Int = 256 * 1024
    }
}

/** Opaque admission token issued only after committed/native tuple agreement. */
class PrivacyExact12CapabilityTupleAdmissionV1 private constructor(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val committedHeight: BigInteger,
    @JvmField val manifestDigest: PrivacyFixed32V1,
    operationSchemas: List<PrivacyOperationSchemaV1>,
    canonicalManifestArchive: ByteArray,
    private val seal: Any,
) {
    private val manifestArchive = canonicalManifestArchive.copyOf()
    @JvmField
    val operationSchemas: List<PrivacyOperationSchemaV1> =
        Collections.unmodifiableList(operationSchemas.toList())

    companion object {
        private val SEAL = Any()

        internal fun issue(
            manifest: PrivacyExact12CapabilityManifestV1,
            row: PrivacyExact12CapabilityRowV1,
        ): PrivacyExact12CapabilityTupleAdmissionV1 {
            val archive = manifest.canonicalBytes()
            PrivacyNativeBridge.requireExact12CapabilityTuple(archive, row.protocolId)
            return PrivacyExact12CapabilityTupleAdmissionV1(
                row.protocolId,
                manifest.committedHeight,
                manifest.manifestDigest,
                row.operationSchemas,
                archive,
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
            )
        }

        internal fun requireSubmitProofConstruction(
            admission: PrivacyExact12CapabilityTupleAdmissionV1,
            protocolId: PrivacyProtocolIdV1,
            instructionArchive: ByteArray,
        ) {
            requireAuthentic(admission, protocolId)
            PrivacyNativeBridge.requireExact12SubmitProofConstruction(
                admission.manifestArchive,
                protocolId,
                instructionArchive,
            )
        }
    }
}

/** Fail-closed bridge from a committed manifest to retained privacy construction. */
object PrivacyExact12CapabilityAdmissionV1 {
    /**
     * Require committed availability and an exact native local compiled-profile tuple match.
     *
     * A local catalog is used only for equality. It never creates network availability, and the
     * retired [PrivacyCapabilitySnapshotV1] type has no admission overload.
     */
    @JvmStatic
    fun requireExact12CapabilityTupleV1(
        manifest: PrivacyExact12CapabilityManifestV1,
        protocolId: PrivacyProtocolIdV1,
    ): PrivacyExact12CapabilityTupleAdmissionV1 {
        val row = manifest.rowFor(protocolId)
        require(row.isNetworkAvailable()) {
            "Exact12 protocol ${protocolId.canonicalLabel} is not active and ready in committed state"
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

    /** Bind one canonical retained submit-proof instruction to its admitted committed tuple. */
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
        val committedHeight = uint64(
            manifest["committed_height"],
            "Exact12 capability manifest.committed_height",
        )
        require(manifest["consensus_policy"] is Map<*, *>) {
            "Exact12 capability manifest.consensus_policy must be present"
        }
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
                "Exact12 capability manifest.protocols[$index]",
            )
        }
        val manifestDigest = fixed32(
            manifest["manifest_digest"],
            "Exact12 capability manifest.manifest_digest",
        )
        return PrivacyExact12CapabilityManifestV1(
            version,
            committedHeight,
            rows,
            manifestDigest,
            canonicalArchive,
        )
    }

    private fun parseRow(
        value: Any?,
        expected: PrivacyProtocolIdV1,
        tupleMatch: Boolean,
        path: String,
    ): PrivacyExact12CapabilityRowV1 {
        val row = exactObject(
            value,
            setOf(
                "protocol_id",
                "operation_schemas",
                "execution_mode",
                "privacy_feature_mask",
                "compiled_profile",
                "readiness",
                "activation_state",
                "activation",
                "limitation",
            ),
            path,
        )
        val protocol = protocolTag(row["protocol_id"], "$path.protocol_id")
        require(protocol == expected) { "$path is out of canonical protocol order" }
        val operationSet = exactObject(
            row["operation_schemas"],
            setOf("primary", "secondary"),
            "$path.operation_schemas",
        )
        val primaryOperation = PrivacyOperationSchemaV1.fromCanonicalLabel(
            taggedUnit(
                operationSet["primary"],
                "operation_schema",
                "value",
                "$path.operation_schemas.primary",
            ),
        )
        val secondaryOperation = operationSet["secondary"]?.let {
            PrivacyOperationSchemaV1.fromCanonicalLabel(
                taggedUnit(
                    it,
                    "operation_schema",
                    "value",
                    "$path.operation_schemas.secondary",
                ),
            )
        }
        val operations = listOfNotNull(primaryOperation, secondaryOperation)
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
        val activationState = parseActivationState(
            row["activation_state"],
            "$path.activation_state",
        )
        require(activationState == projectedActivationState(row["activation"], "$path.activation")) {
            "$path.activation_state differs from the committed lifecycle"
        }
        val limitation = parseLimitation(row["limitation"], "$path.limitation")
        return PrivacyExact12CapabilityRowV1(
            protocol,
            operations,
            execution,
            featureMask,
            compiled,
            readiness,
            activationState,
            limitation,
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
            parseProtocolLimits(profile["protocol_limits"], protocol, "$path.protocol_limits"),
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
                PrivacyCapabilityReadinessV1(PrivacyCapabilityReadinessStateV1.AVAILABLE, null)
            }
            "available-experimental" -> {
                require(readiness["detail"] == null) { "$path.detail must be null" }
                PrivacyCapabilityReadinessV1(
                    PrivacyCapabilityReadinessStateV1.AVAILABLE_EXPERIMENTAL,
                    null,
                )
            }
            "unavailable" -> PrivacyCapabilityReadinessV1(
                PrivacyCapabilityReadinessStateV1.UNAVAILABLE,
                parseUnavailable(readiness["detail"], "$path.detail"),
            )
            else -> throw IllegalArgumentException("$path.readiness is not a closed readiness")
        }
    }

    private fun parseActivationState(value: Any?, path: String): PrivacyCapabilityActivationStateV1 {
        return when (taggedUnit(value, "activation_state", "detail", path)) {
            "not-registered" -> PrivacyCapabilityActivationStateV1.NOT_REGISTERED
            "proposed" -> PrivacyCapabilityActivationStateV1.PROPOSED
            "active" -> PrivacyCapabilityActivationStateV1.ACTIVE
            "suspended" -> PrivacyCapabilityActivationStateV1.SUSPENDED
            "retired" -> PrivacyCapabilityActivationStateV1.RETIRED
            else -> throw IllegalArgumentException("$path is not a closed activation state")
        }
    }

    private fun projectedActivationState(value: Any?, path: String): PrivacyCapabilityActivationStateV1 {
        if (value == null) return PrivacyCapabilityActivationStateV1.NOT_REGISTERED
        val activation = value as? Map<*, *>
            ?: throw IllegalArgumentException("$path must be a committed activation object")
        val lifecycle = activation["lifecycle"] as? Map<*, *>
            ?: throw IllegalArgumentException("$path.lifecycle must be present")
        return when (lifecycle["state"] as? String) {
            "proposed" -> PrivacyCapabilityActivationStateV1.PROPOSED
            "active" -> PrivacyCapabilityActivationStateV1.ACTIVE
            "suspended" -> PrivacyCapabilityActivationStateV1.SUSPENDED
            "retired" -> PrivacyCapabilityActivationStateV1.RETIRED
            else -> throw IllegalArgumentException("$path.lifecycle.state is not closed")
        }
    }

    private fun parseLimitation(value: Any?, path: String): PrivacyCapabilityLimitationV1? {
        if (value == null) return null
        return when (taggedUnit(value, "limitation", "detail", path)) {
            "missing-distribution-wide-knowledge-soundness-evidence" ->
                PrivacyCapabilityLimitationV1
                    .MISSING_DISTRIBUTION_WIDE_KNOWLEDGE_SOUNDNESS_EVIDENCE
            else -> throw IllegalArgumentException("$path is not a closed V1 limitation")
        }
    }

    private fun protocolTag(value: Any?, path: String): PrivacyProtocolIdV1 =
        PrivacyProtocolIdV1.fromCanonicalLabel(taggedUnit(value, "protocol", "value", path))

    private fun taggedUnit(value: Any?, tag: String, content: String, path: String): String {
        val tagged = exactObject(value, setOf(tag, content), path)
        require(tagged[content] == null) { "$path.$content must be null" }
        return text(tagged[tag], "$path.$tag")
    }

    private fun fixed32(value: Any?, path: String): PrivacyFixed32V1 {
        val bytes = list(value, path)
        require(bytes.size == 32) { "$path must contain exactly 32 bytes" }
        return PrivacyFixed32V1(
            ByteArray(32) { index ->
                val byte = uint32(bytes[index], "$path[$index]")
                require(byte <= 255) { "$path[$index] must fit uint8" }
                byte.toByte()
            },
        )
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
}

private fun expectedOperationSchemas(
    protocol: PrivacyProtocolIdV1,
): List<PrivacyOperationSchemaV1> =
    when (protocol) {
        PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0 ->
            listOf(PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1)
        PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 ->
            listOf(PrivacyOperationSchemaV1.ANONYMOUS_PGC_PAYMENT_ACTION_V1)
        PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 ->
            listOf(PrivacyOperationSchemaV1.VERANGE_RANGE_PROOF_V1)
        PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 ->
            listOf(
                PrivacyOperationSchemaV1.ZK_AMS_BATCH_ADMISSION_ACTION_V1,
                PrivacyOperationSchemaV1.ZK_AMS_PROVISION_ACCOUNT_ACTION_V1,
            )
        PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V0 ->
            listOf(PrivacyOperationSchemaV1.VEGA_CREDENTIAL_PRESENTATION_V1)
        PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V0 ->
            listOf(PrivacyOperationSchemaV1.ZK_X509_IDENTITY_PRESENTATION_V1)
        PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0 ->
            listOf(PrivacyOperationSchemaV1.JINDO_POLYNOMIAL_EVALUATION_V1)
        PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1 ->
            listOf(PrivacyOperationSchemaV1.BOOTLE_LANTERN_CREDENTIAL_PRESENTATION_V1)
        PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1 ->
            listOf(PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1)
        PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1 ->
            listOf(PrivacyOperationSchemaV1.FCMP_MEMBERSHIP_PAYMENT_V1)
        PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1 ->
            listOf(PrivacyOperationSchemaV1.IVM_PRIVATE_NOTE_ACTION_V1)
        PrivacyProtocolIdV1.PQ_MASP_STARK_V0 ->
            listOf(PrivacyOperationSchemaV1.PQ_MASP_NOTE_ACTION_V1)
    }

private fun expectedExecutionMode(protocol: PrivacyProtocolIdV1): PrivacyExecutionModeV1 =
    when (protocol) {
        PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0 ->
            PrivacyExecutionModeV1.AUTHORIZATION_ACTION
        PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
        PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
        -> PrivacyExecutionModeV1.PAYMENT_ACTION
        PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
        PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0,
        -> PrivacyExecutionModeV1.COMPONENT
        PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 -> PrivacyExecutionModeV1.ADMISSION_ACTION
        PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V0,
        PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V0,
        PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1,
        -> PrivacyExecutionModeV1.PRESENTATION_ACTION
        PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
        PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
        PrivacyProtocolIdV1.PQ_MASP_STARK_V0,
        -> PrivacyExecutionModeV1.NOTE_ACTION
    }

private fun expectedFeatureMask(protocol: PrivacyProtocolIdV1): Int = when (protocol) {
    PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0,
    PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0,
    -> 0
    PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 -> (1 shl 1) or (1 shl 2)
    PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 -> 1
    PrivacyProtocolIdV1.IROHA_ZK_AMS_V1,
    PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V0,
    PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V0,
    PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1,
    PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
    -> 1 shl 1
    PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
    PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
    -> 1 or (1 shl 1) or (1 shl 2)
    PrivacyProtocolIdV1.PQ_MASP_STARK_V0 ->
        1 or (1 shl 1) or (1 shl 2) or (1 shl 3) or (1 shl 4)
}
