package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b

internal object SccpMessageProofBundles {
    private const val MSG_PREFIX_ASSET_REGISTER_V1: String = "sccp:asset:register:v1"
    private const val MSG_PREFIX_ROUTE_ACTIVATE_V1: String = "sccp:route:activate:v1"
    private const val MSG_PREFIX_TRANSFER_V1: String = "sccp:transfer:v1"
    private const val MSG_PREFIX_TOKEN_ADD_V1: String = "sccp:token:add:v1"
    private const val MSG_PREFIX_TOKEN_PAUSE_V1: String = "sccp:token:pause:v1"
    private const val MSG_PREFIX_TOKEN_RESUME_V1: String = "sccp:token:resume:v1"
    private const val HUB_LEAF_PREFIX_V1: String = "sccp:hub:leaf:v1"
    private const val HUB_NODE_PREFIX_V1: String = "sccp:hub:node:v1"
    private const val PAYLOAD_HASH_PREFIX_V1: String = "sccp:payload:v1"
    private const val CODEC_TEXT_UTF8: Int = 1
    private const val CODEC_EVM_HEX: Int = 2
    private const val CODEC_SOLANA_BASE58: Int = 3
    private const val CODEC_TON_RAW: Int = 4
    private const val CODEC_TRON_BASE58CHECK: Int = 5
    private const val CODEC_SORA_ASSET_ID: Int = 6
    private const val BASE58_ALPHABET: String =
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

    internal data class BundleSummary(
        val sourceDomain: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
        val commitmentRoot: String,
    )

    @JvmStatic
    internal fun requireMatchesPublicInputs(
        targetDomain: Int,
        messageId: String,
        payloadHash: String,
        commitmentRoot: String,
        bundleBytes: ByteArray,
        sourceProofBytes: ByteArray,
    ): BundleSummary {
        val summary = decodeMessageProofBundleSummary(bundleBytes, "bundleBytes")
        require(
            summary.targetDomain == targetDomain &&
                summary.messageId == messageId &&
                summary.payloadHash == payloadHash &&
                summary.commitmentRoot == commitmentRoot,
        ) {
            "bundleBytes must match publicInputs"
        }
        require(summary.sourceDomain == SccpSourceProofs.DOMAIN_SORA || sourceProofBytes.isNotEmpty()) {
            "sourceProofBytes required for non-SORA source bundle"
        }
        return summary
    }

    private fun decodeMessageProofBundleSummary(bundleBytes: ByteArray, label: String): BundleSummary {
        var offset = 0
        val version = readU8At(bundleBytes, offset, "$label.version")
        offset += 1
        require(version == 1) { "$label.version must be 1" }
        require(offset + 32 <= bundleBytes.size) { "$label.commitment_root is too short" }
        val commitmentRoot = "0x" + hexLower(bundleBytes.copyOfRange(offset, offset + 32))
        offset += 32
        val commitmentVec = readCanonicalVec(bundleBytes, offset, "$label.commitment")
        offset = commitmentVec.nextOffset
        val merkleProofVec = readCanonicalVec(bundleBytes, offset, "$label.merkle_proof")
        offset = merkleProofVec.nextOffset
        val payloadVec = readCanonicalVec(bundleBytes, offset, "$label.payload")
        offset = payloadVec.nextOffset
        val finalityProofVec = readCanonicalVec(bundleBytes, offset, "$label.finality_proof")
        offset = finalityProofVec.nextOffset
        requireExactEnd(offset, bundleBytes, label)

        val payload = decodePayloadSummary(payloadVec.bytes, "$label.payload")
        val expectedCommitmentBytes = canonicalCommitmentBytes(
            payload.kind,
            payload.targetDomain,
            payload.messageId,
            payload.payloadHash,
        )
        require(commitmentVec.bytes.contentEquals(expectedCommitmentBytes)) {
            "$label.commitment must match payload"
        }
        val commitment = decodeCommitmentSummary(commitmentVec.bytes, label)
        require(commitment.kindCode == messageKindCode(payload.kind)) {
            "$label.commitment kind must match payload"
        }
        val expectedRoot = merkleRootFromCommitmentBytes(
            commitmentVec.bytes,
            merkleProofVec.bytes,
            "$label.merkle_proof",
        )
        require(commitmentRoot == expectedRoot) {
            "$label.commitment_root must match merkle proof"
        }
        return BundleSummary(
            sourceDomain = payload.sourceDomain,
            targetDomain = commitment.targetDomain,
            messageId = commitment.messageId,
            payloadHash = commitment.payloadHash,
            commitmentRoot = commitmentRoot,
        )
    }

    private fun decodePayloadSummary(payloadBytes: ByteArray, label: String): PayloadSummary {
        require(payloadBytes.size >= 2) { "$label is too short" }
        val discriminant = readU8At(payloadBytes, 0, "$label.kind")
        val body = payloadBytes.copyOfRange(1, payloadBytes.size)
        val version = readU8At(body, 0, "$label.version")
        require(version == 1) { "$label.version must be 1" }
        val cursor = Cursor(1)

        fun readDomain(field: String): Int {
            val domain = readU32LeAt(body, cursor.offset, "$label.$field")
            cursor.offset += 4
            requireSupportedBundleDomain(domain, "$label.$field")
            return domain
        }

        fun readU64(field: String) {
            readU64LeAt(body, cursor.offset, "$label.$field")
            cursor.offset += 8
        }

        fun readCodec(field: String): Int {
            val codec = normalizeCodecId(readU8At(body, cursor.offset, "$label.$field"), "$label.$field")
            cursor.offset += 1
            return codec
        }

        fun readCodecValue(codec: Int, field: String) {
            val value = readCanonicalVec(body, cursor.offset, "$label.$field")
            cursor.offset = value.nextOffset
            validateCodecBytes(codec, value.bytes, "$label.$field")
        }

        fun summary(kind: String, sourceDomain: Int, targetDomain: Int, prefix: String): PayloadSummary =
            PayloadSummary(
                kind = kind,
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                messageId = "0x" + hexLower(prefixedKeccakBytes(prefix, body)),
                payloadHash = "0x" + hexLower(prefixedHashBytes(PAYLOAD_HASH_PREFIX_V1, payloadBytes)),
            )

        when (discriminant) {
            0 -> {
                val targetDomain = readDomain("target_domain")
                val sourceDomain = readDomain("home_domain")
                readU64("nonce")
                readCodecValue(readCodec("asset_id_codec"), "asset_id")
                readU8At(body, cursor.offset, "$label.decimals")
                cursor.offset += 1
                requireExactEnd(cursor.offset, body, label)
                return summary("AssetRegister", sourceDomain, targetDomain, MSG_PREFIX_ASSET_REGISTER_V1)
            }
            1 -> {
                val sourceDomain = readDomain("source_domain")
                val targetDomain = readDomain("target_domain")
                require(sourceDomain != targetDomain) { "$label.target_domain must differ from source_domain" }
                readU64("nonce")
                readCodecValue(readCodec("asset_id_codec"), "asset_id")
                readCodecValue(readCodec("route_id_codec"), "route_id")
                requireExactEnd(cursor.offset, body, label)
                return summary("RouteActivate", sourceDomain, targetDomain, MSG_PREFIX_ROUTE_ACTIVATE_V1)
            }
            2 -> {
                val sourceDomain = readDomain("source_domain")
                val targetDomain = readDomain("dest_domain")
                require(sourceDomain != targetDomain) { "$label.dest_domain must differ from source_domain" }
                readU64("nonce")
                readDomain("asset_home_domain")
                readCodecValue(readCodec("asset_id_codec"), "asset_id")
                val amount = readU128LeAt(body, cursor.offset, "$label.amount")
                cursor.offset += 16
                require(amount > BigInteger.ZERO) { "$label.amount must be greater than zero" }
                val senderCodec = readCodec("sender_codec")
                require(senderCodec == counterpartyAccountCodec(sourceDomain)) {
                    "$label.sender_codec must match source_domain"
                }
                readCodecValue(senderCodec, "sender")
                val recipientCodec = readCodec("recipient_codec")
                require(recipientCodec == counterpartyAccountCodec(targetDomain)) {
                    "$label.recipient_codec must match dest_domain"
                }
                readCodecValue(recipientCodec, "recipient")
                readCodecValue(readCodec("route_id_codec"), "route_id")
                requireExactEnd(cursor.offset, body, label)
                return summary("Transfer", sourceDomain, targetDomain, MSG_PREFIX_TRANSFER_V1)
            }
            3 -> {
                val targetDomain = readDomain("target_domain")
                readU64("nonce")
                val assetId = readFixed(body, cursor, 32, "$label.sora_asset_id")
                require(assetId.any { it.toInt() != 0 }) { "$label.sora_asset_id must be non-zero" }
                readU8At(body, cursor.offset, "$label.decimals")
                cursor.offset += 1
                val name = readFixed(body, cursor, 32, "$label.name")
                require(fixedAsciiFieldIsNonEmpty(name)) { "$label.name must be non-empty" }
                val symbol = readFixed(body, cursor, 32, "$label.symbol")
                require(fixedAsciiFieldIsNonEmpty(symbol)) { "$label.symbol must be non-empty" }
                requireExactEnd(cursor.offset, body, label)
                return summary("TokenAdd", SccpSourceProofs.DOMAIN_SORA, targetDomain, MSG_PREFIX_TOKEN_ADD_V1)
            }
            4, 5 -> {
                val targetDomain = readDomain("target_domain")
                readU64("nonce")
                val assetId = readFixed(body, cursor, 32, "$label.sora_asset_id")
                require(assetId.any { it.toInt() != 0 }) { "$label.sora_asset_id must be non-zero" }
                requireExactEnd(cursor.offset, body, label)
                return if (discriminant == 4) {
                    summary("TokenPause", SccpSourceProofs.DOMAIN_SORA, targetDomain, MSG_PREFIX_TOKEN_PAUSE_V1)
                } else {
                    summary("TokenResume", SccpSourceProofs.DOMAIN_SORA, targetDomain, MSG_PREFIX_TOKEN_RESUME_V1)
                }
            }
            else -> throw IllegalArgumentException("$label contains unsupported SCCP payload kind")
        }
    }

    private fun decodeCommitmentSummary(commitmentBytes: ByteArray, label: String): CommitmentSummary {
        require(commitmentBytes.size == 70) { "$label.commitment must be 70 bytes" }
        val version = readU8At(commitmentBytes, 0, "$label.commitment.version")
        require(version == 1) { "$label.commitment.version must be 1" }
        return CommitmentSummary(
            kindCode = readU8At(commitmentBytes, 1, "$label.commitment.kind"),
            targetDomain = readU32LeAt(commitmentBytes, 2, "$label.commitment.target_domain"),
            messageId = "0x" + hexLower(commitmentBytes.copyOfRange(6, 38)),
            payloadHash = "0x" + hexLower(commitmentBytes.copyOfRange(38, 70)),
        )
    }

    private fun merkleRootFromCommitmentBytes(
        commitmentBytes: ByteArray,
        merkleProofBytes: ByteArray,
        label: String,
    ): String {
        var offset = 0
        val stepCount = readU32LeAt(merkleProofBytes, offset, "$label.steps")
        offset += 4
        var current = prefixedHashBytes(HUB_LEAF_PREFIX_V1, commitmentBytes)
        for (index in 0 until stepCount) {
            require(offset + 33 <= merkleProofBytes.size) { "$label.steps[$index] is too short" }
            val sibling = merkleProofBytes.copyOfRange(offset, offset + 32)
            offset += 32
            val siblingIsLeft = readU8At(merkleProofBytes, offset, "$label.steps[$index].sibling_is_left")
            offset += 1
            require(siblingIsLeft == 0 || siblingIsLeft == 1) {
                "$label.steps[$index].sibling_is_left must be 0 or 1"
            }
            current = prefixedHashBytes(
                HUB_NODE_PREFIX_V1,
                if (siblingIsLeft == 1) sibling + current else current + sibling,
            )
        }
        requireExactEnd(offset, merkleProofBytes, label)
        return "0x" + hexLower(current)
    }

    private fun canonicalCommitmentBytes(
        kind: String,
        targetDomain: Int,
        messageId: String,
        payloadHash: String,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(1)
        out.write(messageKindCode(kind))
        writeU32Le(out, targetDomain)
        out.write(hex32Bytes(messageId, "commitment.messageId"))
        out.write(hex32Bytes(payloadHash, "commitment.payloadHash"))
        return out.toByteArray()
    }

    private fun messageKindCode(kind: String): Int =
        when (kind) {
            "Burn" -> 0
            "TokenAdd" -> 1
            "TokenPause" -> 2
            "TokenResume" -> 3
            "AssetRegister" -> 4
            "RouteActivate" -> 5
            "Transfer" -> 6
            else -> throw IllegalArgumentException("SCCP message kind is unsupported")
        }

    private fun requireSupportedBundleDomain(domain: Int, label: String) {
        require(
            domain == SccpSourceProofs.DOMAIN_SORA ||
                domain == SccpSourceProofs.DOMAIN_ETH ||
                domain == SccpSourceProofs.DOMAIN_BSC ||
                domain == SccpSolana.DOMAIN_SOLANA ||
                domain == SccpTon.DOMAIN_TON ||
                domain == SccpTron.DOMAIN_TRON,
        ) {
            "$label must be a supported SCCP domain"
        }
    }

    private fun normalizeCodecId(value: Int, label: String): Int {
        require(
            value == CODEC_TEXT_UTF8 ||
                value == CODEC_EVM_HEX ||
                value == CODEC_SOLANA_BASE58 ||
                value == CODEC_TON_RAW ||
                value == CODEC_TRON_BASE58CHECK ||
                value == CODEC_SORA_ASSET_ID,
        ) {
            "$label codec is unsupported"
        }
        return value
    }

    private fun counterpartyAccountCodec(domain: Int): Int =
        when (domain) {
            SccpSourceProofs.DOMAIN_SORA -> CODEC_TEXT_UTF8
            SccpSourceProofs.DOMAIN_ETH, SccpSourceProofs.DOMAIN_BSC -> CODEC_EVM_HEX
            SccpSolana.DOMAIN_SOLANA -> CODEC_SOLANA_BASE58
            SccpTon.DOMAIN_TON -> CODEC_TON_RAW
            SccpTron.DOMAIN_TRON -> CODEC_TRON_BASE58CHECK
            else -> throw IllegalArgumentException("SCCP domain must be supported")
        }

    private fun validateCodecBytes(codec: Int, raw: ByteArray, label: String) {
        when (codec) {
            CODEC_TEXT_UTF8 -> {
                require(decodeCanonicalUtf8Bytes(raw, label).isNotEmpty()) { "$label must not be empty" }
            }
            CODEC_EVM_HEX -> validateCanonicalEvmHexAddress(decodeCanonicalUtf8Bytes(raw, label), label)
            CODEC_SOLANA_BASE58 -> decodeBase58Fixed(decodeCanonicalUtf8Bytes(raw, label), label, 32)
            CODEC_TON_RAW -> validateTonRawAddress(decodeCanonicalUtf8Bytes(raw, label), label)
            CODEC_TRON_BASE58CHECK -> tronBase58CheckPayload(decodeCanonicalUtf8Bytes(raw, label), label)
            CODEC_SORA_ASSET_ID -> require(raw.size == 32) { "$label must be 32 bytes" }
            else -> throw IllegalArgumentException("$label codec is unsupported")
        }
    }

    private fun decodeCanonicalUtf8Bytes(raw: ByteArray, label: String): String {
        val text = raw.toString(Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(raw)) { "$label must be canonical UTF-8" }
        return text
    }

    private fun validateCanonicalEvmHexAddress(text: String, label: String) {
        require(text.length == 42 && text.startsWith("0x") && text.drop(2).all { it.isDigit() || it in 'a'..'f' || it in 'A'..'F' }) {
            "$label must be a 0x-prefixed 20-byte EVM address"
        }
    }

    private fun validateTonRawAddress(text: String, label: String) {
        val parts = text.split(":")
        require(parts.size == 2 && parts[0] == "0") { "$label must be a basechain TON raw address" }
        require(parts[1].length == 64 && parts[1].all { it in '0'..'9' || it in 'a'..'f' }) {
            "$label must be a canonical TON raw address"
        }
        val account = hexBytes(parts[1], label)
        require(account.any { it.toInt() != 0 }) { "$label must not be zero" }
    }

    private fun readCanonicalVec(raw: ByteArray, offset: Int, label: String): ReadVec {
        val length = readU32LeAt(raw, offset, "$label.length")
        val start = offset + 4
        val end = start.toLong() + length.toLong()
        require(end <= raw.size.toLong()) { "$label is too short" }
        return ReadVec(raw.copyOfRange(start, end.toInt()), end.toInt())
    }

    private fun readFixed(raw: ByteArray, cursor: Cursor, length: Int, label: String): ByteArray {
        val end = cursor.offset + length
        require(end <= raw.size) { "$label is too short" }
        val out = raw.copyOfRange(cursor.offset, end)
        cursor.offset = end
        return out
    }

    private fun readU8At(raw: ByteArray, offset: Int, label: String): Int {
        require(offset + 1 <= raw.size) { "$label is too short" }
        return raw[offset].toInt() and 0xff
    }

    private fun readU32LeAt(raw: ByteArray, offset: Int, label: String): Int {
        require(offset + 4 <= raw.size) { "$label is too short" }
        val value = (raw[offset].toLong() and 0xffL) or
            ((raw[offset + 1].toLong() and 0xffL) shl 8) or
            ((raw[offset + 2].toLong() and 0xffL) shl 16) or
            ((raw[offset + 3].toLong() and 0xffL) shl 24)
        require(value <= Int.MAX_VALUE.toLong()) { "$label must fit platform size" }
        return value.toInt()
    }

    private fun readU64LeAt(raw: ByteArray, offset: Int, label: String): BigInteger {
        require(offset + 8 <= raw.size) { "$label is too short" }
        var value = BigInteger.ZERO
        for (index in 7 downTo 0) {
            value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index].toLong() and 0xffL))
        }
        return value
    }

    private fun readU128LeAt(raw: ByteArray, offset: Int, label: String): BigInteger {
        require(offset + 16 <= raw.size) { "$label is too short" }
        var value = BigInteger.ZERO
        for (index in 15 downTo 0) {
            value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index].toLong() and 0xffL))
        }
        return value
    }

    private fun requireExactEnd(offset: Int, raw: ByteArray, label: String) {
        require(offset == raw.size) { "$label must not contain trailing bytes" }
    }

    private fun fixedAsciiFieldIsNonEmpty(raw: ByteArray): Boolean {
        val end = raw.indexOf(0.toByte())
        val limit = if (end < 0) raw.size else end
        return raw.copyOfRange(0, limit).any { it.toInt() != 0 }
    }

    private fun decodeBase58Fixed(value: String, field: String, byteLength: Int): ByteArray {
        val raw = decodeBase58(value, field)
        require(raw.size == byteLength) { "$field must decode to $byteLength bytes" }
        return raw
    }

    private fun decodeBase58(value: String, field: String): ByteArray {
        require(value.trim() == value && value.isNotEmpty()) { "$field must be canonical base58" }
        var numeric = BigInteger.ZERO
        value.forEach { char ->
            val digit = BASE58_ALPHABET.indexOf(char)
            require(digit >= 0) { "$field must be canonical base58" }
            numeric = numeric.multiply(BigInteger.valueOf(58)).add(BigInteger.valueOf(digit.toLong()))
        }
        var encoded = if (numeric == BigInteger.ZERO) ByteArray(0) else numeric.toByteArray()
        if (encoded.isNotEmpty() && encoded[0].toInt() == 0) encoded = encoded.copyOfRange(1, encoded.size)
        val leadingZeroes = value.takeWhile { it == '1' }.length
        return ByteArray(leadingZeroes) + encoded
    }

    private fun tronBase58CheckPayload(value: String, field: String): ByteArray {
        val raw = decodeBase58(value, field)
        require(raw.size == 25) { "$field must be a TRON Base58Check address" }
        val payload = raw.copyOfRange(0, 21)
        require((payload[0].toInt() and 0xff) == 0x41) { "$field must be a TRON mainnet address" }
        val checksum = sha256(sha256(payload)).copyOfRange(0, 4)
        require(raw.copyOfRange(21, 25).contentEquals(checksum)) {
            "$field must have a valid Base58Check checksum"
        }
        return payload
    }

    private fun prefixedKeccakBytes(prefix: String, payload: ByteArray): ByteArray =
        keccak256(prefix.toByteArray(Charsets.UTF_8) + payload)

    private fun keccak256(input: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun prefixedHashBytes(prefix: String, payload: ByteArray): ByteArray =
        Blake2b.digest256(prefix.toByteArray(Charsets.UTF_8) + payload)

    private fun sha256(input: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(input)

    private fun hex32Bytes(value: String, field: String): ByteArray {
        var body = value
        if (body.startsWith("0x", ignoreCase = true)) body = body.substring(2)
        require(body.length == 64) { "$field must be 32 bytes" }
        return hexBytes(body, field)
    }

    private fun hexBytes(value: String, field: String): ByteArray {
        require(value.length % 2 == 0) { "$field must have even hex length" }
        val out = ByteArray(value.length / 2)
        for (index in out.indices) {
            val high = hexDigit(value[index * 2])
            val low = hexDigit(value[index * 2 + 1])
            require(high >= 0 && low >= 0) { "$field must be hex" }
            out[index] = ((high shl 4) or low).toByte()
        }
        return out
    }

    private fun hexDigit(char: Char): Int =
        when (char) {
            in '0'..'9' -> char - '0'
            in 'a'..'f' -> char - 'a' + 10
            in 'A'..'F' -> char - 'A' + 10
            else -> -1
        }

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun hexLower(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xff) }

    private data class ReadVec(val bytes: ByteArray, val nextOffset: Int)

    private data class PayloadSummary(
        val kind: String,
        val sourceDomain: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
    )

    private data class CommitmentSummary(
        val kindCode: Int,
        val targetDomain: Int,
        val messageId: String,
        val payloadHash: String,
    )

    private data class Cursor(var offset: Int)
}
