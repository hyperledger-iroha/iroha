package org.hyperledger.iroha.android.offline;

/** Native helpers for computing offline balance commitments and proofs. */
public final class OfflineBalanceProof {
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available;
    try {
      System.loadLibrary("connect_norito_bridge");
      available = true;
    } catch (UnsatisfiedLinkError error) {
      available = false;
    }
    NATIVE_AVAILABLE = available;
  }

  private OfflineBalanceProof() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] updateCommitment(
      final byte[] initialCommitment,
      final String claimedDelta,
      final byte[] initialBlinding,
      final byte[] resultingBlinding) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    return nativeUpdateCommitment(initialCommitment, claimedDelta, initialBlinding, resultingBlinding);
  }

  public static byte[] generateProof(
      final String chainId,
      final byte[] initialCommitment,
      final byte[] resultingCommitment,
      final String claimedDelta,
      final String resultingValue,
      final byte[] initialBlinding,
      final byte[] resultingBlinding) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }
    return nativeGenerate(
        chainId,
        initialCommitment,
        resultingCommitment,
        claimedDelta,
        resultingValue,
        initialBlinding,
        resultingBlinding);
  }

  public static Artifacts advanceCommitment(
      final String chainId,
      final String claimedDelta,
      final String resultingValue,
      final String initialCommitmentHex,
      final String initialBlindingHex,
      final String resultingBlindingHex) {
    final byte[] initialCommitment = decodeHex(initialCommitmentHex, "initialCommitmentHex");
    final byte[] initialBlinding = decodeHex(initialBlindingHex, "initialBlindingHex");
    final byte[] resultingBlinding = decodeHex(resultingBlindingHex, "resultingBlindingHex");
    final byte[] resultingCommitment =
        updateCommitment(initialCommitment, claimedDelta, initialBlinding, resultingBlinding);
    final byte[] proof =
        generateProof(
            chainId,
            initialCommitment,
            resultingCommitment,
            claimedDelta,
            resultingValue,
            initialBlinding,
            resultingBlinding);
    return new Artifacts(resultingCommitment, proof);
  }

  public static final class Artifacts {
    private final byte[] resultingCommitment;
    private final byte[] proof;

    private Artifacts(final byte[] resultingCommitment, final byte[] proof) {
      this.resultingCommitment = resultingCommitment;
      this.proof = proof;
    }

    public byte[] getResultingCommitment() {
      return resultingCommitment.clone();
    }

    public byte[] getProof() {
      return proof.clone();
    }

    public String getResultingCommitmentHex() {
      return encodeHex(resultingCommitment);
    }

    public String getProofHex() {
      return encodeHex(proof);
    }
  }

  private static native byte[] nativeUpdateCommitment(
      byte[] initialCommitment,
      String claimedDelta,
      byte[] initialBlinding,
      byte[] resultingBlinding);

  private static native byte[] nativeGenerate(
      String chainId,
      byte[] initialCommitment,
      byte[] resultingCommitment,
      String claimedDelta,
      String resultingValue,
      byte[] initialBlinding,
      byte[] resultingBlinding);

  private static byte[] decodeHex(final String hex, final String field) {
    final String trimmed = hex == null ? "" : hex.trim();
    if ((trimmed.length() & 1) == 1) {
      throw new IllegalArgumentException(field + " must contain an even number of hex characters");
    }
    final byte[] out = new byte[trimmed.length() / 2];
    for (int i = 0; i < trimmed.length(); i += 2) {
      final int hi = Character.digit(trimmed.charAt(i), 16);
      final int lo = Character.digit(trimmed.charAt(i + 1), 16);
      if (hi == -1 || lo == -1) {
        throw new IllegalArgumentException(field + " contains non-hex characters");
      }
      out[i / 2] = (byte) ((hi << 4) + lo);
    }
    return out;
  }

  private static String encodeHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
