package org.hyperledger.iroha.qualification;

import org.hyperledger.iroha.sdk.crypto.keystore.KeySecurityPreference;

/** Load an actual Android AAR owner in the host JVM; this is not device qualification. */
public final class SorafsAndroidPackageLinkProbe {
  private SorafsAndroidPackageLinkProbe() {}

  /** Prove direct Java linkage to the packaged Kotlin Android enum without reflection. */
  public static void main(final String[] args) {
    if (args.length != 0) throw new IllegalArgumentException("no probe arguments accepted");
    if (KeySecurityPreference.valueOf("SOFTWARE_ONLY") != KeySecurityPreference.SOFTWARE_ONLY) {
      throw new AssertionError("Android package Kotlin owner did not link");
    }
  }
}
