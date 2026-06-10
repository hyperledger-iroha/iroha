package org.hyperledger.iroha.android.offline;

import android.util.Base64;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.hyperledger.iroha.sdk.offline.OfflineNotePaymentToken;
import org.hyperledger.iroha.sdk.offline.OfflineNotePaymentTokenCodec;
import org.hyperledger.iroha.sdk.offline.OfflineNoteReceiptAck;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferCapabilities;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferHandoff;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferModality;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferPayload;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferStreamReceiver;
import org.hyperledger.iroha.sdk.offline.OfflineNoteTransferStreamResult;
import org.junit.Test;
import org.junit.runner.RunWith;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

@RunWith(AndroidJUnit4.class)
public final class OfflineNoteTransferHandoffTest {

  @Test
  public void productionHarnessResolvesOfflineNoteTransferHandoffFixture() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(sdkInterop, "payment_token_norito_base64")));
    final byte[] canonicalPayload =
        base64Bytes(string(sdkInterop, "payment_token_norito_base64"));

    assertArrayEquals(canonicalPayload, OfflineNoteTransferHandoff.rawPaymentTokenBytes(token));
    assertEquals(string(payment, "token_id"), token.tokenIdHex());
    assertEquals(string(payment, "invoice_id"), token.getPaymentRequestId());
    assertEquals(longValue(payment, "created_at_ms"), token.getCreatedAtMs());
    assertEquals(32, token.tokenId().length);
    assertTrue(token.getAudit().noritoEncoded().length > 0);

    final OfflineNoteTransferCapabilities capabilities =
        OfflineNoteTransferCapabilities.current(false, true);
    assertTrue(
        capabilities.supportedModalities().contains(OfflineNoteTransferModality.QR_STREAMING));
    assertTrue(capabilities.supportedModalities().contains(OfflineNoteTransferModality.NEARBY));
    assertFalse(capabilities.supportedModalities().contains(OfflineNoteTransferModality.NFC));
  }

  @Test
  public void nearbyQrAndNfcTokenHandoffRoundTripFixtureBytes() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(sdkInterop, "payment_token_norito_base64")));
    final byte[] canonicalPayload =
        base64Bytes(string(sdkInterop, "payment_token_norito_base64"));

    final OfflineNoteTransferPayload nearby = OfflineNoteTransferHandoff.nearbyPayload(token);
    assertEquals(OfflineNoteTransferModality.NEARBY, nearby.getModality());
    assertEquals(OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE, nearby.getContentType());
    assertArrayEquals(canonicalPayload, nearby.payload());
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteTransferHandoff.decodePaymentToken(nearby).tokenIdHex());
    assertEquals(
        token.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeNearbyPaymentToken(
            OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(token))
            .tokenIdHex());

    final List<byte[]> qrFrames = OfflineNoteTransferHandoff.qrStreamingFrameBytes(token);
    assertTrue(qrFrames.size() > 1);
    assertEquals(token.tokenIdHex(), streamToken(qrFrames).tokenIdHex());

    final List<byte[]> nfcFrames = OfflineNoteTransferHandoff.nfcFrameBytes(token);
    assertTrue(nfcFrames.size() > 1);
    for (final byte[] frame : nfcFrames) {
      assertTrue("NFC frame should fit Android short APDU budget", frame.length <= 250);
    }
    assertEquals(token.tokenIdHex(), streamToken(nfcFrames).tokenIdHex());
  }

  @Test
  public void receiptAckHandoffRoundTripsFixtureRecipient() throws Exception {
    final Map<String, Object> fixture = loadFixture();
    final Map<String, Object> sdkInterop = obj(fixture, "sdk_interop");
    final Map<String, Object> payment = obj(fixture, "payment_token");
    final OfflineNotePaymentToken token =
        OfflineNotePaymentTokenCodec.decodeNorito(
            base64Bytes(string(sdkInterop, "payment_token_norito_base64")));
    final OfflineNoteReceiptAck ack =
        OfflineNoteReceiptAck.fromPaymentToken(
            token,
            string(payment, "recipient_account_id"),
            longValue(obj(fixture, "receipt_ack"), "accepted_at_ms"));

    assertTrue(ack.matchesPaymentToken(token));
    assertEquals(token.getChainId(), ack.getChainId());
    assertEquals(token.getPaymentRequestId(), ack.getPaymentRequestId());
    assertEquals(string(payment, "recipient_account_id"), ack.getRecipientAccountId());

    final OfflineNoteTransferPayload payload =
        OfflineNoteTransferHandoff.receiptAckPayload(ack, OfflineNoteTransferModality.NEARBY);
    assertEquals(OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE, payload.getContentType());
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeReceiptAck(payload).tokenIdHex());
    assertEquals(
        ack.tokenIdHex(),
        OfflineNoteTransferHandoff.decodeNearbyReceiptAck(
            OfflineNoteTransferHandoff.nearbyReceiptAckEnvelopeBytes(ack))
            .tokenIdHex());
  }

  private static OfflineNotePaymentToken streamToken(final List<byte[]> frames) {
    final OfflineNoteTransferStreamReceiver receiver = new OfflineNoteTransferStreamReceiver();
    OfflineNoteTransferStreamResult result = null;
    for (final byte[] frame : frames) {
      result = receiver.ingestFrame(frame);
    }
    assertTrue(result != null && result.isComplete());
    assertTrue(result.getToken() != null);
    return result.getToken();
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadFixture() throws Exception {
    final InputStream stream =
        InstrumentationRegistry.getInstrumentation()
            .getContext()
            .getAssets()
            .open("interop_contract.json");
    try {
      final ByteArrayOutputStream out = new ByteArrayOutputStream();
      final byte[] buffer = new byte[8192];
      int read;
      while ((read = stream.read(buffer)) != -1) {
        out.write(buffer, 0, read);
      }
      return (Map<String, Object>) JsonParser.parse(out.toString("UTF-8"));
    } finally {
      stream.close();
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> obj(final Map<String, Object> map, final String key) {
    final Object value = map.get(key);
    if (!(value instanceof Map)) {
      throw new AssertionError(key + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  private static String string(final Map<String, Object> map, final String key) {
    return (String) map.get(key);
  }

  private static long longValue(final Map<String, Object> map, final String key) {
    return ((Number) map.get(key)).longValue();
  }

  private static byte[] base64Bytes(final String value) {
    return Base64.decode(value, Base64.DEFAULT);
  }
}
