package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.Locale;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Encodes the atomic {@code CommitContractDeployment} transaction instruction. */
public final class CommitContractDeploymentWirePayloadEncoder {
  public static final String WIRE_NAME =
      "iroha.instruction.v1::smart_contract_code::CommitContractDeployment";
  private static final String SCHEMA_NAME =
      "iroha_data_model::isi::smart_contract_code::CommitContractDeployment";
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Optional<BigInteger>> OPTIONAL_U64 =
      NoritoAdapters.option(new U64Adapter());
  private static final TypeAdapter<Optional<String>> OPTIONAL_STRING =
      NoritoAdapters.option(STRING);

  private CommitContractDeploymentWirePayloadEncoder() {}

  /** Builds a wire-framed instruction accepted by the standard transaction encoder. */
  public static InstructionBox encode(
      final BigInteger expectedDeployNonce,
      final String contractAddress,
      final String codeHashHex,
      final String contractAlias,
      final BigInteger leaseExpiryMs,
      final String expectedPreviousContractAddress) {
    final Payload payload =
        new Payload(
            requireU64(expectedDeployNonce, "expectedDeployNonce"),
            exact(contractAddress, "contractAddress"),
            hash(codeHashHex),
            exact(contractAlias, "contractAlias"),
            Optional.ofNullable(leaseExpiryMs)
                .map(value -> requireU64(value, "leaseExpiryMs")),
            Optional.ofNullable(expectedPreviousContractAddress)
                .map(value -> exact(value, "expectedPreviousContractAddress")));
    return InstructionBox.fromWirePayload(
        WIRE_NAME, NoritoCodec.encode(payload, SCHEMA_NAME, new PayloadAdapter()));
  }

  private static String exact(final String value, final String field) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be an exact non-empty string");
    }
    return value;
  }

  private static BigInteger requireU64(final BigInteger value, final String field) {
    if (value == null || value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    return value;
  }

  private static byte[] hash(final String value) {
    final String normalized = exact(value, "codeHashHex").toLowerCase(Locale.ROOT);
    if (!normalized.matches("[0-9a-f]{64}")) {
      throw new IllegalArgumentException("codeHashHex must contain exactly 64 hexadecimal characters");
    }
    final byte[] bytes = new byte[32];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(normalized.substring(index * 2, index * 2 + 2), 16);
    }
    requireCanonicalHashBytes(bytes);
    return bytes;
  }

  static byte[] decodeCanonicalCodeHashBytes(final byte[] value) {
    final NoritoDecoder decoder = new NoritoDecoder(Objects.requireNonNull(value, "value"), 0);
    final byte[] decoded = FixedHashAdapter.INSTANCE.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException("code_hash must contain exactly 32 bytes");
    }
    return decoded;
  }

  private static void requireCanonicalHashBytes(final byte[] value) {
    if (Objects.requireNonNull(value, "code_hash").length != 32) {
      throw new IllegalArgumentException("code_hash must contain exactly 32 bytes");
    }
    if ((value[value.length - 1] & 1) == 0) {
      throw new IllegalArgumentException(
          "code_hash must carry the canonical iroha_crypto::Hash marker bit");
    }
  }

  private static final class Payload {
    private final BigInteger nonce;
    private final String address;
    private final byte[] hash;
    private final String alias;
    private final Optional<BigInteger> expiry;
    private final Optional<String> previous;
    private Payload(final BigInteger nonce, final String address, final byte[] hash,
        final String alias, final Optional<BigInteger> expiry, final Optional<String> previous) {
      this.nonce = nonce; this.address = address; this.hash = hash.clone();
      this.alias = alias; this.expiry = expiry; this.previous = previous;
    }
  }

  private static final class U64Adapter implements TypeAdapter<BigInteger> {
    @Override public void encode(final NoritoEncoder encoder, final BigInteger value) {
      encoder.writeUInt(requireU64(value, "u64").longValue(), 64);
    }
    @Override public BigInteger decode(final NoritoDecoder decoder) {
      final long value = decoder.readUInt(64);
      return value >= 0 ? BigInteger.valueOf(value) : BigInteger.valueOf(value & Long.MAX_VALUE).setBit(63);
    }
  }

  private static final class PayloadAdapter implements TypeAdapter<Payload> {
    private static final TypeAdapter<BigInteger> U64 = new U64Adapter();
    @Override public void encode(final NoritoEncoder encoder, final Payload value) {
      field(encoder, U64, value.nonce);
      field(encoder, STRING, value.address);
      field(encoder, FixedHashAdapter.INSTANCE, value.hash);
      field(encoder, STRING, value.alias);
      field(encoder, OPTIONAL_U64, value.expiry);
      field(encoder, OPTIONAL_STRING, value.previous);
    }
    @Override public Payload decode(final NoritoDecoder decoder) {
      throw new UnsupportedOperationException("deployment instruction decoding is not exposed");
    }
  }

  private static final class FixedHashAdapter implements TypeAdapter<byte[]> {
    private static final FixedHashAdapter INSTANCE = new FixedHashAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final byte[] value) {
      requireCanonicalHashBytes(value);
      encoder.writeBytes(value);
    }

    @Override
    public byte[] decode(final NoritoDecoder decoder) {
      final byte[] value = decoder.readBytes(32);
      requireCanonicalHashBytes(value);
      return value;
    }
  }

  private static <T> void field(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] bytes = child.toByteArray();
    encoder.writeLength(bytes.length, false);
    encoder.writeBytes(bytes);
  }
}
