package org.bouncycastle.jce.provider;

import java.security.KeyPair;
import java.security.KeyPairGeneratorSpi;
import java.security.PrivateKey;
import java.security.Provider;
import java.security.PublicKey;
import java.security.SecureRandom;
import java.security.spec.AlgorithmParameterSpec;

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
    private static final byte[] ENCODED = new byte[] {0x01, 0x02, 0x03};

    @Override
    public String getAlgorithm() {
      return "EdDSA";
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
    private static final byte[] ENCODED = new byte[] {0x04, 0x05, 0x06};

    @Override
    public String getAlgorithm() {
      return "EdDSA";
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
}
