package org.hyperledger.iroha.android.connect;

import java.util.Arrays;

public final class ConnectEnvelopeCodecTest {

  private ConnectEnvelopeCodecTest() {}

  public static void main(final String[] args) throws Exception {
    decodeLiveSignRequestRawFixture();
    encodeAndDecodeSignResultOkEnvelope();
    decodeSignRequestRawEnvelope();
    encryptedRoundTripForSignResultErr();
    System.out.println("[IrohaAndroid] ConnectEnvelopeCodecTest passed.");
  }

  private static void decodeLiveSignRequestRawFixture() throws Exception {
    final String hex =
        "4e5254300000f35017c774558f19f35017c774558f19006c00000000000000a7d18c206f893dcf00"
            + "0800000000000000020000000000000054000000000000000100000018000000000000001000000000000000"
            + "46495f50524f504f53414c5f5349474e28000000000000002000000000000000"
            + "e08e634b9557a3e16c52fb9a45662e0ad0c4f79ba8a6e4a0a9b46165a8a7f293";
    final byte[] framed = decodeHex(hex);
    final ConnectEnvelopeCodec.DecodedEnvelope decoded = ConnectEnvelopeCodec.decodeEnvelope(framed);

    assert decoded.sequence() == 2L : "live fixture sequence mismatch";
    assert decoded.payload().kind() == ConnectEnvelopeCodec.PayloadKind.SIGN_REQUEST_RAW
        : "live fixture payload kind mismatch";
    final ConnectEnvelopeCodec.SignRequestRawPayload payload =
        (ConnectEnvelopeCodec.SignRequestRawPayload) decoded.payload();
    assert "FI_PROPOSAL_SIGN".equals(payload.domainTag()) : "live fixture domain tag mismatch";
    assert payload.bytes().length == 32 : "live fixture sign bytes length mismatch";
  }

  private static void encodeAndDecodeSignResultOkEnvelope() throws Exception {
    final byte[] signature = new byte[64];
    for (int i = 0; i < signature.length; i++) {
      signature[i] = (byte) i;
    }

    final byte[] encoded =
        ConnectEnvelopeCodec.encodeSignResultOkEnvelope(7L, signature, "ed25519");
    final ConnectEnvelopeCodec.DecodedEnvelope decoded = ConnectEnvelopeCodec.decodeEnvelope(encoded);

    assert decoded.sequence() == 7L : "sequence mismatch";
    assert decoded.payload().kind() == ConnectEnvelopeCodec.PayloadKind.SIGN_RESULT_OK
        : "payload kind mismatch";

    final ConnectEnvelopeCodec.SignResultOkPayload payload =
        (ConnectEnvelopeCodec.SignResultOkPayload) decoded.payload();
    assert "ed25519".equals(payload.algorithm()) : "algorithm mismatch";
    assert Arrays.equals(signature, payload.signature()) : "signature mismatch";
  }

  private static void decodeSignRequestRawEnvelope() throws Exception {
    final byte[] bytes = new byte[] {0x01, 0x22, (byte) 0xFE};
    final byte[] encoded =
        ConnectEnvelopeCodec.encodeSignRequestRawEnvelope(42L, "iroha-connect/v1/test", bytes);
    final ConnectEnvelopeCodec.DecodedEnvelope decoded = ConnectEnvelopeCodec.decodeEnvelope(encoded);

    assert decoded.sequence() == 42L : "sequence mismatch";
    assert decoded.payload().kind() == ConnectEnvelopeCodec.PayloadKind.SIGN_REQUEST_RAW
        : "payload kind mismatch";

    final ConnectEnvelopeCodec.SignRequestRawPayload payload =
        (ConnectEnvelopeCodec.SignRequestRawPayload) decoded.payload();
    assert "iroha-connect/v1/test".equals(payload.domainTag()) : "domain tag mismatch";
    assert Arrays.equals(bytes, payload.bytes()) : "request bytes mismatch";
  }

  private static void encryptedRoundTripForSignResultErr() throws Exception {
    final byte[] sid = new byte[32];
    final byte[] key = new byte[32];
    for (int i = 0; i < sid.length; i++) {
      sid[i] = (byte) (0xA0 + i);
      key[i] = (byte) (0x11 + i);
    }

    final byte[] envelope =
        ConnectEnvelopeCodec.encodeSignResultErrEnvelope(2L, "USER_DENIED", "Rejected by test");
    final byte[] ciphertext =
        ConnectCrypto.encryptEnvelope(envelope, key, sid, ConnectDirection.WALLET_TO_APP, 2L);
    final byte[] frame =
        ConnectFrameCodec.encodeCiphertextFrame(sid, ConnectDirection.WALLET_TO_APP, 2L, ciphertext);

    final ConnectFrameCodec.DecodedFrame decodedFrame = ConnectFrameCodec.decode(frame);
    assert decodedFrame.type() == ConnectFrameCodec.FrameType.CIPHERTEXT : "frame type mismatch";

    final byte[] plaintext =
        ConnectCrypto.decryptCiphertext(
            decodedFrame.ciphertext().aead(), key, sid, ConnectDirection.WALLET_TO_APP, 2L);
    final ConnectEnvelopeCodec.DecodedEnvelope decodedEnvelope =
        ConnectEnvelopeCodec.decodeEnvelope(plaintext);

    assert decodedEnvelope.payload().kind() == ConnectEnvelopeCodec.PayloadKind.SIGN_RESULT_ERR
        : "payload kind mismatch";
    final ConnectEnvelopeCodec.SignResultErrPayload payload =
        (ConnectEnvelopeCodec.SignResultErrPayload) decodedEnvelope.payload();
    assert "USER_DENIED".equals(payload.code()) : "error code mismatch";
    assert "Rejected by test".equals(payload.message()) : "error message mismatch";
  }

  private static byte[] decodeHex(final String hex) {
    if ((hex.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[hex.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int index = i * 2;
      out[i] = (byte) Integer.parseInt(hex.substring(index, index + 2), 16);
    }
    return out;
  }
}
