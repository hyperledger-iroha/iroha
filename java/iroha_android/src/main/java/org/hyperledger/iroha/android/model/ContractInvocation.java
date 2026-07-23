package org.hyperledger.iroha.android.model;

import java.util.Arrays;
import java.util.Objects;

/** A signature-bound invocation of one deployed contract instance. */
public final class ContractInvocation {

  /** Maximum canonical argument-record size accepted by the transaction wire format. */
  public static final int MAX_ARGUMENT_BYTES = 1024 * 1024;

  private static final int EXPECTED_CODE_HASH_BYTES = 32;

  private final String contractAddress;
  private final byte[] expectedCodeHash;
  private final String entrypoint;
  private final byte[] arguments;

  /**
   * Creates a deployed-contract invocation.
   *
   * @param contractAddress canonical Bech32m contract address
   * @param expectedCodeHash exact 32-byte code hash authorized by the signer
   * @param entrypoint public contract entrypoint
   * @param arguments optional canonical argument-record bytes, or {@code null}
   */
  public ContractInvocation(
      final String contractAddress,
      final byte[] expectedCodeHash,
      final String entrypoint,
      final byte[] arguments) {
    this.contractAddress = ContractAddressValidator.requireCanonicalV1(contractAddress);
    this.expectedCodeHash = requireCodeHash(expectedCodeHash);
    this.entrypoint = requireExact(entrypoint, "entrypoint");
    this.arguments = requireArguments(arguments);
  }

  public String contractAddress() {
    return contractAddress;
  }

  public byte[] expectedCodeHash() {
    return expectedCodeHash.clone();
  }

  public String entrypoint() {
    return entrypoint;
  }

  /** Returns a defensive copy of the argument record, or {@code null} when absent. */
  public byte[] arguments() {
    return arguments == null ? null : arguments.clone();
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof ContractInvocation)) {
      return false;
    }
    final ContractInvocation that = (ContractInvocation) other;
    return contractAddress.equals(that.contractAddress)
        && Arrays.equals(expectedCodeHash, that.expectedCodeHash)
        && entrypoint.equals(that.entrypoint)
        && Arrays.equals(arguments, that.arguments);
  }

  @Override
  public int hashCode() {
    int result = Objects.hash(contractAddress, entrypoint);
    result = 31 * result + Arrays.hashCode(expectedCodeHash);
    result = 31 * result + Arrays.hashCode(arguments);
    return result;
  }

  private static String requireExact(final String value, final String field) {
    final String nonNull = Objects.requireNonNull(value, field);
    if (nonNull.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    if (!nonNull.trim().equals(nonNull)) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    return nonNull;
  }

  private static byte[] requireCodeHash(final byte[] value) {
    final byte[] nonNull = Objects.requireNonNull(value, "expectedCodeHash");
    if (nonNull.length != EXPECTED_CODE_HASH_BYTES) {
      throw new IllegalArgumentException("expectedCodeHash must contain exactly 32 bytes");
    }
    if ((nonNull[EXPECTED_CODE_HASH_BYTES - 1] & 1) != 1) {
      throw new IllegalArgumentException(
          "expectedCodeHash must use Iroha's marked hash encoding");
    }
    return nonNull.clone();
  }

  private static byte[] requireArguments(final byte[] value) {
    if (value == null) {
      return null;
    }
    if (value.length > MAX_ARGUMENT_BYTES) {
      throw new IllegalArgumentException(
          "arguments must not exceed " + MAX_ARGUMENT_BYTES + " bytes");
    }
    return value.clone();
  }
}
