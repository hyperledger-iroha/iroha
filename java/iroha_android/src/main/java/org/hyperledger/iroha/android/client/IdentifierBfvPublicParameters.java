package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Objects;

/** Structured BFV public parameters published by identifier policies. */
public final class IdentifierBfvPublicParameters {
  private final Parameters parameters;
  private final PublicKey publicKey;
  private final int maxInputBytes;

  public IdentifierBfvPublicParameters(
      final Parameters parameters, final PublicKey publicKey, final int maxInputBytes) {
    this.parameters = Objects.requireNonNull(parameters, "parameters");
    this.publicKey = Objects.requireNonNull(publicKey, "publicKey");
    this.maxInputBytes = maxInputBytes;
  }

  public Parameters parameters() {
    return parameters;
  }

  public PublicKey publicKey() {
    return publicKey;
  }

  public int maxInputBytes() {
    return maxInputBytes;
  }

  /** Scalar BFV parameter set. */
  public static final class Parameters {
    private final long polynomialDegree;
    private final long plaintextModulus;
    private final long ciphertextModulus;
    private final int decompositionBaseLog;

    public Parameters(
        final long polynomialDegree,
        final long plaintextModulus,
        final long ciphertextModulus,
        final int decompositionBaseLog) {
      this.polynomialDegree = polynomialDegree;
      this.plaintextModulus = plaintextModulus;
      this.ciphertextModulus = ciphertextModulus;
      this.decompositionBaseLog = decompositionBaseLog;
    }

    public long polynomialDegree() {
      return polynomialDegree;
    }

    public long plaintextModulus() {
      return plaintextModulus;
    }

    public long ciphertextModulus() {
      return ciphertextModulus;
    }

    public int decompositionBaseLog() {
      return decompositionBaseLog;
    }
  }

  /** BFV public-key polynomials. */
  public static final class PublicKey {
    private final List<Long> b;
    private final List<Long> a;

    public PublicKey(final List<Long> b, final List<Long> a) {
      this.b = List.copyOf(Objects.requireNonNull(b, "b"));
      this.a = List.copyOf(Objects.requireNonNull(a, "a"));
    }

    public List<Long> b() {
      return b;
    }

    public List<Long> a() {
      return a;
    }
  }
}
