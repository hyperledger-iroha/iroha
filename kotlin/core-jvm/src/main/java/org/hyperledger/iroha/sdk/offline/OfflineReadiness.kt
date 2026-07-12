package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder

/** A reason Torii cannot currently accept Offline operations for an asset definition. */
class OfflineReadinessBlocker(
    code: String,
    message: String,
) {
    @JvmField
    val code: String = requireReadinessStableCode(code, "code")

    @JvmField
    val message: String = requireExactText(message, "message").also {
        require(it.codePointCount(0, it.length) <= 1024) {
            "message must not exceed 1024 Unicode characters"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is OfflineReadinessBlocker && code == other.code && message == other.message

    override fun hashCode(): Int = 31 * code.hashCode() + message.hashCode()
}

/** Stable registry identity of the verifier selected for Offline transfers. */
class OfflineVerifierId(
    backend: String,
    name: String,
) {
    @JvmField
    val backend: String = requireBoundedText(backend, "backend", 256)

    @JvmField
    val name: String = requireBoundedText(name, "name", 256)

    override fun equals(other: Any?): Boolean =
        other is OfflineVerifierId && backend == other.backend && name == other.name

    override fun hashCode(): Int = 31 * backend.hashCode() + name.hashCode()
}

/** Key-material-free transfer verifier active at a readiness snapshot. */
class OfflineActiveTransferVerifier(
    @JvmField val id: OfflineVerifierId,
    version: Long,
    circuitId: String,
    commitment: String,
    publicInputsSchemaHash: String,
    maxProofBytes: Long,
    activationHeight: BigInteger,
    withdrawalHeight: BigInteger?,
) {
    @JvmField
    val version: Long = version.also {
        require(it in 0..U32_MAX) { "version must fit in an unsigned 32-bit integer" }
    }

    @JvmField
    val circuitId: String = requireExactText(circuitId, "circuitId")

    @JvmField
    val commitment: String = requireLowercaseHash(commitment, "commitment")

    @JvmField
    val publicInputsSchemaHash: String =
        requireLowercaseHash(publicInputsSchemaHash, "publicInputsSchemaHash")

    @JvmField
    val maxProofBytes: Long = maxProofBytes.also {
        require(it in 1..U32_MAX) {
            "maxProofBytes must fit in a positive unsigned 32-bit integer"
        }
    }

    @JvmField
    val activationHeight: BigInteger =
        requireReadinessU64(activationHeight, "activationHeight")

    @JvmField
    val withdrawalHeight: BigInteger? = withdrawalHeight?.let {
        requireReadinessU64(it, "withdrawalHeight").also { height ->
            require(height > BigInteger.ZERO && height > activationHeight) {
                "withdrawalHeight must be greater than activationHeight"
            }
        }
    }

    fun isActiveAt(height: BigInteger): Boolean =
        activationHeight <= height && (withdrawalHeight == null || height < withdrawalHeight)

    override fun equals(other: Any?): Boolean =
        other is OfflineActiveTransferVerifier &&
            id == other.id &&
            version == other.version &&
            circuitId == other.circuitId &&
            commitment == other.commitment &&
            publicInputsSchemaHash == other.publicInputsSchemaHash &&
            maxProofBytes == other.maxProofBytes &&
            activationHeight == other.activationHeight &&
            withdrawalHeight == other.withdrawalHeight

    override fun hashCode(): Int {
        var result = id.hashCode()
        result = 31 * result + version.hashCode()
        result = 31 * result + circuitId.hashCode()
        result = 31 * result + commitment.hashCode()
        result = 31 * result + publicInputsSchemaHash.hashCode()
        result = 31 * result + maxProofBytes.hashCode()
        result = 31 * result + activationHeight.hashCode()
        result = 31 * result + (withdrawalHeight?.hashCode() ?: 0)
        return result
    }
}

/** Key-material-free top-up shield verifier active at a readiness snapshot. */
typealias OfflineActiveTopUpShieldVerifier = OfflineActiveTransferVerifier

/** Readiness of the requested asset definition for Offline operations. */
class OfflineReadiness(
    assetDefinitionId: String,
    assetScale: Long?,
    evaluatedBlockHeight: BigInteger,
    evaluatedBlockHash: String,
    @JvmField val activeTransferVerifier: OfflineActiveTransferVerifier?,
    @JvmField val activeTopUpShieldVerifier: OfflineActiveTopUpShieldVerifier?,
    @JvmField val ready: Boolean,
    blockers: List<OfflineReadinessBlocker>,
) {
    @JvmField
    val assetDefinitionId: String = requireExactText(assetDefinitionId, "assetDefinitionId").also {
        require(AssetDefinitionIdEncoder.isCanonicalAddress(it)) {
            "assetDefinitionId must be a canonical unprefixed Base58 asset definition id"
        }
    }

    /** Authoritative u32 scale; values above 28 accompany asset_scale_unsupported. */
    @JvmField
    val assetScale: Long? = assetScale?.also {
        require(it in 0..U32_MAX) { "assetScale must fit in an unsigned 32-bit integer" }
    }

    @JvmField
    val evaluatedBlockHeight: BigInteger =
        requireReadinessU64(evaluatedBlockHeight, "evaluatedBlockHeight")

    @JvmField
    val evaluatedBlockHash: String = requireLowercaseHash(
        evaluatedBlockHash,
        "evaluatedBlockHash",
    )

    @JvmField
    val blockers: List<OfflineReadinessBlocker> = blockers.toList().also { copy ->
        require(copy.map { it.code }.toSet().size == copy.size) {
            "blockers must not repeat blocker codes"
        }
        require(ready == copy.isEmpty()) {
            "ready must be true exactly when blockers is empty"
        }
        val codes = copy.mapTo(HashSet()) { it.code }
        require(codes.contains("asset_scale_unavailable") == (assetScale == null)) {
            "asset_scale_unavailable must be present exactly when assetScale is null"
        }
        require(codes.contains("asset_scale_unsupported") == (assetScale != null && assetScale > 28)) {
            "asset_scale_unsupported must be present exactly when assetScale exceeds 28"
        }
        require(codes.contains("transfer_verifier_unavailable") == (activeTransferVerifier == null)) {
            "transfer_verifier_unavailable must be present exactly when no active verifier is reported"
        }
        require(
            codes.contains("topup_shield_verifier_unavailable") ==
                (activeTopUpShieldVerifier == null),
        ) {
            "topup_shield_verifier_unavailable must be present exactly when no active top-up shield verifier is reported"
        }
        require(activeTransferVerifier == null || activeTransferVerifier.isActiveAt(evaluatedBlockHeight)) {
            "activeTransferVerifier must be active at evaluatedBlockHeight"
        }
        require(
            activeTopUpShieldVerifier == null ||
                activeTopUpShieldVerifier.isActiveAt(evaluatedBlockHeight),
        ) {
            "activeTopUpShieldVerifier must be active at evaluatedBlockHeight"
        }
        require(
            !ready ||
                (
                    assetScale != null &&
                        assetScale <= 28 &&
                        activeTransferVerifier != null &&
                        activeTopUpShieldVerifier != null
                    ),
        ) {
            "ready requires a supported asset scale, active transfer verifier, and active top-up shield verifier"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is OfflineReadiness &&
            assetDefinitionId == other.assetDefinitionId &&
            assetScale == other.assetScale &&
            evaluatedBlockHeight == other.evaluatedBlockHeight &&
            evaluatedBlockHash == other.evaluatedBlockHash &&
            activeTransferVerifier == other.activeTransferVerifier &&
            activeTopUpShieldVerifier == other.activeTopUpShieldVerifier &&
            ready == other.ready &&
            blockers == other.blockers

    override fun hashCode(): Int {
        var result = assetDefinitionId.hashCode()
        result = 31 * result + (assetScale?.hashCode() ?: 0)
        result = 31 * result + evaluatedBlockHeight.hashCode()
        result = 31 * result + evaluatedBlockHash.hashCode()
        result = 31 * result + (activeTransferVerifier?.hashCode() ?: 0)
        result = 31 * result + (activeTopUpShieldVerifier?.hashCode() ?: 0)
        result = 31 * result + ready.hashCode()
        result = 31 * result + blockers.hashCode()
        return result
    }
}

private const val U32_MAX: Long = 0xffff_ffffL
private val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

private fun requireReadinessU64(value: BigInteger, field: String): BigInteger = value.also {
    require(it >= BigInteger.ZERO && it <= U64_MAX) {
        "$field must fit in an unsigned 64-bit integer"
    }
}

private fun requireExactText(value: String, field: String): String {
    require(
        value.isNotEmpty() &&
            value == value.trim() &&
            value.none { it.isISOControl() } &&
            hasWellFormedUtf16(value),
    ) {
        "$field must be exact non-empty text"
    }
    return value
}

private fun hasWellFormedUtf16(value: String): Boolean {
    var index = 0
    while (index < value.length) {
        val character = value[index]
        when {
            Character.isHighSurrogate(character) -> {
                if (index + 1 >= value.length || !Character.isLowSurrogate(value[index + 1])) {
                    return false
                }
                index += 2
            }
            Character.isLowSurrogate(character) -> return false
            else -> index++
        }
    }
    return true
}

private fun requireBoundedText(value: String, field: String, maximum: Int): String =
    requireExactText(value, field).also {
        require(it.codePointCount(0, it.length) <= maximum) {
            "$field must not exceed $maximum Unicode characters"
        }
    }

private fun requireReadinessStableCode(value: String, field: String): String =
    requireExactText(value, field).also {
        require(
            it.length <= 64 &&
                (it.first() in 'a'..'z' || it.first() in '0'..'9') &&
                it.all { character ->
                    character in 'a'..'z' || character in '0'..'9' || character == '_'
                },
        ) {
            "$field must be a 1-64 character lowercase stable identifier"
        }
    }

private fun requireLowercaseHash(value: String, field: String): String {
    require(value.length == 64 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "$field must be exact lowercase 32-byte hexadecimal"
    }
    return value
}
