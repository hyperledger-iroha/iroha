package org.hyperledger.iroha.android.crypto;

import java.security.KeyPairGenerator;
import java.security.Provider;
import java.security.Security;
import java.util.Optional;

public final class SoftwareKeyProviderFallbackTests {

  private SoftwareKeyProviderFallbackTests() {}

  public static void main(final String[] args) {
    fallbackLoadsBouncyCastle();
    removeUnknownProviderIsSafe();
    System.out.println("[IrohaAndroid] Software key provider fallback tests passed.");
  }

  private static void fallbackLoadsBouncyCastle() {
    final String providerName = "BC";
    Security.removeProvider(providerName);
    final Optional<KeyPairGenerator> generator = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert generator.isPresent() : "Expected BouncyCastle fallback to be present";
    final Provider provider = generator.get().getProvider();
    assert provider.getName().equals(providerName) : "Unexpected provider name";
    Security.removeProvider(provider.getName());
  }

  private static void removeUnknownProviderIsSafe() {
    Security.removeProvider("BC");
    final Optional<KeyPairGenerator> generator = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert generator.isPresent() : "Fallback provider should be available";
    Security.removeProvider(generator.get().getProvider().getName());
    final Optional<KeyPairGenerator> secondAttempt = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert secondAttempt.isPresent() : "Fallback should remain available on subsequent attempts";
    Security.removeProvider(secondAttempt.get().getProvider().getName());
  }
}
