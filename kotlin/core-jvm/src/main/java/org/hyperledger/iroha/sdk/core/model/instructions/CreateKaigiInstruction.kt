package org.hyperledger.iroha.sdk.core.model.instructions

private const val CREATE_KAIGI_ACTION = "CreateKaigi"
private val CREATE_KAIGI_ARGUMENTS = setOf(
    "action",
    "call.domain_id",
    "call.call_name",
    "host",
    "title",
    "description",
    "max_participants",
    "gas_rate_per_minute",
    "scheduled_start_ms",
    "billing_account",
    "privacy.mode",
    "privacy.state",
    "room_policy.policy",
    "room_policy.state",
    "relay_manifest.expiry_ms",
    "commitment.commitment",
    "commitment.alias_tag",
    "nullifier.digest",
    "nullifier.issued_at_ms",
    "roster_root",
    "proof",
)

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
    @JvmField internal val roomPolicy: KaigiInstructionUtils.RoomPolicy,
    @JvmField internal val relayManifest: KaigiInstructionUtils.RelayManifest?,
    @JvmField val commitment: String?,
    @JvmField val commitmentAliasTag: String?,
    @JvmField val nullifierDigest: String?,
    @JvmField val nullifierIssuedAtMs: Long?,
    @JvmField val rosterRoot: String?,
    @JvmField val proofBase64: String?,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    private val _metadata: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(metadata.toSortedMap())

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(host.isNotBlank()) { "host must not be blank" }
        if (maxParticipants != null) {
            require(maxParticipants != 0) { "maxParticipants must be greater than zero when provided" }
        }
        require(commitmentAliasTag == null) {
            "commitment aliasTag is off-chain only and must be omitted"
        }
        require(nullifierIssuedAtMs == null || nullifierIssuedAtMs == 0L) {
            "nullifier issuedAtMs is off-chain only and must be zero when provided"
        }
        require(nullifierIssuedAtMs == null || nullifierDigest != null) {
            "nullifier issuedAtMs requires nullifier digest"
        }
        if (proofBase64 != null) {
            KaigiInstructionUtils.requireBase64(proofBase64, "proof")
        }
        if (relayManifest != null) {
            KaigiInstructionUtils.validateRelayManifest(relayManifest)
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
            && roomPolicy.policy == other.roomPolicy.policy
            && roomPolicy.state == other.roomPolicy.state
            && relayManifestEquals(relayManifest, other.relayManifest)
            && commitment == other.commitment
            && commitmentAliasTag == other.commitmentAliasTag
            && nullifierDigest == other.nullifierDigest
            && nullifierIssuedAtMs == other.nullifierIssuedAtMs
            && rosterRoot == other.rosterRoot
            && proofBase64 == other.proofBase64
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
        result = 31 * result + roomPolicy.policy.hashCode()
        result = 31 * result + (roomPolicy.state?.hashCode() ?: 0)
        result = 31 * result + relayManifestHash(relayManifest)
        result = 31 * result + (commitment?.hashCode() ?: 0)
        result = 31 * result + (commitmentAliasTag?.hashCode() ?: 0)
        result = 31 * result + (nullifierDigest?.hashCode() ?: 0)
        result = 31 * result + (nullifierIssuedAtMs?.hashCode() ?: 0)
        result = 31 * result + (rosterRoot?.hashCode() ?: 0)
        result = 31 * result + (proofBase64?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CreateKaigiInstruction {
            KaigiInstructionUtils.requireKnownArguments(
                arguments,
                CREATE_KAIGI_ARGUMENTS,
                "metadata.",
                "relay_manifest.hop.",
            )
            KaigiInstructionUtils.requireAction(arguments, CREATE_KAIGI_ACTION)
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
            val roomPolicy = KaigiInstructionUtils.parseRoomPolicy(arguments, "room_policy")
            val relayManifest = KaigiInstructionUtils.parseRelayManifest(arguments, "relay_manifest")
            val commitment = arguments["commitment.commitment"]
                ?.let(KaigiInstructionUtils::canonicalizeHash)
            require(arguments["commitment.alias_tag"] == null) {
                "commitment aliasTag is off-chain only and must be omitted"
            }
            val nullifier = arguments["nullifier.digest"]
                ?.let(KaigiInstructionUtils::canonicalizeHash)
            val parsedNullifierIssuedAt = KaigiInstructionUtils.parseOptionalUnsignedLong(
                arguments["nullifier.issued_at_ms"], "nullifier.issued_at_ms",
            )
            require(parsedNullifierIssuedAt == null || parsedNullifierIssuedAt == 0L) {
                "nullifier issuedAtMs is off-chain only and must be zero when provided"
            }
            require(parsedNullifierIssuedAt == null || nullifier != null) {
                "nullifier issuedAtMs requires nullifier digest"
            }
            val nullifierIssuedAt = parsedNullifierIssuedAt.takeIf { nullifier != null }

            return create(
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
                roomPolicy = roomPolicy,
                relayManifest = relayManifest,
                commitment = commitment,
                commitmentAliasTag = null,
                nullifierDigest = nullifier,
                nullifierIssuedAtMs = nullifierIssuedAt,
                rosterRoot = arguments["roster_root"]?.let(KaigiInstructionUtils::canonicalizeHash),
                proofBase64 = arguments["proof"],
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
            roomPolicy: KaigiInstructionUtils.RoomPolicy = KaigiInstructionUtils.RoomPolicy("Authenticated", null),
            relayManifest: KaigiInstructionUtils.RelayManifest? = null,
            commitment: String? = null,
            commitmentAliasTag: String? = null,
            nullifierDigest: String? = null,
            nullifierIssuedAtMs: Long? = null,
            rosterRoot: String? = null,
            proofBase64: String? = null,
        ): CreateKaigiInstruction {
            val canonicalCommitment = KaigiInstructionUtils.canonicalizeOptionalHash(commitment)
            val canonicalNullifier = KaigiInstructionUtils.canonicalizeOptionalHash(nullifierDigest)
            val canonicalRosterRoot = KaigiInstructionUtils.canonicalizeOptionalHash(rosterRoot)
            val canonicalRelayManifest = relayManifest?.let(KaigiInstructionUtils::validateRelayManifest)
            val canonicalArguments = buildCanonicalArguments(
                callId, host, title, description, maxParticipants,
                gasRatePerMinute, metadata, scheduledStartMs, billingAccount,
                privacyMode, roomPolicy, canonicalRelayManifest, canonicalCommitment,
                canonicalNullifier, nullifierIssuedAtMs, canonicalRosterRoot, proofBase64,
            )
            return CreateKaigiInstruction(
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
                roomPolicy = roomPolicy,
                relayManifest = canonicalRelayManifest,
                commitment = canonicalCommitment,
                commitmentAliasTag = commitmentAliasTag,
                nullifierDigest = canonicalNullifier,
                nullifierIssuedAtMs = nullifierIssuedAtMs,
                rosterRoot = canonicalRosterRoot,
                proofBase64 = proofBase64,
                _arguments = KaigiInstructionUtils.immutableArguments(canonicalArguments),
            )
        }

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
            roomPolicy: KaigiInstructionUtils.RoomPolicy,
            relayManifest: KaigiInstructionUtils.RelayManifest?,
            commitment: String?,
            nullifierDigest: String?,
            nullifierIssuedAtMs: Long?,
            rosterRoot: String?,
            proofBase64: String?,
        ): Map<String, String> {
            val args = LinkedHashMap<String, String>()
            args["action"] = CREATE_KAIGI_ACTION
            KaigiInstructionUtils.appendCallId(callId, args, "call")
            args["host"] = host
            if (title != null) args["title"] = title
            if (description != null) args["description"] = description
            if (maxParticipants != null) {
                args["max_participants"] = Integer.toUnsignedString(maxParticipants)
            }
            args["gas_rate_per_minute"] = java.lang.Long.toUnsignedString(gasRatePerMinute)
            KaigiInstructionUtils.appendMetadata(metadata, args, "metadata")
            if (scheduledStartMs != null) {
                args["scheduled_start_ms"] = java.lang.Long.toUnsignedString(scheduledStartMs)
            }
            if (billingAccount != null) args["billing_account"] = billingAccount
            KaigiInstructionUtils.appendPrivacyMode(privacyMode, args, "privacy")
            KaigiInstructionUtils.appendRoomPolicy(roomPolicy, args, "room_policy")
            KaigiInstructionUtils.appendRelayManifest(relayManifest, args, "relay_manifest")
            if (commitment != null) {
                args["commitment.commitment"] = commitment
            }
            if (nullifierDigest != null) {
                args["nullifier.digest"] = nullifierDigest
                if (nullifierIssuedAtMs != null) {
                    args["nullifier.issued_at_ms"] = java.lang.Long.toUnsignedString(nullifierIssuedAtMs)
                }
            }
            if (rosterRoot != null) args["roster_root"] = rosterRoot
            if (proofBase64 != null) args["proof"] = proofBase64
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
