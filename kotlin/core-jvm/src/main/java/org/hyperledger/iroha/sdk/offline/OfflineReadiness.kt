package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** A reason Torii cannot currently accept Offline operations for an asset definition. */
class OfflineReadinessBlocker(
    code: String,
    message: String,
) {
    @JvmField
    val code: String = requireStableErrorCode(code, "code")

    @JvmField
    val message: String = requireExactText(message, "message")

    override fun equals(other: Any?): Boolean =
        other is OfflineReadinessBlocker && code == other.code && message == other.message

    override fun hashCode(): Int = 31 * code.hashCode() + message.hashCode()
}

/** Readiness of the requested asset definition for Offline operations. */
class OfflineReadiness(
    assetDefinitionId: String,
    evaluatedBlockHeight: BigInteger,
    evaluatedBlockHash: String,
    @JvmField val ready: Boolean,
    blockers: List<OfflineReadinessBlocker>,
) {
    @JvmField
    val assetDefinitionId: String = requireExactText(assetDefinitionId, "assetDefinitionId")

    @JvmField
    val evaluatedBlockHeight: BigInteger = evaluatedBlockHeight.also {
        require(it >= BigInteger.ZERO && it <= U64_MAX) {
            "evaluatedBlockHeight must fit in an unsigned 64-bit integer"
        }
    }

    @JvmField
    val evaluatedBlockHash: String = requireLowercaseHash(evaluatedBlockHash)

    @JvmField
    val blockers: List<OfflineReadinessBlocker> = blockers.toList().also {
        require(ready == it.isEmpty()) {
            "ready must be true exactly when blockers is empty"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is OfflineReadiness &&
            assetDefinitionId == other.assetDefinitionId &&
            evaluatedBlockHeight == other.evaluatedBlockHeight &&
            evaluatedBlockHash == other.evaluatedBlockHash &&
            ready == other.ready &&
            blockers == other.blockers

    override fun hashCode(): Int {
        var result = assetDefinitionId.hashCode()
        result = 31 * result + evaluatedBlockHeight.hashCode()
        result = 31 * result + evaluatedBlockHash.hashCode()
        result = 31 * result + ready.hashCode()
        result = 31 * result + blockers.hashCode()
        return result
    }

    private companion object {
        val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    }
}

private fun requireExactText(value: String, field: String): String {
    require(value.isNotEmpty() && value == value.trim()) { "$field must be exact non-empty text" }
    return value
}

private fun requireLowercaseHash(value: String): String {
    require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "evaluatedBlockHash must be exact lowercase 32-byte hexadecimal"
    }
    return value
}
