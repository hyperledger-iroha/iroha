// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Canonical one-instruction transaction for the ABI-20 device-attestation path. */
public final class RegisterOfflineDeviceAttestation {

  private final String chainId;
  private final String authority;
  private final DeviceAttestationRegistration registration;
  private final long creationTimeMs;
  private final Long timeToLiveMs;
  private final Integer nonce;
  private final Map<String, JsonValue> metadata;

  public RegisterOfflineDeviceAttestation(
      final String chainId,
      final String authority,
      final DeviceAttestationRegistration registration,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Integer nonce,
      final Map<String, JsonValue> metadata) {
    this.chainId = requireExactText(chainId, "chainId");
    this.authority = requireExactText(authority, "authority");
    this.registration = Objects.requireNonNull(registration, "registration");
    if (creationTimeMs < 0) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
    if (timeToLiveMs != null && timeToLiveMs <= 0) {
      throw new IllegalArgumentException("timeToLiveMs must be positive when present");
    }
    if (nonce != null && nonce <= 0) {
      throw new IllegalArgumentException("nonce must be positive when present");
    }
    if (timeToLiveMs != null) {
      final long validUntil;
      try {
        validUntil = Math.addExact(creationTimeMs, timeToLiveMs);
      } catch (final ArithmeticException ex) {
        throw new IllegalArgumentException("transaction lifetime overflows milliseconds", ex);
      }
      if (validUntil > registration.expiresAtMs()) {
        throw new IllegalArgumentException(
            "transaction lifetime must not outlive the device attestation");
      }
    }
    this.creationTimeMs = creationTimeMs;
    this.timeToLiveMs = timeToLiveMs;
    this.nonce = nonce;
    this.metadata =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(metadata == null ? Collections.emptyMap() : metadata));
    // Reuse the canonical transaction model's I105 and metadata validation at construction time.
    transactionPayload();
  }

  public RegisterOfflineDeviceAttestation(
      final String chainId,
      final String authority,
      final DeviceAttestationRegistration registration,
      final long creationTimeMs) {
    this(chainId, authority, registration, creationTimeMs, null, null, Collections.emptyMap());
  }

  /** Exact native instruction carried by this transaction. */
  public InstructionBox instruction() {
    return OfflineDeviceAttestationCodec.instruction(registration);
  }

  /** Decode and validate one current instruction archive. */
  public static DeviceAttestationRegistration decodeInstructionPayloadCanonical(
      final byte[] archive) {
    return OfflineDeviceAttestationCodec.decodeInstructionPayloadCanonical(archive);
  }

  /** Build a payload containing exactly one registration instruction. */
  public TransactionPayload transactionPayload() {
    return TransactionPayload.builder()
        .setChainId(chainId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(Executable.instructions(List.of(instruction())))
        .setTimeToLiveMs(timeToLiveMs)
        .setNonce(nonce)
        .setMetadata(metadata)
        .build();
  }

  /**
   * Reject a payload that has been changed after construction, including an added instruction.
   */
  public void validateExactPayload(final TransactionPayload payload) {
    final TransactionPayload expected = transactionPayload();
    Objects.requireNonNull(payload, "payload");
    if (!payload.chainId().equals(expected.chainId())
        || !payload.authority().equals(expected.authority())
        || payload.creationTimeMs() != expected.creationTimeMs()
        || !payload.timeToLiveMs().equals(expected.timeToLiveMs())
        || !payload.nonce().equals(expected.nonce())
        || !payload.metadata().equals(expected.metadata())
        || !payload.executable().isInstructions()
        || payload.executable().instructions().size() != 1
        || !payload.executable().instructions().get(0).equals(instruction())) {
      throw new IllegalArgumentException(
          "RegisterOfflineDeviceAttestation requires its exact one-instruction payload");
    }
  }

  /** Encode and sign with the canonical transaction builder. */
  public SignedTransaction encodeAndSign(final TransactionBuilder builder, final Signer signer)
      throws NoritoException, SigningException {
    final TransactionPayload payload = transactionPayload();
    validateExactPayload(payload);
    return Objects.requireNonNull(builder, "builder")
        .encodeAndSign(payload, Objects.requireNonNull(signer, "signer"));
  }

  public DeviceAttestationRegistration registration() {
    return registration;
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
