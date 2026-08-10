package org.hyperledger.iroha.android.connect;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.Base64;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.android.testing.TestNetworkIds;
import org.junit.Test;

public final class ConnectWalletRequestTests {

  private final NetworkId networkId = TestNetworkIds.canonical();
  private final byte[] appPublicKey = sequence(32, 1);
  private final byte[] nonce = sequence(16, 65);
  private final byte[] sid = deriveSid(networkId, appPublicKey, nonce);

  @Test
  public void acceptsOnlyCanonicalLaunchIdentity() throws Exception {
    final ConnectWalletRequest request = parseRequest();

    assert b64(sid).equals(request.sidBase64Url()) : "sid mismatch";
    assert "wallet-token".equals(request.token()) : "token mismatch";
    assert "relay-token".equals(request.relayToken()) : "relay token mismatch";
    assert networkId.equals(request.networkId()) : "network mismatch";
    assert Arrays.equals(appPublicKey, request.appPublicKey()) : "app key mismatch";
    assert Arrays.equals(nonce, request.nonce()) : "nonce mismatch";
    assert "https://taira.sora.org".equals(request.baseUri().toString()) : "base URI mismatch";
    assert ("wss://taira.sora.org/v1/connect/ws?sid=" + b64(sid) + "&role=wallet")
        .equals(request.webSocketUri().toString()) : "ws URI mismatch";
  }

  @Test
  public void rejectsWrongNetworkSidAndAppKeySubstitution() throws Exception {
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                canonicalUri(TestNetworkIds.fromSeed(77), sid, appPublicKey),
                new URI("https://default.sora.org")));

    final byte[] wrongSid = sid.clone();
    wrongSid[0] ^= 1;
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                canonicalUri(networkId, wrongSid, appPublicKey),
                new URI("https://default.sora.org")));

    final byte[] wrongAppKey = appPublicKey.clone();
    wrongAppKey[0] ^= 1;
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                canonicalUri(networkId, sid, wrongAppKey),
                new URI("https://default.sora.org")));
  }

  @Test
  public void rejectsDuplicateAndRetiredLaunchParameters() throws Exception {
    final String uri = canonicalUri(networkId, sid, appPublicKey);
    expectProtocolFailure(
        () -> ConnectWalletRequest.parse(uri + "&sid=" + b64(sid), new URI("https://default.sora.org")));
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                uri.replace("iroha://", "irohaconnect://"),
                new URI("https://default.sora.org")));
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                uri + "&chain_id=taira-testnet", new URI("https://default.sora.org")));
    expectProtocolFailure(
        () ->
            ConnectWalletRequest.parse(
                uri.replace("token=wallet-token", "token_wallet=wallet-token"),
                new URI("https://default.sora.org")));
  }

  @Test
  public void openIsBoundToLaunchAndCannotBeReplayed() throws Exception {
    final ConnectWalletRequest request = parseRequest();
    final byte[] openFrame = ConnectFrameCodec.encodeOpenFrame(sid, appPublicKey, networkId);
    final ConnectFrameCodec.OpenControl open = request.acceptOpen(openFrame);
    assert networkId.equals(open.networkId()) : "open network mismatch";
    assert Arrays.equals(appPublicKey, open.appPublicKey()) : "open app key mismatch";
    expectProtocolFailure(() -> request.acceptOpen(openFrame));

    final ConnectWalletRequest wrongNetworkRequest = parseRequest();
    expectProtocolFailure(
        () ->
            wrongNetworkRequest.acceptOpen(
                ConnectFrameCodec.encodeOpenFrame(
                    sid, appPublicKey, TestNetworkIds.fromSeed(7))));

    final byte[] wrongAppKey = appPublicKey.clone();
    wrongAppKey[0] ^= 1;
    final ConnectWalletRequest wrongAppRequest = parseRequest();
    expectProtocolFailure(
        () -> wrongAppRequest.acceptOpen(ConnectFrameCodec.encodeOpenFrame(sid, wrongAppKey, networkId)));

    final byte[] wrongSid = sid.clone();
    wrongSid[0] ^= 1;
    final ConnectWalletRequest wrongSidRequest = parseRequest();
    expectProtocolFailure(
        () -> wrongSidRequest.acceptOpen(ConnectFrameCodec.encodeOpenFrame(wrongSid, appPublicKey, networkId)));
  }

  @Test
  public void approvalRequiresOpenAndVerifiesExactBindings() throws Exception {
    final ConnectWalletRequest request = parseRequest();
    final byte[] walletPublicKey = sequence(32, 99);
    final Ed25519PrivateKeyParameters signer =
        new Ed25519PrivateKeyParameters(fill(0x42, 32), 0);
    final String account =
        AccountAddress.fromAccount(signer.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);

    expectProtocolFailure(
        () -> request.buildApprovePreimage(walletPublicKey, account, null, null));
    request.acceptOpen(ConnectFrameCodec.encodeOpenFrame(sid, appPublicKey, networkId));
    final byte[] preimage =
        request.buildApprovePreimage(walletPublicKey, account, null, null);
    final Ed25519Signer edSigner = new Ed25519Signer();
    edSigner.init(true, signer);
    edSigner.update(preimage, 0, preimage.length);
    final byte[] signature = edSigner.generateSignature();
    final byte[] relayAuth = ConnectCrypto.relayAuthHash(sid, "relay-token");

    assert ConnectCrypto.verifyApprovalSignature(
        networkId,
        sid,
        appPublicKey,
        walletPublicKey,
        account,
        null,
        null,
        relayAuth,
        "ed25519",
        signature) : "exact approval must verify";
    assert !verifyApproval(
        TestNetworkIds.fromSeed(8), account, relayAuth, signature, walletPublicKey)
        : "wrong network must fail";
    assert !verifyApproval(networkId, sampleI105(0x45), relayAuth, signature, walletPublicKey)
        : "substituted account must fail";
    assert !verifyApproval(
        networkId,
        account,
        ConnectCrypto.relayAuthHash(sid, "other-relay"),
        signature,
        walletPublicKey) : "wrong relay must fail";
    final byte[] forged = signature.clone();
    forged[0] ^= 1;
    assert !verifyApproval(networkId, account, relayAuth, forged, walletPublicKey)
        : "forged signature must fail";
  }

  @Test
  public void sessionIdBindsNetworkKeyAndNonceAndRejectsZeros() throws Exception {
    assert !Arrays.equals(
        sid, ConnectCrypto.deriveSessionId(TestNetworkIds.fromSeed(9), appPublicKey, nonce));
    final byte[] otherApp = appPublicKey.clone();
    otherApp[0] ^= 1;
    assert !Arrays.equals(sid, ConnectCrypto.deriveSessionId(networkId, otherApp, nonce));
    final byte[] otherNonce = nonce.clone();
    otherNonce[0] ^= 1;
    assert !Arrays.equals(sid, ConnectCrypto.deriveSessionId(networkId, appPublicKey, otherNonce));
    expectProtocolFailure(() -> ConnectCrypto.deriveSessionId(networkId, new byte[32], nonce));
    expectProtocolFailure(() -> ConnectCrypto.deriveSessionId(networkId, appPublicKey, new byte[16]));
  }

  @Test
  public void derivesRelayAuthHash() throws Exception {
    final MessageDigest digest = MessageDigest.getInstance("SHA-256");
    digest.update("iroha-connect|relay-auth|v1".getBytes(StandardCharsets.UTF_8));
    digest.update(sid);
    digest.update("relay-token".getBytes(StandardCharsets.UTF_8));
    final byte[] expected = digest.digest();

    assert Arrays.equals(expected, ConnectCrypto.relayAuthHash(sid, "relay-token"))
        : "relay auth hash mismatch";
  }

  @Test
  public void relayAuthHashMatchesSharedFixture() throws Exception {
    final byte[] fixtureSid = new byte[32];
    for (int i = 0; i < fixtureSid.length; i++) {
      fixtureSid[i] = (byte) i;
    }

    assert "65de07a9c6110f16b6b7c64e63c71437d88d122344e1a67d2c932a16187cce2f"
            .equals(hex(ConnectCrypto.relayAuthHash(fixtureSid, "relay-token-vector")))
        : "relay auth fixture mismatch";
  }

  private boolean verifyApproval(
      final NetworkId network,
      final String account,
      final byte[] relayAuth,
      final byte[] signature,
      final byte[] walletPublicKey) {
    return ConnectCrypto.verifyApprovalSignature(
        network,
        sid,
        appPublicKey,
        walletPublicKey,
        account,
        null,
        null,
        relayAuth,
        "ed25519",
        signature);
  }

  private ConnectWalletRequest parseRequest() throws Exception {
    return ConnectWalletRequest.parse(
        canonicalUri(networkId, sid, appPublicKey), new URI("https://default.sora.org"));
  }

  private String canonicalUri(
      final NetworkId network, final byte[] sidBytes, final byte[] appKey) throws Exception {
    return "iroha://connect?sid="
        + b64(sidBytes)
        + "&network_id="
        + URLEncoder.encode(network.literal(), StandardCharsets.UTF_8.name())
        + "&app_pk="
        + b64(appKey)
        + "&nonce="
        + b64(nonce)
        + "&node=taira.sora.org&v=1&role=wallet&token=wallet-token&relay=relay-token";
  }

  private static String sampleI105(final int seed) throws Exception {
    return AccountAddress.fromAccount(TestEd25519Keys.publicKey(seed), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static byte[] deriveSid(
      final NetworkId network, final byte[] appKey, final byte[] launchNonce) {
    try {
      return ConnectCrypto.deriveSessionId(network, appKey, launchNonce);
    } catch (final ConnectProtocolException ex) {
      throw new AssertionError(ex);
    }
  }

  private static byte[] sequence(final int size, final int first) {
    final byte[] out = new byte[size];
    for (int i = 0; i < size; i++) {
      out[i] = (byte) (first + i);
    }
    return out;
  }

  private static byte[] fill(final int value, final int size) {
    final byte[] out = new byte[size];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static String b64(final byte[] value) {
    return Base64.getUrlEncoder().withoutPadding().encodeToString(value);
  }

  private static void expectProtocolFailure(final ThrowingOperation operation) throws Exception {
    try {
      operation.run();
      throw new AssertionError("expected ConnectProtocolException");
    } catch (final ConnectProtocolException expected) {
      // Expected.
    }
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      out.append(String.format("%02x", b & 0xff));
    }
    return out.toString();
  }

  @FunctionalInterface
  private interface ThrowingOperation {
    void run() throws Exception;
  }
}
