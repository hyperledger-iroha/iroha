package org.hyperledger.iroha.android.crypto;

/** Supported transaction and offline signing algorithms exposed by the Java/Android SDKs. */
public enum SigningAlgorithm {
  ED25519(0, "Ed25519", "ed25519"),
  SECP256K1(1, "Secp256k1", "secp256k1"),
  BLS_NORMAL(2, "BlsNormal", "bls_normal"),
  BLS_SMALL(3, "BlsSmall", "bls_small"),
  ML_DSA(4, "MlDsa", "ml-dsa"),
  GOST_2012_256_A(5, "Gost3410_2012_256ParamSetA", "gost3410-2012-256-paramset-a"),
  GOST_2012_256_B(6, "Gost3410_2012_256ParamSetB", "gost3410-2012-256-paramset-b"),
  GOST_2012_256_C(7, "Gost3410_2012_256ParamSetC", "gost3410-2012-256-paramset-c"),
  GOST_2012_512_A(8, "Gost3410_2012_512ParamSetA", "gost3410-2012-512-paramset-a"),
  GOST_2012_512_B(9, "Gost3410_2012_512ParamSetB", "gost3410-2012-512-paramset-b"),
  SM2(10, "Sm2", "sm2");

  private final int bridgeCode;
  private final String providerName;
  private final String wireName;

  SigningAlgorithm(final int bridgeCode, final String providerName, final String wireName) {
    this.bridgeCode = bridgeCode;
    this.providerName = providerName;
    this.wireName = wireName;
  }

  public int bridgeCode() {
    return bridgeCode;
  }

  public String providerName() {
    return providerName;
  }

  public String wireName() {
    return wireName;
  }

  public boolean supportsHardwareBackedKeys() {
    return this == ED25519;
  }

  public boolean isNativeBacked() {
    return this != ED25519;
  }

  public static SigningAlgorithm fromAlgorithmName(final String name) {
    final String normalized = normalize(name);
    if ("ed25519".equals(normalized) || "eddsa".equals(normalized)) {
      return ED25519;
    }
    if ("secp256k1".equals(normalized)
        || "secp".equals(normalized)
        || "secpk1".equals(normalized)) {
      return SECP256K1;
    }
    if ("mldsa".equals(normalized)
        || "mldsa65".equals(normalized)
        || "mldsa44".equals(normalized)
        || "mldsa87".equals(normalized)) {
      return ML_DSA;
    }
    if ("blsnormal".equals(normalized) || "bls12381g1".equals(normalized)) {
      return BLS_NORMAL;
    }
    if ("blssmall".equals(normalized) || "bls12381g2".equals(normalized)) {
      return BLS_SMALL;
    }
    if ("gost256a".equals(normalized)
        || "gost34102012256paramseta".equals(normalized)) {
      return GOST_2012_256_A;
    }
    if ("gost256b".equals(normalized)
        || "gost34102012256paramsetb".equals(normalized)) {
      return GOST_2012_256_B;
    }
    if ("gost256c".equals(normalized)
        || "gost34102012256paramsetc".equals(normalized)) {
      return GOST_2012_256_C;
    }
    if ("gost512a".equals(normalized)
        || "gost34102012512paramseta".equals(normalized)) {
      return GOST_2012_512_A;
    }
    if ("gost512b".equals(normalized)
        || "gost34102012512paramsetb".equals(normalized)) {
      return GOST_2012_512_B;
    }
    if ("sm2".equals(normalized)) {
      return SM2;
    }
    return ED25519;
  }

  public static SigningAlgorithm fromBridgeCode(final int code) {
    for (final SigningAlgorithm algorithm : values()) {
      if (algorithm.bridgeCode == code) {
        return algorithm;
      }
    }
    throw new IllegalArgumentException("Unsupported signing algorithm code: " + code);
  }

  private static String normalize(final String name) {
    if (name == null || name.isBlank()) {
      return ED25519.wireName;
    }
    final StringBuilder builder = new StringBuilder(name.length());
    for (int i = 0; i < name.length(); i++) {
      final char ch = Character.toLowerCase(name.charAt(i));
      if (Character.isLetterOrDigit(ch)) {
        builder.append(ch);
      }
    }
    return builder.toString();
  }
}
