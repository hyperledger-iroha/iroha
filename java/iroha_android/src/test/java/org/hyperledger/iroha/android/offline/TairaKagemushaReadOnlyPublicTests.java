package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.time.Duration;
import java.util.concurrent.TimeUnit;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.TairaTestnetProfile;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.model.NetworkId;
import org.junit.Test;

/** Opt-in, credential-free public Taira Kagemusha capability probe. */
public final class TairaKagemushaReadOnlyPublicTests {
  private static final String OPT_IN_ENV = "IROHA_TAIRA_KAGEMUSHA_READ_ONLY";
  private static final String PUBLIC_ROOT_ENV = "IROHA_TAIRA_PUBLIC_ROOT";
  private static final Duration DEADLINE = Duration.ofSeconds(20);

  // The read-only endpoint never consumes this required construction context.
  private static final String NON_SIGNING_TEST_NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";

  @Test
  public void publicCapabilityMatchesExactUniversalContract() throws Exception {
    assertEquals(URI.create("https://taira.sora.org"), TairaTestnetProfile.TORII_BASE_URI);
    if (!"1".equals(System.getenv(OPT_IN_ENV))) {
      return;
    }

    final KagemushaRecursiveSpendProver.ToriiClient client =
        KagemushaRecursiveSpendProver.newToriiClient(
            publicRoot(),
            new UrlConnectionTransportExecutor(DEADLINE),
            new LocalSigningContext(NetworkId.parse(NON_SIGNING_TEST_NETWORK_ID)));
    final KagemushaRecursiveSpendProver.OfflineStatus capability =
        client.getOfflineCapability().get(DEADLINE.getSeconds(), TimeUnit.SECONDS);

    assertEquals("cash_handoff_v1", capability.cashHandoffCapability());
    assertEquals(23, capability.requiredBridgeAbiVersion());
    assertEquals(8, capability.maximumHops());
    assertTrue(capability.ready());
  }

  private static URI publicRoot() {
    final String configured = System.getenv(PUBLIC_ROOT_ENV);
    final String raw =
        configured == null ? TairaTestnetProfile.TORII_BASE_URI.toString() : configured;
    if (!raw.equals(raw.trim())) {
      throw new IllegalArgumentException(
          PUBLIC_ROOT_ENV + " must not contain surrounding whitespace");
    }
    final URI root = URI.create(raw);
    final String path = root.getRawPath();
    if (!root.isAbsolute()
        || root.isOpaque()
        || !"https".equalsIgnoreCase(root.getScheme())
        || root.getHost() == null
        || root.getHost().isEmpty()
        || root.getRawUserInfo() != null
        || root.getRawQuery() != null
        || root.getRawFragment() != null
        || !(path == null || path.isEmpty() || "/".equals(path))) {
      throw new IllegalArgumentException(
          PUBLIC_ROOT_ENV
              + " must be a credential-free HTTPS origin without a path, query, or fragment");
    }
    return URI.create(raw.endsWith("/") ? raw.substring(0, raw.length() - 1) : raw);
  }
}
