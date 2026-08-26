package org.hyperledger.iroha.android.crypto;

import java.security.KeyPairGenerator;
import java.security.Provider;
import java.security.Security;
import java.util.Optional;

public final class SoftwareKeyProviderBouncyCastleTests {

  private SoftwareKeyProviderBouncyCastleTests() {}

  public static void main(final String[] args) {
    helperLoadsDirectlyLinkedProvider();
    System.out.println("[IrohaAndroid] Software key provider BouncyCastle tests passed.");
  }

  private static void helperLoadsDirectlyLinkedProvider() {
    final Provider registeredProvider = Security.getProvider("BC");
    final Optional<KeyPairGenerator> generator = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert generator.isPresent() : "BouncyCastle provider should be available";
    final Optional<KeyPairGenerator> secondAttempt = SoftwareKeyProvider.tryBouncyCastleGenerator();
    assert secondAttempt.isPresent()
        : "BouncyCastle provider should remain available on subsequent attempts";
    assert generator.get().getProvider().getName().equals("BC") : "Unexpected provider name";
    assert secondAttempt.get().getProvider().getName().equals("BC") : "Unexpected provider name";
    final Provider activeProvider = Security.getProvider("BC");
    assert activeProvider != null : "BouncyCastle provider should be registered";
    assert registeredProvider == null || activeProvider == registeredProvider
        : "Helper must retain an already registered provider";
  }
}
