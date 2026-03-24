package org.hyperledger.iroha.sdk.address

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.crypto.Blake2s

private val LOCAL_DOMAIN_KEY = "SORA-LOCAL-K:v1".toByteArray(StandardCharsets.UTF_8)
private val IH58_CHECKSUM_PREFIX = "IH58PRE".toByteArray(StandardCharsets.UTF_8)
private const val COMPRESSED_SENTINEL = "sora"
private const val COMPRESSED_CHECKSUM_LEN = 6
private const val BECH32M_CONST = 0x2bc830a3

private const val COMPRESSED_WARNING =
    "Compressed Sora addresses rely on half-width kana and are only interoperable inside " +
        "Sora-aware apps. Prefer IH58 when sharing with explorers, wallets, or QR codes."

private val IH58_ALPHABET = arrayOf(
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H",
    "J", "K", "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b",
    "c", "d", "e", "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t",
    "u", "v", "w", "x", "y", "z",
)

private val SORA_KANA = arrayOf(
    "\uFF72", "\uFF9B", "\uFF8A", "\uFF86", "\uFF8E", "\uFF8D", "\uFF84", "\uFF81",
    "\uFF98", "\uFF87", "\uFF99", "\uFF66", "\uFF9C", "\uFF76", "\uFF96", "\uFF80",
    "\uFF9A", "\uFF7F", "\uFF82", "\uFF88", "\uFF85", "\uFF97", "\uFF91", "\uFF73",
    "\u30F0", "\uFF89", "\uFF75", "\uFF78", "\uFF94", "\uFF8F", "\uFF79", "\uFF8C",
    "\uFF7A", "\uFF74", "\uFF83", "\uFF71", "\uFF7B", "\uFF77", "\uFF95", "\uFF92",
    "\uFF90", "\uFF7C", "\u30F1", "\uFF8B", "\uFF93", "\uFF7E", "\uFF7D",
)

private val SORA_KANA_FULLWIDTH = arrayOf(
    "\u30A4", "\u30ED", "\u30CF", "\u30CB", "\u30DB", "\u30D8", "\u30C8", "\u30C1",
    "\u30EA", "\u30CC", "\u30EB", "\u30F2", "\u30EF", "\u30AB", "\u30E8", "\u30BF",
    "\u30EC", "\u30BD", "\u30C4", "\u30CD", "\u30CA", "\u30E9", "\u30E0", "\u30A6",
    "\u30F0", "\u30CE", "\u30AA", "\u30AF", "\u30E4", "\u30DE", "\u30B1", "\u30D5",
    "\u30B3", "\u30A8", "\u30C6", "\u30A2", "\u30B5", "\u30AD", "\u30E6", "\u30E1",
    "\u30DF", "\u30B7", "\u30F1", "\u30D2", "\u30E2", "\u30BB", "\u30B9",
)

private val COMPRESSED_ALPHABET: Array<String> = IH58_ALPHABET + SORA_KANA
private val COMPRESSED_ALPHABET_FULLWIDTH: Array<String> = IH58_ALPHABET + SORA_KANA_FULLWIDTH
private val COMPRESSED_BASE: Int = COMPRESSED_ALPHABET.size
private val BASE_58: BigInteger = BigInteger.valueOf(58L)

private val COMPRESSED_INDEX: Map<String, Int> = buildMap {
    for (i in COMPRESSED_ALPHABET.indices) put(COMPRESSED_ALPHABET[i], i)
    for (i in COMPRESSED_ALPHABET_FULLWIDTH.indices) put(COMPRESSED_ALPHABET_FULLWIDTH[i], i)
}

private val IH58_INDEX: Map<String, Int> = buildMap {
    for (i in IH58_ALPHABET.indices) put(IH58_ALPHABET[i], i)
}

@Volatile
private var allowMlDsa = false
@Volatile
private var allowGost = false
@Volatile
private var allowSm2 = false

class AccountAddress private constructor(canonicalBytes: ByteArray) {

    private val _canonicalBytes: ByteArray = canonicalBytes.copyOf()

    val canonicalBytes: ByteArray get() = _canonicalBytes.copyOf()

    /**
     * Re-encodes this address with a domain selector derived from the provided domain label when
     * this address currently uses the `default` domain selector (tag `0x00`).
     *
     * Some Core API deployments return account IDs encoded with the default-domain selector,
     * while Torii/explorer interactions require the FI-local (Local12) selector (tag `0x01`)
     * derived from the FI's domain label.
     *
     * This helper is intentionally conservative:
     * - If this address already uses `local12` or `global` selectors, it returns `this`.
     * - If `domainLabel` canonicalizes to `default`, it returns `this`.
     */
    @Throws(AccountAddressException::class)
    fun rebasedFromDefaultDomain(domainLabel: String): AccountAddress {
        parseCanonical(_canonicalBytes)
        if (_canonicalBytes.size < 2) return this

        val tag = _canonicalBytes[1].toInt() and 0xFF
        if (tag != 0x00) return this

        val canonicalLabel = domainLabel.trim().lowercase()
        if (canonicalLabel.isBlank() || canonicalLabel.equals(DEFAULT_DOMAIN_NAME, ignoreCase = true)) {
            return this
        }

        val out = ByteArrayOutputStream()
        out.write(_canonicalBytes[0].toInt())
        out.write(0x01)
        val digest = computeLocalDigest(canonicalLabel)
        out.write(digest, 0, digest.size)
        out.write(_canonicalBytes, 2, _canonicalBytes.size - 2)
        return fromCanonicalBytes(out.toByteArray())
    }

    fun canonicalHex(): String = "0x${bytesToHex(_canonicalBytes)}"

    @Throws(AccountAddressException::class)
    fun toIH58(networkPrefix: Int): String = encodeIh58(networkPrefix, _canonicalBytes)

    @Throws(AccountAddressException::class)
    fun toI105(prefix: Int): String = toIH58(prefix)

    @Throws(AccountAddressException::class)
    fun toI105Default(): String = toCompressedSora()

    @Throws(AccountAddressException::class)
    fun toCompressedSora(): String = encodeCompressed(_canonicalBytes, COMPRESSED_ALPHABET)

    @Throws(AccountAddressException::class)
    fun toCompressedSoraFullWidth(): String =
        encodeCompressed(_canonicalBytes, COMPRESSED_ALPHABET_FULLWIDTH)

    @Throws(AccountAddressException::class)
    fun displayFormats(): DisplayFormats = displayFormats(DEFAULT_IH58_PREFIX)

    @Throws(AccountAddressException::class)
    fun displayFormats(networkPrefix: Int): DisplayFormats {
        val ih58 = toIH58(networkPrefix)
        val compressed = toCompressedSora()
        return DisplayFormats(ih58, compressed, networkPrefix, COMPRESSED_WARNING)
    }

    /**
     * Returns the single-key controller payload when this address encodes a single-key controller.
     *
     * Multisig addresses return `null`.
     */
    @Throws(AccountAddressException::class)
    fun singleKeyPayload(): SingleKeyPayload? {
        parseCanonical(_canonicalBytes)
        return extractSingleKeyPayload(_canonicalBytes, false)
    }

    @Throws(AccountAddressException::class)
    fun singleKeyPayloadIgnoringCurveSupport(): SingleKeyPayload? {
        parseCanonical(_canonicalBytes, true)
        return extractSingleKeyPayload(_canonicalBytes, true)
    }

    /**
     * Returns the multisig policy payload when this address encodes a multisig controller.
     *
     * Single-key addresses return `null`.
     */
    @Throws(AccountAddressException::class)
    fun multisigPolicyPayload(): MultisigPolicyPayload? {
        parseCanonical(_canonicalBytes)
        return extractMultisigPayload(_canonicalBytes, false)
    }

    @Throws(AccountAddressException::class)
    fun multisigPolicyPayloadIgnoringCurveSupport(): MultisigPolicyPayload? {
        parseCanonical(_canonicalBytes, true)
        return extractMultisigPayload(_canonicalBytes, true)
    }

    companion object {
        const val DEFAULT_DOMAIN_NAME = "default"
        const val DEFAULT_IH58_PREFIX = 753
        const val DEFAULT_I105_DISCRIMINANT = DEFAULT_IH58_PREFIX

        @JvmStatic
        fun compressedWarningMessage(): String = COMPRESSED_WARNING

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromAccount(
            domain: String,
            publicKey: ByteArray,
            algorithm: String,
        ): AccountAddress {
            if (publicKey.size > 0xFF) {
                throw AccountAddressException(
                    AccountAddressErrorCode.KEY_PAYLOAD_TOO_LONG,
                    "key payload too long: ${publicKey.size}",
                )
            }
            val header = encodeHeader(0, 0, 1)

            val out = ByteArrayOutputStream()
            out.write(header.toInt())

            if (domain.equals(DEFAULT_DOMAIN_NAME, ignoreCase = true)) {
                out.write(0x00)
            } else {
                out.write(0x01)
                val digest = computeLocalDigest(domain)
                out.write(digest, 0, digest.size)
            }

            out.write(0x00)
            out.write(curveIdForAlgorithm(algorithm).toInt())
            out.write(publicKey.size)
            out.write(publicKey, 0, publicKey.size)

            return fromCanonicalBytes(out.toByteArray())
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromAccount(publicKey: ByteArray, algorithm: String): AccountAddress =
            fromAccount(DEFAULT_DOMAIN_NAME, publicKey, algorithm)

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromMultisigPolicy(policy: MultisigPolicyPayload): AccountAddress =
            fromMultisigPolicy(DEFAULT_DOMAIN_NAME, policy)

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromMultisigPolicy(domain: String, policy: MultisigPolicyPayload): AccountAddress {
            if (domain.isBlank()) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_LENGTH, "domain must not be blank",
                )
            }

            val members = policy.members
            if (members.isEmpty()) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: zero members",
                )
            }
            if (members.size > 0xFF) {
                throw AccountAddressException(
                    AccountAddressErrorCode.MULTISIG_MEMBER_OVERFLOW,
                    "InvalidMultisigPolicy: too many members (${members.size})",
                )
            }

            var totalWeight = 0L
            for (member in members) {
                if (member.weight <= 0) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: non-positive weight",
                    )
                }
                if (member.weight > 0xFFFF) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: weight too large",
                    )
                }
                ensureCurveEnabled(member.curveId, "curve id ${member.curveId}")
                if (member.publicKey.isEmpty()) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: invalid key length",
                    )
                }
                if (member.publicKey.size > 0xFFFF) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: key too long",
                    )
                }
                totalWeight += member.weight
            }
            if (policy.threshold <= 0) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: ZeroThreshold",
                )
            }
            if (totalWeight < policy.threshold) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: threshold exceeds total weight",
                )
            }

            val header = encodeHeader(0, 0, 1)
            val out = ByteArrayOutputStream()
            out.write(header.toInt())

            if (domain.equals(DEFAULT_DOMAIN_NAME, ignoreCase = true)) {
                out.write(0x00)
            } else {
                out.write(0x01)
                val digest = computeLocalDigest(domain)
                out.write(digest, 0, digest.size)
            }

            out.write(0x01) // multisig controller tag
            out.write(policy.version and 0xFF)
            out.write((policy.threshold shr 8) and 0xFF)
            out.write(policy.threshold and 0xFF)
            out.write(members.size and 0xFF)

            for (member in members) {
                val curveId = member.curveId and 0xFF
                val weight = member.weight
                val keyBytes = member.publicKey
                out.write(curveId)
                out.write((weight shr 8) and 0xFF)
                out.write(weight and 0xFF)
                out.write((keyBytes.size shr 8) and 0xFF)
                out.write(keyBytes.size and 0xFF)
                out.write(keyBytes, 0, keyBytes.size)
            }

            return fromCanonicalBytes(out.toByteArray())
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromCanonicalBytes(canonical: ByteArray): AccountAddress {
            val copy = canonical.copyOf()
            parseCanonical(copy)
            return AccountAddress(copy)
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromCanonicalHex(encoded: String): AccountAddress {
            val body = if (encoded.startsWith("0x") || encoded.startsWith("0X")) {
                encoded.substring(2)
            } else {
                encoded
            }
            return fromCanonicalBytes(hexToBytes(body))
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromIH58(encoded: String, expectedPrefix: Int?): AccountAddress {
            val decode = decodeIh58(encoded)
            if (expectedPrefix != null && decode.networkPrefix != expectedPrefix) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
                    "unexpected IH58 network prefix: expected $expectedPrefix, found ${decode.networkPrefix}",
                )
            }
            val address = fromCanonicalBytes(decode.canonical)
            val reencoded = address.toIH58(decode.networkPrefix)
            if (reencoded != encoded) {
                throw AccountAddressException(
                    AccountAddressErrorCode.CHECKSUM_MISMATCH, "IH58 checksum mismatch",
                )
            }
            return address
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun fromCompressedSora(encoded: String): AccountAddress {
            val canonical = decodeCompressed(encoded)
            return fromCanonicalBytes(canonical)
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun parseAny(input: String, expectedPrefix: Int?): ParseResult {
            val trimmed = input.trim()
            if (trimmed.isEmpty()) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_LENGTH, "address string is empty",
                )
            }
            if (trimmed.startsWith(COMPRESSED_SENTINEL)) {
                return ParseResult(fromCompressedSora(trimmed), AccountAddressFormat.COMPRESSED)
            }
            if (containsCompressedGlyph(trimmed)) {
                throw AccountAddressException(
                    AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
                    "compressed address must start with sora sentinel",
                )
            }
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                return ParseResult(fromCanonicalHex(trimmed), AccountAddressFormat.CANONICAL_HEX)
            }
            try {
                return ParseResult(fromIH58(trimmed, expectedPrefix), AccountAddressFormat.IH58)
            } catch (ex: AccountAddressException) {
                val message = ex.message
                if (message != null &&
                    (message.startsWith("unexpected IH58 network prefix") || message.contains("IH58"))
                ) {
                    throw ex
                }
            }
            throw AccountAddressException(
                AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format",
            )
        }

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun parseEncoded(input: String, expectedPrefix: Int?): ParseResult =
            parseAny(input, expectedPrefix)

        @JvmStatic
        @Throws(AccountAddressException::class)
        fun parseEncodedIgnoringCurveSupport(input: String, expectedPrefix: Int?): ParseResult {
            val trimmed = input.trim()
            if (trimmed.isEmpty()) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_LENGTH, "address string is empty",
                )
            }
            if (trimmed.startsWith(COMPRESSED_SENTINEL)) {
                val canonical = decodeCompressed(trimmed)
                parseCanonical(canonical, true)
                return ParseResult(AccountAddress(canonical), AccountAddressFormat.COMPRESSED)
            }
            if (containsCompressedGlyph(trimmed)) {
                throw AccountAddressException(
                    AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
                    "compressed address must start with sora sentinel",
                )
            }
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                val body = trimmed.substring(2)
                val canonical = hexToBytes(body)
                parseCanonical(canonical, true)
                return ParseResult(AccountAddress(canonical), AccountAddressFormat.CANONICAL_HEX)
            }
            try {
                val decode = decodeIh58(trimmed)
                if (expectedPrefix != null && decode.networkPrefix != expectedPrefix) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
                        "unexpected IH58 network prefix: expected $expectedPrefix, found ${decode.networkPrefix}",
                    )
                }
                parseCanonical(decode.canonical, true)
                return ParseResult(AccountAddress(decode.canonical), AccountAddressFormat.IH58)
            } catch (ex: AccountAddressException) {
                val message = ex.message
                if (message != null &&
                    (message.startsWith("unexpected IH58 network prefix") || message.contains("IH58"))
                ) {
                    throw ex
                }
            }
            throw AccountAddressException(
                AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT, "unsupported address format",
            )
        }

        @JvmStatic
        fun configureCurveSupport(config: CurveSupportConfig) {
            allowMlDsa = config.allowMlDsa
            allowGost = config.allowGost
            allowSm2 = config.allowSm2
        }
    }
}

// -- Private helper classes --

private class Ih58DecodeResult(val networkPrefix: Int, val canonical: ByteArray)

private class PrefixResult(val networkPrefix: Int, val prefixLength: Int)

// -- Canonical decoding helpers --

@Throws(AccountAddressException::class)
private fun parseCanonical(canonical: ByteArray, ignoreCurveSupport: Boolean = false) {
    if (canonical.size < 4) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    decodeHeader(canonical[0])
    var cursor = 1

    val domainTag = canonical[cursor++]
    when (domainTag.toInt()) {
        0x00 -> {}
        0x01 -> {
            if (cursor + 12 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 12
        }
        0x02 -> {
            if (cursor + 4 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 4
        }
        else -> throw AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG, "unknown domain selector tag: $domainTag",
        )
    }

    if (cursor >= canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val controllerTag = canonical[cursor++]
    when (controllerTag.toInt()) {
        0x00 -> {
            if (cursor + 2 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            val curveId = canonical[cursor++].toInt() and 0xFF
            if (!ignoreCurveSupport) {
                ensureCurveEnabled(curveId, "curve id $curveId")
            }
            val keyLen = canonical[cursor++].toInt() and 0xFF
            val end = cursor + keyLen
            if (end > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            if (end != canonical.size) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
                    "unexpected trailing bytes in canonical payload",
                )
            }
        }
        0x01 -> {
            if (cursor + 4 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor++ // version
            val threshold = ((canonical[cursor].toInt() and 0xFF) shl 8) or
                (canonical[cursor + 1].toInt() and 0xFF)
            cursor += 2
            val memberCount = canonical[cursor++].toInt() and 0xFF
            if (memberCount == 0) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: zero members",
                )
            }
            var totalWeight = 0L
            for (i in 0 until memberCount) {
                if (cursor + 5 > canonical.size) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length",
                    )
                }
                val curveId = canonical[cursor++].toInt() and 0xFF
                if (!ignoreCurveSupport) {
                    ensureCurveEnabled(curveId, "curve id $curveId")
                }
                val weight = ((canonical[cursor].toInt() and 0xFF) shl 8) or
                    (canonical[cursor + 1].toInt() and 0xFF)
                cursor += 2
                if (weight <= 0) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: non-positive weight",
                    )
                }
                val keyLen = ((canonical[cursor].toInt() and 0xFF) shl 8) or
                    (canonical[cursor + 1].toInt() and 0xFF)
                cursor += 2
                if (keyLen <= 0) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                        "InvalidMultisigPolicy: invalid key length",
                    )
                }
                if (cursor + keyLen > canonical.size) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length",
                    )
                }
                cursor += keyLen
                totalWeight += weight
            }
            if (threshold <= 0) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: ZeroThreshold",
                )
            }
            if (totalWeight < threshold) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: threshold exceeds total weight",
                )
            }
            if (cursor != canonical.size) {
                if (cursor > canonical.size) {
                    throw AccountAddressException(
                        AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length",
                    )
                }
                throw AccountAddressException(
                    AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
                    "unexpected trailing bytes in canonical payload",
                )
            }
        }
        else -> throw AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_CONTROLLER_TAG,
            "unknown controller tag: $controllerTag",
        )
    }
}

@Throws(AccountAddressException::class)
private fun extractSingleKeyPayload(
    canonical: ByteArray,
    ignoreCurveSupport: Boolean,
): SingleKeyPayload? {
    if (canonical.size < 4) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    var cursor = 0
    decodeHeader(canonical[cursor++])

    val domainTag = canonical[cursor++]
    when (domainTag.toInt()) {
        0x00 -> {}
        0x01 -> {
            if (cursor + 12 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 12
        }
        0x02 -> {
            if (cursor + 4 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 4
        }
        else -> throw AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG, "unknown domain selector tag: $domainTag",
        )
    }

    if (cursor >= canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val controllerTag = canonical[cursor++]
    if (controllerTag.toInt() != 0x00) return null
    if (cursor + 2 > canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val curveId = canonical[cursor++].toInt() and 0xFF
    if (!ignoreCurveSupport) {
        ensureCurveEnabled(curveId, "curve id $curveId")
    }
    val keyLen = canonical[cursor++].toInt() and 0xFF
    val end = cursor + keyLen
    if (end > canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    if (end != canonical.size) {
        throw AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
            "unexpected trailing bytes in canonical payload",
        )
    }
    val key = canonical.copyOfRange(cursor, end)
    return SingleKeyPayload(curveId, key)
}

@Throws(AccountAddressException::class)
private fun extractMultisigPayload(
    canonical: ByteArray,
    ignoreCurveSupport: Boolean,
): MultisigPolicyPayload? {
    if (canonical.size < 4) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    var cursor = 0
    decodeHeader(canonical[cursor++])

    val domainTag = canonical[cursor++]
    when (domainTag.toInt()) {
        0x00 -> {}
        0x01 -> {
            if (cursor + 12 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 12
        }
        0x02 -> {
            if (cursor + 4 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor += 4
        }
        else -> throw AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_DOMAIN_TAG, "unknown domain selector tag: $domainTag",
        )
    }

    if (cursor >= canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val controllerTag = canonical[cursor++]
    if (controllerTag.toInt() != 0x01) return null
    if (cursor + 4 > canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val version = canonical[cursor++].toInt() and 0xFF
    val threshold = ((canonical[cursor].toInt() and 0xFF) shl 8) or
        (canonical[cursor + 1].toInt() and 0xFF)
    cursor += 2
    val memberCount = canonical[cursor++].toInt() and 0xFF
    if (memberCount == 0) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
            "InvalidMultisigPolicy: zero members",
        )
    }

    val members = ArrayList<MultisigMemberPayload>(memberCount)
    for (i in 0 until memberCount) {
        if (cursor + 5 > canonical.size) {
            throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
        }
        val curveId = canonical[cursor++].toInt() and 0xFF
        if (!ignoreCurveSupport) {
            ensureCurveEnabled(curveId, "curve id $curveId")
        }
        val weight = ((canonical[cursor].toInt() and 0xFF) shl 8) or
            (canonical[cursor + 1].toInt() and 0xFF)
        cursor += 2
        val keyLen = ((canonical[cursor].toInt() and 0xFF) shl 8) or
            (canonical[cursor + 1].toInt() and 0xFF)
        cursor += 2
        if (keyLen <= 0) {
            throw AccountAddressException(
                AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                "InvalidMultisigPolicy: invalid key length",
            )
        }
        if (cursor + keyLen > canonical.size) {
            throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
        }
        val key = canonical.copyOfRange(cursor, cursor + keyLen)
        cursor += keyLen
        members.add(MultisigMemberPayload(curveId, weight, key))
    }
    if (cursor != canonical.size) {
        if (cursor > canonical.size) {
            throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
        }
        throw AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_TRAILING_BYTES,
            "unexpected trailing bytes in canonical payload",
        )
    }
    return MultisigPolicyPayload.of(version, threshold, members)
}

@Throws(AccountAddressException::class)
private fun encodeHeader(version: Int, classId: Int, normVersion: Int): Byte {
    if (version < 0 || version > 0b111) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_HEADER_VERSION,
            "invalid address header version: $version",
        )
    }
    if (normVersion < 0 || normVersion > 0b11) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_NORM_VERSION,
            "invalid normalization version: $normVersion",
        )
    }
    return (((version and 0b111) shl 5) or ((classId and 0b11) shl 3) or ((normVersion and 0b11) shl 1)).toByte()
}

@Throws(AccountAddressException::class)
private fun decodeHeader(header: Byte) {
    val classBits = (header.toInt() shr 3) and 0b11
    val extFlag = header.toInt() and 0x01
    if (extFlag != 0) {
        throw AccountAddressException(
            AccountAddressErrorCode.UNEXPECTED_EXTENSION_FLAG,
            "address header reserves extension flag but it was set",
        )
    }
    if (classBits != 0 && classBits != 1) {
        throw AccountAddressException(
            AccountAddressErrorCode.UNKNOWN_ADDRESS_CLASS, "unknown address class: $classBits",
        )
    }
}

// -- Encoding helpers --

@Throws(AccountAddressException::class)
private fun curveIdForAlgorithm(algorithm: String): Byte {
    val normalized = algorithm.trim().lowercase()
    val curveId = when (normalized) {
        "ed25519", "ed" -> 0x01
        "ml-dsa", "mldsa", "ml_dsa" -> 0x02
        "gost256a", "gost-256-a" -> 0x0A
        "gost256b", "gost-256-b" -> 0x0B
        "gost256c", "gost-256-c" -> 0x0C
        "gost512a", "gost-512-a" -> 0x0D
        "gost512b", "gost-512-b" -> 0x0E
        "sm2", "sm-2" -> 0x0F
        else -> throw AccountAddressException(
            AccountAddressErrorCode.UNSUPPORTED_ALGORITHM,
            "unsupported signing algorithm: $algorithm",
        )
    }
    ensureCurveEnabled(curveId, "signing algorithm: $normalized")
    return curveId.toByte()
}

@Throws(AccountAddressException::class)
private fun ensureCurveEnabled(curveId: Int, context: String) {
    if (!isCurveEnabled(curveId)) {
        val known = isKnownCurveId(curveId)
        val code = if (known) AccountAddressErrorCode.UNSUPPORTED_ALGORITHM
        else AccountAddressErrorCode.UNKNOWN_CURVE
        val reason = if (known) "$context disabled by configuration: ${curveName(curveId)}"
        else "unknown curve id: ${curveName(curveId)}"
        throw AccountAddressException(code, reason)
    }
}

private fun isCurveEnabled(curveId: Int): Boolean = when (curveId and 0xFF) {
    0x01 -> true
    0x02 -> allowMlDsa
    0x0A, 0x0B, 0x0C, 0x0D, 0x0E -> allowGost
    0x0F -> allowSm2
    else -> false
}

private fun isKnownCurveId(curveId: Int): Boolean = when (curveId and 0xFF) {
    0x01, 0x02, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F -> true
    else -> false
}

private fun curveName(curveId: Int): String = when (curveId and 0xFF) {
    0x01 -> "ed25519"
    0x02 -> "ml-dsa"
    0x0A -> "gost256a"
    0x0B -> "gost256b"
    0x0C -> "gost256c"
    0x0D -> "gost512a"
    0x0E -> "gost512b"
    0x0F -> "sm2"
    else -> "0x${Integer.toHexString(curveId and 0xFF)}"
}

private fun computeLocalDigest(label: String): ByteArray {
    val digest = Blake2s.digest(label.toByteArray(StandardCharsets.UTF_8), LOCAL_DOMAIN_KEY, 32)
    return digest.copyOf(12)
}

@Throws(AccountAddressException::class)
private fun encodeIh58(prefix: Int, canonical: ByteArray): String {
    val prefixBytes = encodeIh58Prefix(prefix)
    val body = ByteArray(prefixBytes.size + canonical.size)
    prefixBytes.copyInto(body)
    canonical.copyInto(body, prefixBytes.size)

    val checksumInput = ByteArray(IH58_CHECKSUM_PREFIX.size + body.size)
    IH58_CHECKSUM_PREFIX.copyInto(checksumInput)
    body.copyInto(checksumInput, IH58_CHECKSUM_PREFIX.size)

    val checksum = Blake2b.digest512(checksumInput)
    val payload = ByteArray(body.size + 2)
    body.copyInto(payload)
    payload[payload.size - 2] = checksum[0]
    payload[payload.size - 1] = checksum[1]

    return encodeBase58(payload)
}

@Throws(AccountAddressException::class)
private fun decodeIh58(encoded: String): Ih58DecodeResult {
    val body = decodeBase58(encoded)
    if (body.size < 3) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload",
        )
    }
    val payload = body.copyOf(body.size - 2)
    val checksumBytes = body.copyOfRange(body.size - 2, body.size)
    val prefixResult = decodeIh58Prefix(payload)

    val checksumInput = ByteArray(IH58_CHECKSUM_PREFIX.size + payload.size)
    IH58_CHECKSUM_PREFIX.copyInto(checksumInput)
    payload.copyInto(checksumInput, IH58_CHECKSUM_PREFIX.size)
    val expected = Blake2b.digest512(checksumInput).copyOf(2)
    if (!checksumBytes.contentEquals(expected)) {
        throw AccountAddressException(
            AccountAddressErrorCode.CHECKSUM_MISMATCH, "IH58 checksum mismatch",
        )
    }

    val canonical = payload.copyOfRange(prefixResult.prefixLength, payload.size)
    return Ih58DecodeResult(prefixResult.networkPrefix, canonical)
}

@Throws(AccountAddressException::class)
private fun encodeIh58Prefix(prefix: Int): ByteArray {
    if (prefix < 0 || prefix > 0x3FFF) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_IH58_PREFIX, "invalid IH58 prefix: $prefix",
        )
    }
    if (prefix <= 63) return byteArrayOf(prefix.toByte())
    val lower = (prefix and 0b0011_1111) or 0b0100_0000
    val upper = prefix shr 6
    return byteArrayOf(lower.toByte(), upper.toByte())
}

@Throws(AccountAddressException::class)
private fun decodeIh58Prefix(payload: ByteArray): PrefixResult {
    if (payload.isEmpty()) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload",
        )
    }
    val first = payload[0].toInt() and 0xFF
    if (first <= 63) return PrefixResult(first, 1)
    if ((first and 0b0100_0000) != 0) {
        if (payload.size < 2) {
            throw AccountAddressException(
                AccountAddressErrorCode.INVALID_LENGTH, "invalid length for IH58 payload",
            )
        }
        val value = ((payload[1].toInt() and 0xFF) shl 6) or (first and 0x3F)
        return PrefixResult(value, 2)
    }
    throw AccountAddressException(
        AccountAddressErrorCode.INVALID_IH58_PREFIX_ENCODING,
        "invalid IH58 prefix encoding: $first",
    )
}

@Throws(AccountAddressException::class)
private fun encodeCompressed(canonical: ByteArray, alphabet: Array<String>): String {
    val digits = encodeBaseN(canonical, COMPRESSED_BASE)
    val checksum = compressedChecksumDigits(canonical)
    val sb = StringBuilder()
    sb.append(COMPRESSED_SENTINEL)
    for (digit in digits) sb.append(alphabet[digit])
    for (digit in checksum) sb.append(alphabet[digit])
    return sb.toString()
}

@Throws(AccountAddressException::class)
private fun decodeCompressed(encoded: String): ByteArray {
    if (!encoded.startsWith(COMPRESSED_SENTINEL)) {
        throw AccountAddressException(
            AccountAddressErrorCode.MISSING_COMPRESSED_SENTINEL,
            "compressed address must start with sora sentinel",
        )
    }
    val payload = encoded.substring(COMPRESSED_SENTINEL.length)
    if (payload.length <= COMPRESSED_CHECKSUM_LEN) {
        throw AccountAddressException(
            AccountAddressErrorCode.COMPRESSED_TOO_SHORT,
            "compressed address is too short",
        )
    }
    val digits = IntArray(payload.length)
    for (i in payload.indices) {
        val symbol = payload[i].toString()
        val value = COMPRESSED_INDEX[symbol]
            ?: throw AccountAddressException(
                AccountAddressErrorCode.INVALID_COMPRESSED_CHAR,
                "invalid compressed alphabet symbol: $symbol",
            )
        digits[i] = value
    }
    val dataDigits = digits.copyOf(digits.size - COMPRESSED_CHECKSUM_LEN)
    val checksumDigits = digits.copyOfRange(digits.size - COMPRESSED_CHECKSUM_LEN, digits.size)
    val canonical = decodeBaseN(dataDigits, COMPRESSED_BASE)
    val expected = compressedChecksumDigits(canonical)
    if (!checksumDigits.contentEquals(expected)) {
        throw AccountAddressException(
            AccountAddressErrorCode.CHECKSUM_MISMATCH, "compressed checksum mismatch",
        )
    }
    return canonical
}

@Throws(AccountAddressException::class)
private fun encodeBaseN(input: ByteArray, base: Int): IntArray {
    if (base < 2) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for encoding",
        )
    }
    if (input.isEmpty()) return intArrayOf(0)
    val value = IntArray(input.size) { input[it].toInt() and 0xFF }
    var leadingZeros = 0
    while (leadingZeros < value.size && value[leadingZeros] == 0) leadingZeros++
    val digits = ArrayList<Int>()
    var start = leadingZeros
    while (start < value.size) {
        var remainder = 0
        for (i in start until value.size) {
            val acc = (remainder shl 8) or value[i]
            value[i] = acc / base
            remainder = acc % base
        }
        digits.add(remainder)
        while (start < value.size && value[start] == 0) start++
    }
    repeat(leadingZeros) { digits.add(0) }
    if (digits.isEmpty()) digits.add(0)
    digits.reverse()
    return digits.toIntArray()
}

@Throws(AccountAddressException::class)
private fun encodeBase58(input: ByteArray): String {
    if (input.isEmpty()) return IH58_ALPHABET[0]
    var value = BigInteger(1, input)
    val builder = StringBuilder()
    while (value > BigInteger.ZERO) {
        val (div, rem) = value.divideAndRemainder(BASE_58)
        builder.append(IH58_ALPHABET[rem.toInt()])
        value = div
    }
    val zeroSymbol = IH58_ALPHABET[0]
    for (i in input.indices) {
        if (input[i] != 0.toByte()) break
        builder.append(zeroSymbol)
    }
    if (builder.isEmpty()) builder.append(zeroSymbol)
    return builder.reverse().toString()
}

@Throws(AccountAddressException::class)
private fun decodeBase58(encoded: String): ByteArray {
    if (encoded.isEmpty()) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_LENGTH, "invalid IH58 payload length",
        )
    }
    var value = BigInteger.ZERO
    for (ch in encoded) {
        val digit = IH58_INDEX[ch.toString()]
            ?: throw AccountAddressException(
                AccountAddressErrorCode.INVALID_IH58_ENCODING, "invalid IH58 base58 encoding",
            )
        value = value.multiply(BASE_58).add(BigInteger.valueOf(digit.toLong()))
    }
    var decoded = value.toByteArray()
    if (decoded.isNotEmpty() && decoded[0] == 0.toByte()) {
        decoded = decoded.copyOfRange(1, decoded.size)
    }
    val zeroChar = IH58_ALPHABET[0][0]
    var leadingZeros = 0
    while (leadingZeros < encoded.length && encoded[leadingZeros] == zeroChar) leadingZeros++
    val result = ByteArray(leadingZeros + decoded.size)
    decoded.copyInto(result, leadingZeros)
    return result
}

@Throws(AccountAddressException::class)
private fun decodeBaseN(digits: IntArray, base: Int): ByteArray {
    if (base < 2) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_COMPRESSED_BASE, "invalid base for decoding",
        )
    }
    if (digits.isEmpty()) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_LENGTH, "invalid length for address payload",
        )
    }
    for (digit in digits) {
        if (digit < 0 || digit >= base) {
            throw AccountAddressException(
                AccountAddressErrorCode.INVALID_COMPRESSED_DIGIT,
                "invalid digit $digit for base $base",
            )
        }
    }
    val value = digits.copyOf()
    var leadingZeros = 0
    while (leadingZeros < value.size && value[leadingZeros] == 0) leadingZeros++
    val bytes = ArrayList<Byte>()
    var start = leadingZeros
    while (start < value.size) {
        var remainder = 0
        for (i in start until value.size) {
            val acc = remainder * base + value[i]
            value[i] = acc / 256
            remainder = acc % 256
        }
        bytes.add(remainder.toByte())
        while (start < value.size && value[start] == 0) start++
    }
    repeat(leadingZeros) { bytes.add(0) }
    if (bytes.isEmpty()) bytes.add(0)
    bytes.reverse()
    return bytes.toByteArray()
}

private fun compressedChecksumDigits(canonical: ByteArray): IntArray {
    val values = convertToBase32(canonical)
    val expanded = expandHrp("snx", values, COMPRESSED_CHECKSUM_LEN)
    val polymod = bech32Polymod(expanded) xor BECH32M_CONST
    return IntArray(COMPRESSED_CHECKSUM_LEN) { i ->
        val shift = 5 * (COMPRESSED_CHECKSUM_LEN - 1 - i)
        (polymod shr shift) and 0x1F
    }
}

private fun convertToBase32(data: ByteArray): IntArray {
    var acc = 0
    var bits = 0
    val out = ArrayList<Int>()
    for (b in data) {
        acc = (acc shl 8) or (b.toInt() and 0xFF)
        bits += 8
        while (bits >= 5) {
            bits -= 5
            out.add((acc shr bits) and 0x1F)
        }
    }
    if (bits > 0) {
        out.add((acc shl (5 - bits)) and 0x1F)
    }
    return out.toIntArray()
}

private fun expandHrp(hrp: String, data: IntArray, checksumLen: Int): IntArray {
    val values = IntArray(hrp.length * 2 + 1 + data.size + checksumLen)
    var idx = 0
    for (ch in hrp) {
        values[idx++] = ch.code shr 5
    }
    values[idx++] = 0
    for (ch in hrp) {
        values[idx++] = ch.code and 0x1F
    }
    data.copyInto(values, idx)
    return values
}

private fun bech32Polymod(values: IntArray): Int {
    val generators = intArrayOf(0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3)
    var chk = 1
    for (v in values) {
        val top = chk shr 25
        chk = ((chk and 0x1ffffff) shl 5) xor v
        for (i in generators.indices) {
            if (((top shr i) and 1) == 1) {
                chk = chk xor generators[i]
            }
        }
    }
    return chk
}

private fun containsCompressedGlyph(value: String): Boolean {
    for (ch in value) {
        val symbol = ch.toString()
        if (COMPRESSED_INDEX.containsKey(symbol) && !IH58_INDEX.containsKey(symbol)) return true
    }
    return false
}

private fun bytesToHex(bytes: ByteArray): String = buildString(bytes.size * 2) {
    for (b in bytes) {
        append(String.format("%02x", b.toInt() and 0xFF))
    }
}

@Throws(AccountAddressException::class)
private fun hexToBytes(hex: String): ByteArray {
    if ((hex.length and 1) == 1) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_HEX_ADDRESS, "hex string must have even length",
        )
    }
    val out = ByteArray(hex.length / 2)
    for (i in out.indices) {
        try {
            out[i] = Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16).toByte()
        } catch (_: NumberFormatException) {
            throw AccountAddressException(
                AccountAddressErrorCode.INVALID_HEX_ADDRESS, "invalid hex string",
            )
        }
    }
    return out
}
