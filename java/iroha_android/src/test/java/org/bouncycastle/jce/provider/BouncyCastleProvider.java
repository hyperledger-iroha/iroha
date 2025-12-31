package org.bouncycastle.jce.provider;

import java.security.KeyPair;
import java.security.KeyPairGeneratorSpi;
import java.security.PrivateKey;
import java.security.Provider;
import java.security.PublicKey;
import java.security.SecureRandom;
import java.security.spec.AlgorithmParameterSpec;
import java.util.Arrays;

/** Minimal BouncyCastle provider stub used for SoftwareKeyProvider fallback tests. */
public final class BouncyCastleProvider extends Provider {
  private static final long serialVersionUID = 1L;

  public BouncyCastleProvider() {
    super("BC", 1.0, "Test BouncyCastle provider");
    put("KeyPairGenerator.EdDSA", TestEdDsaGenerator.class.getName());
  }

  public static final class TestEdDsaGenerator extends KeyPairGeneratorSpi {
    @Override
    public void initialize(final int keysize, final SecureRandom random) {
      // No-op: fixed-size EdDSA key pairs.
    }

    @Override
    public void initialize(final AlgorithmParameterSpec params, final SecureRandom random) {
      // No-op: fixed-size EdDSA key pairs.
    }

    @Override
    public KeyPair generateKeyPair() {
      return new KeyPair(new TestPublicKey(), new TestPrivateKey());
    }
  }

  private static final class TestPublicKey implements PublicKey {
    private static final long serialVersionUID = 1L;
    private static final byte[] ENCODED = buildSpki();

    @Override
    public String getAlgorithm() {
      return "Ed25519";
    }

    @Override
    public String getFormat() {
      return "X.509";
    }

    @Override
    public byte[] getEncoded() {
      return ENCODED.clone();
    }
  }

  private static final class TestPrivateKey implements PrivateKey {
    private static final long serialVersionUID = 1L;
    private static final byte[] ENCODED = buildPkcs8();

    @Override
    public String getAlgorithm() {
      return "Ed25519";
    }

    @Override
    public String getFormat() {
      return "PKCS#8";
    }

    @Override
    public byte[] getEncoded() {
      return ENCODED.clone();
    }
  }

  private static byte[] buildSpki() {
    final byte[] prefix =
        new byte[] {
          0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
        };
    final byte[] publicKey =
        new byte[] {
          0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28,
          0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
          0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
          0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40
        };
    return concat(prefix, publicKey);
  }

  private static byte[] buildPkcs8() {
    final byte[] prefix =
        new byte[] {
          0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06,
          0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20
        };
    final byte[] privateKey =
        new byte[] {
          0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
          0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
          0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
          0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20
        };
    return concat(prefix, privateKey);
  }

  private static byte[] concat(final byte[] prefix, final byte[] suffix) {
    final byte[] out = Arrays.copyOf(prefix, prefix.length + suffix.length);
    System.arraycopy(suffix, 0, out, prefix.length, suffix.length);
    return out;
  }
}
