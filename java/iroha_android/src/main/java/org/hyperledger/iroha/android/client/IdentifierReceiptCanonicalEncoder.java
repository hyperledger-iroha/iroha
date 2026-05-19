package org.hyperledger.iroha.android.client;

import java.util.Base64;
import java.util.Locale;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Canonical Norito encoder for identifier receipt payloads. */
public final class IdentifierReceiptCanonicalEncoder {
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<byte[]> SIGNATURE_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTE_VEC_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<Long> U64_ADAPTER = NoritoAdapters.uint(64);

  private IdentifierReceiptCanonicalEncoder() {}

  public static byte[] encodePayload(final IdentifierResolutionPayload payload) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, PassthroughBytesAdapter.INSTANCE, encodePolicyId(payload.policyId()));
    encodeSizedField(writer, PassthroughBytesAdapter.INSTANCE, encodeExecution(payload.execution()));
    encodeSizedField(writer, PassthroughBytesAdapter.INSTANCE, encodeOutputOpening(payload.opening()));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        encodeOpaqueHash(payload.opaqueId(), "opaque:", "payload.opaque_id"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.receiptHash(), "payload.receipt_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        encodeOpaqueHash(payload.uaid(), "uaid:", "payload.uaid"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        TransferWirePayloadEncoder.encodeAccountIdPayload(payload.accountId()));
    return writer.toByteArray();
  }

  public static byte[] encodeAttestation(final IdentifierReceiptAttestation attestation) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    switch (attestation.kind().toLowerCase(Locale.ROOT)) {
      case "signed":
        writer.writeUInt(0, 32);
        encodeSizedField(
            writer,
            SIGNATURE_ADAPTER,
            decodeHex(
                Objects.requireNonNull(attestation.signature(), "signed attestation requires signature"),
                "attestation.signature"));
        return writer.toByteArray();
      case "proof":
        writer.writeUInt(1, 32);
        encodeSizedField(
            writer,
            ProofBoxAdapter.INSTANCE,
            new ProofBoxPayload(
                Objects.requireNonNull(attestation.proofBackend(), "proof attestation requires proofBackend"),
                Base64.getDecoder().decode(
                    Objects.requireNonNull(attestation.proofB64(), "proof attestation requires proofB64"))));
        return writer.toByteArray();
      default:
        throw new IllegalArgumentException("attestation.kind must be signed or proof");
    }
  }

  private static byte[] encodePolicyId(final String raw) {
    final String[] parts = raw.trim().split("#", 2);
    if (parts.length != 2 || parts[0].trim().isEmpty() || parts[1].trim().isEmpty()) {
      throw new IllegalArgumentException("payload.policy_id must use kind#rule");
    }
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, STRING_ADAPTER, parts[0].trim());
    encodeSizedField(writer, STRING_ADAPTER, parts[1].trim());
    return writer.toByteArray();
  }

  private static byte[] encodeExecution(final IdentifierResolutionExecutionPayload execution) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, PassthroughBytesAdapter.INSTANCE, encodeProgramId(execution.programId()));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.programDigest(), "payload.execution.program_digest"));
    encodeSizedField(writer, U32Adapter.INSTANCE, (long) backendTag(execution.backend()));
    encodeSizedField(writer, U32Adapter.INSTANCE, (long) verificationModeTag(execution.verificationMode()));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.inputCiphertextHash(), "payload.execution.input_ciphertext_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.outputCiphertextHash(), "payload.execution.output_ciphertext_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.parameterDigest(), "payload.execution.parameter_digest"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.evaluationKeyDigest(), "payload.execution.evaluation_key_digest"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.outputHash(), "payload.execution.output_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(execution.associatedDataHash(), "payload.execution.associated_data_hash"));
    encodeSizedField(writer, U64_ADAPTER, execution.executedAtMs());
    encodeSizedField(writer, OptionalU64Adapter.INSTANCE, execution.expiresAtMs());
    return writer.toByteArray();
  }

  private static byte[] encodeOutputOpening(final RamLfeOutputOpening opening) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        encodeOutputOpeningPayload(opening.payload()));
    encodeSizedField(
        writer,
        SIGNATURE_ADAPTER,
        decodeHex(opening.signature(), "payload.opening.signature"));
    return writer.toByteArray();
  }

  private static byte[] encodeOutputOpeningPayload(final RamLfeOutputOpeningPayload payload) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, PassthroughBytesAdapter.INSTANCE, encodeProgramId(payload.programId()));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.inputCiphertextHash(), "payload.opening.payload.input_ciphertext_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.outputCiphertextHash(), "payload.opening.payload.output_ciphertext_hash"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.parameterDigest(), "payload.opening.payload.parameter_digest"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.evaluationKeyDigest(), "payload.opening.payload.evaluation_key_digest"));
    encodeSizedField(
        writer,
        PassthroughBytesAdapter.INSTANCE,
        decodeHash(payload.openedOutputHash(), "payload.opening.payload.opened_output_hash"));
    encodeSizedField(writer, U64_ADAPTER, payload.openedAtMs());
    encodeSizedField(writer, OptionalU64Adapter.INSTANCE, payload.expiresAtMs());
    return writer.toByteArray();
  }

  private static byte[] encodeProgramId(final String raw) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, STRING_ADAPTER, requireNonBlank(raw, "payload.execution.program_id"));
    return writer.toByteArray();
  }

  private static int backendTag(final String raw) {
    switch (raw.trim().toLowerCase(Locale.ROOT)) {
      case "hkdf-sha3-512-prf-v1":
        return 0;
      case "bfv-affine-sha3-256-v1":
        return 1;
      case "bfv-programmed-sha3-256-v1":
        return 2;
      default:
        throw new IllegalArgumentException("unsupported RAM-LFE backend: " + raw);
    }
  }

  private static int verificationModeTag(final String raw) {
    switch (raw.trim().toLowerCase(Locale.ROOT)) {
      case "signed":
        return 0;
      case "proof":
        return 1;
      default:
        throw new IllegalArgumentException("unsupported RAM-LFE verification mode: " + raw);
    }
  }

  private static byte[] encodePrefixedHash(
      final String raw, final String prefix, final String field) {
    final String normalized = raw.trim().toLowerCase(Locale.ROOT);
    return decodeHash(
        normalized.startsWith(prefix) ? normalized.substring(prefix.length()) : normalized, field);
  }

  private static byte[] encodeOpaqueHash(
      final String raw, final String prefix, final String field) {
    final byte[] hash = encodePrefixedHash(raw, prefix, field);
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    final boolean compact = (writer.flags() & NoritoHeader.COMPACT_LEN) != 0;
    writer.writeLength(hash.length, compact);
    writer.writeBytes(hash);
    return writer.toByteArray();
  }

  private static byte[] decodeHash(final String raw, final String field) {
    String body = requireNonBlank(raw, field);
    if (body.toLowerCase(Locale.ROOT).startsWith("hash:")) {
      body = body.substring("hash:".length());
    }
    final int suffixIndex = body.indexOf('#');
    if (suffixIndex >= 0) {
      body = body.substring(0, suffixIndex);
    }
    final byte[] bytes = decodeHex(body, field);
    if (bytes.length != 32) {
      throw new IllegalArgumentException(field + " must contain 32 bytes");
    }
    return bytes;
  }

  private static byte[] decodeHex(final String raw, final String field) {
    String trimmed = requireNonBlank(raw, field);
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) == 1) {
      throw new IllegalArgumentException(field + " must contain an even number of hex characters");
    }
    final byte[] out = new byte[trimmed.length() / 2];
    for (int i = 0; i < trimmed.length(); i += 2) {
      final int high = Character.digit(trimmed.charAt(i), 16);
      final int low = Character.digit(trimmed.charAt(i + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException(field + " contains non-hex characters");
      }
      out[i / 2] = (byte) ((high << 4) | low);
    }
    return out;
  }

  private static String requireNonBlank(final String value, final String field) {
    final String trimmed = value == null ? "" : value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return trimmed;
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private enum U32Adapter implements TypeAdapter<Long> {
    INSTANCE;

    @Override
    public void encode(final NoritoEncoder encoder, final Long value) {
      encoder.writeUInt(value, 32);
    }

    @Override
    public Long decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("decode not supported");
    }
  }

  private enum OptionalU64Adapter implements TypeAdapter<Long> {
    INSTANCE;

    @Override
    public void encode(final NoritoEncoder encoder, final Long value) {
      if (value == null) {
        encoder.writeByte(0);
      } else {
        encoder.writeByte(1);
        encodeSizedField(encoder, U64_ADAPTER, value);
      }
    }

    @Override
    public Long decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("decode not supported");
    }
  }

  private enum PassthroughBytesAdapter implements TypeAdapter<byte[]> {
    INSTANCE;

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("decode not supported");
    }
  }

  private static final class ProofBoxPayload {
    private final String backend;
    private final byte[] bytes;

    private ProofBoxPayload(final String backend, final byte[] bytes) {
      this.backend = backend;
      this.bytes = bytes.clone();
    }
  }

  private enum ProofBoxAdapter implements TypeAdapter<ProofBoxPayload> {
    INSTANCE;

    @Override
    public void encode(final NoritoEncoder encoder, final ProofBoxPayload value) {
      encodeSizedField(encoder, STRING_ADAPTER, value.backend);
      encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value.bytes);
    }

    @Override
    public ProofBoxPayload decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("decode not supported");
    }
  }
}
