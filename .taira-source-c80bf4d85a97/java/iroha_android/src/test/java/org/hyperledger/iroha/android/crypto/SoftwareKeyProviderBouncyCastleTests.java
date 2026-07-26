package org.hyperledger.iroha.android.crypto;

import java.security.KeyPairGenerator;
import java.security.Provider;
import java.security.Security;
import java.util.Optional;

public final class SoftwareKeyProviderBouncyCastleTests {

  private SoftwareKeyProviderBouncyCastleTests() {}

  public static void main(final String[] args) {
    helperLoadsBouncyCastle();
    removeUnknownProviderIsSafe();
    System.out.println("[IrohaAndroid] Software key provider BouncyCastle tests passed.");
  }

  private static void helperLoadsBouncyCastle() {
    final String providerName = "BC";
    Security.removeProvider(providerName);
    final Optional<KeyPairGenerator> generator = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert generator.isPresent() : "Expected BouncyCastle provider to be present";
    final Provider provider = generator.get().getProvider();
    assert provider.getName().equals(providerName) : "Unexpected provider name";
    Security.removeProvider(provider.getName());
  }

  private static void removeUnknownProviderIsSafe() {
    Security.removeProvider("BC");
    final Optional<KeyPairGenerator> generator = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert generator.isPresent() : "BouncyCastle provider should be available";
    Security.removeProvider(generator.get().getProvider().getName());
    final Optional<KeyPairGenerator> secondAttempt = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert secondAttempt.isPresent() : "BouncyCastle provider should remain available on subsequent attempts";
    Security.removeProvider(secondAttempt.get().getProvider().getName());
  }
}
