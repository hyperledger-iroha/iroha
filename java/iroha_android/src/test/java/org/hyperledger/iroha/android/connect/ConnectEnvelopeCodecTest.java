package org.hyperledger.iroha.android.connect;

import java.util.Arrays;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class ConnectEnvelopeCodecTest {
  private static final int NORITO_CHECKSUM_OFFSET = 4 + 1 + 1 + 16 + 1 + 8;

  private ConnectEnvelopeCodecTest() {}

  public static void main(final String[] args) throws Exception {
    decodeLiveSignRequestRawFixture();
    encodeAndDecodeSignResultOkEnvelope();
    decodeSignRequestRawEnvelope();
    encryptedRoundTripForSignResultErr();
    lowOrderPeerPublicKeyRejected();
    negativeSequenceRejectedAcrossConnectSurfaces();
    envelopeDecodeRejectsHighBitUint64Sequence();
    frameDecodeRejectsHighBitUint64Sequence();
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

  private static void lowOrderPeerPublicKeyRejected() throws Exception {
    try {
      ConnectCrypto.deriveDirectionKeys(filled(0x01, 32), new byte[32], filled(0x02, 32));
      throw new AssertionError("low-order X25519 peer public key should be rejected");
    } catch (final ConnectProtocolException err) {
      assert err.getMessage().contains("all-zero") : "unexpected error: " + err.getMessage();
    }
  }

  private static void negativeSequenceRejectedAcrossConnectSurfaces() throws Exception {
    final byte[] sessionId = filled(0x02, 32);
    final byte[] key = filled(0x03, 32);

    expectProtocolSequenceFailure(
        () -> ConnectCrypto.nonceFromSequence(-1L),
        "negative nonce sequence should be rejected");
    expectProtocolSequenceFailure(
        () ->
            ConnectCrypto.encryptEnvelope(
                new byte[] {0x01},
                key,
                sessionId,
                ConnectDirection.APP_TO_WALLET,
                -1L),
        "negative encrypt sequence should be rejected");
    expectProtocolSequenceFailure(
        () ->
            ConnectCrypto.decryptCiphertext(
                new byte[] {0x01},
                key,
                sessionId,
                ConnectDirection.APP_TO_WALLET,
                -1L),
        "negative decrypt sequence should be rejected");
    expectProtocolSequenceFailure(
        () -> ConnectEnvelopeCodec.encodeSignResultErrEnvelope(-1L, "ERR", "message"),
        "negative envelope sequence should be rejected");
    expectProtocolSequenceFailure(
        () ->
            ConnectFrameCodec.encodeCiphertextFrame(
                sessionId,
                ConnectDirection.APP_TO_WALLET,
                -1L,
                new byte[] {0x01}),
        "negative frame sequence should be rejected");
  }

  private static void frameDecodeRejectsHighBitUint64Sequence() throws Exception {
    final byte[] frame =
        ConnectFrameCodec.encodeCiphertextFrame(
            filled(0x04, 32),
            ConnectDirection.APP_TO_WALLET,
            0L,
            new byte[] {0x01});
    final byte[] mutated = Arrays.copyOf(frame, frame.length);
    final int sequenceOffset = lengthPrefixedFieldPayloadOffset(mutated, 2);
    for (int i = 0; i < Long.BYTES; i++) {
      mutated[sequenceOffset + i] = (byte) 0xff;
    }

    expectProtocolSequenceFailure(
        () -> ConnectFrameCodec.decode(mutated),
        "high-bit uint64 frame sequence should be rejected");
  }

  private static void envelopeDecodeRejectsHighBitUint64Sequence() throws Exception {
    final byte[] envelope = ConnectEnvelopeCodec.encodeSignResultErrEnvelope(0L, "ERR", "message");
    final byte[] mutated = Arrays.copyOf(envelope, envelope.length);
    final int sequenceOffset = envelopeSequencePayloadOffset(mutated);
    for (int i = 0; i < Long.BYTES; i++) {
      mutated[sequenceOffset + i] = (byte) 0xff;
    }
    rewriteNoritoChecksum(mutated);

    expectProtocolSequenceFailure(
        () -> ConnectEnvelopeCodec.decodeEnvelope(mutated),
        "high-bit uint64 envelope sequence should be rejected");
  }

  private static void expectProtocolSequenceFailure(
      final CheckedRunnable runnable, final String label) throws Exception {
    try {
      runnable.run();
      throw new AssertionError(label);
    } catch (final ConnectProtocolException err) {
      assert messageChainContains(err, "sequence") : "unexpected error: " + err.getMessage();
    }
  }

  private static boolean messageChainContains(final Throwable error, final String text) {
    Throwable current = error;
    while (current != null) {
      if (current.getMessage() != null && current.getMessage().contains(text)) {
        return true;
      }
      current = current.getCause();
    }
    return false;
  }

  private static int lengthPrefixedFieldPayloadOffset(final byte[] frame, final int fieldIndex) {
    int offset = 0;
    for (int i = 0; i < fieldIndex; i++) {
      final long length = readU64Le(frame, offset);
      if (length < 0L || length > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("invalid field length at index " + i + ": " + length);
      }
      offset += Long.BYTES + (int) length;
    }
    final long length = readU64Le(frame, offset);
    if (length != Long.BYTES) {
      throw new IllegalArgumentException("unexpected sequence field length: " + length);
    }
    return offset + Long.BYTES;
  }

  private static int envelopeSequencePayloadOffset(final byte[] envelope) {
    final int fieldLengthOffset = NoritoHeader.HEADER_LENGTH;
    final long length = readU64Le(envelope, fieldLengthOffset);
    if (length != Long.BYTES) {
      throw new IllegalArgumentException("unexpected envelope sequence field length: " + length);
    }
    return fieldLengthOffset + Long.BYTES;
  }

  private static long readU64Le(final byte[] bytes, final int offset) {
    long value = 0L;
    for (int i = 0; i < Long.BYTES; i++) {
      value |= ((long) bytes[offset + i] & 0xffL) << (8 * i);
    }
    return value;
  }

  private static void rewriteNoritoChecksum(final byte[] envelope) {
    final byte[] payload =
        Arrays.copyOfRange(envelope, NoritoHeader.HEADER_LENGTH, envelope.length);
    writeU64Le(envelope, NORITO_CHECKSUM_OFFSET, CRC64.compute(payload));
  }

  private static void writeU64Le(final byte[] bytes, final int offset, final long value) {
    for (int i = 0; i < Long.BYTES; i++) {
      bytes[offset + i] = (byte) ((value >>> (8 * i)) & 0xffL);
    }
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

  private static byte[] filled(final int value, final int length) {
    final byte[] out = new byte[length];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private interface CheckedRunnable {
    void run() throws Exception;
  }
}
