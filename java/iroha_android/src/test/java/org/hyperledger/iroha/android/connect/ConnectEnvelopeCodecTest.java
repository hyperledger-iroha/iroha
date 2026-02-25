package org.hyperledger.iroha.android.connect;

import java.util.Arrays;

public final class ConnectEnvelopeCodecTest {

  private ConnectEnvelopeCodecTest() {}

  public static void main(final String[] args) throws Exception {
    encodeAndDecodeSignResultOkEnvelope();
    decodeSignRequestRawEnvelope();
    encryptedRoundTripForSignResultErr();
    System.out.println("[IrohaAndroid] ConnectEnvelopeCodecTest passed.");
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
}
