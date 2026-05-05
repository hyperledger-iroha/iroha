package org.hyperledger.iroha.sdk.core.model.instructions

private const val CREATE_KAIGI_ACTION = "CreateKaigi"

/** Typed representation of `CreateKaigi` instructions. */
class CreateKaigiInstruction internal constructor(
    @JvmField internal val callId: KaigiInstructionUtils.CallId,
    @JvmField val host: String,
    @JvmField val title: String?,
    @JvmField val description: String?,
    @JvmField val maxParticipants: Int?,
    @JvmField val gasRatePerMinute: Long,
    metadata: Map<String, String>,
    @JvmField val scheduledStartMs: Long?,
    @JvmField val billingAccount: String?,
    @JvmField internal val privacyMode: KaigiInstructionUtils.PrivacyMode,
    @JvmField internal val relayManifest: KaigiInstructionUtils.RelayManifest?,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(host.isNotBlank()) { "host must not be blank" }
        if (maxParticipants != null) {
            require(maxParticipants > 0) { "maxParticipants must be greater than zero when provided" }
        }
        require(gasRatePerMinute >= 0) { "gasRatePerMinute must be non-negative" }
        if (scheduledStartMs != null) {
            require(scheduledStartMs >= 0) { "scheduledStartMs must be non-negative" }
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CreateKaigiInstruction) return false
        return callId.domainId == other.callId.domainId
            && callId.callName == other.callId.callName
            && host == other.host
            && title == other.title
            && description == other.description
            && maxParticipants == other.maxParticipants
            && gasRatePerMinute == other.gasRatePerMinute
            && _metadata == other._metadata
            && scheduledStartMs == other.scheduledStartMs
            && billingAccount == other.billingAccount
            && privacyMode.mode == other.privacyMode.mode
            && privacyMode.state == other.privacyMode.state
            && relayManifestEquals(relayManifest, other.relayManifest)
    }

    override fun hashCode(): Int {
        var result = callId.domainId.hashCode()
        result = 31 * result + callId.callName.hashCode()
        result = 31 * result + host.hashCode()
        result = 31 * result + (title?.hashCode() ?: 0)
        result = 31 * result + (description?.hashCode() ?: 0)
        result = 31 * result + (maxParticipants ?: 0)
        result = 31 * result + gasRatePerMinute.hashCode()
        result = 31 * result + _metadata.hashCode()
        result = 31 * result + (scheduledStartMs?.hashCode() ?: 0)
        result = 31 * result + (billingAccount?.hashCode() ?: 0)
        result = 31 * result + privacyMode.mode.hashCode()
        result = 31 * result + (privacyMode.state?.hashCode() ?: 0)
        result = 31 * result + relayManifestHash(relayManifest)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CreateKaigiInstruction {
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val host = KaigiInstructionUtils.require(arguments, "host")
            val title = arguments["title"]
            val description = arguments["description"]
            val maxParticipants = KaigiInstructionUtils.parseOptionalPositiveInt(
                arguments["max_participants"], "max_participants",
            )
            val gasRate = KaigiInstructionUtils.parseUnsignedLong(
                arguments.getOrDefault("gas_rate_per_minute", "0"), "gas_rate_per_minute",
            )
            val metadata = KaigiInstructionUtils.extractMetadata(arguments, "metadata")
            val scheduled = KaigiInstructionUtils.parseOptionalUnsignedLong(
                arguments["scheduled_start_ms"], "scheduled_start_ms",
            )
            val billingAccount = arguments["billing_account"]
            val privacyMode = KaigiInstructionUtils.parsePrivacyMode(arguments, "privacy")
            val relayManifest = KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest")

            return CreateKaigiInstruction(
                callId = callId,
                host = host,
                title = title,
                description = description,
                maxParticipants = maxParticipants,
                gasRatePerMinute = gasRate,
                metadata = metadata,
                scheduledStartMs = scheduled,
                billingAccount = billingAccount,
                privacyMode = privacyMode,
                relayManifest = relayManifest,
                _arguments = LinkedHashMap(arguments),
            )
        }

        @JvmStatic
        internal fun create(
            callId: KaigiInstructionUtils.CallId,
            host: String,
            title: String? = null,
            description: String? = null,
            maxParticipants: Int? = null,
            gasRatePerMinute: Long = 0,
            metadata: Map<String, String> = emptyMap(),
            scheduledStartMs: Long? = null,
            billingAccount: String? = null,
            privacyMode: KaigiInstructionUtils.PrivacyMode = KaigiInstructionUtils.PrivacyMode("Transparent", null),
            relayManifest: KaigiInstructionUtils.RelayManifest? = null,
        ): CreateKaigiInstruction = CreateKaigiInstruction(
            callId = callId,
            host = host,
            title = title,
            description = description,
            maxParticipants = maxParticipants,
            gasRatePerMinute = gasRatePerMinute,
            metadata = metadata,
            scheduledStartMs = scheduledStartMs,
            billingAccount = billingAccount,
            privacyMode = privacyMode,
            relayManifest = relayManifest,
            _arguments = buildCanonicalArguments(
                callId, host, title, description, maxParticipants,
                gasRatePerMinute, metadata, scheduledStartMs, billingAccount,
                privacyMode, relayManifest,
            ),
        )

        private fun buildCanonicalArguments(
            callId: KaigiInstructionUtils.CallId,
            host: String,
            title: String?,
            description: String?,
            maxParticipants: Int?,
            gasRatePerMinute: Long,
            metadata: Map<String, String>,
            scheduledStartMs: Long?,
            billingAccount: String?,
            privacyMode: KaigiInstructionUtils.PrivacyMode,
            relayManifest: KaigiInstructionUtils.RelayManifest?,
        ): Map<String, String> {
            val args = LinkedHashMap<String, String>()
            args["action"] = CREATE_KAIGI_ACTION
            KaigiInstructionUtils.appendCallId(callId, args, "call")
            args["host"] = host
            if (title != null) args["title"] = title
            if (description != null) args["description"] = description
            if (maxParticipants != null) args["max_participants"] = maxParticipants.toString()
            args["gas_rate_per_minute"] = java.lang.Long.toUnsignedString(gasRatePerMinute)
            KaigiInstructionUtils.appendMetadata(metadata, args, "metadata")
            if (scheduledStartMs != null) {
                args["scheduled_start_ms"] = java.lang.Long.toUnsignedString(scheduledStartMs)
            }
            if (billingAccount != null) args["billing_account"] = billingAccount
            KaigiInstructionUtils.appendPrivacyMode(privacyMode, args, "privacy")
            KaigiInstructionUtils.appendRelayManifest(relayManifest, args, "relay_manifest")
            return args
        }

        internal fun relayManifestEquals(
            first: KaigiInstructionUtils.RelayManifest?,
            second: KaigiInstructionUtils.RelayManifest?,
        ): Boolean {
            if (first === second) return true
            if (first == null || second == null) return false
            if (first.expiryMs != second.expiryMs) return false
            val hops1 = first.hops
            val hops2 = second.hops
            if (hops1.size != hops2.size) return false
            for (index in hops1.indices) {
                val left = hops1[index]
                val right = hops2[index]
                if (left.relayId != right.relayId
                    || left.hpkePublicKey != right.hpkePublicKey
                    || left.weight != right.weight
                ) return false
            }
            return true
        }

        internal fun relayManifestHash(manifest: KaigiInstructionUtils.RelayManifest?): Int {
            if (manifest == null) return 0
            var result = (manifest.expiryMs?.hashCode() ?: 0)
            for (hop in manifest.hops) {
                result = 31 * result + (hop.relayId?.hashCode() ?: 0)
                result = 31 * result + (hop.hpkePublicKey?.hashCode() ?: 0)
                result = 31 * result + (hop.weight?.hashCode() ?: 0)
            }
            return result
        }
    }
}
