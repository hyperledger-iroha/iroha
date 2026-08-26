// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.governance;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.File;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.junit.Test;

public final class ParliamentTimedOvnWalletV1Tests {
  private static final String AUTHORITY =
      "ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

  @Test
  public void opaqueHandleAndClosedChoiceProduceOnlyFixedPublicRecords() {
    final FakeBackend backend = new FakeBackend(true);
    final ParliamentTimedOvnWalletV1 wallet =
        ParliamentTimedOvnWalletV1.withBackendForTests(backend);
    final ParliamentTimedOvnSeedHandleV1 handle = wallet.createSeedHandle("member-one");

    assertTrue(wallet.isAvailable());
    assertEquals("member-one", handle.alias());
    assertEquals("ParliamentTimedOvnSeedHandleV1(redacted)", handle.toString());
    assertEquals(handle, wallet.seedHandle("member-one"));
    assertArrayEquals(
        filled(ParliamentTimedOvnWalletV1.REGISTRATION_RECORD_BYTES, 0x31),
        wallet.registrationFromProofV1(new byte[] {1}, trustAnchor(), AUTHORITY, handle));
    assertArrayEquals(
        filled(ParliamentTimedOvnWalletV1.BALLOT_RECORD_BYTES, 0x42),
        wallet.ballotFromProofV1(
            new byte[] {2},
            trustAnchor(),
            AUTHORITY,
            handle,
            ParliamentTimedOvnBallotChoiceV1.ABSTAIN));
    assertEquals(ParliamentTimedOvnBallotChoiceV1.ABSTAIN, backend.lastChoice);
    assertTrue(backend.lastProofWasCleared());
    assertTrue(wallet.deleteSeedHandle(handle));
    assertNull(wallet.seedHandle("member-one"));
  }

  @Test
  public void unavailableOrMalformedInputsFailClosed() {
    final ParliamentTimedOvnWalletV1 unavailable =
        ParliamentTimedOvnWalletV1.withBackendForTests(new FakeBackend(false));
    final ParliamentTimedOvnSeedHandleV1 unavailableHandle =
        unavailable.createSeedHandle("member-two");
    assertFalse(unavailable.isAvailable());
    assertThrows(
        IllegalStateException.class,
        () ->
            unavailable.registrationFromProofV1(
                new byte[] {1}, trustAnchor(), AUTHORITY, unavailableHandle));

    final FakeBackend backend = new FakeBackend(true);
    final ParliamentTimedOvnWalletV1 wallet =
        ParliamentTimedOvnWalletV1.withBackendForTests(backend);
    final ParliamentTimedOvnSeedHandleV1 handle = wallet.createSeedHandle("member-three");
    assertThrows(
        IllegalArgumentException.class,
        () -> wallet.registrationFromProofV1(new byte[0], trustAnchor(), AUTHORITY, handle));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            wallet.registrationFromProofV1(
                new byte[] {1}, trustAnchor(), "bad\0authority", handle));
    assertThrows(IllegalArgumentException.class, () -> wallet.createSeedHandle("   "));

    backend.registrationBytes = 1;
    assertThrows(
        IllegalStateException.class,
        () ->
            wallet.registrationFromProofV1(
                new byte[] {1}, trustAnchor(), AUTHORITY, handle));
  }

  @Test
  public void trustAnchorSnapshotsEveryArrayAndHasNoDefaults() {
    final byte[] network = filled(32, 1);
    final byte[] context = filled(32, 3);
    final byte[] ballot = filled(32, 5);
    final ParliamentTimedOvnCastingTrustAnchorV1 anchor =
        new ParliamentTimedOvnCastingTrustAnchorV1(network, 7, context, ballot);
    java.util.Arrays.fill(network, (byte) 9);
    java.util.Arrays.fill(context, (byte) 9);
    java.util.Arrays.fill(ballot, (byte) 9);
    assertArrayEquals(filled(32, 1), anchor.networkId());
    assertArrayEquals(filled(32, 3), anchor.trustedCheckpointContextId());
    assertArrayEquals(filled(32, 5), anchor.expectedBallotAttemptId());
    final byte[] returned = anchor.networkId();
    java.util.Arrays.fill(returned, (byte) 8);
    assertArrayEquals(filled(32, 1), anchor.networkId());
    assertThrows(
        IllegalArgumentException.class,
        () -> new ParliamentTimedOvnCastingTrustAnchorV1(new byte[31], 7, context, ballot));
    assertThrows(
        IllegalArgumentException.class,
        () -> new ParliamentTimedOvnCastingTrustAnchorV1(network, 0, context, ballot));
    assertThrows(
        IllegalArgumentException.class,
        () -> new ParliamentTimedOvnCastingTrustAnchorV1(network, 7, context, new byte[32]));
  }

  @Test
  public void deleteAndRecreateCannotRetargetAStaleJavaHandle() {
    final FakeBackend backend = new FakeBackend(true);
    final ParliamentTimedOvnWalletV1 wallet =
        ParliamentTimedOvnWalletV1.withBackendForTests(backend);
    final ParliamentTimedOvnSeedHandleV1 stale = wallet.createSeedHandle("rotating-member");

    assertTrue(wallet.deleteSeedHandle(stale));
    final ParliamentTimedOvnSeedHandleV1 current = wallet.createSeedHandle("rotating-member");
    assertFalse(stale.equals(current));
    assertFalse(wallet.deleteSeedHandle(stale));
    assertThrows(
        IllegalStateException.class,
        () ->
            wallet.registrationFromProofV1(
                new byte[] {1}, trustAnchor(), AUTHORITY, stale));
    assertEquals(
        ParliamentTimedOvnWalletV1.REGISTRATION_RECORD_BYTES,
        wallet.registrationFromProofV1(new byte[] {1}, trustAnchor(), AUTHORITY, current).length);
  }

  @Test
  public void deleteWaitsForAnInFlightUseOfTheSameJavaHandle() throws Exception {
    final FakeBackend backend = new FakeBackend(true);
    backend.blockRegistration = true;
    final ParliamentTimedOvnWalletV1 wallet =
        ParliamentTimedOvnWalletV1.withBackendForTests(backend);
    final ParliamentTimedOvnSeedHandleV1 handle = wallet.createSeedHandle("concurrent-member");
    final ExecutorService executor = Executors.newFixedThreadPool(2);
    try {
      final Future<byte[]> use =
          executor.submit(
              () ->
                  wallet.registrationFromProofV1(
                      new byte[] {1}, trustAnchor(), AUTHORITY, handle));
      assertTrue(backend.registrationStarted.await(5, TimeUnit.SECONDS));

      final Future<Boolean> delete = executor.submit(() -> wallet.deleteSeedHandle(handle));
      assertTrue(backend.deleteAttempted.await(5, TimeUnit.SECONDS));
      assertFalse(delete.isDone());

      backend.releaseRegistration.countDown();
      assertEquals(
          ParliamentTimedOvnWalletV1.REGISTRATION_RECORD_BYTES,
          use.get(5, TimeUnit.SECONDS).length);
      assertTrue(delete.get(5, TimeUnit.SECONDS));
      assertNull(wallet.seedHandle("concurrent-member"));
    } finally {
      backend.releaseRegistration.countDown();
      executor.shutdownNow();
    }
  }

  @Test
  public void JavaFacadeDelegatesToTheExactAbi23ProofKotlinJniContract() throws Exception {
    assertEquals(23, ParliamentTimedOvnWalletV1.REQUIRED_BRIDGE_ABI_VERSION);
    assertEquals(
        8 * 1024 * 1024, ParliamentTimedOvnWalletV1.MAXIMUM_CASTING_PROOF_RESPONSE_BYTES);
    assertEquals(3_624, ParliamentTimedOvnWalletV1.REGISTRATION_RECORD_BYTES);
    assertEquals(2_858, ParliamentTimedOvnWalletV1.BALLOT_RECORD_BYTES);
    assertEquals(0, ParliamentTimedOvnBallotChoiceV1.AYE.code());
    assertEquals(1, ParliamentTimedOvnBallotChoiceV1.NAY.code());
    assertEquals(2, ParliamentTimedOvnBallotChoiceV1.ABSTAIN.code());

    final Class<?> endpoint =
        Class.forName(
            "org.hyperledger.iroha.sdk.governance.ParliamentTimedOvnNativeEndpointV1");
    final Method abi = endpoint.getDeclaredMethod("nativeBridgeAbiVersion");
    final Method verify =
        endpoint.getDeclaredMethod(
            "nativeVerifyCastingProofV1",
            byte[].class,
            byte[].class,
            long.class,
            byte[].class,
            byte[].class);
    final Method registration =
        endpoint.getDeclaredMethod(
            "nativeRegistrationFromProofV1",
            byte[].class,
            byte[].class,
            long.class,
            byte[].class,
            byte[].class,
            String.class,
            byte[].class);
    final Method ballot =
        endpoint.getDeclaredMethod(
            "nativeBallotFromProofV1",
            byte[].class,
            byte[].class,
            long.class,
            byte[].class,
            byte[].class,
            String.class,
            byte[].class,
            int.class);
    assertEquals(int.class, abi.getReturnType());
    assertEquals(boolean.class, verify.getReturnType());
    assertEquals(byte[].class, registration.getReturnType());
    assertEquals(byte[].class, ballot.getReturnType());
    for (final Method method : new Method[] {abi, verify, registration, ballot}) {
      assertTrue(Modifier.isPrivate(method.getModifiers()));
      assertTrue(Modifier.isStatic(method.getModifiers()));
      assertTrue(Modifier.isNative(method.getModifiers()));
    }

    for (final Method method : ParliamentTimedOvnWalletV1.class.getDeclaredMethods()) {
      assertFalse("Java facade must not add a second JNI corridor", Modifier.isNative(method.getModifiers()));
      final String lowerName = method.getName().toLowerCase(java.util.Locale.ROOT);
      assertFalse(lowerName.contains("rawseed"));
      assertFalse(lowerName.equals("seedbytes"));
      assertFalse(lowerName.equals("importseed"));
    }
    for (final java.lang.reflect.Field field :
        ParliamentTimedOvnSeedHandleV1.class.getDeclaredFields()) {
      assertFalse("opaque handle must not retain seed bytes", field.getType() == byte[].class);
    }
  }

  @Test
  public void RustExportsStayAlignedWithTheSharedKotlinEndpoint() throws Exception {
    final String source =
        new String(
            Files.readAllBytes(
                locateRepositoryFile("crates/connect_norito_bridge/src/platform_jni/part_3.rs")
                    .toPath()),
            StandardCharsets.UTF_8);
    final String[] symbols = {
      "Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeBridgeAbiVersion",
      "Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeVerifyCastingProofV1",
      "Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeRegistrationFromProofV1",
      "Java_org_hyperledger_iroha_sdk_governance_ParliamentTimedOvnNativeEndpointV1_nativeBallotFromProofV1",
    };
    for (final String symbol : symbols) {
      assertEquals(1, countOccurrences(source, "pub unsafe extern \"system\" fn " + symbol + "("));
    }
    for (final String required :
        new String[] {
          "CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint",
          "CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1",
          "CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1",
          "CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1",
          "AUTHORITY_UTF8_MAX_BYTES_V1",
          "TIMED_OVN_REGISTRATION_RECORD_BYTES_V1",
          "TIMED_OVN_BALLOT_RECORD_BYTES_V1",
          "Zeroizing::new",
          "clear_parliament_jni_exception",
          ".filter(|choice| *choice <= 2)",
          "verified_casting_context_from_proof_v1",
          "registration_from_verified_context_v1",
          "ballot_from_verified_context_v1",
        }) {
      assertTrue("missing JNI source contract: " + required, source.contains(required));
    }
    assertFalse(source.contains("nativeRegistrationFromSeedV1"));
    assertFalse(source.contains("nativeBallotFromSeedV1"));
  }

  private static byte[] filled(final int length, final int value) {
    final byte[] bytes = new byte[length];
    java.util.Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static ParliamentTimedOvnCastingTrustAnchorV1 trustAnchor() {
    return new ParliamentTimedOvnCastingTrustAnchorV1(
        filled(32, 1), 7, filled(32, 3), filled(32, 5));
  }

  private static File locateRepositoryFile(final String path) throws Exception {
    File current = new File(".").getCanonicalFile();
    while (current != null) {
      final File candidate = new File(current, path);
      if (candidate.isFile()) {
        return candidate;
      }
      current = current.getParentFile();
    }
    throw new IllegalStateException("cannot locate repository file " + path);
  }

  private static int countOccurrences(final String source, final String needle) {
    int count = 0;
    int offset = 0;
    while ((offset = source.indexOf(needle, offset)) >= 0) {
      count++;
      offset += needle.length();
    }
    return count;
  }

  private static final class FakeBackend implements ParliamentTimedOvnWalletV1.Backend {
    private final boolean available;
    private final Map<String, HandleToken> handles = new HashMap<>();
    private final Map<String, Object> locks = new HashMap<>();
    private int nextGeneration = 1;
    private int registrationBytes = ParliamentTimedOvnWalletV1.REGISTRATION_RECORD_BYTES;
    private ParliamentTimedOvnBallotChoiceV1 lastChoice;
    private byte[] lastProof;
    private boolean blockRegistration;
    private final CountDownLatch registrationStarted = new CountDownLatch(1);
    private final CountDownLatch releaseRegistration = new CountDownLatch(1);
    private final CountDownLatch deleteAttempted = new CountDownLatch(1);

    private FakeBackend(final boolean available) {
      this.available = available;
    }

    @Override
    public boolean isAvailable() {
      return available;
    }

    @Override
    public Object createSeedHandle(final String alias) {
      synchronized (lock(alias)) {
        if (handles.containsKey(alias)) {
          throw new IllegalArgumentException("duplicate alias");
        }
        final HandleToken token = new HandleToken(alias, nextGeneration++);
        handles.put(alias, token);
        return token;
      }
    }

    @Override
    public Object seedHandle(final String alias) {
      synchronized (lock(alias)) {
        return handles.get(alias);
      }
    }

    @Override
    public boolean deleteSeedHandle(final Object handle) {
      final HandleToken token = (HandleToken) handle;
      deleteAttempted.countDown();
      synchronized (lock(token.alias)) {
        if (!token.equals(handles.get(token.alias))) {
          return false;
        }
        handles.remove(token.alias);
        return true;
      }
    }

    @Override
    public byte[] registration(
        final byte[] proofResponse,
        final ParliamentTimedOvnCastingTrustAnchorV1 trustAnchor,
        final String authority,
        final Object handle) {
      final HandleToken token = (HandleToken) handle;
      synchronized (lock(token.alias)) {
        requireCurrent(token);
        registrationStarted.countDown();
        if (blockRegistration) {
          try {
            if (!releaseRegistration.await(5, TimeUnit.SECONDS)) {
              throw new IllegalStateException("registration test release timed out");
            }
          } catch (final InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("registration test interrupted", error);
          }
        }
        rememberProof(proofResponse, trustAnchor, authority);
        return filled(registrationBytes, 0x31);
      }
    }

    @Override
    public byte[] ballot(
        final byte[] proofResponse,
        final ParliamentTimedOvnCastingTrustAnchorV1 trustAnchor,
        final String authority,
        final Object handle,
        final ParliamentTimedOvnBallotChoiceV1 choice) {
      final HandleToken token = (HandleToken) handle;
      synchronized (lock(token.alias)) {
        requireCurrent(token);
        rememberProof(proofResponse, trustAnchor, authority);
        lastChoice = choice;
        return filled(ParliamentTimedOvnWalletV1.BALLOT_RECORD_BYTES, 0x42);
      }
    }

    private void rememberProof(
        final byte[] proofResponse,
        final ParliamentTimedOvnCastingTrustAnchorV1 trustAnchor,
        final String authority) {
      assertTrue(proofResponse.length > 0);
      assertEquals(7L, trustAnchor.trustedCheckpointHeight());
      assertEquals(AUTHORITY, authority);
      lastProof = proofResponse;
    }

    private void requireCurrent(final HandleToken token) {
      if (!token.equals(handles.get(token.alias))) {
        throw new IllegalStateException("stale seed handle");
      }
    }

    private Object lock(final String alias) {
      synchronized (locks) {
        Object lock = locks.get(alias);
        if (lock == null) {
          lock = new Object();
          locks.put(alias, lock);
        }
        return lock;
      }
    }

    private boolean lastProofWasCleared() {
      if (lastProof == null) {
        return false;
      }
      for (final byte value : lastProof) {
        if (value != 0) {
          return false;
        }
      }
      return true;
    }
  }

  private static final class HandleToken {
    private final String alias;
    private final int generation;

    private HandleToken(final String alias, final int generation) {
      this.alias = alias;
      this.generation = generation;
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof HandleToken
          && generation == ((HandleToken) other).generation
          && alias.equals(((HandleToken) other).alias);
    }

    @Override
    public int hashCode() {
      return Objects.hash(alias, generation);
    }
  }
}
