package org.hyperledger.iroha.sdk.tx.offline

import java.util.LinkedHashMap
import java.util.Optional
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Norito adapter that encodes/decodes `OfflineSigningEnvelope` values. */
internal class OfflineSigningEnvelopeAdapter : TypeAdapter<OfflineSigningEnvelope> {

    override fun encode(encoder: NoritoEncoder, value: OfflineSigningEnvelope) {
        STRING_ADAPTER.encode(encoder, value.schemaName)
        STRING_ADAPTER.encode(encoder, value.keyAlias)
        UINT64_ADAPTER.encode(encoder, value.issuedAtMs)
        METADATA_ADAPTER.encode(encoder, value.metadata)
        BYTES_ADAPTER.encode(encoder, value.encodedPayload)
        BYTES_ADAPTER.encode(encoder, value.signature)
        BYTES_ADAPTER.encode(encoder, value.publicKey)
        OPTIONAL_BYTES_ADAPTER.encode(encoder, Optional.ofNullable(value.exportedKeyBundle))
    }

    override fun decode(decoder: NoritoDecoder): OfflineSigningEnvelope {
        val schemaName = STRING_ADAPTER.decode(decoder)
        val keyAlias = STRING_ADAPTER.decode(decoder)
        val issuedAtMs = UINT64_ADAPTER.decode(decoder)
        val metadata = LinkedHashMap(METADATA_ADAPTER.decode(decoder))
        val payload = BYTES_ADAPTER.decode(decoder)
        val signature = BYTES_ADAPTER.decode(decoder)
        val publicKey = BYTES_ADAPTER.decode(decoder)
        val exportedKey: Optional<ByteArray> = OPTIONAL_BYTES_ADAPTER.decode(decoder)

        return OfflineSigningEnvelope(
            encodedPayload = payload,
            signature = signature,
            publicKey = publicKey,
            schemaName = schemaName,
            keyAlias = keyAlias,
            issuedAtMs = issuedAtMs,
            metadata = metadata,
            exportedKeyBundle = exportedKey.orElse(null),
        )
    }

    companion object {
        private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
        private val UINT64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
        private val BYTES_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.bytesAdapter()
        private val METADATA_ADAPTER: TypeAdapter<Map<String, String>> =
            NoritoAdapters.map(STRING_ADAPTER, STRING_ADAPTER)
        private val OPTIONAL_BYTES_ADAPTER: TypeAdapter<Optional<ByteArray>> =
            NoritoAdapters.option(BYTES_ADAPTER)
    }
}
