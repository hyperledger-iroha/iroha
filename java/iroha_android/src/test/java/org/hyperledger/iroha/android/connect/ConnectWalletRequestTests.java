package org.hyperledger.iroha.android.connect;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import org.junit.Test;

public final class ConnectWalletRequestTests {

  @Test
  public void acceptsIrohaconnectLaunchUri() throws Exception {
    final ConnectWalletRequest request =
        ConnectWalletRequest.parse(
            "irohaconnect://connect?sid="
                + sampleSid()
                + "&chain_id=taira-testnet&node=taira.sora.org&token=wallet-token",
            new URI("https://default.sora.org"));

    assert sampleSid().equals(request.sidBase64Url()) : "sid mismatch";
    assert "wallet-token".equals(request.token()) : "token mismatch";
    assert "taira-testnet".equals(request.chainId()) : "chain mismatch";
    assert "https://taira.sora.org".equals(request.baseUri().toString()) : "base URI mismatch";
    assert "wss://taira.sora.org/v1/connect/ws?sid="
            .concat(sampleSid())
            .concat("&role=wallet")
            .equals(request.webSocketUri().toString())
        : "ws URI mismatch";
  }

  @Test
  public void acceptsWrappedIrohaconnectLaunchUri() throws Exception {
    final String embeddedUri =
        "irohaconnect://connect?sid="
            + sampleSid()
            + "&chain_id=taira-testnet&token=wallet-token";
    final ConnectWalletRequest request =
        ConnectWalletRequest.parse(
            "irohaconnect://wc?uri="
                + URLEncoder.encode(embeddedUri, StandardCharsets.UTF_8.name()),
            new URI("https://taira.sora.org"));

    assert sampleSid().equals(request.sidBase64Url()) : "sid mismatch";
    assert "wallet-token".equals(request.token()) : "token mismatch";
    assert "taira-testnet".equals(request.chainId()) : "chain mismatch";
    assert "https://taira.sora.org".equals(request.baseUri().toString()) : "base URI mismatch";
  }

  private static String sampleSid() {
    return Base64.getUrlEncoder().withoutPadding().encodeToString(new byte[32]);
  }
}
