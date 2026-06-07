package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Native record-backed Kagemusha compact payment token prover. */
public final class KagemushaCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
  private static final int NORITO_HEADER_BYTES = 40;
  private static final int NORITO_MAX_HEADER_PADDING_BYTES = 64;
  private static final int NORITO_SUPPORTED_FLAGS_MASK = 0x27;
  private static final int NORITO_FIELD_BITSET_FLAG = 0x20;
  private static final int NORITO_FIELD_BITSET_REQUIRED_FLAGS = 0x06;
  private static final long CRC64_REFLECTED_POLY = 0xC96C5795D7870F42L;
  private static final byte[] NORITO_MAGIC = new byte[] {'N', 'R', 'T', '0'};
  private static final long[] CRC64_TABLE = buildCrc64Table();
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaCompactPaymentTokenProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] proveVerifiedCompactPaymentTokenWithRecords(
      final byte[] recordBundleArchive) {
    final byte[] recordBundle = ownedNativeInput(recordBundleArchive, "recordBundleArchive");
    requireNative();
    final byte[] tokenArchive =
        nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundle);
    return requireNativeOutput(
        tokenArchive, "nativeProveVerifiedCompactPaymentTokenWithRecords");
  }

  static byte[] ownedNativeInput(final byte[] archive, final String archiveName) {
    requireNativeInput(archive, archiveName);
    return Arrays.copyOf(archive, archive.length);
  }

  static void requireNativeInput(final byte[] archive, final String archiveName) {
    if (archive == null || archive.length == 0) {
      throw new IllegalArgumentException(archiveName + " must not be empty");
    }
    if (archive.length > NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalArgumentException(
          archiveName + " must not exceed " + NATIVE_ARCHIVE_MAX_BYTES + " bytes");
    }
    if (!isValidNoritoArchive(archive)) {
      throw new IllegalArgumentException(archiveName + " must be a valid Norito archive");
    }
    if (!hasNonEmptyNoritoPayload(archive)) {
      throw new IllegalArgumentException(
          archiveName + " must contain a non-empty Norito payload");
    }
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        () ->
            expectIllegalArgumentProbe(
                () -> nativeProveVerifiedCompactPaymentTokenWithRecords(new byte[0])));
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary, final NativeSymbolProbe probeSymbol) {
    try {
      loadLibrary.run();
    } catch (final IllegalArgumentException error) {
      return false;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
    try {
      return probeSymbol.run();
    } catch (final IllegalArgumentException error) {
      return false;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  interface NativeProbe {
    void run();
  }

  interface NativeSymbolProbe {
    boolean run();
  }

  static boolean expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    }
  }

  static byte[] requireNativeOutput(final byte[] output, final String label) {
    if (output == null) {
      throw new IllegalStateException(label + " returned no output");
    }
    if (output.length == 0) {
      throw new IllegalStateException(label + " returned empty output");
    }
    if (output.length > NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalStateException(label + " returned oversized output");
    }
    if (!isValidNoritoArchive(output)) {
      throw new IllegalStateException(label + " returned invalid Norito archive");
    }
    if (!hasNonEmptyNoritoPayload(output)) {
      throw new IllegalStateException(label + " returned empty Norito payload");
    }
    return output;
  }

  static boolean isValidNoritoArchive(final byte[] output) {
    if (output == null
        || output.length < NORITO_HEADER_BYTES
        || output.length > NATIVE_ARCHIVE_MAX_BYTES) {
      return false;
    }
    for (int index = 0; index < NORITO_MAGIC.length; index++) {
      if (output[index] != NORITO_MAGIC[index]) {
        return false;
      }
    }
    if (output[4] != 0 || output[5] != 0 || output[22] != 0) {
      return false;
    }
    final int flags = output[39] & 0xFF;
    if ((flags & ~NORITO_SUPPORTED_FLAGS_MASK) != 0) {
      return false;
    }
    if ((flags & NORITO_FIELD_BITSET_FLAG) != 0
        && (flags & NORITO_FIELD_BITSET_REQUIRED_FLAGS) != NORITO_FIELD_BITSET_REQUIRED_FLAGS) {
      return false;
    }
    final long payloadLengthLong = readLongLittleEndian(output, 23);
    if (payloadLengthLong < 0 || payloadLengthLong > Integer.MAX_VALUE - NORITO_HEADER_BYTES) {
      return false;
    }
    final int payloadLength = (int) payloadLengthLong;
    final int minimumLength = NORITO_HEADER_BYTES + payloadLength;
    if (output.length < minimumLength) {
      return false;
    }
    final int paddingLength = output.length - minimumLength;
    if (paddingLength > NORITO_MAX_HEADER_PADDING_BYTES) {
      return false;
    }
    for (int index = NORITO_HEADER_BYTES; index < NORITO_HEADER_BYTES + paddingLength; index++) {
      if (output[index] != 0) {
        return false;
      }
    }
    final int payloadOffset = NORITO_HEADER_BYTES + paddingLength;
    final long expectedCrc = readLongLittleEndian(output, 31);
    return crc64(output, payloadOffset, output.length - payloadOffset) == expectedCrc;
  }

  static boolean hasNonEmptyNoritoPayload(final byte[] output) {
    return isValidNoritoArchive(output) && readLongLittleEndian(output, 23) > 0;
  }

  private static long[] buildCrc64Table() {
    final long[] table = new long[256];
    for (int index = 0; index < table.length; index++) {
      long crc = index;
      for (int bit = 0; bit < 8; bit++) {
        crc = (crc & 1L) != 0L ? (crc >>> 1) ^ CRC64_REFLECTED_POLY : crc >>> 1;
      }
      table[index] = crc;
    }
    return table;
  }

  private static long crc64(final byte[] output, final int offset, final int length) {
    long crc = -1L;
    for (int index = offset; index < offset + length; index++) {
      crc = CRC64_TABLE[((int) crc ^ output[index]) & 0xFF] ^ (crc >>> 8);
    }
    return crc ^ -1L;
  }

  private static long readLongLittleEndian(final byte[] output, final int offset) {
    long value = 0L;
    for (int index = 0; index < 8; index++) {
      value |= ((long) output[offset + index] & 0xFFL) << (8 * index);
    }
    return value;
  }

  private static native byte[] nativeProveVerifiedCompactPaymentTokenWithRecords(
      byte[] recordBundleArchive);
}
