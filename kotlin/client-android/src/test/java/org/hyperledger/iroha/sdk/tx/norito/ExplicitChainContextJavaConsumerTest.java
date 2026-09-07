package org.hyperledger.iroha.sdk.tx.norito;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Arrays;
import java.util.Collections;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.hyperledger.iroha.sdk.IrohaKeyManager;
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm;
import org.hyperledger.iroha.sdk.crypto.keystore.KeySecurityPreference;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.core.model.Executable;
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent;
import org.hyperledger.iroha.sdk.core.model.JsonValue;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.sdk.core.model.TransactionPayload;
import org.hyperledger.iroha.sdk.norito.NoritoCodec;
import org.hyperledger.iroha.sdk.norito.NoritoDecoder;
import org.hyperledger.iroha.sdk.norito.NoritoEncoder;
import org.hyperledger.iroha.sdk.sccp.SccpV1;
import org.hyperledger.iroha.sdk.tx.SignedTransaction;
import org.junit.jupiter.api.Test;

/** Java Android consumers retain explicit, isolated I105 codec contexts and canonical envelopes. */
class ExplicitChainContextJavaConsumerTest {
  private static final int TAIRA = SccpV1.TAIRA_I105_DISCRIMINANT_V1;
  private static final int OTHER = AccountAddress.DEFAULT_I105_DISCRIMINANT;
  private static final IrohaKeyManager ACCOUNTS = IrohaKeyManager.withSoftwareProvider(SigningAlgorithm.ED25519);

  @Test
  public void adaptersRequireBoundedExplicitContextAndRejectMismatchedPrefixes() throws Exception {
    expectIllegalArgument(() -> new NoritoJavaCodecAdapter(-1));
    expectIllegalArgument(() -> new NoritoJavaCodecAdapter(0x1_0000));

    final String tairaAuthority = account(0x41, TAIRA);
    final String otherAuthority = account(0x41, OTHER);
    final TransactionPayload tairaPayload = payload(tairaAuthority);
    final TransactionPayload otherPayload = payload(otherAuthority);
    final NoritoJavaCodecAdapter tairaAdapter = new NoritoJavaCodecAdapter(TAIRA);
    final NoritoJavaCodecAdapter otherAdapter = new NoritoJavaCodecAdapter(OTHER);

    final byte[] tairaBytes = tairaAdapter.encodeTransaction(tairaPayload);
    final byte[] otherBytes = otherAdapter.encodeTransaction(otherPayload);
    assertArrayEquals(
        tairaBytes, otherBytes, "chain context only changes I105 projection");
    assertEquals(tairaAuthority, tairaAdapter.decodeTransaction(tairaBytes).getAuthority());
    assertEquals(otherAuthority, otherAdapter.decodeTransaction(tairaBytes).getAuthority());
    assertNotEquals(tairaAuthority, otherAdapter.decodeTransaction(tairaBytes).getAuthority());
    expectNoritoFailure(() -> otherAdapter.encodeTransaction(tairaPayload));
    expectNoritoFailure(() -> tairaAdapter.encodeTransaction(otherPayload));
  }

  @Test
  public void concurrentAdaptersDoNotLeakChainContext() throws Exception {
    final NoritoJavaCodecAdapter tairaAdapter = new NoritoJavaCodecAdapter(TAIRA);
    final NoritoJavaCodecAdapter otherAdapter = new NoritoJavaCodecAdapter(OTHER);
    final TransactionPayload tairaPayload = payload(account(0x42, TAIRA));
    final TransactionPayload otherPayload = payload(account(0x43, OTHER));
    final CountDownLatch start = new CountDownLatch(1);
    final ExecutorService executor = Executors.newFixedThreadPool(2);
    try {
      final Future<?> tairaFuture =
          executor.submit(
              () -> {
                await(start);
                for (int iteration = 0; iteration < 250; iteration++) {
                  assertEquals(
                      tairaPayload.getAuthority(),
                      tairaAdapter
                          .decodeTransaction(tairaAdapter.encodeTransaction(tairaPayload))
                          .getAuthority());
                }
                return null;
              });
      final Future<?> otherFuture =
          executor.submit(
              () -> {
                await(start);
                for (int iteration = 0; iteration < 250; iteration++) {
                  assertEquals(
                      otherPayload.getAuthority(),
                      otherAdapter
                          .decodeTransaction(otherAdapter.encodeTransaction(otherPayload))
                          .getAuthority());
                }
                return null;
              });
      start.countDown();
      tairaFuture.get(30, TimeUnit.SECONDS);
      otherFuture.get(30, TimeUnit.SECONDS);
    } finally {
      executor.shutdownNow();
    }
  }

  @Test
  public void signedEnvelopesPreserveCanonicalPayloadAndRejectAllNoncanonicalInnerForms()
      throws Exception {
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter(TAIRA);
    final java.util.Map<String, JsonValue> metadata = new java.util.LinkedHashMap<>();
    metadata.put("b", JsonValue.string("two"));
    metadata.put("a", JsonValue.string("one"));
    final TransactionPayload payload = payload(account(0x46, TAIRA), metadata);
    final byte[] canonicalPayload = adapter.encodeTransaction(payload);
    final byte[] noncanonicalPayload = swapMetadataEntries(canonicalPayload);
    final byte[] trailingPayload =
        Arrays.copyOf(canonicalPayload, canonicalPayload.length + 1);
    final byte[] malformedPayload = new byte[] {0x01, 0x02, 0x03};

    assertFalse(Arrays.equals(canonicalPayload, noncanonicalPayload));
    assertEquals(
        payload.getAuthority(), adapter.decodeTransaction(noncanonicalPayload).getAuthority());

    final SignedTransaction canonicalSigned = signed(canonicalPayload, adapter.schemaName());
    final byte[] canonicalEnvelope = SignedTransactionEncoder.encode(canonicalSigned);
    assertArrayEquals(
        canonicalPayload,
        SignedTransactionEncoder.decode(canonicalEnvelope).encodedPayload());

    for (final byte[] rejected :
        new byte[][] {malformedPayload, trailingPayload, noncanonicalPayload}) {
      expectNoritoFailure(
          () -> SignedTransactionEncoder.encode(signed(rejected, adapter.schemaName())));
      expectNoritoFailure(
          () ->
              SignedTransactionEncoder.decode(
                  replaceSizedField(canonicalEnvelope, 1, rejected)));
    }
  }

  private static TransactionPayload payload(String authority) {
    return payload(authority, Collections.emptyMap());
  }

  private static TransactionPayload payload(String authority, java.util.Map<String, JsonValue> metadata) {
    return new TransactionPayload(
        NetworkId.parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"),
        authority, 1_735_369_000_000L, Executable.ivm(new byte[] {1}), 100_000L, null,
        FeePaymentIntent.authority(Collections.emptyList(), 1L),
        TransactionAdmissionIntent.ORDINARY, metadata, null);
  }

  private static String account(int seed, int chain) throws Exception {
    // One cached key per fixture alias makes repeated projections use the same identity.
    byte[] spki = ACCOUNTS.generateOrLoad("chain-fixture-" + seed, KeySecurityPreference.SOFTWARE_ONLY).getPublic().getEncoded();
    assertEquals(44, spki.length, "Ed25519 SubjectPublicKeyInfo is 12-byte prefix plus 32-byte key");
    byte[] publicKey = Arrays.copyOfRange(spki, 12, spki.length);
    return AccountAddress.fromAccount(publicKey, "ed25519").toI105(chain);
  }

  private static SignedTransaction signed(final byte[] payload, final String schemaName) {
    return new SignedTransaction(payload, fill(0x55, 64), new byte[0], schemaName);
  }

  private static byte[] swapMetadataEntries(final byte[] canonicalPayload) {
    final byte[][] fields = decodeSizedFields(canonicalPayload, 10);
    final NoritoDecoder metadata =
        new NoritoDecoder(fields[8], NoritoCodec.DEFAULT_FLAGS);
    assertEquals(2L, metadata.readLength(false));
    final byte[] first = readSizedField(metadata);
    final byte[] second = readSizedField(metadata);
    assertEquals(0, metadata.remaining());

    final NoritoEncoder swapped = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    swapped.writeLength(2, false);
    writeSizedField(swapped, second);
    writeSizedField(swapped, first);
    fields[8] = swapped.toByteArray();
    return encodeSizedFields(fields);
  }

  private static byte[] replaceSizedField(
      final byte[] encoded, final int fieldIndex, final byte[] replacement) {
    final byte[][] fields = decodeSizedFields(encoded, 3);
    fields[fieldIndex] = replacement.clone();
    return encodeSizedFields(fields);
  }

  private static byte[][] decodeSizedFields(final byte[] encoded, final int count) {
    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS);
    final byte[][] fields = new byte[count][];
    for (int index = 0; index < count; index++) {
      fields[index] = readSizedField(decoder);
    }
    assertEquals(0, decoder.remaining(), "unexpected trailing bytes");
    return fields;
  }

  private static byte[] readSizedField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(true);
    return decoder.readBytes(Math.toIntExact(length));
  }

  private static byte[] encodeSizedFields(final byte[][] fields) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoCodec.DEFAULT_FLAGS);
    for (final byte[] field : fields) {
      writeSizedField(encoder, field);
    }
    return encoder.toByteArray();
  }

  private static void writeSizedField(final NoritoEncoder encoder, final byte[] field) {
    encoder.writeLength(field.length, true);
    encoder.writeBytes(field);
  }

  private static byte[] fill(final int value, final int length) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static void await(final CountDownLatch latch) {
    try {
      latch.await();
    } catch (final InterruptedException interrupted) {
      Thread.currentThread().interrupt();
      throw new AssertionError("interrupted while waiting for concurrent chain test", interrupted);
    }
  }

  private static void expectIllegalArgument(final ThrowingRunnable operation) {
    try {
      operation.run();
      fail("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    } catch (final Exception unexpected) {
      throw new AssertionError("unexpected failure type", unexpected);
    }
  }

  private static void expectNoritoFailure(final ThrowingRunnable operation) {
    try {
      operation.run();
      fail("expected NoritoException");
    } catch (final NoritoException expected) {
      // Expected.
    } catch (final Exception unexpected) {
      throw new AssertionError("unexpected failure type", unexpected);
    }
  }

  @FunctionalInterface
  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
