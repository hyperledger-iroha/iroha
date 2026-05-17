package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2Store
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2WalletNote
import org.hyperledger.iroha.sdk.offline.OfflineNoteV2WalletNoteState

/** Android Keystore-backed encrypted store for Offline Note V2 wallet notes. */
class AndroidOfflineNoteV2SecureStore @JvmOverloads constructor(
    context: Context,
    preferencesName: String = DEFAULT_PREFERENCES_NAME,
    private val keyAlias: String = DEFAULT_KEY_ALIAS,
) : OfflineNoteV2Store {
    private val preferences = (context.applicationContext ?: context)
        .getSharedPreferences(requireNonBlank(preferencesName, "preferencesName"), Context.MODE_PRIVATE)

    @Synchronized
    override fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteV2WalletNote>) -> T): T {
        val notes = loadNotes()
        val result = mutator(notes)
        saveNotes(notes)
        return result
    }

    @Synchronized
    override fun listNotes(): List<OfflineNoteV2WalletNote> =
        loadNotes().values.sortedWith(compareBy<OfflineNoteV2WalletNote> { it.createdAtMs }.thenBy { it.noteCommitmentHex() })

    @Synchronized
    override fun findNote(noteCommitment: ByteArray): OfflineNoteV2WalletNote? =
        loadNotes()[hexLower(noteCommitment)]

    @Synchronized
    override fun upsert(note: OfflineNoteV2WalletNote) {
        mutateNotes {
            it[note.noteCommitmentHex()] = note
        }
    }

    @Synchronized
    fun delete(noteCommitment: ByteArray) {
        val commitmentHex = hexLower(noteCommitment)
        val index = indexSnapshot().toMutableSet()
        index.remove(commitmentHex)
        check(
            preferences.edit()
                .remove(noteKey(commitmentHex))
                .putStringSet(INDEX_KEY, index)
                .commit()
        ) { "failed to delete Offline Note V2 wallet note" }
    }

    @Synchronized
    fun clear() {
        check(preferences.edit().clear().commit()) { "failed to clear Offline Note V2 wallet notes" }
    }

    private fun loadNotes(): MutableMap<String, OfflineNoteV2WalletNote> {
        val notes = LinkedHashMap<String, OfflineNoteV2WalletNote>()
        for (commitmentHex in indexSnapshot()) {
            val encrypted = preferences.getString(noteKey(commitmentHex), null) ?: continue
            notes[commitmentHex] = WalletNoteJsonCodec.decode(decrypt(encrypted))
        }
        return notes
    }

    private fun saveNotes(notes: Map<String, OfflineNoteV2WalletNote>) {
        val oldIndex = indexSnapshot()
        val newIndex = notes.keys.toSet()
        val editor = preferences.edit()
        for (oldCommitment in oldIndex) {
            if (!newIndex.contains(oldCommitment)) {
                editor.remove(noteKey(oldCommitment))
            }
        }
        for ((commitmentHex, note) in notes) {
            editor.putString(noteKey(commitmentHex), encrypt(WalletNoteJsonCodec.encode(note)))
        }
        editor.putStringSet(INDEX_KEY, newIndex)
        check(editor.commit()) { "failed to persist Offline Note V2 wallet notes" }
    }

    private fun indexSnapshot(): Set<String> =
        preferences.getStringSet(INDEX_KEY, emptySet())?.toSet() ?: emptySet()

    private fun encrypt(plaintext: ByteArray): String {
        val cipher = Cipher.getInstance(AES_GCM)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        return VALUE_PREFIX +
            b64(cipher.iv) +
            ":" +
            b64(cipher.doFinal(plaintext))
    }

    private fun decrypt(envelope: String): ByteArray {
        require(envelope.startsWith(VALUE_PREFIX)) { "unknown Offline Note V2 wallet note envelope" }
        val parts = envelope.removePrefix(VALUE_PREFIX).split(':')
        require(parts.size == 2) { "invalid Offline Note V2 wallet note envelope" }
        val cipher = Cipher.getInstance(AES_GCM)
        cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(GCM_TAG_BITS, b64decode(parts[0])))
        return cipher.doFinal(b64decode(parts[1]))
    }

    private fun secretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER)
        keyStore.load(null)
        (keyStore.getEntry(keyAlias, null) as? KeyStore.SecretKeyEntry)?.let {
            return it.secretKey
        }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(
                keyAlias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(AES_KEY_BITS)
                .setRandomizedEncryptionRequired(true)
                .build()
        )
        return generator.generateKey()
    }

    private fun noteKey(commitmentHex: String): String = "$NOTE_PREFIX$commitmentHex"

    private object WalletNoteJsonCodec {
        fun encode(note: OfflineNoteV2WalletNote): ByteArray {
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
                "state" to note.state.name,
                "created_at_ms" to note.createdAtMs,
                "updated_at_ms" to note.updatedAtMs,
            )
            return JsonEncoder.encode(payload).toByteArray(Charsets.UTF_8)
        }

        fun decode(payload: ByteArray): OfflineNoteV2WalletNote {
            val obj = requireObject(JsonParser.parse(String(payload, Charsets.UTF_8)), "wallet note")
            require(asLong(obj["version"], "version") == 1L) { "unsupported Offline Note V2 wallet note version" }
            return OfflineNoteV2WalletNote(
                chainId = asString(obj["chain_id"], "chain_id"),
                accountId = asString(obj["account_id"], "account_id"),
                assetId = asString(obj["asset_id"], "asset_id"),
                amount = asString(obj["amount"], "amount"),
                keyCertificate = OfflineNoteV2.decodeCertificate(
                    b64decode(asString(obj["key_certificate_norito_base64"], "key_certificate_norito_base64"))
                ),
                noteCommitment = hexBytes(asString(obj["note_commitment_hex"], "note_commitment_hex")),
                noteSecret = b64decode(asString(obj["note_secret_base64"], "note_secret_base64")),
                origin = decodeOrigin(requireObject(obj["origin"], "origin")),
                state = OfflineNoteV2WalletNoteState.valueOf(asString(obj["state"], "state")),
                createdAtMs = asLong(obj["created_at_ms"], "created_at_ms"),
                updatedAtMs = asLong(obj["updated_at_ms"], "updated_at_ms"),
            )
        }

        private fun encodeOrigin(origin: OfflineNoteV2.CommitmentOriginV2): Map<String, Any?> =
            when (origin) {
                is OfflineNoteV2.CommitmentOriginV2.IssuerLoad -> linkedMapOf(
                    "kind" to "issuer_load",
                    "operation_id" to origin.operationId,
                    "lineage_id" to origin.lineageId,
                    "local_revision" to origin.localRevision,
                )
                is OfflineNoteV2.CommitmentOriginV2.P2pOutput -> linkedMapOf(
                    "kind" to "p2p_output",
                    "payment_request_id" to origin.paymentRequestId,
                    "output_index" to origin.outputIndex,
                )
            }

        private fun decodeOrigin(obj: Map<String, Any?>): OfflineNoteV2.CommitmentOriginV2 =
            when (asString(obj["kind"], "kind")) {
                "issuer_load" -> OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                    operationId = asString(obj["operation_id"], "operation_id"),
                    lineageId = asString(obj["lineage_id"], "lineage_id"),
                    localRevision = asLong(obj["local_revision"], "local_revision"),
                )
                "p2p_output" -> OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                    paymentRequestId = asString(obj["payment_request_id"], "payment_request_id"),
                    outputIndex = asLong(obj["output_index"], "output_index").toInt(),
                )
                else -> throw IllegalArgumentException("unsupported Offline Note V2 origin kind")
            }
    }

    companion object {
        const val DEFAULT_PREFERENCES_NAME = "org.hyperledger.iroha.offline_note_v2"
        const val DEFAULT_KEY_ALIAS = "org.hyperledger.iroha.offline_note_v2.store"
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val AES_GCM = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val AES_KEY_BITS = 256
        private const val INDEX_KEY = "note_index"
        private const val NOTE_PREFIX = "note."
        private const val VALUE_PREFIX = "v1:"
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
