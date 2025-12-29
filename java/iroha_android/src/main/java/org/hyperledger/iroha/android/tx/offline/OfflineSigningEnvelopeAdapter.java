package org.hyperledger.iroha.android.tx.offline;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Norito adapter that encodes/decodes {@link OfflineSigningEnvelope} values. */
final class OfflineSigningEnvelopeAdapter implements TypeAdapter<OfflineSigningEnvelope> {

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<byte[]> BYTES_ADAPTER = NoritoAdapters.bytesAdapter();
  private static final TypeAdapter<Map<String, String>> METADATA_ADAPTER =
      NoritoAdapters.map(STRING_ADAPTER, STRING_ADAPTER);
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_BYTES_ADAPTER =
      NoritoAdapters.option(BYTES_ADAPTER);

  @Override
  public void encode(final NoritoEncoder encoder, final OfflineSigningEnvelope value) {
    STRING_ADAPTER.encode(encoder, value.schemaName());
    STRING_ADAPTER.encode(encoder, value.keyAlias());
    UINT64_ADAPTER.encode(encoder, value.issuedAtMs());
    METADATA_ADAPTER.encode(encoder, value.metadata());
    BYTES_ADAPTER.encode(encoder, value.encodedPayload());
    BYTES_ADAPTER.encode(encoder, value.signature());
    BYTES_ADAPTER.encode(encoder, value.publicKey());
    OPTIONAL_BYTES_ADAPTER.encode(encoder, value.exportedKeyBundle());
  }

  @Override
  public OfflineSigningEnvelope decode(final NoritoDecoder decoder) {
    final String schemaName = STRING_ADAPTER.decode(decoder);
    final String keyAlias = STRING_ADAPTER.decode(decoder);
    final long issuedAtMs = UINT64_ADAPTER.decode(decoder);
    final Map<String, String> metadata = new LinkedHashMap<>(METADATA_ADAPTER.decode(decoder));
    final byte[] payload = BYTES_ADAPTER.decode(decoder);
    final byte[] signature = BYTES_ADAPTER.decode(decoder);
    final byte[] publicKey = BYTES_ADAPTER.decode(decoder);
    final Optional<byte[]> exportedKey = OPTIONAL_BYTES_ADAPTER.decode(decoder);

    final OfflineSigningEnvelope.Builder builder =
        OfflineSigningEnvelope.builder()
            .setSchemaName(schemaName)
            .setKeyAlias(keyAlias)
            .setIssuedAtMs(issuedAtMs)
            .setMetadata(metadata)
            .setEncodedPayload(payload)
            .setSignature(signature)
            .setPublicKey(publicKey);
    exportedKey.ifPresent(builder::setExportedKeyBundle);
    return builder.build();
  }
}
