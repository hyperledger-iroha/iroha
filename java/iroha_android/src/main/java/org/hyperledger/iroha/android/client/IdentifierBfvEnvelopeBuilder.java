package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/** Builds framed Norito BFV identifier ciphertext envelopes from plaintext input. */
final class IdentifierBfvEnvelopeBuilder {
  private static final String SCHEMA_NAME =
      "iroha_crypto::fhe_bfv::BfvIdentifierCiphertext";
  private static final byte[] PRG_DOMAIN =
      "iroha.sdk.identifier.bfv.prg.v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] SLOT_DOMAIN =
      "iroha.sdk.identifier.bfv.slot.v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] U_DOMAIN =
      "iroha.sdk.identifier.bfv.u.v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] E1_DOMAIN =
      "iroha.sdk.identifier.bfv.e1.v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] E2_DOMAIN =
      "iroha.sdk.identifier.bfv.e2.v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] NORITO_MAGIC = {'N', 'R', 'T', '0'};
  private static final long FNV_OFFSET = 0xcbf29ce484222325L;
  private static final long FNV_PRIME = 0x100000001b3L;
  private static final long CRC64_POLY = 0xC96C5795D7870F42L;
  private static final long[] CRC64_TABLE = buildCrc64Table();
  private static final SecureRandom SECURE_RANDOM = new SecureRandom();

  private IdentifierBfvEnvelopeBuilder() {}

  static String encrypt(
      final IdentifierPolicySummary policy, final String input, final byte[] seedOverride) {
    Objects.requireNonNull(policy, "policy");
    if (!"bfv-v1".equalsIgnoreCase(policy.inputEncryption())) {
      throw new IllegalArgumentException(
          "Policy " + policy.policyId() + " does not publish BFV encrypted-input support");
    }
    final IdentifierBfvPublicParameters publicParameters = policy.inputEncryptionPublicParametersDecoded();
    if (publicParameters == null) {
      throw new IllegalArgumentException(
          "Policy " + policy.policyId() + " is missing decoded BFV public parameters");
    }
    final String normalizedInput = policy.normalization().normalize(input, "input");
    final ValidatedParameters params = validate(publicParameters);
    final byte[] inputBytes = normalizedInput.getBytes(StandardCharsets.UTF_8);
    if (inputBytes.length > params.maxInputBytes) {
      throw new IllegalArgumentException(
          "input exceeds maxInputBytes " + params.maxInputBytes);
    }
    final byte[] seed =
        seedOverride == null ? randomSeed() : Arrays.copyOf(seedOverride, seedOverride.length);
    final List<Long> scalars = encodeIdentifierScalars(params.maxInputBytes, inputBytes);
    final List<CiphertextSlot> slots = new ArrayList<>(scalars.size());
    for (int index = 0; index < scalars.size(); index++) {
      slots.add(encryptScalar(params, scalars.get(index), seed, index));
    }
    return bytesToHex(frameNorito(encodeEnvelopePayload(slots)));
  }

  private static byte[] randomSeed() {
    final byte[] seed = new byte[32];
    SECURE_RANDOM.nextBytes(seed);
    return seed;
  }

  private static ValidatedParameters validate(final IdentifierBfvPublicParameters publicParameters) {
    Objects.requireNonNull(publicParameters, "publicParameters");
    final IdentifierBfvPublicParameters.Parameters params = publicParameters.parameters();
    final int polynomialDegree = Math.toIntExact(params.polynomialDegree());
    if (polynomialDegree < 2 || (polynomialDegree & (polynomialDegree - 1)) != 0) {
      throw new IllegalArgumentException(
          "BFV polynomialDegree must be a power of two and at least 2");
    }
    if (params.decompositionBaseLog() < 1 || params.decompositionBaseLog() > 16) {
      throw new IllegalArgumentException("BFV decompositionBaseLog must be within 1..=16");
    }
    final BigInteger plaintextModulus = toUnsignedBigInteger(params.plaintextModulus());
    final BigInteger ciphertextModulus = toUnsignedBigInteger(params.ciphertextModulus());
    if (plaintextModulus.compareTo(BigInteger.TWO) < 0) {
      throw new IllegalArgumentException("BFV plaintextModulus must be at least 2");
    }
    if (ciphertextModulus.compareTo(plaintextModulus) <= 0) {
      throw new IllegalArgumentException(
          "BFV ciphertextModulus must be greater than plaintextModulus");
    }
    if (!ciphertextModulus.mod(plaintextModulus).equals(BigInteger.ZERO)) {
      throw new IllegalArgumentException(
          "BFV ciphertextModulus must be divisible by plaintextModulus");
    }
    final int maxInputBytes = publicParameters.maxInputBytes();
    if (maxInputBytes < 1) {
      throw new IllegalArgumentException("BFV maxInputBytes must be at least 1");
    }
    if (BigInteger.valueOf(maxInputBytes).compareTo(plaintextModulus) >= 0) {
      throw new IllegalArgumentException(
          "BFV maxInputBytes must fit into one plaintext slot");
    }
    final List<Long> rawA = publicParameters.publicKey().a();
    final List<Long> rawB = publicParameters.publicKey().b();
    if (rawA.size() != polynomialDegree || rawB.size() != polynomialDegree) {
      throw new IllegalArgumentException(
          "BFV public-key polynomials must match polynomialDegree");
    }
    final BigInteger[] a = new BigInteger[polynomialDegree];
    final BigInteger[] b = new BigInteger[polynomialDegree];
    for (int index = 0; index < polynomialDegree; index++) {
      a[index] = toUnsignedBigInteger(rawA.get(index));
      b[index] = toUnsignedBigInteger(rawB.get(index));
      if (a[index].compareTo(ciphertextModulus) >= 0 || b[index].compareTo(ciphertextModulus) >= 0) {
        throw new IllegalArgumentException(
            "BFV public-key coefficient exceeds ciphertextModulus");
      }
    }
    return new ValidatedParameters(
        polynomialDegree,
        ciphertextModulus,
        ciphertextModulus.divide(plaintextModulus),
        maxInputBytes,
        a,
        b);
  }

  private static List<Long> encodeIdentifierScalars(final int maxInputBytes, final byte[] inputBytes) {
    final List<Long> scalars = new ArrayList<>(maxInputBytes + 1);
    scalars.add((long) inputBytes.length);
    for (final byte inputByte : inputBytes) {
      scalars.add((long) (inputByte & 0xff));
    }
    while (scalars.size() < maxInputBytes + 1) {
      scalars.add(0L);
    }
    return scalars;
  }

  private static CiphertextSlot encryptScalar(
      final ValidatedParameters params, final long scalar, final byte[] seed, final int slotIndex) {
    final byte[] slotSeed =
        sha512(
            SLOT_DOMAIN,
            seed,
            littleEndianUInt64(slotIndex & 0xffff_ffffL));
    final BigInteger[] u =
        sampleSmallPolynomial(params, new DeterministicStream(slotSeed, U_DOMAIN));
    final BigInteger[] e1 =
        sampleSmallPolynomial(params, new DeterministicStream(slotSeed, E1_DOMAIN));
    final BigInteger[] e2 =
        sampleSmallPolynomial(params, new DeterministicStream(slotSeed, E2_DOMAIN));
    final BigInteger[] encoded = zeroPolynomial(params.polynomialDegree);
    encoded[0] = BigInteger.valueOf(scalar).multiply(params.delta).mod(params.ciphertextModulus);
    return new CiphertextSlot(
        addPolynomialMod(
            addPolynomialMod(
                multiplyPolynomialMod(params, params.publicKeyB, u), e1, params.ciphertextModulus),
            encoded,
            params.ciphertextModulus),
        addPolynomialMod(
            multiplyPolynomialMod(params, params.publicKeyA, u), e2, params.ciphertextModulus));
  }

  private static BigInteger[] sampleSmallPolynomial(
      final ValidatedParameters params, final DeterministicStream stream) {
    final BigInteger[] output = new BigInteger[params.polynomialDegree];
    for (int index = 0; index < output.length; index++) {
      final int sample = stream.nextByte() & 0xff;
      switch (sample % 3) {
        case 0:
          output[index] = BigInteger.ZERO;
          break;
        case 1:
          output[index] = BigInteger.ONE;
          break;
        default:
          output[index] = params.ciphertextModulus.subtract(BigInteger.ONE);
          break;
      }
    }
    return output;
  }

  private static BigInteger[] zeroPolynomial(final int degree) {
    final BigInteger[] output = new BigInteger[degree];
    Arrays.fill(output, BigInteger.ZERO);
    return output;
  }

  private static BigInteger[] addPolynomialMod(
      final BigInteger[] lhs, final BigInteger[] rhs, final BigInteger modulus) {
    final BigInteger[] output = new BigInteger[lhs.length];
    for (int index = 0; index < lhs.length; index++) {
      output[index] = lhs[index].add(rhs[index]).mod(modulus);
    }
    return output;
  }

  private static BigInteger[] multiplyPolynomialMod(
      final ValidatedParameters params, final BigInteger[] lhs, final BigInteger[] rhs) {
    final BigInteger[] output = zeroPolynomial(params.polynomialDegree);
    for (int i = 0; i < params.polynomialDegree; i++) {
      for (int j = 0; j < params.polynomialDegree; j++) {
        final BigInteger term = lhs[i].multiply(rhs[j]).mod(params.ciphertextModulus);
        final int target = i + j;
        if (target < params.polynomialDegree) {
          output[target] = output[target].add(term).mod(params.ciphertextModulus);
        } else {
          output[target - params.polynomialDegree] =
              output[target - params.polynomialDegree]
                  .subtract(term)
                  .mod(params.ciphertextModulus);
        }
      }
    }
    return output;
  }

  private static byte[] encodeEnvelopePayload(final List<CiphertextSlot> slots) {
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    writeField(payload, encodeVecSlots(slots));
    return payload.toByteArray();
  }

  private static byte[] encodeVecSlots(final List<CiphertextSlot> slots) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeUInt64(out, slots.size());
    for (final CiphertextSlot slot : slots) {
      final byte[] payload = encodeSlot(slot);
      writeUInt64(out, payload.length);
      out.writeBytes(payload);
    }
    return out.toByteArray();
  }

  private static byte[] encodeSlot(final CiphertextSlot slot) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeField(out, encodeVecU64(slot.c0));
    writeField(out, encodeVecU64(slot.c1));
    return out.toByteArray();
  }

  private static byte[] encodeVecU64(final BigInteger[] values) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeUInt64(out, values.length);
    for (final BigInteger value : values) {
      final byte[] payload = littleEndianUInt64(toUnsignedLong(value));
      writeUInt64(out, payload.length);
      out.writeBytes(payload);
    }
    return out.toByteArray();
  }

  private static byte[] frameNorito(final byte[] payload) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.writeBytes(NORITO_MAGIC);
    out.write(0);
    out.write(0);
    final byte[] schemaHash = schemaHash(SCHEMA_NAME);
    out.writeBytes(schemaHash);
    out.write(0);
    out.writeBytes(littleEndianUInt64(payload.length));
    out.writeBytes(littleEndianUInt64(crc64(payload)));
    out.write(0);
    out.writeBytes(payload);
    return out.toByteArray();
  }

  private static void writeField(final ByteArrayOutputStream out, final byte[] payload) {
    writeUInt64(out, payload.length);
    out.writeBytes(payload);
  }

  private static void writeUInt64(final ByteArrayOutputStream out, final long value) {
    out.writeBytes(littleEndianUInt64(value));
  }

  private static byte[] littleEndianUInt64(final long value) {
    final byte[] out = new byte[8];
    for (int index = 0; index < 8; index++) {
      out[index] = (byte) ((value >>> (index * 8)) & 0xff);
    }
    return out;
  }

  private static byte[] schemaHash(final String typeName) {
    long hash = FNV_OFFSET;
    for (final byte value : typeName.getBytes(StandardCharsets.UTF_8)) {
      hash ^= value & 0xffL;
      hash *= FNV_PRIME;
    }
    final byte[] part = littleEndianUInt64(hash);
    final byte[] output = new byte[16];
    System.arraycopy(part, 0, output, 0, 8);
    System.arraycopy(part, 0, output, 8, 8);
    return output;
  }

  private static long crc64(final byte[] payload) {
    long crc = -1L;
    for (final byte value : payload) {
      final int index = (int) ((crc ^ (value & 0xffL)) & 0xffL);
      crc = CRC64_TABLE[index] ^ (crc >>> 8);
    }
    return crc ^ -1L;
  }

  private static long[] buildCrc64Table() {
    final long[] table = new long[256];
    for (int index = 0; index < table.length; index++) {
      long crc = index;
      for (int bit = 0; bit < 8; bit++) {
        if ((crc & 1L) != 0L) {
          crc = (crc >>> 1) ^ CRC64_POLY;
        } else {
          crc >>>= 1;
        }
      }
      table[index] = crc;
    }
    return table;
  }

  private static BigInteger toUnsignedBigInteger(final long value) {
    if (value >= 0) {
      return BigInteger.valueOf(value);
    }
    return BigInteger.valueOf(value & Long.MAX_VALUE).setBit(Long.SIZE - 1);
  }

  private static long toUnsignedLong(final BigInteger value) {
    return value.longValue();
  }

  private static byte[] sha512(final byte[]... parts) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-512");
      for (final byte[] part : parts) {
        digest.update(part);
      }
      return digest.digest();
    } catch (final NoSuchAlgorithmException error) {
      throw new IllegalStateException("SHA-512 is unavailable", error);
    }
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(Character.forDigit((value >>> 4) & 0xf, 16));
      builder.append(Character.forDigit(value & 0xf, 16));
    }
    return builder.toString();
  }

  private static final class DeterministicStream {
    private final byte[] seed;
    private final byte[] domain;
    private long counter;
    private byte[] buffer = new byte[0];
    private int index;

    private DeterministicStream(final byte[] seed, final byte[] domain) {
      this.seed = Arrays.copyOf(seed, seed.length);
      this.domain = Arrays.copyOf(domain, domain.length);
    }

    private byte nextByte() {
      if (index >= buffer.length) {
        refill();
      }
      return buffer[index++];
    }

    private void refill() {
      buffer = sha512(PRG_DOMAIN, domain, seed, littleEndianUInt64(counter));
      index = 0;
      counter++;
    }
  }

  private static final class ValidatedParameters {
    private final int polynomialDegree;
    private final BigInteger ciphertextModulus;
    private final BigInteger delta;
    private final int maxInputBytes;
    private final BigInteger[] publicKeyA;
    private final BigInteger[] publicKeyB;

    private ValidatedParameters(
        final int polynomialDegree,
        final BigInteger ciphertextModulus,
        final BigInteger delta,
        final int maxInputBytes,
        final BigInteger[] publicKeyA,
        final BigInteger[] publicKeyB) {
      this.polynomialDegree = polynomialDegree;
      this.ciphertextModulus = ciphertextModulus;
      this.delta = delta;
      this.maxInputBytes = maxInputBytes;
      this.publicKeyA = publicKeyA;
      this.publicKeyB = publicKeyB;
    }
  }

  private static final class CiphertextSlot {
    private final BigInteger[] c0;
    private final BigInteger[] c1;

    private CiphertextSlot(final BigInteger[] c0, final BigInteger[] c1) {
      this.c0 = c0;
      this.c1 = c1;
    }
  }
}
