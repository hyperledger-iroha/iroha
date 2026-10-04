package org.hyperledger.iroha.android.crypto.keystore;

import java.security.Key;
import java.security.KeyStoreException;
import java.util.Optional;
import javax.crypto.spec.SecretKeySpec;
import org.hyperledger.iroha.android.KeyManagementException;

/** Tri-state alias probe of {@link SystemAndroidKeystoreBackend} (mirrors the Kotlin SDK tests). */
public final class AndroidKeystoreAliasProbeTests {

  private static final Key KEY = new SecretKeySpec(new byte[32], "AES");

  public static void main(final String[] args) throws Exception {
    final AndroidKeystoreAliasProbeTests tests = new AndroidKeystoreAliasProbeTests();
    tests.keystore2NullIsADefinitiveAbsence();
    tests.keystoreErrorIsNeverAbsence();
    tests.keystore1NullIsNeverDefinitive();
    tests.presentKeyIsReturnedOnEveryApiLevel();
    tests.desktopRuntimeDoesNotOfferTheBackend();
    System.out.println("[IrohaAndroid] Android keystore alias probe tests passed.");
  }

  private static void check(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private void keystore2NullIsADefinitiveAbsence() throws Exception {
    final Optional<Key> probed =
        SystemAndroidKeystoreBackend.probeAlias(31, "alias", alias -> null, "probe");
    check(probed.isEmpty(), "keystore2 null must be a definitive absence");
  }

  private void keystoreErrorIsNeverAbsence() {
    for (final int api : new int[] {28, 31, 35}) {
      final KeyStoreException failure = new KeyStoreException("keystore2 binder failure");
      try {
        SystemAndroidKeystoreBackend.probeAlias(api, "alias", alias -> { throw failure; }, "probe");
        throw new AssertionError("a Keystore error must not read as absence");
      } catch (final KeyManagementException expected) {
        check(expected.getCause() == failure, "the Keystore error must be the cause");
      }
    }
  }

  private void keystore1NullIsNeverDefinitive() {
    for (final int api : new int[] {24, 28, 30}) {
      try {
        SystemAndroidKeystoreBackend.probeAlias(api, "alias", alias -> null, "probe");
        throw new AssertionError("keystore1 null must not read as absence");
      } catch (final KeyManagementException expected) {
        // keystore1 cannot prove absence.
      }
    }
  }

  private void presentKeyIsReturnedOnEveryApiLevel() throws Exception {
    for (final int api : new int[] {24, 30, 31}) {
      final Optional<Key> probed =
          SystemAndroidKeystoreBackend.probeAlias(api, "alias", alias -> KEY, "probe");
      check(probed.isPresent() && probed.get() == KEY, "a returned key is present");
    }
  }

  private void desktopRuntimeDoesNotOfferTheBackend() {
    check(
        SystemAndroidKeystoreBackend.create().isEmpty(),
        "the platform backend must not be offered without keystore2");
  }
}
