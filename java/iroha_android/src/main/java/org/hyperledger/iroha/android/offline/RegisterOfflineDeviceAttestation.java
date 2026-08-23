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
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Canonical one-instruction transaction for the ABI-21 device-attestation path. */
public final class RegisterOfflineDeviceAttestation {

  private static final long DEFAULT_TRANSACTION_TTL_MS = 100_000L;

  private final NetworkId networkId;
  private final String authority;
  private final DeviceAttestationRegistration registration;
  private final long creationTimeMs;
  private final Long timeToLiveMs;
  private final Long nonce;
  private final FeePaymentIntent feePayment;
  private final Map<String, JsonValue> metadata;

  public RegisterOfflineDeviceAttestation(
      final NetworkId networkId,
      final String authority,
      final DeviceAttestationRegistration registration,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Long nonce,
      final FeePaymentIntent feePayment,
      final Map<String, JsonValue> metadata) {
    this.networkId = Objects.requireNonNull(networkId, "networkId");
    this.authority = requireExactText(authority, "authority");
    this.registration = Objects.requireNonNull(registration, "registration");
    if (creationTimeMs < 0) {
      throw new IllegalArgumentException("creationTimeMs must be non-negative");
    }
    final long effectiveTimeToLiveMs =
        timeToLiveMs == null ? DEFAULT_TRANSACTION_TTL_MS : timeToLiveMs;
    if (effectiveTimeToLiveMs <= 0) {
      throw new IllegalArgumentException("timeToLiveMs must be positive when present");
    }
    final long validUntil;
    try {
      validUntil = Math.addExact(creationTimeMs, effectiveTimeToLiveMs);
    } catch (final ArithmeticException ex) {
      throw new IllegalArgumentException("transaction lifetime overflows milliseconds", ex);
    }
    if (validUntil > registration.expiresAtMs()) {
      throw new IllegalArgumentException(
          "transaction lifetime must not outlive the device attestation");
    }
    this.creationTimeMs = creationTimeMs;
    this.timeToLiveMs = effectiveTimeToLiveMs;
    this.nonce = nonce;
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    this.metadata =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(metadata == null ? Collections.emptyMap() : metadata));
    // Reuse the canonical transaction model's I105 and metadata validation at construction time.
    transactionPayload();
  }

  /** Exact native instruction carried by this transaction. */
  public InstructionBox instruction() {
    return OfflineDeviceAttestationCodec.instruction(registration);
  }

  /** Decode and validate one current instruction archive. */
  public static DeviceAttestationRegistration decodeInstructionPayloadCanonical(
      final byte[] archive, final int chainDiscriminant) {
    return OfflineDeviceAttestationCodec.decodeInstructionPayloadCanonical(
        archive, chainDiscriminant);
  }

  /** Build a payload containing exactly one registration instruction. */
  public TransactionPayload transactionPayload() {
    return TransactionPayload.builder()
        .setNetworkId(networkId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(Executable.instructions(Collections.singletonList(instruction())))
        .setTimeToLiveMs(timeToLiveMs)
        .setNonce(nonce)
        .setFeePayment(feePayment)
        .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
        .setMetadata(metadata)
        .build();
  }

  /**
   * Reject a payload that has been changed after construction, including an added instruction.
   */
  public void validateExactPayload(final TransactionPayload payload) {
    final TransactionPayload expected = transactionPayload();
    Objects.requireNonNull(payload, "payload");
    if (!payload.networkId().equals(expected.networkId())
        || !payload.authority().equals(expected.authority())
        || payload.creationTimeMs() != expected.creationTimeMs()
        || !payload.timeToLiveMs().equals(expected.timeToLiveMs())
        || !payload.nonce().equals(expected.nonce())
        || !payload.feePayment().equals(expected.feePayment())
        || payload.admissionIntent() != expected.admissionIntent()
        || !payload.metadata().equals(expected.metadata())
        || !payload.attachments().equals(expected.attachments())
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
