package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import org.junit.Test;

/** Sparse-assembly checks for the Kagemusha NFC protocol. */
public final class KagemushaNfcProtocolTests {
  @Test
  public void bulkWriterRequiresCanonicalMinimumButAllowsASmallerFinalChunk() {
    final byte[] payload = new byte[KagemushaNfcProtocol.SAFE_CHUNK_BYTES + 1];
    Arrays.fill(payload, (byte) 0x5A);
    assertThrows(
        IllegalArgumentException.class,
        () ->
            KagemushaNfcProtocol.writePayloadApdus(
                KagemushaNfcProtocol.PayloadKind.PAYMENT,
                payload,
                KagemushaNfcProtocol.SAFE_CHUNK_BYTES - 1));

    final List<byte[]> commands =
        KagemushaNfcProtocol.writePayloadApdus(
            KagemushaNfcProtocol.PayloadKind.PAYMENT,
            payload,
            KagemushaNfcProtocol.SAFE_CHUNK_BYTES);
    assertEquals(4, commands.size());
    final KagemushaNfcProtocol.Command first =
        KagemushaNfcProtocol.parseCommand(commands.get(1));
    final KagemushaNfcProtocol.Command last =
        KagemushaNfcProtocol.parseCommand(commands.get(2));
    assertEquals(KagemushaNfcProtocol.Type.WRITE_CHUNK, first.type());
    assertEquals(0, first.offset());
    assertEquals(KagemushaNfcProtocol.SAFE_CHUNK_BYTES, first.bytes().length);
    assertEquals(KagemushaNfcProtocol.Type.WRITE_CHUNK, last.type());
    assertEquals(KagemushaNfcProtocol.SAFE_CHUNK_BYTES, last.offset());
    assertArrayEquals(new byte[] {(byte) 0x5A}, last.bytes());
  }

  @Test
  public void responseSliceRejectsOverflowingLength() {
    assertArrayEquals(
        new byte[] {2, 3, (byte) 0x90, 0},
        KagemushaNfcProtocol.response(new byte[] {1, 2, 3}, 1, 2));
    assertThrows(
        IllegalArgumentException.class,
        () -> KagemushaNfcProtocol.response(new byte[4], 4, Integer.MAX_VALUE - 3));
  }

  @Test
  public void payloadAssemblerBuffersOnlyAcceptedSparseBytes() {
    final byte[] nonzeroDigest = new byte[32];
    Arrays.fill(nonzeroDigest, (byte) 0x5A);
    final KagemushaNfcProtocol.PayloadAssembler maximum =
        new KagemushaNfcProtocol.PayloadAssembler(
            KagemushaNfcProtocol.PayloadKind.PAYMENT,
            KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES,
            nonzeroDigest);
    assertEquals(0, maximum.bufferedByteCount());
    assertFalse(maximum.isComplete());
    assertTrue(
        maximum.write(
            KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES - 3, new byte[] {7, 8, 9}));
    assertEquals(3, maximum.bufferedByteCount());
    maximum.clear();
    assertEquals(0, maximum.bufferedByteCount());
    assertArrayEquals(new byte[32], maximum.expectedSha256());

    final byte[] payload = "abcdefgh".getBytes(StandardCharsets.US_ASCII);
    final KagemushaNfcProtocol.PayloadAssembler assembler =
        new KagemushaNfcProtocol.PayloadAssembler(
            KagemushaNfcProtocol.PayloadKind.PAYMENT,
            payload.length,
            KagemushaNfcProtocol.sha256(payload));
    assertTrue(assembler.write(4, "efgh".getBytes(StandardCharsets.US_ASCII)));
    assertEquals(4, assembler.bufferedByteCount());
    assertTrue(assembler.write(2, "cdef".getBytes(StandardCharsets.US_ASCII)));
    assertEquals(6, assembler.bufferedByteCount());
    assertTrue(assembler.write(3, "def".getBytes(StandardCharsets.US_ASCII)));
    assertEquals(6, assembler.bufferedByteCount());
    assertFalse(assembler.write(3, "dXf".getBytes(StandardCharsets.US_ASCII)));
    assertEquals(6, assembler.bufferedByteCount());
    assertTrue(assembler.write(0, "ab".getBytes(StandardCharsets.US_ASCII)));
    assertEquals(payload.length, assembler.bufferedByteCount());
    assertTrue(assembler.isComplete());
    assertArrayEquals(payload, assembler.commit());
  }

  @Test
  public void fragmentBudgetViolationTerminallyClearsAssembler() {
    final byte[] digest = new byte[32];
    Arrays.fill(digest, (byte) 1);
    final KagemushaNfcProtocol.PayloadAssembler assembler =
        new KagemushaNfcProtocol.PayloadAssembler(
            KagemushaNfcProtocol.PayloadKind.PAYMENT, 131, digest);
    int accepted = 0;
    for (int offset = 0; offset < 131; offset += 2) {
      if (assembler.write(offset, new byte[] {(byte) offset})) {
        accepted += 1;
      } else {
        assertEquals(130, offset);
        break;
      }
    }
    assertEquals(65, accepted);
    assertEquals(0, assembler.bufferedByteCount());
    assertFalse(assembler.isComplete());
    assertFalse(assembler.write(1, new byte[] {1}));
    assertThrows(IllegalStateException.class, assembler::commit);
  }

  @Test
  public void completeBadDigestIsTerminalAndCannotBeCommittedAgain() {
    final byte[] payload = "abcdef".getBytes(StandardCharsets.US_ASCII);
    final KagemushaNfcProtocol.PayloadAssembler assembler =
        new KagemushaNfcProtocol.PayloadAssembler(
            KagemushaNfcProtocol.PayloadKind.PAYMENT,
            payload.length,
            KagemushaNfcProtocol.sha256("abcdeg".getBytes(StandardCharsets.US_ASCII)));
    assertTrue(assembler.write(0, payload));
    assertThrows(IllegalStateException.class, assembler::commit);
    assertEquals(0, assembler.bufferedByteCount());
    assertFalse(assembler.write(0, payload));
    assertThrows(IllegalStateException.class, assembler::commit);
  }

  @Test
  public void incompleteCommitIsRetryableAndSuccessConsumesAssembler() {
    final byte[] payload = "abcdefgh".getBytes(StandardCharsets.US_ASCII);
    final KagemushaNfcProtocol.PayloadAssembler assembler =
        new KagemushaNfcProtocol.PayloadAssembler(
            KagemushaNfcProtocol.PayloadKind.PAYMENT,
            payload.length,
            KagemushaNfcProtocol.sha256(payload));
    assertTrue(assembler.write(0, "abcd".getBytes(StandardCharsets.US_ASCII)));
    assertThrows(IllegalStateException.class, assembler::commit);
    assertEquals(4, assembler.bufferedByteCount());
    assertTrue(assembler.write(4, "efgh".getBytes(StandardCharsets.US_ASCII)));
    assertArrayEquals(payload, assembler.commit());
    assertEquals(0, assembler.bufferedByteCount());
    assertFalse(assembler.isComplete());
    assertFalse(assembler.write(0, payload));
    assertThrows(IllegalStateException.class, assembler::commit);
  }
}
