package org.hyperledger.iroha.sdk.address

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets

private const val I105_WARNING =
    "I105 addresses are the canonical account literal encoding. " +
        "Render and validate them with the intended chain discriminant."
private const val I105_DISCRIMINANT_MAX = 0xFFFF
private const val I105_DISCRIMINANT_SORA = 0x02F1
private const val I105_DISCRIMINANT_TEST = 0x0171
private const val I105_DISCRIMINANT_DEV = 0x0000
private const val I105_CHECKSUM_LEN = 6
private const val BECH32M_CONST = 0x2bc830a3
private const val I105_SENTINEL_SORA = "sora"
private const val I105_SENTINEL_TEST = "test"
private const val I105_SENTINEL_DEV = "dev"
private const val I105_SENTINEL_NUMERIC_PREFIX = "n"
private const val I105_SENTINEL_SORA_FULLWIDTH = "ｓｏｒａ"
private const val I105_SENTINEL_TEST_FULLWIDTH = "ｔｅｓｔ"
private const val I105_SENTINEL_DEV_FULLWIDTH = "ｄｅｖ"
private const val I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH = "ｎ"
private const val I105_MAX_SYMBOL_CHARS = 2

private val I105_KATAKANA_ALPHABET = arrayOf(
    "ア", "イ", "ウ", "エ", "オ", "カ", "キ", "ク", "ケ", "コ", "サ", "シ", "ス", "セ", "ソ",
    "タ", "チ", "ツ", "テ", "ト", "ナ", "ニ", "ヌ", "ネ", "ノ", "ハ", "ヒ", "フ", "ヘ", "ホ",
    "マ", "ミ", "ム", "メ", "モ", "ヤ", "ユ", "ヨ", "ラ", "リ", "ル", "レ", "ロ", "ワ", "ヰ",
    "ヱ", "ヲ", "ン", "ガ", "ギ", "グ", "ゲ", "ゴ", "ザ", "ジ", "ズ", "ゼ", "ゾ", "ダ", "ヂ",
    "ヅ", "デ", "ド", "バ", "ビ", "ブ", "ベ", "ボ", "パ", "ピ", "プ", "ペ", "ポ", "ヴ", "ヷ",
    "ヸ", "ヹ", "ヺ", "ァ", "ィ", "ゥ", "ェ", "ォ", "ャ", "ュ", "ョ", "ッ", "ヮ", "ヵ", "ヶ",
    "キャ", "キュ", "キョ", "シャ", "シュ", "ショ", "チャ", "チュ", "チョ", "ニャ", "ニュ",
    "ニョ", "ヒャ", "ヒュ", "ヒョ",
)
private val I105_ALPHABET = I105_KATAKANA_ALPHABET

@Volatile
private var allowMlDsa = false
@Volatile
private var allowGost = false
@Volatile
private var allowSm2 = false

private fun lookupI105Digit(symbol: String): Int? {
    val canonicalIndex = I105_ALPHABET.indexOf(symbol)
    return canonicalIndex.takeIf { it >= 0 }
}

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
        require(domainLabel.isNotBlank()) { "domainLabel must not be blank" }
        parseCanonical(_canonicalBytes)
        return this
    }

    fun canonicalHex(): String = "0x${bytesToHex(_canonicalBytes)}"

    @Throws(AccountAddressException::class)
    fun toI105(prefix: Int): String = encodeI105(prefix, _canonicalBytes)

    @Throws(AccountAddressException::class)
    fun displayFormats(): DisplayFormats = displayFormats(DEFAULT_I105_DISCRIMINANT)

    @Throws(AccountAddressException::class)
    fun displayFormats(discriminant: Int): DisplayFormats {
        val i105 = toI105(discriminant)
        return DisplayFormats(i105, discriminant, I105_WARNING)
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
        const val DEFAULT_I105_DISCRIMINANT = 753

        @JvmStatic
        fun i105WarningMessage(): String = I105_WARNING

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
            val members = policy.members
            if (members.isEmpty()) {
                throw AccountAddressException(
                    AccountAddressErrorCode.INVALID_MULTISIG_POLICY,
                    "InvalidMultisigPolicy: zero members",
                )
            }
            if (members.size > 0xFFFF) {
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

            out.write(0x01) // multisig controller tag
            out.write(policy.version and 0xFF)
            out.write((policy.threshold shr 8) and 0xFF)
            out.write(policy.threshold and 0xFF)
            out.write((members.size shr 8) and 0xFF)
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
        fun fromI105(encoded: String, expectedDiscriminant: Int?): AccountAddress {
            val canonical = decodeI105(encoded, expectedDiscriminant)
            val address = fromCanonicalBytes(canonical)
            ensureCanonicalI105Literal(encoded.trim(), address)
            return address
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
            if (trimmed.contains("@")) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
                    "account address literals must not include @domain; use canonical Katakana i105 form",
                )
            }
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
                    "canonical hex account addresses are not accepted; use canonical Katakana i105 form",
                )
            }
            return ParseResult(fromI105(trimmed, expectedPrefix), AccountAddressFormat.I105)
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
            if (trimmed.contains("@")) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
                    "account address literals must not include @domain; use canonical Katakana i105 form",
                )
            }
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
                throw AccountAddressException(
                    AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
                    "canonical hex account addresses are not accepted; use canonical Katakana i105 form",
                )
            }
            val canonical = decodeI105(trimmed, expectedPrefix)
            parseCanonical(canonical, true)
            val address = AccountAddress(canonical)
            ensureCanonicalI105Literal(trimmed, address)
            return ParseResult(address, AccountAddressFormat.I105)
        }

        @JvmStatic
        fun detectI105Discriminant(input: String): Int? =
            parseI105SentinelAndPayload(input.trim())?.first

        @JvmStatic
        fun configureCurveSupport(config: CurveSupportConfig) {
            allowMlDsa = config.allowMlDsa
            allowGost = config.allowGost
            allowSm2 = config.allowSm2
        }
    }
}

// -- Private helper classes --

private class I105PrefixResult(val discriminant: Int, val prefixLength: Int)

@Throws(AccountAddressException::class)
private fun ensureCanonicalI105Literal(literal: String, address: AccountAddress) {
    val discriminant = AccountAddress.detectI105Discriminant(literal) ?: return
    val canonical = address.toI105(discriminant)
    if (canonical != literal) {
        throw AccountAddressException(
            AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
            "account address literals must use canonical katakana i105 form",
        )
    }
}

// -- Canonical decoding helpers --

@Throws(AccountAddressException::class)
private fun parseCanonical(canonical: ByteArray, ignoreCurveSupport: Boolean = false) {
    if (canonical.size < 4) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    decodeHeader(canonical[0])
    var cursor = 1

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
            if (cursor + 5 > canonical.size) {
                throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
            }
            cursor++ // version
            val threshold = ((canonical[cursor].toInt() and 0xFF) shl 8) or
                (canonical[cursor + 1].toInt() and 0xFF)
            cursor += 2
            val memberCount = ((canonical[cursor].toInt() and 0xFF) shl 8) or
                (canonical[cursor + 1].toInt() and 0xFF)
            cursor += 2
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

    if (cursor >= canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val controllerTag = canonical[cursor++]
    if (controllerTag.toInt() != 0x01) return null
    if (cursor + 5 > canonical.size) {
        throw AccountAddressException(AccountAddressErrorCode.INVALID_LENGTH, "invalid canonical length")
    }
    val version = canonical[cursor++].toInt() and 0xFF
    val threshold = ((canonical[cursor].toInt() and 0xFF) shl 8) or
        (canonical[cursor + 1].toInt() and 0xFF)
    cursor += 2
    val memberCount = ((canonical[cursor].toInt() and 0xFF) shl 8) or
        (canonical[cursor + 1].toInt() and 0xFF)
    cursor += 2
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

@Throws(AccountAddressException::class)
private fun encodeI105(prefix: Int, canonical: ByteArray): String {
    val discriminant = normalizeI105Discriminant(prefix, "I105 discriminant")
    val digits = encodeBaseN(canonical, I105_ALPHABET.size)
    val checksum = i105ChecksumDigits(canonical)
    return buildString {
        append(i105SentinelForDiscriminant(discriminant))
        for (digit in digits) append(I105_ALPHABET[digit])
        for (digit in checksum) append(I105_ALPHABET[digit])
    }
}

@Throws(AccountAddressException::class)
private fun decodeI105(encoded: String, expectedDiscriminant: Int?): ByteArray {
    val parsed = parseI105SentinelAndPayload(encoded)
        ?: throw AccountAddressException(
            AccountAddressErrorCode.MISSING_I105_SENTINEL,
            "I105 address is missing the expected chain-discriminant sentinel",
        )
    val (discriminant, payload) = parsed
    if (expectedDiscriminant != null) {
        val normalizedExpected = normalizeI105Discriminant(expectedDiscriminant, "expected I105 discriminant")
        if (discriminant != normalizedExpected) {
            throw AccountAddressException(
                AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
                "unexpected I105 discriminant: expected $normalizedExpected, found $discriminant",
            )
        }
    }
    return decodeI105Payload(payload)
}

@Throws(AccountAddressException::class)
private fun decodeI105Payload(payload: String): ByteArray {
    val digits = ArrayList<Int>()
    var sawTooShort = false
    var sawChecksumMismatch = false
    var invalidChar: Char? = null

    fun backtrack(index: Int): ByteArray? {
        if (index == payload.length) {
            if (digits.size <= I105_CHECKSUM_LEN) {
                sawTooShort = true
                return null
            }
            val splitAt = digits.size - I105_CHECKSUM_LEN
            val dataDigits = digits.subList(0, splitAt).toIntArray()
            val checksumDigits = digits.subList(splitAt, digits.size).toIntArray()
            val canonical = decodeBaseN(dataDigits, I105_ALPHABET.size)
            val expected = i105ChecksumDigits(canonical)
            if (checksumDigits.contentEquals(expected)) {
                return canonical
            }
            sawChecksumMismatch = true
            return null
        }

        val currentChar = payload[index]
        var matched = false
        for (symbolLength in I105_MAX_SYMBOL_CHARS downTo 1) {
            if (index + symbolLength > payload.length) {
                continue
            }
            val candidate = payload.substring(index, index + symbolLength)
            val digit = lookupI105Digit(candidate) ?: continue
            matched = true
            digits.add(digit)
            val canonical = backtrack(index + symbolLength)
            digits.removeAt(digits.lastIndex)
            if (canonical != null) {
                return canonical
            }
        }

        if (!matched && invalidChar == null) {
            invalidChar = currentChar
        }
        return null
    }

    val canonical = backtrack(0)
    if (canonical != null) {
        return canonical
    }
    if (sawChecksumMismatch) {
        throw AccountAddressException(
            AccountAddressErrorCode.CHECKSUM_MISMATCH, "I105 checksum mismatch",
        )
    }
    if (sawTooShort) {
        throw AccountAddressException(
            AccountAddressErrorCode.I105_TOO_SHORT,
            "I105 address is too short",
        )
    }
    if (invalidChar != null) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_I105_CHAR,
            "invalid I105 alphabet symbol: $invalidChar",
        )
    }
    throw AccountAddressException(
        AccountAddressErrorCode.CHECKSUM_MISMATCH, "I105 checksum mismatch",
    )
}

@Throws(AccountAddressException::class)
private fun normalizeI105Discriminant(discriminant: Int, context: String): Int {
    if (discriminant < 0 || discriminant > I105_DISCRIMINANT_MAX) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_I105_PREFIX,
            "$context out of range: $discriminant",
        )
    }
    return discriminant
}

private fun i105SentinelForDiscriminant(discriminant: Int): String = when (discriminant) {
    I105_DISCRIMINANT_SORA -> I105_SENTINEL_SORA
    I105_DISCRIMINANT_TEST -> I105_SENTINEL_TEST
    I105_DISCRIMINANT_DEV -> I105_SENTINEL_DEV
    else -> "$I105_SENTINEL_NUMERIC_PREFIX$discriminant"
}

private fun parseI105SentinelAndPayload(encoded: String): Pair<Int, String>? {
    when {
        encoded.startsWith(I105_SENTINEL_SORA) || encoded.startsWith(I105_SENTINEL_SORA_FULLWIDTH) ->
            return I105_DISCRIMINANT_SORA to encoded.drop(I105_SENTINEL_SORA.length)
        encoded.startsWith(I105_SENTINEL_TEST) || encoded.startsWith(I105_SENTINEL_TEST_FULLWIDTH) ->
            return I105_DISCRIMINANT_TEST to encoded.drop(I105_SENTINEL_TEST.length)
        encoded.startsWith(I105_SENTINEL_DEV) || encoded.startsWith(I105_SENTINEL_DEV_FULLWIDTH) ->
            return I105_DISCRIMINANT_DEV to encoded.drop(I105_SENTINEL_DEV.length)
    }

    val tail = when {
        encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX) -> encoded.drop(I105_SENTINEL_NUMERIC_PREFIX.length)
        encoded.startsWith(I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH) -> encoded.drop(I105_SENTINEL_NUMERIC_PREFIX_FULLWIDTH.length)
        else -> return null
    }
    val digitsBuilder = StringBuilder()
    for (symbol in tail) {
        val asciiDigit = asciiDigit(symbol) ?: break
        digitsBuilder.append(asciiDigit)
    }
    val digits = digitsBuilder.toString()
    if (digits.isEmpty()) {
        return null
    }
    val discriminant = digits.toIntOrNull() ?: return null
    normalizeI105Discriminant(discriminant, "I105 discriminant sentinel")
    return discriminant to tail.drop(digits.length)
}

private fun asciiDigit(character: Char): Char? = when (character) {
    in '0'..'9' -> character
    in '０'..'９' -> (character.code - 0xFEE0).toChar()
    else -> null
}

private fun i105ChecksumDigits(canonical: ByteArray): IntArray = bech32mChecksum(canonical)

private fun convertToBase32(data: ByteArray): IntArray {
    var acc = 0
    var bits = 0
    val out = ArrayList<Int>((data.size * 8 + 4) / 5)
    for (byte in data) {
        acc = (acc shl 8) or (byte.toInt() and 0xFF)
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

private fun expandHrp(hrp: String): IntArray {
    val out = ArrayList<Int>(hrp.length * 2 + 1)
    for (character in hrp) {
        val code = character.code
        out.add(code shr 5)
    }
    out.add(0)
    for (character in hrp) {
        out.add(character.code and 0x1F)
    }
    return out.toIntArray()
}

private fun bech32Polymod(values: IntArray): Int {
    val generators = intArrayOf(0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3)
    var chk = 1
    for (value in values) {
        val top = chk ushr 25
        chk = ((chk and 0x1ff_ffff) shl 5) xor value
        for (index in generators.indices) {
            if (((top ushr index) and 1) == 1) {
                chk = chk xor generators[index]
            }
        }
    }
    return chk
}

private fun bech32mChecksum(data: ByteArray): IntArray {
    val values = ArrayList<Int>()
    values.addAll(expandHrp("snx").toList())
    values.addAll(convertToBase32(data).toList())
    repeat(I105_CHECKSUM_LEN) {
        values.add(0)
    }
    val polymod = bech32Polymod(values.toIntArray()) xor BECH32M_CONST
    return IntArray(I105_CHECKSUM_LEN) { index ->
        val shift = 5 * (I105_CHECKSUM_LEN - 1 - index)
        (polymod ushr shift) and 0x1F
    }
}

@Throws(AccountAddressException::class)
private fun encodeBaseN(input: ByteArray, base: Int): IntArray {
    if (base < 2) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_I105_BASE,
            "invalid base for encoding",
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
private fun decodeBaseN(digits: IntArray, base: Int): ByteArray {
    if (base < 2) {
        throw AccountAddressException(
            AccountAddressErrorCode.INVALID_I105_BASE,
            "invalid base for decoding",
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
                AccountAddressErrorCode.INVALID_I105_DIGIT,
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
