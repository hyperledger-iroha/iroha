package org.hyperledger.iroha.sdk.client.stream

import java.util.Collections
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/** Status of a transaction in a pipeline event (`status`, a variant name). */
enum class TransactionEventStatus(val wireName: String) {
    QUEUED("Queued"),
    EXPIRED("Expired"),
    APPROVED("Approved"),
    REJECTED("Rejected"),
}

/** Status of a block in a pipeline event (`status`, a variant name). */
enum class BlockEventStatus(val wireName: String) {
    CREATED("Created"),
    APPROVED("Approved"),
    REJECTED("Rejected"),
    COMMITTED("Committed"),
    APPLIED("Applied"),
}

/** Public rejection class of a rejected transaction (`rejection_code`). */
enum class TransactionRejectionCode(val wireName: String) {
    ACCOUNT_DOES_NOT_EXIST("account_does_not_exist"),
    LIMIT_CHECK("limit_check"),
    VALIDATION("validation"),
    INSTRUCTION_EXECUTION("instruction_execution"),
    IVM_EXECUTION("ivm_execution"),
    TRIGGER_EXECUTION("trigger_execution"),
}

/** Kinds of data events reported with category `Data` (besides proof events). */
enum class DataEventKind(val wireName: String) {
    PEER("Peer"),
    DOMAIN("Domain"),
    ACCOUNT("Account"),
    ASSET("Asset"),
    ASSET_DEFINITION("AssetDefinition"),
    TRIGGER("Trigger"),
    ROLE("Role"),
    CONFIGURATION("Configuration"),
    EXECUTOR("Executor"),
    VERIFYING_KEY("VerifyingKey"),
    RUNTIME_UPGRADE("RuntimeUpgrade"),
    SMART_CONTRACT("SmartContract"),
    SORADNS("Soradns"),
    SORAFS("Sorafs"),
    MUSUBI("Musubi"),
    SPACE_DIRECTORY("SpaceDirectory"),
    ESCROW("Escrow"),
    ORACLE("Oracle"),
    GOVERNANCE("Governance"),
    SOCIAL("Social"),
    BRIDGE("Bridge"),
    GAME_SESSION("GameSession"),
    SCCP("Sccp"),
}

/** Kinds of events reported with category `Other`. */
enum class OtherEventKind(val wireName: String) {
    TIME("Time"),
    EXECUTE_TRIGGER("ExecuteTrigger"),
    TRIGGER_COMPLETED("TriggerCompleted"),
    OTHER("Other"),
}

/** Why proofs were pruned (`origin`). */
enum class ProofPruneOrigin(val wireName: String) {
    INSERT("Insert"),
    MANUAL("Manual"),
}

/**
 * One decoded `/v1/events/sse` payload (`specs/torii/collection_queries.md`, "Event streams").
 *
 * Every payload is a JSON object with `category` (`Pipeline`, `Data` or `Other`) and `event`.
 * [parse] never throws: a payload with an unknown `event`, an unknown status or a shape this SDK
 * does not know decodes as [Unknown], so new server events never fail a stream. [json] keeps the
 * complete payload. `summary` members are diagnostic text without a stable format.
 */
sealed class ToriiEvent(
    /** The complete payload. */
    @JvmField val json: JsonObject,
) {
    /** The `category` member. */
    val category: String? get() = (json["category"] as? JsonString)?.value

    /** The `event` member. */
    val event: String? get() = (json["event"] as? JsonString)?.value

    override fun equals(other: Any?): Boolean =
        other != null && other.javaClass == javaClass && (other as ToriiEvent).json == json

    override fun hashCode(): Int = json.hashCode()

    override fun toString(): String = "${javaClass.simpleName}$json"

    /** `Pipeline`/`Transaction`: a transaction changed status. */
    class Transaction internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val hash: String = read.string("hash")
        @JvmField val laneId: Long = read.unsigned("lane_id")
        @JvmField val dataspaceId: Long = read.unsigned("dataspace_id")
        @JvmField val blockHeight: Long? = read.unsignedOrNull("block_height")
        @JvmField val status: TransactionEventStatus = read.enum("status", TransactionEventStatus.values()) { it.wireName }

        /** Set for [TransactionEventStatus.REJECTED]. */
        @JvmField val rejectionCode: TransactionRejectionCode? =
            read.enumOrNull("rejection_code", TransactionRejectionCode.values()) { it.wireName }

        /** Fixed public text for the rejection, when rejected. */
        @JvmField val rejectionReason: String? = read.stringOrNull("rejection_reason")
    }

    /** `Pipeline`/`Block`: a block changed status. */
    class Block internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val status: BlockEventStatus = read.enum("status", BlockEventStatus.values()) { it.wireName }

        /** The block rejection variant name (e.g. `EmptyBlock`) for [BlockEventStatus.REJECTED]. */
        @JvmField val rejectionCode: String? = read.stringOrNull("rejection_code")
    }

    /** `Pipeline`/`Warning`. */
    class Warning internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val kind: String = read.string("kind")
        @JvmField val details: String = read.string("details")
        @JvmField val height: Long = read.unsigned("height")
    }

    /** `Pipeline`/`Witness`: execution witness summary of a block. */
    class Witness internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val blockHash: String = read.string("block_hash")
        @JvmField val height: Long = read.unsigned("height")
        @JvmField val view: Long = read.unsigned("view")
        @JvmField val epoch: Long = read.unsigned("epoch")
        @JvmField val readCount: Long = read.unsigned("read_count")
        @JvmField val writeCount: Long = read.unsigned("write_count")
    }

    /** Common fields of `Data`/`ProofVerified` and `Data`/`ProofRejected`. */
    abstract class ProofVerdict internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val backend: String = read.string("backend")
        @JvmField val proofHash: String = read.string("proof_hash")
        @JvmField val callHash: String? = read.stringOrNull("call_hash")
        @JvmField val envelopeHash: String? = read.stringOrNull("envelope_hash")

        /** `backend::name` of the verifying key, when known. */
        @JvmField val vkRef: String? = read.stringOrNull("vk_ref")
        @JvmField val vkCommitment: String? = read.stringOrNull("vk_commitment")
    }

    /** `Data`/`ProofVerified`. */
    class ProofVerified internal constructor(json: JsonObject) : ProofVerdict(json)

    /** `Data`/`ProofRejected`. */
    class ProofRejected internal constructor(json: JsonObject) : ProofVerdict(json)

    /** `Data`/`ProofPruned`. */
    class ProofPruned internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val backend: String = read.string("backend")
        @JvmField val removedCount: Long = read.unsigned("removed_count")
        @JvmField val remaining: Long = read.unsigned("remaining")
        @JvmField val cap: Long = read.unsigned("cap")
        @JvmField val graceBlocks: Long = read.unsigned("grace_blocks")
        @JvmField val pruneBatch: Long = read.unsigned("prune_batch")
        @JvmField val prunedAtHeight: Long = read.unsigned("pruned_at_height")
        @JvmField val prunedBy: String = read.string("pruned_by")
        @JvmField val origin: ProofPruneOrigin = read.enum("origin", ProofPruneOrigin.values()) { it.wireName }
        @JvmField val removed: List<PrunedProof> = read.objects("removed").map(::PrunedProof)

        /** One pruned proof. */
        class PrunedProof internal constructor(json: JsonObject) {
            private val read = EventReader(json)

            @JvmField val backend: String = read.string("backend")
            @JvmField val proofHash: String = read.string("proof_hash")
        }
    }

    /** Any other `Data` event; [summary] is diagnostic text. */
    class DataChange internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val kind: DataEventKind = read.enum("event", DataEventKind.values()) { it.wireName }
        @JvmField val summary: String? = read.stringOrNull("summary")
    }

    /** A non-data, non-pipeline event (category `Other`); [summary] is diagnostic text. */
    class Other internal constructor(json: JsonObject) : ToriiEvent(json) {
        private val read = EventReader(json)

        @JvmField val kind: OtherEventKind = read.enum("event", OtherEventKind.values()) { it.wireName }
        @JvmField val summary: String? = read.stringOrNull("summary")
    }

    /** A payload this SDK does not recognise; [raw] is the exact `data` text. */
    class Unknown internal constructor(json: JsonObject, @JvmField val raw: String) : ToriiEvent(json)

    companion object {
        /** Decode one SSE `data` payload; never throws. */
        @JvmStatic
        fun parse(data: String): ToriiEvent {
            val json = try {
                Json.parse(data) as? JsonObject
            } catch (_: IllegalArgumentException) {
                null
            } ?: return Unknown(JsonObject.EMPTY, data)
            return try {
                decode(json) ?: Unknown(json, data)
            } catch (_: IllegalArgumentException) {
                Unknown(json, data)
            }
        }

        private fun decode(json: JsonObject): ToriiEvent? {
            val category = (json["category"] as? JsonString)?.value ?: return null
            val event = (json["event"] as? JsonString)?.value ?: return null
            return when (category) {
                "Pipeline" -> when (event) {
                    "Transaction" -> Transaction(json)
                    "Block" -> Block(json)
                    "Warning" -> Warning(json)
                    "Witness" -> Witness(json)
                    else -> null
                }
                "Data" -> when (event) {
                    "ProofVerified" -> ProofVerified(json)
                    "ProofRejected" -> ProofRejected(json)
                    "ProofPruned" -> ProofPruned(json)
                    else -> DataChange(json)
                }
                "Other" -> Other(json)
                else -> null
            }
        }
    }
}

/** Typed member access for event decoding; failures become [IllegalArgumentException]. */
internal class EventReader(private val json: JsonObject) {
    private fun value(name: String): Json? = json[name]?.takeUnless { it === JsonNull }

    fun string(name: String): String = stringOrNull(name) ?: throw IllegalArgumentException("`$name` is missing")

    fun stringOrNull(name: String): String? = when (val value = value(name)) {
        null -> null
        is JsonString -> value.value
        else -> throw IllegalArgumentException("`$name` must be a string")
    }

    fun unsigned(name: String): Long = unsignedOrNull(name) ?: throw IllegalArgumentException("`$name` is missing")

    fun unsignedOrNull(name: String): Long? = when (val value = value(name)) {
        null -> null
        is JsonNumber -> {
            val integer = if (value.isInteger) value.toBigInteger() else null
            require(integer != null && integer.signum() >= 0 && integer.bitLength() < 64) {
                "`$name` must be an unsigned 64-bit integer"
            }
            integer.toLong()
        }
        else -> throw IllegalArgumentException("`$name` must be an unsigned integer")
    }

    fun <E> enum(name: String, values: Array<E>, wireName: (E) -> String): E =
        enumOrNull(name, values, wireName) ?: throw IllegalArgumentException("`$name` is missing")

    fun <E> enumOrNull(name: String, values: Array<E>, wireName: (E) -> String): E? {
        val raw = stringOrNull(name) ?: return null
        return values.firstOrNull { wireName(it) == raw } ?: throw IllegalArgumentException("unknown `$name` `$raw`")
    }

    fun objects(name: String): List<JsonObject> = when (val value = value(name)) {
        null -> emptyList()
        is JsonArray -> Collections.unmodifiableList(
            value.items.map { it as? JsonObject ?: throw IllegalArgumentException("`$name` must hold objects") },
        )
        else -> throw IllegalArgumentException("`$name` must be an array")
    }
}

/** Typed listener for [ToriiEventStreamClient.subscribe]. */
interface ToriiEventListener {
    /** The stream was established. */
    fun onOpen() {}

    /** One decoded event; unknown payloads arrive as [ToriiEvent.Unknown]. */
    fun onEvent(event: ToriiEvent)

    /** Torii ended the stream with a terminal `stream_error` (for example `stream_lagged`). */
    fun onStreamError(error: ToriiStreamException) {}

    /** The stream ended normally. */
    fun onClosed() {}

    /** The stream failed (transport, HTTP error such as `invalid_filter`, or a malformed frame). */
    fun onError(error: Throwable) {}
}
