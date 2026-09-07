package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.JsonValue

/** Canonical V1 CreateKaigi with mandatory host authorization for a private call. */
class CreateKaigiInstruction private constructor(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val host: String,
    @JvmField val title: String?,
    @JvmField val description: String?,
    @JvmField val maxParticipants: Int?,
    @JvmField val gasRatePerMinute: Long,
    metadata: Map<String, JsonValue>,
    @JvmField val scheduledStartMs: Long?,
    @JvmField val billingAccount: String?,
    @JvmField val privacyMode: KaigiInstructionUtils.PrivacyMode,
    @JvmField val roomPolicy: KaigiInstructionUtils.RoomPolicy,
    relayManifest: KaigiInstructionUtils.RelayManifest?,
    @JvmField val commitment: KaigiAuthorizationScalarV1?,
    @JvmField val nullifierDigest: KaigiAuthorizationScalarV1?,
    rosterRoot: String?,
    @JvmField val proofBase64: String?,
) : KaigiWireInstructionV1 {
    val metadata: Map<String, JsonValue> = Collections.unmodifiableMap(LinkedHashMap(metadata.toSortedMap()))
    @JvmField val relayManifest = relayManifest?.let(KaigiInstructionUtils::validateRelayManifest)
    @JvmField val rosterRoot = rosterRoot?.let(KaigiInstructionUtils::canonicalRosterRoot)

    init {
        require(host.isNotBlank()) { "host must not be blank" }
        require(maxParticipants == null || maxParticipants in 1..MAX_PARTICIPANTS_V1) {
            "maxParticipants must be between 1 and $MAX_PARTICIPANTS_V1 when provided"
        }
        KaigiInstructionUtils.requireAuthorizationArtifacts(commitment, nullifierDigest, this.rosterRoot, proofBase64)
        if (privacyMode.mode == "ZkRosterV1") {
            require(commitment != null) { "private CreateKaigi requires complete host authorization" }
        } else {
            require(commitment == null) { "transparent CreateKaigi must omit privacy artifacts" }
        }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> by lazy {
        KaigiInstructionUtils.immutableArguments(buildMap {
            put("action", "CreateKaigi")
            KaigiInstructionUtils.appendCallId(callId, this, "call")
            put("host", host)
            title?.let { put("title", it) }
            description?.let { put("description", it) }
            maxParticipants?.let { put("max_participants", it.toString()) }
            put("gas_rate_per_minute", java.lang.Long.toUnsignedString(gasRatePerMinute))
            this@CreateKaigiInstruction.metadata.forEach { (key, value) -> put("metadata.$key", value.canonicalJson) }
            scheduledStartMs?.let { put("scheduled_start_ms", java.lang.Long.toUnsignedString(it)) }
            billingAccount?.let { put("billing_account", it) }
            KaigiInstructionUtils.appendPrivacyMode(privacyMode, this, "privacy")
            KaigiInstructionUtils.appendRoomPolicy(roomPolicy, this, "room_policy")
            KaigiInstructionUtils.appendRelayManifest(this@CreateKaigiInstruction.relayManifest, this, "relay_manifest")
            commitment?.let { put("commitment.commitment", it.toHex()) }
            nullifierDigest?.let { put("nullifier.digest", it.toHex()) }
            this@CreateKaigiInstruction.rosterRoot?.let { put("roster_root", it) }
            proofBase64?.let { put("proof", it) }
        })
    }
    override fun equals(other: Any?): Boolean = other is CreateKaigiInstruction && arguments == other.arguments
    override fun hashCode(): Int = arguments.hashCode()

    companion object {
        const val MAX_PARTICIPANTS_V1 = 4_096

        @JvmStatic
        @JvmOverloads
        fun create(
            callId: KaigiInstructionUtils.CallId,
            host: String,
            title: String? = null,
            description: String? = null,
            maxParticipants: Int? = null,
            gasRatePerMinute: Long = 0,
            metadata: Map<String, JsonValue> = emptyMap(),
            scheduledStartMs: Long? = null,
            billingAccount: String? = null,
            privacyMode: KaigiInstructionUtils.PrivacyMode = KaigiInstructionUtils.PrivacyMode("Transparent", null),
            roomPolicy: KaigiInstructionUtils.RoomPolicy = KaigiInstructionUtils.RoomPolicy("Authenticated", null),
            relayManifest: KaigiInstructionUtils.RelayManifest? = null,
            commitment: KaigiAuthorizationScalarV1? = null,
            nullifierDigest: KaigiAuthorizationScalarV1? = null,
            rosterRoot: String? = null,
            proofBase64: String? = null,
        ): CreateKaigiInstruction = CreateKaigiInstruction(
            callId, host, title, description, maxParticipants, gasRatePerMinute, metadata,
            scheduledStartMs, billingAccount, privacyMode, roomPolicy, relayManifest,
            commitment, nullifierDigest, rosterRoot, proofBase64,
        )

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CreateKaigiInstruction {
            KaigiInstructionUtils.requireKnownArguments(arguments, setOf(
                "action", "call.domain_id", "call.call_name", "host", "title", "description",
                "max_participants", "gas_rate_per_minute", "scheduled_start_ms", "billing_account",
                "privacy.mode", "privacy.state", "room_policy.policy", "room_policy.state",
                "relay_manifest.expiry_ms", "commitment.commitment", "nullifier.digest", "roster_root", "proof",
            ), "metadata.", "relay_manifest.hop.")
            KaigiInstructionUtils.requireAction(arguments, "CreateKaigi")
            return create(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                host = KaigiInstructionUtils.require(arguments, "host"),
                title = arguments["title"], description = arguments["description"],
                maxParticipants = KaigiInstructionUtils.parseOptionalPositiveInt(arguments["max_participants"], "max_participants"),
                gasRatePerMinute = KaigiInstructionUtils.parseUnsignedLong(arguments.getOrDefault("gas_rate_per_minute", "0"), "gas_rate_per_minute"),
                metadata = KaigiInstructionUtils.extractMetadata(arguments, "metadata").mapValues { JsonValue.parse(it.value) },
                scheduledStartMs = KaigiInstructionUtils.parseOptionalUnsignedLong(arguments["scheduled_start_ms"], "scheduled_start_ms"),
                billingAccount = arguments["billing_account"],
                privacyMode = KaigiInstructionUtils.parsePrivacyMode(arguments, "privacy"),
                roomPolicy = KaigiInstructionUtils.parseRoomPolicy(arguments, "room_policy"),
                relayManifest = KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest"),
                commitment = arguments["commitment.commitment"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                nullifierDigest = arguments["nullifier.digest"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                rosterRoot = arguments["roster_root"], proofBase64 = arguments["proof"],
            )
        }

        internal fun relayManifestEquals(first: KaigiInstructionUtils.RelayManifest?, second: KaigiInstructionUtils.RelayManifest?): Boolean = first == second
        internal fun relayManifestHash(manifest: KaigiInstructionUtils.RelayManifest?): Int = manifest?.hashCode() ?: 0
    }
}
