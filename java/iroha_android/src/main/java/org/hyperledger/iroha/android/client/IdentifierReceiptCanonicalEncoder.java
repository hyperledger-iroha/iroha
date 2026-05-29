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

  static IdentifierResolutionPayload decodePayload(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final String policyId =
        decodePolicyId(decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.policy_id"));
    final IdentifierResolutionExecutionPayload execution =
        decodeExecution(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.execution"));
    final RamLfeOutputOpening opening =
        decodeOutputOpening(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.opening"));
    final String opaqueId =
        decodeOpaqueHash(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.opaque_id"),
            "opaque:",
            "payload.opaque_id");
    final String receiptHash =
        hashHex(decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.receipt_hash"));
    final String uaid =
        decodeOpaqueHash(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.uaid"),
            "uaid:",
            "payload.uaid");
    final String accountId =
        TransferWirePayloadEncoder.decodeAccountIdPayload(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.account_id"),
            decoder.flags(),
            decoder.flagsHint());
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after identifier receipt payload");
    }
    return new IdentifierResolutionPayload(
        policyId, execution, opening, opaqueId, receiptHash, uaid, accountId);
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

  static IdentifierReceiptAttestation decodeAttestation(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final long tag = decoder.readUInt(32);
    final IdentifierReceiptAttestation attestation;
    if (tag == 0L) {
      final byte[] signature =
          decodeSizedField(decoder, SIGNATURE_ADAPTER, "attestation.signature");
      attestation = new IdentifierReceiptAttestation("signed", hexLower(signature), null, null);
    } else if (tag == 1L) {
      final ProofBoxPayload proof = decodeSizedField(decoder, ProofBoxAdapter.INSTANCE, "attestation.proof");
      attestation =
          new IdentifierReceiptAttestation(
              "proof", null, proof.backend, Base64.getEncoder().encodeToString(proof.bytes));
    } else {
      throw new IllegalArgumentException("Unsupported identifier attestation tag: " + tag);
    }
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after identifier receipt attestation");
    }
    return attestation;
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

  private static String decodePolicyId(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final String kind = decodeSizedField(decoder, STRING_ADAPTER, "payload.policy_id.kind");
    final String rule = decodeSizedField(decoder, STRING_ADAPTER, "payload.policy_id.rule");
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after payload.policy_id");
    }
    return kind + "#" + rule;
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

  private static IdentifierResolutionExecutionPayload decodeExecution(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final String programId =
        decodeProgramId(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.execution.program_id"));
    final String programDigest =
        hashHex(
            decodeSizedField(
                decoder, PassthroughBytesAdapter.INSTANCE, "payload.execution.program_digest"));
    final String backend =
        backendName(
            Math.toIntExact(
                decodeSizedField(decoder, U32Adapter.INSTANCE, "payload.execution.backend")));
    final String verificationMode =
        verificationModeName(
            Math.toIntExact(
                decodeSizedField(
                    decoder, U32Adapter.INSTANCE, "payload.execution.verification_mode")));
    final String inputCiphertextHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.execution.input_ciphertext_hash"));
    final String outputCiphertextHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.execution.output_ciphertext_hash"));
    final String parameterDigest =
        hashHex(
            decodeSizedField(
                decoder, PassthroughBytesAdapter.INSTANCE, "payload.execution.parameter_digest"));
    final String evaluationKeyDigest =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.execution.evaluation_key_digest"));
    final String outputHash =
        hashHex(
            decodeSizedField(
                decoder, PassthroughBytesAdapter.INSTANCE, "payload.execution.output_hash"));
    final String associatedDataHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.execution.associated_data_hash"));
    final long executedAtMs =
        decodeSizedField(decoder, U64_ADAPTER, "payload.execution.executed_at_ms");
    final Long expiresAtMs =
        decodeSizedField(decoder, OptionalU64Adapter.INSTANCE, "payload.execution.expires_at_ms");
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after payload.execution");
    }
    return new IdentifierResolutionExecutionPayload(
        programId,
        programDigest,
        backend,
        verificationMode,
        inputCiphertextHash,
        outputCiphertextHash,
        parameterDigest,
        evaluationKeyDigest,
        outputHash,
        associatedDataHash,
        executedAtMs,
        expiresAtMs);
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

  private static RamLfeOutputOpening decodeOutputOpening(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final RamLfeOutputOpeningPayload payload =
        decodeOutputOpeningPayload(
            decodeSizedField(decoder, PassthroughBytesAdapter.INSTANCE, "payload.opening.payload"));
    final String signature =
        hexLower(decodeSizedField(decoder, SIGNATURE_ADAPTER, "payload.opening.signature"));
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after payload.opening");
    }
    return new RamLfeOutputOpening(payload, signature);
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

  private static RamLfeOutputOpeningPayload decodeOutputOpeningPayload(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final String programId =
        decodeProgramId(
            decodeSizedField(
                decoder, PassthroughBytesAdapter.INSTANCE, "payload.opening.payload.program_id"));
    final String inputCiphertextHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.opening.payload.input_ciphertext_hash"));
    final String outputCiphertextHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.opening.payload.output_ciphertext_hash"));
    final String parameterDigest =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.opening.payload.parameter_digest"));
    final String evaluationKeyDigest =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.opening.payload.evaluation_key_digest"));
    final String openedOutputHash =
        hashHex(
            decodeSizedField(
                decoder,
                PassthroughBytesAdapter.INSTANCE,
                "payload.opening.payload.opened_output_hash"));
    final long openedAtMs =
        decodeSizedField(decoder, U64_ADAPTER, "payload.opening.payload.opened_at_ms");
    final Long expiresAtMs =
        decodeSizedField(decoder, OptionalU64Adapter.INSTANCE, "payload.opening.payload.expires_at_ms");
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after payload.opening.payload");
    }
    return new RamLfeOutputOpeningPayload(
        programId,
        inputCiphertextHash,
        outputCiphertextHash,
        parameterDigest,
        evaluationKeyDigest,
        openedOutputHash,
        openedAtMs,
        expiresAtMs);
  }

  private static byte[] encodeProgramId(final String raw) {
    final NoritoEncoder writer = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    encodeSizedField(writer, STRING_ADAPTER, requireNonBlank(raw, "payload.execution.program_id"));
    return writer.toByteArray();
  }

  private static String decodeProgramId(final byte[] encoded) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final String programId = decodeSizedField(decoder, STRING_ADAPTER, "program_id");
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after program_id");
    }
    return programId;
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

  private static String backendName(final int tag) {
    switch (tag) {
      case 0:
        return "hkdf-sha3-512-prf-v1";
      case 1:
        return "bfv-affine-sha3-256-v1";
      case 2:
        return "bfv-programmed-sha3-256-v1";
      default:
        throw new IllegalArgumentException("unsupported RAM-LFE backend tag: " + tag);
    }
  }

  private static String verificationModeName(final int tag) {
    switch (tag) {
      case 0:
        return "signed";
      case 1:
        return "proof";
      default:
        throw new IllegalArgumentException("unsupported RAM-LFE verification mode tag: " + tag);
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

  private static String decodeOpaqueHash(
      final byte[] encoded, final String prefix, final String field) {
    final NoritoDecoder decoder =
        new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
    final long length = decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    if (length != 32L) {
      throw new IllegalArgumentException(field + " must contain 32 bytes");
    }
    final byte[] hash = decoder.readBytes(32);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + field);
    }
    return prefix + hashHex(hash);
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

  private static <T> T decodeSizedField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter, final String fieldName) {
    final long length = decoder.readLength((decoder.flags() & NoritoHeader.COMPACT_LEN) != 0);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException("Trailing bytes after " + fieldName + " payload");
    }
    return value;
  }

  private static String hashHex(final byte[] bytes) {
    if (bytes.length != 32) {
      throw new IllegalArgumentException("hash value must contain 32 bytes");
    }
    return hexLower(bytes);
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format("%02x", value & 0xFF));
    }
    return builder.toString();
  }

  private enum U32Adapter implements TypeAdapter<Long> {
    INSTANCE;

    @Override
    public void encode(final NoritoEncoder encoder, final Long value) {
      encoder.writeUInt(value, 32);
    }

    @Override
    public Long decode(final NoritoDecoder decoder) {
      return decoder.readUInt(32);
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
      final int tag = decoder.readByte();
      if (tag == 0) {
        return null;
      }
      if (tag != 1) {
        throw new IllegalArgumentException("Invalid optional u64 tag: " + tag);
      }
      return decodeSizedField(decoder, U64_ADAPTER, "optional u64");
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
      return decoder.readBytes(decoder.remaining());
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
      final String backend = decodeSizedField(decoder, STRING_ADAPTER, "proof.backend");
      final byte[] bytes = decodeSizedField(decoder, RAW_BYTE_VEC_ADAPTER, "proof.bytes");
      return new ProofBoxPayload(backend, bytes);
    }
  }
}
