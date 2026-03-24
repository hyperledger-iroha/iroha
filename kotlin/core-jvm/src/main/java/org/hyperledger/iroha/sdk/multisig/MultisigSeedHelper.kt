package org.hyperledger.iroha.sdk.multisig

import java.util.SortedMap
import java.util.TreeMap
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

private const val ALGO_TAG_ED25519 = 0
private const val ALGO_TAG_ML_DSA = 4
private const val ALGO_TAG_GOST_256_A = 5
private const val ALGO_TAG_GOST_256_B = 6
private const val ALGO_TAG_GOST_256_C = 7
private const val ALGO_TAG_GOST_512_A = 8
private const val ALGO_TAG_GOST_512_B = 9
private const val ALGO_TAG_SM2 = 10

private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
private val U8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
private val U16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
private val U64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
private val ENUM_TAG_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)

/** Helpers for computing deterministic multisig controller seeds. */
object MultisigSeedHelper {

    /**
     * Returns true when [accountId] matches the deterministic controller derived from [spec].
     *
     * If the seed cannot be computed (unsupported account formats/curves), this returns false.
     */
    @JvmStatic
    fun isDeterministicDerivedControllerId(accountId: String, spec: MultisigSpec): Boolean {
        val targetParts = parseAccountIdParts(accountId) ?: return false
        if (!targetParts.isEd25519()) return false
        val derived = deriveDeterministicPublicKey(targetParts.domain, spec) ?: return false
        return targetParts.publicKey.contentEquals(derived)
    }

    internal fun deriveDeterministicPublicKey(domain: String?, spec: MultisigSpec): ByteArray? {
        if (domain.isNullOrBlank()) return null
        val signatories: SortedMap<AccountIdParts, Long> = TreeMap()
        for ((accountId, weight) in spec.signatories) {
            val parts = parseAccountIdParts(accountId) ?: return null
            signatories[parts] = weight.toLong()
        }

        val encoder = NoritoEncoder(NoritoHeader.MINOR_VERSION)
        encodeSizedField(encoder, STRING_ADAPTER, domain)
        encodeMultisigSpecField(encoder, signatories, spec)

        val seed = Blake2b.digest256(encoder.toByteArray())
        seed[seed.size - 1] = (seed[seed.size - 1].toInt() or 1).toByte()
        return try {
            val privateKey = Ed25519PrivateKeyParameters(seed, 0)
            privateKey.generatePublicKey().encoded
        } catch (_: Exception) {
            null
        }
    }

    private fun encodeMultisigSpecField(
        encoder: NoritoEncoder,
        signatories: SortedMap<AccountIdParts, Long>,
        spec: MultisigSpec,
    ) {
        val child = encoder.childEncoder()
        encodeMultisigSpec(child, signatories, spec)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun encodeMultisigSpec(
        encoder: NoritoEncoder,
        signatories: SortedMap<AccountIdParts, Long>,
        spec: MultisigSpec,
    ) {
        val signatoryAdapter: TypeAdapter<Map<AccountIdParts, Long>> =
            NoritoAdapters.map(ACCOUNT_ID_ADAPTER, U8_ADAPTER)
        encodeSizedField(encoder, signatoryAdapter, signatories)
        encodeSizedField(encoder, U16_ADAPTER, spec.quorum.toLong())
        encodeSizedField(encoder, U64_ADAPTER, spec.transactionTtlMs)
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun parseAccountIdParts(accountId: String?): AccountIdParts? {
        if (accountId.isNullOrBlank()) return null
        val trimmed = accountId.trim()
        val atIndex = trimmed.lastIndexOf('@')
        if (atIndex <= 0 || atIndex == trimmed.length - 1) return null
        val identifier = trimmed.substring(0, atIndex)
        val domain = trimmed.substring(atIndex + 1).trim()
        if (domain.isBlank()) return null
        val payload = parseSingleKeyIdentifier(identifier) ?: return null
        val algorithmTag = algorithmTagForCurveId(payload.curveId)
        if (algorithmTag < 0) return null
        return AccountIdParts(domain, payload.curveId, algorithmTag, payload.keyBytes)
    }

    private fun parseSingleKeyIdentifier(identifier: String): KeyPayload? {
        try {
            val parsed = AccountAddress.parseAny(identifier, null)
            val singleKey = parsed.address.singleKeyPayload()
            if (singleKey != null) {
                return KeyPayload(singleKey.curveId, singleKey.publicKey)
            }
        } catch (_: AccountAddressException) {
            // fall through to multihash parsing
        }
        val decoded = decodePublicKeyLiteral(identifier) ?: return null
        return KeyPayload(decoded.curveId, decoded.keyBytes)
    }

    private fun algorithmTagForCurveId(curveId: Int): Int = when (curveId) {
        0x01 -> ALGO_TAG_ED25519
        0x02 -> ALGO_TAG_ML_DSA
        0x0A -> ALGO_TAG_GOST_256_A
        0x0B -> ALGO_TAG_GOST_256_B
        0x0C -> ALGO_TAG_GOST_256_C
        0x0D -> ALGO_TAG_GOST_512_A
        0x0E -> ALGO_TAG_GOST_512_B
        0x0F -> ALGO_TAG_SM2
        else -> -1
    }

    private class KeyPayload(val curveId: Int, keyBytes: ByteArray) {
        val keyBytes: ByteArray = keyBytes.copyOf()
    }

    private class AccountIdParts(
        val domain: String,
        private val curveId: Int,
        private val algorithmTag: Int,
        publicKey: ByteArray,
    ) : Comparable<AccountIdParts> {

        val publicKey: ByteArray = publicKey.copyOf()
        val publicKeyLiteral: String = encodePublicKeyMultihash(curveId, publicKey)

        fun isEd25519(): Boolean = algorithmTag == ALGO_TAG_ED25519

        override fun compareTo(other: AccountIdParts): Int {
            val domainCmp = domain.compareTo(other.domain)
            if (domainCmp != 0) return domainCmp
            val algoCmp = algorithmTag.compareTo(other.algorithmTag)
            if (algoCmp != 0) return algoCmp
            val min = minOf(publicKey.size, other.publicKey.size)
            for (i in 0 until min) {
                val left = publicKey[i].toInt() and 0xFF
                val right = other.publicKey[i].toInt() and 0xFF
                if (left != right) return left.compareTo(right)
            }
            return publicKey.size.compareTo(other.publicKey.size)
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is AccountIdParts) return false
            return algorithmTag == other.algorithmTag
                && domain == other.domain
                && publicKey.contentEquals(other.publicKey)
        }

        override fun hashCode(): Int {
            var result = domain.hashCode()
            result = 31 * result + algorithmTag
            result = 31 * result + publicKey.contentHashCode()
            return result
        }
    }

    private val ACCOUNT_CONTROLLER_ADAPTER: TypeAdapter<AccountIdParts> =
        object : TypeAdapter<AccountIdParts> {
            override fun encode(encoder: NoritoEncoder, value: AccountIdParts) {
                ENUM_TAG_ADAPTER.encode(encoder, 0L)
                encodeSizedField(encoder, STRING_ADAPTER, value.publicKeyLiteral)
            }

            override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AccountIdParts {
                throw UnsupportedOperationException("AccountController decode is not supported")
            }
        }

    private val ACCOUNT_ID_ADAPTER: TypeAdapter<AccountIdParts> =
        object : TypeAdapter<AccountIdParts> {
            override fun encode(encoder: NoritoEncoder, value: AccountIdParts) {
                encodeSizedField(encoder, STRING_ADAPTER, value.domain)
                encodeSizedField(encoder, ACCOUNT_CONTROLLER_ADAPTER, value)
            }

            override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): AccountIdParts {
                throw UnsupportedOperationException("AccountIdParts decode is not supported")
            }
        }
}
