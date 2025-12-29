package org.hyperledger.iroha.android.norito;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

/**
 * Norito adapter that mirrors the {@link TransactionPayload} structure used by the Android library.
 * The current implementation focuses on the IVM bytecode variant of `Executable`; instruction-list
 * support will be added alongside dedicated builders.
 */
final class TransactionPayloadAdapter implements TypeAdapter<TransactionPayload> {

  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_AS_LONG_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<byte[]> BYTES_ADAPTER = NoritoAdapters.bytesAdapter();
  private static final TypeAdapter<Map<String, String>> STRING_MAP_ADAPTER =
      NoritoAdapters.map(STRING_ADAPTER, STRING_ADAPTER);
  private static final TypeAdapter<List<InstructionBox>> INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new InstructionAdapter());
  private static final TypeAdapter<Long> ENUM_TAG_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<Optional<Long>> TTL_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<Optional<Long>> NONCE_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(32));
  private static final TypeAdapter<Map<String, String>> METADATA_ADAPTER =
      NoritoAdapters.map(STRING_ADAPTER, STRING_ADAPTER);

  @Override
  public void encode(final NoritoEncoder encoder, final TransactionPayload value) {
    STRING_ADAPTER.encode(encoder, value.chainId());
    STRING_ADAPTER.encode(encoder, value.authority());
    UINT64_ADAPTER.encode(encoder, value.creationTimeMs());
    encodeExecutable(encoder, value.executable());
    TTL_ADAPTER.encode(encoder, value.timeToLiveMs());
    NONCE_ADAPTER.encode(encoder, value.nonce().map(Integer::longValue));
    METADATA_ADAPTER.encode(encoder, value.metadata());
  }

  @Override
  public TransactionPayload decode(final NoritoDecoder decoder) {
    final String chainId = STRING_ADAPTER.decode(decoder);
    final String authority = STRING_ADAPTER.decode(decoder);
    final long creationTimeMs = UINT64_ADAPTER.decode(decoder);
    final Executable executable = decodeExecutable(decoder);
    final Optional<Long> ttl = TTL_ADAPTER.decode(decoder);
    final Optional<Long> nonceRaw = NONCE_ADAPTER.decode(decoder);
    final Map<String, String> metadata = new LinkedHashMap<>(METADATA_ADAPTER.decode(decoder));

    final TransactionPayload.Builder builder =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(authority)
            .setCreationTimeMs(creationTimeMs)
            .setExecutable(executable)
            .setMetadata(metadata);
    ttl.ifPresent(builder::setTimeToLiveMs);
    nonceRaw.ifPresent(value -> builder.setNonce(Math.toIntExact(value)));
    return builder.build();
  }

  private void encodeExecutable(final NoritoEncoder encoder, final Executable executable) {
    if (executable.isIvm()) {
      ENUM_TAG_ADAPTER.encode(encoder, 1L);
      BYTES_ADAPTER.encode(encoder, executable.ivmBytes());
      return;
    }
    ENUM_TAG_ADAPTER.encode(encoder, 0L);
    INSTRUCTION_LIST_ADAPTER.encode(encoder, executable.instructions());
  }

  private Executable decodeExecutable(final NoritoDecoder decoder) {
    final long tag = ENUM_TAG_ADAPTER.decode(decoder);
    if (tag == 1L) {
      final byte[] bytes = BYTES_ADAPTER.decode(decoder);
      return Executable.ivm(bytes);
    }
    if (tag == 0L) {
      final List<InstructionBox> instructions = INSTRUCTION_LIST_ADAPTER.decode(decoder);
      return Executable.instructions(instructions);
    }
    throw new IllegalArgumentException("Unknown Executable discriminant: " + tag);
  }

  private static final class InstructionAdapter implements TypeAdapter<InstructionBox> {
    @Override
    public void encode(final NoritoEncoder encoder, final InstructionBox value) {
      ENUM_TAG_ADAPTER.encode(encoder, (long) value.kind().discriminant());
      STRING_MAP_ADAPTER.encode(encoder, value.arguments());
    }

    @Override
    public InstructionBox decode(final NoritoDecoder decoder) {
      final long discriminant = ENUM_TAG_ADAPTER.decode(decoder);
      final Map<String, String> arguments = new LinkedHashMap<>(STRING_MAP_ADAPTER.decode(decoder));
      final InstructionKind kind = InstructionKind.fromDiscriminant(discriminant);
      return InstructionBox.fromNorito(kind, arguments);
    }
  }
}
