package org.hyperledger.iroha.android.connect;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Single Connect queue journal entry encoded as {@code ConnectJournalRecordV1}. */
public final class ConnectJournalRecord {
  static final String SCHEMA_NAME = "ConnectJournalRecordV1";
  private static final byte[] SCHEMA_HASH = SchemaHash.hash16(SCHEMA_NAME);
  private static final int PAYLOAD_FIXED_LENGTH = 1 + 8 + 8 + 8 + 4 + 32;
  private static final TypeAdapter<ConnectJournalRecord> ADAPTER = new RecordAdapter();

  private final ConnectDirection direction;
  private final long sequence;
  private final byte[] payloadHash;
  private final byte[] ciphertext;
  private final long receivedAtMs;
  private final long expiresAtMs;

  public ConnectJournalRecord(
      final ConnectDirection direction,
      final long sequence,
      final byte[] payloadHash,
      final byte[] ciphertext,
      final long receivedAtMs,
      final long expiresAtMs)
      throws ConnectJournalException {
    this.direction = Objects.requireNonNull(direction, "direction");
    this.sequence = sequence;
    this.payloadHash = normalizeHash(payloadHash);
    this.ciphertext = Objects.requireNonNull(ciphertext, "ciphertext").clone();
    if (this.ciphertext.length > 0xFFFF_FFFFL) {
      throw new ConnectJournalException("ciphertext too large for journal entry");
    }
    this.receivedAtMs = receivedAtMs;
    this.expiresAtMs = expiresAtMs;
  }

  public ConnectDirection direction() {
    return direction;
  }

  public long sequence() {
    return sequence;
  }

  public byte[] payloadHash() {
    return payloadHash.clone();
  }

  public byte[] ciphertext() {
    return ciphertext.clone();
  }

  public long receivedAtMs() {
    return receivedAtMs;
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  public int encodedLength() {
    return NoritoHeader.HEADER_LENGTH + PAYLOAD_FIXED_LENGTH + ciphertext.length;
  }

  public byte[] encode() throws ConnectJournalException {
    try {
      return NoritoCodec.encode(this, SCHEMA_NAME, ADAPTER);
    } catch (RuntimeException ex) {
      throw new ConnectJournalException("failed to encode journal record", ex);
    }
  }

  public static DecodeResult decode(final byte[] data, final int offset)
      throws ConnectJournalException {
    Objects.requireNonNull(data, "data");
    if (offset < 0 || offset >= data.length) {
      throw new ConnectJournalException("offset outside journal bounds");
    }
    final int remaining = data.length - offset;
    if (remaining < NoritoHeader.HEADER_LENGTH) {
      throw new ConnectJournalException("insufficient bytes for Norito header");
    }
    final ByteBuffer header =
        ByteBuffer.wrap(data, offset, NoritoHeader.HEADER_LENGTH).order(ByteOrder.LITTLE_ENDIAN);
    final byte[] magic = new byte[4];
    header.get(magic);
    if (!Arrays.equals(magic, NoritoHeader.MAGIC)) {
      throw new ConnectJournalException("invalid Norito magic");
    }
    final int major = header.get() & 0xFF;
    final int minor = header.get() & 0xFF;
    if (major != NoritoHeader.MAJOR_VERSION || minor != NoritoHeader.MINOR_VERSION) {
      throw new ConnectJournalException("unsupported Norito version: " + major + "." + minor);
    }
    final byte[] schemaHash = new byte[16];
    header.get(schemaHash);
    if (!Arrays.equals(schemaHash, SCHEMA_HASH)) {
      throw new ConnectJournalException("schema hash mismatch in journal entry");
    }
    final int compression = header.get() & 0xFF;
    if (compression != NoritoHeader.COMPRESSION_NONE) {
      throw new ConnectJournalException("compressed journal entries are not supported");
    }
    final long payloadLength = header.getLong() & 0xFFFF_FFFF_FFFF_FFFFL;
    if (payloadLength > Integer.MAX_VALUE) {
      throw new ConnectJournalException("payload too large for journal entry");
    }
    final long checksum = header.getLong();
    header.get(); // flags
    final int intPayloadLength = (int) payloadLength;
    final int paddingLength =
        detectPaddingLength(data, offset, intPayloadLength, checksum);
    final int recordLength = NoritoHeader.HEADER_LENGTH + paddingLength + intPayloadLength;
    if (recordLength < 0 || recordLength > remaining) {
      throw new ConnectJournalException("record length exceeds file bounds");
    }
    final byte[] slice = Arrays.copyOfRange(data, offset, offset + recordLength);
    try {
      final ConnectJournalRecord record = NoritoCodec.decode(slice, ADAPTER, SCHEMA_NAME);
      return new DecodeResult(record, recordLength);
    } catch (RuntimeException ex) {
      throw new ConnectJournalException("failed to decode journal entry", ex);
    }
  }

  public static byte[] computePayloadHash(final byte[] ciphertext) {
    Objects.requireNonNull(ciphertext, "ciphertext");
    return Blake2b.digest(ciphertext, 32);
  }

  private static byte[] normalizeHash(final byte[] hash) throws ConnectJournalException {
    if (hash == null || hash.length != 32) {
      throw new ConnectJournalException("payload hash must contain 32 bytes");
    }
    return hash.clone();
  }

  private static int detectPaddingLength(
      final byte[] data,
      final int offset,
      final int payloadLength,
      final long checksum)
      throws ConnectJournalException {
    if (payloadLength < 0) {
      throw new ConnectJournalException("payload too large for journal entry");
    }
    final int headerEnd = offset + NoritoHeader.HEADER_LENGTH;
    final int maxAvailable = data.length - headerEnd - payloadLength;
    if (maxAvailable < 0) {
      throw new ConnectJournalException("record length exceeds file bounds");
    }
    final int maxPadding = Math.min(NoritoHeader.MAX_HEADER_PADDING, maxAvailable);
    for (int padding = 0; padding <= maxPadding; padding++) {
      boolean paddingOk = true;
      for (int i = 0; i < padding; i++) {
        if (data[headerEnd + i] != 0) {
          paddingOk = false;
          break;
        }
      }
      if (!paddingOk) {
        continue;
      }
      int payloadStart = headerEnd + padding;
      int payloadEnd = payloadStart + payloadLength;
      if (payloadEnd > data.length) {
        break;
      }
      byte[] payload = Arrays.copyOfRange(data, payloadStart, payloadEnd);
      if (CRC64.compute(payload) == checksum) {
        return padding;
      }
    }
    throw new ConnectJournalException("failed to locate payload bytes for journal entry");
  }

  public static final class DecodeResult {
    private final ConnectJournalRecord record;
    private final int bytesConsumed;

    DecodeResult(final ConnectJournalRecord record, final int bytesConsumed) {
      this.record = record;
      this.bytesConsumed = bytesConsumed;
    }

    public ConnectJournalRecord record() {
      return record;
    }

    public int bytesConsumed() {
      return bytesConsumed;
    }
  }

  private static final class RecordAdapter implements TypeAdapter<ConnectJournalRecord> {
    private static final TypeAdapter<Long> UINT8 = NoritoAdapters.uint(8);
    private static final TypeAdapter<Long> UINT32 = NoritoAdapters.uint(32);
    private static final TypeAdapter<Long> UINT64 = NoritoAdapters.uint(64);
    private static final TypeAdapter<byte[]> HASH = NoritoAdapters.fixedBytes(32);

    @Override
    public void encode(final NoritoEncoder encoder, final ConnectJournalRecord value) {
      UINT8.encode(encoder, (long) value.direction.tag());
      UINT64.encode(encoder, value.sequence);
      UINT64.encode(encoder, value.receivedAtMs);
      UINT64.encode(encoder, value.expiresAtMs);
      UINT32.encode(encoder, Integer.toUnsignedLong(value.ciphertext.length));
      HASH.encode(encoder, value.payloadHash);
      encoder.writeBytes(value.ciphertext);
    }

    @Override
    public ConnectJournalRecord decode(final NoritoDecoder decoder) {
      final int directionTag = (int) (long) UINT8.decode(decoder);
      final ConnectDirection direction = ConnectDirection.fromTag(directionTag);
      final long sequence = UINT64.decode(decoder);
      final long receivedAt = UINT64.decode(decoder);
      final long expiresAt = UINT64.decode(decoder);
      final long ciphertextLength = UINT32.decode(decoder);
      if (ciphertextLength < 0 || ciphertextLength > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("ciphertext length too large");
      }
      final byte[] hash = HASH.decode(decoder);
      final byte[] ciphertext = decoder.readBytes((int) ciphertextLength);
      try {
        return new ConnectJournalRecord(direction, sequence, hash, ciphertext, receivedAt, expiresAt);
      } catch (final ConnectJournalException ex) {
        throw new IllegalArgumentException("invalid journal record payload", ex);
      }
    }
  }
}
