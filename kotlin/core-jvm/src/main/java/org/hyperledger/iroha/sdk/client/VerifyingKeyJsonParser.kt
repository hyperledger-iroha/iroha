package org.hyperledger.iroha.sdk.client

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag

/** Strict parser for the bounded `ids_only=true` verifying-key projection. */
internal object VerifyingKeyJsonParser {
    private const val MAX_IDS = 1_000
    private const val MAX_ID_FIELD_BYTES = 256
    private val ID_FIELDS = setOf("backend", "name")
    private val FORBIDDEN_PORTABLE_SEPARATORS = listOf(
        "..",
        "//",
        ":::",
        "/:",
        ":/",
        "/.",
        "./",
        ":.",
        ".:",
    )

    internal fun parseActiveIds(body: ByteArray): List<VerifyingKeyId> {
        val root = JsonParser.parse(decodeUtf8Strict(body))
        check(root is List<*>) {
            "active verifying-key response must be a JSON array"
        }
        check(root.size <= MAX_IDS) {
            "active verifying-key response exceeds 1000 ids"
        }

        val ids = ArrayList<VerifyingKeyId>(root.size)
        val unique = HashSet<VerifyingKeyId>()
        var previous: VerifyingKeyId? = null
        root.forEachIndexed { index, row ->
            check(row is Map<*, *>) {
                "active verifying-key id[$index] must be an object"
            }
            check(row.keys == ID_FIELDS) {
                "active verifying-key id[$index] must contain only backend and name"
            }
            val backend = row["backend"] as? String
                ?: throw IllegalStateException(
                    "active verifying-key id[$index] fields must be strings",
                )
            val name = row["name"] as? String
                ?: throw IllegalStateException(
                    "active verifying-key id[$index] fields must be strings",
                )
            check(isPortableRegistryIdField(backend)) {
                "active verifying-key backend must use portable registry syntax"
            }
            try {
                VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(
                    backend,
                    "active verifying-key backend",
                )
            } catch (error: IllegalArgumentException) {
                throw IllegalStateException(error.message, error)
            }
            check(isPortableRegistryIdField(name)) {
                "active verifying-key name must use portable registry syntax"
            }

            val id = VerifyingKeyId(backend, name)
            check(unique.add(id)) {
                "active verifying-key response contains duplicate id $id"
            }
            previous?.let { prior ->
                check(
                    prior.name < id.name ||
                        prior.name == id.name && prior.backend <= id.backend,
                ) {
                    "active verifying-key response is not in requested ascending order"
                }
            }
            ids.add(id)
            previous = id
        }
        return Collections.unmodifiableList(ids)
    }

    private fun decodeUtf8Strict(body: ByteArray): String = try {
        StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(body))
            .toString()
    } catch (error: CharacterCodingException) {
        throw IllegalStateException("active verifying-key response must be valid UTF-8", error)
    }

    /** Mirrors `verifying_key_id_field_is_portable` in `iroha_data_model::proof`. */
    private fun isPortableRegistryIdField(field: String): Boolean {
        val bytes = field.toByteArray(StandardCharsets.UTF_8)
        if (bytes.isEmpty() || bytes.size > MAX_ID_FIELD_BYTES) return false

        fun isLowercaseAsciiOrDigit(byte: Byte): Boolean =
            byte in 'a'.code.toByte()..'z'.code.toByte() ||
                byte in '0'.code.toByte()..'9'.code.toByte()

        if (!isLowercaseAsciiOrDigit(bytes.first()) ||
            !isLowercaseAsciiOrDigit(bytes.last())
        ) {
            return false
        }
        if (FORBIDDEN_PORTABLE_SEPARATORS.any(field::contains)) return false
        return bytes.all { byte ->
            isLowercaseAsciiOrDigit(byte) ||
                byte == '-'.code.toByte() ||
                byte == '_'.code.toByte() ||
                byte == '/'.code.toByte() ||
                byte == ':'.code.toByte() ||
                byte == '.'.code.toByte()
        }
    }
}
