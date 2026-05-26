package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.offline.OfflineNote
import org.hyperledger.iroha.sdk.offline.OfflineNoteStore
import org.hyperledger.iroha.sdk.offline.OfflineNoteWalletNote
import org.hyperledger.iroha.sdk.offline.OfflineNoteWalletNoteState

/** Android Keystore-backed encrypted store for Offline Note wallet notes. */
class AndroidOfflineNoteSecureStore @JvmOverloads constructor(
    context: Context,
    preferencesName: String = DEFAULT_PREFERENCES_NAME,
    private val keyAlias: String = DEFAULT_KEY_ALIAS,
) : OfflineNoteStore {
    private val preferences = (context.applicationContext ?: context)
        .getSharedPreferences(requireNonBlank(preferencesName, "preferencesName"), Context.MODE_PRIVATE)

    @Synchronized
    override fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteWalletNote>) -> T): T {
        val notes = loadNotes()
        val result = mutator(notes)
        saveNotes(notes)
        return result
    }

    @Synchronized
    override fun listNotes(): List<OfflineNoteWalletNote> =
        loadNotes().values.sortedWith(compareBy<OfflineNoteWalletNote> { it.createdAtMs }.thenBy { it.noteCommitmentHex() })

    @Synchronized
    override fun findNote(noteCommitment: ByteArray): OfflineNoteWalletNote? =
        loadNotes()[hexLower(noteCommitment)]

    @Synchronized
    override fun upsert(note: OfflineNoteWalletNote) {
        mutateNotes {
            it[note.noteCommitmentHex()] = note
        }
    }

    @Synchronized
    fun delete(noteCommitment: ByteArray) {
        val notes = loadNotes()
        notes.remove(hexLower(noteCommitment))
        saveNotes(notes)
    }

    @Synchronized
    fun clear() {
        val revision = currentRevision()
        check(preferences.edit().clear().commit()) { "failed to clear Offline Note wallet notes" }
        deleteKeyAlias(keyAlias)
        if (revision > 0L) {
            deleteKeyAlias(storeKeyAlias(revision))
        }
    }

    private fun loadNotes(): MutableMap<String, OfflineNoteWalletNote> {
        val notes = LinkedHashMap<String, OfflineNoteWalletNote>()
        for (commitmentHex in indexSnapshot()) {
            val encrypted = preferences.getString(noteKey(commitmentHex), null)
                ?: error("missing Offline Note wallet note ciphertext")
            val note = WalletNoteJsonCodec.decode(decrypt(encrypted))
            require(note.noteCommitmentHex() == commitmentHex) {
                "Offline Note wallet note commitment mismatch"
            }
            notes[commitmentHex] = note
        }
        return notes
    }

    private fun saveNotes(notes: Map<String, OfflineNoteWalletNote>) {
        val oldIndex = indexSnapshot()
        val newIndex = notes.keys.toSet()
        val oldRevision = currentRevision()
        val revision = oldRevision + 1L
        val editor = preferences.edit()
        for (oldCommitment in oldIndex) {
            if (!newIndex.contains(oldCommitment)) {
                editor.remove(noteKey(oldCommitment))
            }
        }
        for ((commitmentHex, note) in notes) {
            editor.putString(noteKey(commitmentHex), encrypt(WalletNoteJsonCodec.encode(note), revision))
        }
        editor.putStringSet(INDEX_KEY, newIndex)
        editor.putLong(STORE_REVISION_KEY, revision)
        check(editor.commit()) { "failed to persist Offline Note wallet notes" }
        if (oldRevision == 0L) {
            deleteKeyAlias(keyAlias)
        } else {
            deleteKeyAlias(storeKeyAlias(oldRevision))
        }
    }

    private fun indexSnapshot(): Set<String> =
        preferences.getStringSet(INDEX_KEY, emptySet())?.toSet() ?: emptySet()

    private fun currentRevision(): Long = preferences.getLong(STORE_REVISION_KEY, 0L)

    private fun encrypt(plaintext: ByteArray, revision: Long): String {
        val cipher = Cipher.getInstance(AES_GCM)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey(storeKeyAlias(revision), createIfMissing = true))
        return VALUE_PREFIX +
            revision +
            ":" +
            b64(cipher.iv) +
            ":" +
            b64(cipher.doFinal(plaintext))
    }

    private fun decrypt(envelope: String): ByteArray {
        if (envelope.startsWith(VALUE_PREFIX)) {
            val parts = envelope.removePrefix(VALUE_PREFIX).split(':')
            require(parts.size == 2) { "invalid Offline Note wallet note envelope" }
            return decryptWithKeyAlias(keyAlias, parts[0], parts[1])
        }
        require(envelope.startsWith(VALUE_PREFIX)) { "unknown Offline Note wallet note envelope" }
        val parts = envelope.removePrefix(VALUE_PREFIX).split(':')
        require(parts.size == 3) { "invalid Offline Note wallet note envelope" }
        val revision = parts[0].toLongOrNull()
            ?: throw IllegalArgumentException("invalid Offline Note wallet note revision")
        return decryptWithKeyAlias(storeKeyAlias(revision), parts[1], parts[2])
    }

    private fun decryptWithKeyAlias(alias: String, ivBase64: String, ciphertextBase64: String): ByteArray {
        try {
            val cipher = Cipher.getInstance(AES_GCM)
            cipher.init(
                Cipher.DECRYPT_MODE,
                secretKey(alias, createIfMissing = false),
                GCMParameterSpec(GCM_TAG_BITS, b64decode(ivBase64)),
            )
            return cipher.doFinal(b64decode(ciphertextBase64))
        } catch (e: GeneralSecurityException) {
            throw IllegalStateException("failed to decrypt Offline Note wallet note", e)
        }
    }

    private fun secretKey(alias: String, createIfMissing: Boolean): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
        keyStore.load(null)
        (keyStore.getEntry(alias, null) as? KeyStore.SecretKeyEntry)?.let {
            return it.secretKey
        }
        require(createIfMissing) { "missing Offline Note store key for $alias" }
        return generateSecretKey(alias)
    }

    private fun generateSecretKey(alias: String): SecretKey {
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        val builder = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(AES_KEY_BITS)
            .setRandomizedEncryptionRequired(true)
        generator.init(builder.build())
        return generator.generateKey()
    }

    private fun deleteKeyAlias(alias: String) {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
        keyStore.load(null)
        if (keyStore.containsAlias(alias)) {
            keyStore.deleteEntry(alias)
        }
    }

    private fun storeKeyAlias(revision: Long): String = "$keyAlias.rev.$revision"

    private fun noteKey(commitmentHex: String): String = "$NOTE_PREFIX$commitmentHex"

    private object WalletNoteJsonCodec {
        fun encode(note: OfflineNoteWalletNote): ByteArray {
            val payload = linkedMapOf<String, Any?>(
                "version" to 1,
                "chain_id" to note.chainId,
                "account_id" to note.accountId,
                "asset_id" to note.assetId,
                "amount" to note.canonicalAmount,
                "key_certificate_norito_base64" to b64(note.keyCertificate.noritoEncoded()),
                "note_commitment_hex" to note.noteCommitmentHex(),
                "note_secret_base64" to b64(note.noteSecret()),
                "origin" to encodeOrigin(note.origin),
                "bearer_audit_trail_norito_base64" to note.bearerAuditTrail().map {
                    b64(it.noritoEncoded())
                },
                "state" to note.state.name,
                "created_at_ms" to note.createdAtMs,
                "updated_at_ms" to note.updatedAtMs,
            )
            return JsonEncoder.encode(payload).toByteArray(Charsets.UTF_8)
        }

        fun decode(payload: ByteArray): OfflineNoteWalletNote {
            val obj = requireObject(JsonParser.parse(String(payload, Charsets.UTF_8)), "wallet note")
            require(asLong(obj["version"], "version") == 1L) { "unsupported Offline Note wallet note version" }
            return OfflineNoteWalletNote(
                chainId = asString(obj["chain_id"], "chain_id"),
                accountId = asString(obj["account_id"], "account_id"),
                assetId = asString(obj["asset_id"], "asset_id"),
                amount = asString(obj["amount"], "amount"),
                keyCertificate = OfflineNote.decodeCertificate(
                    b64decode(asString(obj["key_certificate_norito_base64"], "key_certificate_norito_base64"))
                ),
                noteCommitment = hexBytes(asString(obj["note_commitment_hex"], "note_commitment_hex")),
                noteSecret = b64decode(asString(obj["note_secret_base64"], "note_secret_base64")),
                origin = decodeOrigin(requireObject(obj["origin"], "origin")),
                bearerAuditTrail = decodeAuditTrail(obj["bearer_audit_trail_norito_base64"]),
                state = decodeState(asString(obj["state"], "state")),
                createdAtMs = asLong(obj["created_at_ms"], "created_at_ms"),
                updatedAtMs = asLong(obj["updated_at_ms"], "updated_at_ms"),
            )
        }

        private fun decodeAuditTrail(value: Any?): List<OfflineNote.AuditBundle> {
            if (value == null) return emptyList()
            val raw = requireArray(value, "bearer_audit_trail_norito_base64")
            return raw.mapIndexed { index, item ->
                OfflineNote.decodeAudit(
                    b64decode(asString(item, "bearer_audit_trail_norito_base64[$index]"))
                )
            }
        }

        private fun encodeOrigin(origin: OfflineNote.CommitmentOrigin): Map<String, Any?> =
            when (origin) {
                is OfflineNote.CommitmentOrigin.IssuerLoad -> linkedMapOf(
                    "kind" to "issuer_load",
                    "operation_id" to origin.operationId,
                    "lineage_id" to origin.lineageId,
                    "local_revision" to origin.localRevision,
                )
                is OfflineNote.CommitmentOrigin.P2pOutput -> linkedMapOf(
                    "kind" to "p2p_output",
                    "payment_request_id" to origin.paymentRequestId,
                    "output_index" to origin.outputIndex,
                )
            }

        private fun decodeOrigin(obj: Map<String, Any?>): OfflineNote.CommitmentOrigin =
            when (asString(obj["kind"], "kind")) {
                "issuer_load" -> OfflineNote.CommitmentOrigin.IssuerLoad(
                    operationId = asString(obj["operation_id"], "operation_id"),
                    lineageId = asString(obj["lineage_id"], "lineage_id"),
                    localRevision = asLong(obj["local_revision"], "local_revision"),
                )
                "p2p_output" -> OfflineNote.CommitmentOrigin.P2pOutput(
                    paymentRequestId = asString(obj["payment_request_id"], "payment_request_id"),
                    outputIndex = asLong(obj["output_index"], "output_index").toInt(),
                )
                else -> throw IllegalArgumentException("unsupported Offline Note origin kind")
            }

        private fun decodeState(value: String): OfflineNoteWalletNoteState =
            when (value) {
                "SPEND_PENDING", "spendPending" -> OfflineNoteWalletNoteState.SPENT
                "CHANGE_PENDING", "changePending" -> OfflineNoteWalletNoteState.SPENDABLE
                else -> OfflineNoteWalletNoteState.valueOf(value)
            }
    }

    companion object {
        const val DEFAULT_PREFERENCES_NAME = "org.hyperledger.iroha.offline_note"
        const val DEFAULT_KEY_ALIAS = "org.hyperledger.iroha.offline_note.store"
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val AES_GCM = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val AES_KEY_BITS = 256
        private const val INDEX_KEY = "note_index"
        private const val STORE_REVISION_KEY = "store_revision"
        private const val NOTE_PREFIX = "note."
        private const val VALUE_PREFIX = "enc:"
    }
}

private fun requireNonBlank(value: String, field: String): String {
    require(value.trim().isNotEmpty()) { "$field must not be blank" }
    return value
}

@Suppress("UNCHECKED_CAST")
private fun requireObject(value: Any?, field: String): Map<String, Any?> {
    require(value is Map<*, *>) { "$field must be an object" }
    return value as Map<String, Any?>
}

@Suppress("UNCHECKED_CAST")
private fun requireArray(value: Any?, field: String): List<Any?> {
    require(value is List<*>) { "$field must be an array" }
    return value as List<Any?>
}

private fun asString(value: Any?, field: String): String {
    require(value is String && value.isNotBlank()) { "$field must be a non-empty string" }
    return value
}

private fun asLong(value: Any?, field: String): Long = when (value) {
    is Number -> value.toLong()
    is String -> value.toLong()
    else -> throw IllegalArgumentException("$field must be an integer")
}

private fun b64(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

private fun b64decode(value: String): ByteArray = Base64.decode(value, Base64.DEFAULT)

private fun hexLower(bytes: ByteArray): String {
    val out = StringBuilder(bytes.size * 2)
    for (byte in bytes) {
        val value = byte.toInt() and 0xff
        if (value < 16) out.append('0')
        out.append(value.toString(16))
    }
    return out.toString()
}

private fun hexBytes(value: String): ByteArray {
    val normalized = value.removePrefix("0x").removePrefix("0X").lowercase()
    require(normalized.length % 2 == 0) { "hex string must have an even length" }
    val out = ByteArray(normalized.length / 2)
    for (index in out.indices) {
        val hi = Character.digit(normalized[index * 2], 16)
        val lo = Character.digit(normalized[index * 2 + 1], 16)
        require(hi >= 0 && lo >= 0) { "hex string must contain only hex digits" }
        out[index] = ((hi shl 4) or lo).toByte()
    }
    return out
}
