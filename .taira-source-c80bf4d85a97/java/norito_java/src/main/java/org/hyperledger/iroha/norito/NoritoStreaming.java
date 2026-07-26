// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.Optional;
import java.util.function.Function;

/** Norito streaming helpers mirroring the Rust codec surface. */
public final class NoritoStreaming {
  private NoritoStreaming() {}

  public static final int HASH_LEN = 32;
  public static final int SIGNATURE_LEN = 64;

  private static final TypeAdapter<Long> UINT8 = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> UINT16 = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32 = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT64 = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> SINT16 = NoritoAdapters.sint(16);
  private static final TypeAdapter<Long> SINT32 = NoritoAdapters.sint(32);
  private static final TypeAdapter<Boolean> BOOL = NoritoAdapters.boolAdapter();
  private static final TypeAdapter<byte[]> HASH = NoritoAdapters.fixedBytes(HASH_LEN);
  private static final TypeAdapter<byte[]> SIGNATURE = NoritoAdapters.fixedBytes(SIGNATURE_LEN);
  private static final TypeAdapter<byte[]> BYTES = NoritoAdapters.bytesAdapter();
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<List<String>> STRING_LIST = NoritoAdapters.sequence(STRING);
  private static final TypeAdapter<List<Long>> INT16_LIST = NoritoAdapters.sequence(SINT16);
  private static final TypeAdapter<List<Long>> INT32_LIST = NoritoAdapters.sequence(SINT32);
  private static final TypeAdapter<List<Long>> UINT32_LIST = NoritoAdapters.sequence(UINT32);
  private static final BigInteger UINT128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);

  private static byte[] cloneBytes(byte[] value) {
    return value.clone();
  }

  private static Optional<byte[]> cloneOptionalBytes(Optional<byte[]> value) {
    return value.map(NoritoStreaming::cloneBytes);
  }

  private static boolean bytesEquals(byte[] a, byte[] b) {
    return Arrays.equals(a, b);
  }

  private static int bytesHash(byte[] value) {
    return Arrays.hashCode(value);
  }

  private static boolean optionalBytesEquals(Optional<byte[]> a, Optional<byte[]> b) {
    if (a.isEmpty() && b.isEmpty()) {
      return true;
    }
    if (a.isPresent() != b.isPresent()) {
      return false;
    }
    return Arrays.equals(a.orElseThrow(), b.orElseThrow());
  }

  private static int optionalBytesHash(Optional<byte[]> value) {
    return value.map(NoritoStreaming::bytesHash).orElse(0);
  }

  private static byte[] toLittleEndian(BigInteger value, int length, BigInteger max) {
    if (value.signum() < 0 || value.compareTo(max) > 0) {
      throw new IllegalArgumentException("value out of range for unsigned " + (length * 8) + "-bit integer");
    }
    byte[] be = value.toByteArray();
    int offset = 0;
    int beLen = be.length;
    if (beLen > length) {
      if (be[0] == 0 && beLen == length + 1) {
        offset = 1;
        beLen -= 1;
      } else {
        throw new IllegalArgumentException("value does not fit in " + (length * 8) + " bits");
      }
    }
    byte[] le = new byte[length];
    for (int i = 0; i < beLen; i++) {
      le[i] = be[offset + beLen - 1 - i];
    }
    return le;
  }

  private static BigInteger fromLittleEndian(byte[] data) {
    byte[] be = new byte[data.length];
    for (int i = 0; i < data.length; i++) {
      be[be.length - 1 - i] = data[i];
    }
    return new BigInteger(1, be);
  }

  public record ProfileId(int value) {
    public ProfileId {
      if (value < 0 || value > 0xFFFF) {
        throw new IllegalArgumentException("ProfileId must fit in u16");
      }
    }
  }

  public record CapabilityFlags(long bits) {
    public CapabilityFlags {
      if (bits < 0 || bits > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("CapabilityFlags must fit in u32");
      }
    }
  }

  public record TicketCapabilities(int bits) {
    public static final int LIVE = 1 << 0;
    public static final int VOD = 1 << 1;
    public static final int PREMIUM_PROFILE = 1 << 2;
    public static final int HDR = 1 << 3;
    public static final int SPATIAL_AUDIO = 1 << 4;

    public TicketCapabilities {
      if (bits < 0 || bits > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("TicketCapabilities must fit in u32");
      }
    }

    public boolean contains(int mask) {
      return (bits & mask) == mask;
    }

    public TicketCapabilities insert(int mask) {
      return new TicketCapabilities(bits | mask);
    }

    public TicketCapabilities remove(int mask) {
      return new TicketCapabilities(bits & ~mask);
    }
  }

  public record TicketPolicy(int maxRelays, List<String> allowedRegions, Optional<Long> maxBandwidthKbps) {
    public TicketPolicy {
      if (maxRelays < 0 || maxRelays > 0xFFFF) {
        throw new IllegalArgumentException("maxRelays must fit in u16");
      }
      Objects.requireNonNull(allowedRegions, "allowedRegions");
      allowedRegions = Collections.unmodifiableList(new ArrayList<>(allowedRegions));
      for (String region : allowedRegions) {
        Objects.requireNonNull(region, "region");
        if (region.isEmpty()) {
          throw new IllegalArgumentException("region codes must not be empty");
        }
      }
      Objects.requireNonNull(maxBandwidthKbps, "maxBandwidthKbps");
      maxBandwidthKbps.ifPresent(
          value -> {
            if (value < 0 || value > 0xFFFF_FFFFL) {
              throw new IllegalArgumentException("maxBandwidthKbps must fit in u32");
            }
          });
    }
  }

  public record StreamingTicket(
      byte[] ticketId,
      String owner,
      long dsid,
      int laneId,
      long settlementBucket,
      long startSlot,
      long expireSlot,
      BigInteger prepaidTeu,
      long chunkTeu,
      int fanoutQuota,
      byte[] keyCommitment,
      long nonce,
      byte[] contractSignature,
      byte[] commitment,
      byte[] nullifier,
      byte[] proofId,
      long issuedAt,
      long expiresAt,
      Optional<TicketPolicy> policy,
      TicketCapabilities capabilities) {
    public StreamingTicket {
      Objects.requireNonNull(ticketId, "ticketId");
      Objects.requireNonNull(owner, "owner");
      Objects.requireNonNull(prepaidTeu, "prepaidTeu");
      Objects.requireNonNull(keyCommitment, "keyCommitment");
      Objects.requireNonNull(contractSignature, "contractSignature");
      Objects.requireNonNull(commitment, "commitment");
      Objects.requireNonNull(nullifier, "nullifier");
      Objects.requireNonNull(proofId, "proofId");
      Objects.requireNonNull(policy, "policy");
      Objects.requireNonNull(capabilities, "capabilities");
      if (ticketId.length != HASH_LEN) {
        throw new IllegalArgumentException("ticketId must be 32 bytes");
      }
      if (dsid < 0) {
        throw new IllegalArgumentException("dsid must be non-negative");
      }
      if (laneId < 0 || laneId > 0xFF) {
        throw new IllegalArgumentException("laneId must fit in u8");
      }
      if (settlementBucket < 0) {
        throw new IllegalArgumentException("settlementBucket must be non-negative");
      }
      if (startSlot < 0 || expireSlot < 0) {
        throw new IllegalArgumentException("slot values must be non-negative");
      }
      if (chunkTeu < 0 || chunkTeu > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("chunkTeu must fit in u32");
      }
      if (fanoutQuota < 0 || fanoutQuota > 0xFFFF) {
        throw new IllegalArgumentException("fanoutQuota must fit in u16");
      }
      if (keyCommitment.length != HASH_LEN) {
        throw new IllegalArgumentException("keyCommitment must be 32 bytes");
      }
      if (nonce < 0) {
        throw new IllegalArgumentException("nonce must be non-negative");
      }
      if (contractSignature.length != SIGNATURE_LEN) {
        throw new IllegalArgumentException("contractSignature must be 64 bytes");
      }
      if (commitment.length != HASH_LEN) {
        throw new IllegalArgumentException("commitment must be 32 bytes");
      }
      if (nullifier.length != HASH_LEN) {
        throw new IllegalArgumentException("nullifier must be 32 bytes");
      }
      if (proofId.length != HASH_LEN) {
        throw new IllegalArgumentException("proofId must be 32 bytes");
      }
      if (issuedAt < 0 || expiresAt < 0) {
        throw new IllegalArgumentException("timestamp must be non-negative");
      }
      ticketId = cloneBytes(ticketId);
      keyCommitment = cloneBytes(keyCommitment);
      contractSignature = cloneBytes(contractSignature);
      commitment = cloneBytes(commitment);
      nullifier = cloneBytes(nullifier);
      proofId = cloneBytes(proofId);
    }

    @Override
    public byte[] ticketId() {
      return cloneBytes(ticketId);
    }

    @Override
    public byte[] keyCommitment() {
      return cloneBytes(keyCommitment);
    }

    @Override
    public byte[] contractSignature() {
      return cloneBytes(contractSignature);
    }

    @Override
    public byte[] commitment() {
      return cloneBytes(commitment);
    }

    @Override
    public byte[] nullifier() {
      return cloneBytes(nullifier);
    }

    @Override
    public byte[] proofId() {
      return cloneBytes(proofId);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof StreamingTicket other)) {
        return false;
      }
      return bytesEquals(ticketId, other.ticketId)
          && owner.equals(other.owner)
          && dsid == other.dsid
          && laneId == other.laneId
          && settlementBucket == other.settlementBucket
          && startSlot == other.startSlot
          && expireSlot == other.expireSlot
          && prepaidTeu.equals(other.prepaidTeu)
          && chunkTeu == other.chunkTeu
          && fanoutQuota == other.fanoutQuota
          && bytesEquals(keyCommitment, other.keyCommitment)
          && nonce == other.nonce
          && bytesEquals(contractSignature, other.contractSignature)
          && bytesEquals(commitment, other.commitment)
          && bytesEquals(nullifier, other.nullifier)
          && bytesEquals(proofId, other.proofId)
          && issuedAt == other.issuedAt
          && expiresAt == other.expiresAt
          && policy.equals(other.policy)
          && capabilities.equals(other.capabilities);
    }

    @Override
    public int hashCode() {
      int result = bytesHash(ticketId);
      result = 31 * result + owner.hashCode();
      result = 31 * result + Long.hashCode(dsid);
      result = 31 * result + Integer.hashCode(laneId);
      result = 31 * result + Long.hashCode(settlementBucket);
      result = 31 * result + Long.hashCode(startSlot);
      result = 31 * result + Long.hashCode(expireSlot);
      result = 31 * result + prepaidTeu.hashCode();
      result = 31 * result + Long.hashCode(chunkTeu);
      result = 31 * result + Integer.hashCode(fanoutQuota);
      result = 31 * result + bytesHash(keyCommitment);
      result = 31 * result + Long.hashCode(nonce);
      result = 31 * result + bytesHash(contractSignature);
      result = 31 * result + bytesHash(commitment);
      result = 31 * result + bytesHash(nullifier);
      result = 31 * result + bytesHash(proofId);
      result = 31 * result + Long.hashCode(issuedAt);
      result = 31 * result + Long.hashCode(expiresAt);
      result = 31 * result + policy.hashCode();
      result = 31 * result + capabilities.hashCode();
      return result;
    }
  }

  public record TicketRevocation(byte[] ticketId, byte[] nullifier, int reasonCode, byte[] revocationSignature) {
    public TicketRevocation {
      Objects.requireNonNull(ticketId, "ticketId");
      Objects.requireNonNull(nullifier, "nullifier");
      Objects.requireNonNull(revocationSignature, "revocationSignature");
      if (ticketId.length != HASH_LEN) {
        throw new IllegalArgumentException("ticketId must be 32 bytes");
      }
      if (nullifier.length != HASH_LEN) {
        throw new IllegalArgumentException("nullifier must be 32 bytes");
      }
      if (revocationSignature.length != SIGNATURE_LEN) {
        throw new IllegalArgumentException("revocationSignature must be 64 bytes");
      }
      if (reasonCode < 0 || reasonCode > 0xFFFF) {
        throw new IllegalArgumentException("reasonCode must fit in u16");
      }
      ticketId = cloneBytes(ticketId);
      nullifier = cloneBytes(nullifier);
      revocationSignature = cloneBytes(revocationSignature);
    }

    @Override
    public byte[] ticketId() {
      return cloneBytes(ticketId);
    }

    @Override
    public byte[] nullifier() {
      return cloneBytes(nullifier);
    }

    @Override
    public byte[] revocationSignature() {
      return cloneBytes(revocationSignature);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof TicketRevocation other)) {
        return false;
      }
      return bytesEquals(ticketId, other.ticketId)
          && bytesEquals(nullifier, other.nullifier)
          && reasonCode == other.reasonCode
          && bytesEquals(revocationSignature, other.revocationSignature);
    }

    @Override
    public int hashCode() {
      int result = bytesHash(ticketId);
      result = 31 * result + bytesHash(nullifier);
      result = 31 * result + Integer.hashCode(reasonCode);
      result = 31 * result + bytesHash(revocationSignature);
      return result;
    }
  }

  public record PrivacyCapabilities(long bits) {
    public PrivacyCapabilities {
      if (bits < 0 || bits > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("PrivacyCapabilities must fit in u32");
      }
    }
  }

  public enum HpkeSuite {
    KYBER768_AUTH_PSK(0, 0x0001),
    KYBER1024_AUTH_PSK(1, 0x0002);

    private final int bit;
    private final int suiteId;

    HpkeSuite(int bit, int suiteId) {
      this.bit = bit;
      this.suiteId = suiteId;
    }

    public int bit() {
      return bit;
    }

    public int suiteId() {
      return suiteId;
    }
  }

  public record HpkeSuiteMask(int bits) {
    public HpkeSuiteMask {
      if (bits < 0 || bits > 0xFFFF) {
        throw new IllegalArgumentException("HpkeSuiteMask must fit in u16");
      }
    }

    public boolean contains(HpkeSuite suite) {
      return (bits & (1 << suite.bit())) != 0;
    }
  }

  public enum PrivacyBucketGranularity {
    STANDARD_V1
  }

  public enum FecScheme {
    RS12_10,
    RS_WIN14_10,
    RS18_14
  }

  public enum StorageClass {
    EPHEMERAL,
    PERMANENT
  }

  public enum CapabilityRole {
    PUBLISHER,
    VIEWER
  }

  public enum AudioLayout {
    MONO,
    STEREO,
    FIRST_ORDER_AMBISONICS
  }

  public enum ErrorCode {
    UNKNOWN_CHUNK,
    ACCESS_DENIED,
    RATE_LIMITED,
    PROTOCOL_VIOLATION
  }

  public enum ResolutionKind {
    R720P,
    R1080P,
    R1440P,
    R2160P,
    CUSTOM
  }

  public enum ControlFrameVariant {
    MANIFEST_ANNOUNCE,
    CHUNK_REQUEST,
    CHUNK_ACKNOWLEDGE,
    TRANSPORT_CAPABILITIES,
    CAPABILITY_REPORT,
    CAPABILITY_ACK,
    FEEDBACK_HINT,
    RECEIVER_REPORT,
    KEY_UPDATE,
    CONTENT_KEY_UPDATE,
    PRIVACY_ROUTE_UPDATE,
    PRIVACY_ROUTE_ACK,
    ERROR
  }

  public sealed interface EncryptionSuite permits X25519ChaCha20Poly1305Suite, Kyber768XChaCha20Poly1305Suite {}

  public record X25519ChaCha20Poly1305Suite(byte[] fingerprint) implements EncryptionSuite {
    public X25519ChaCha20Poly1305Suite {
      Objects.requireNonNull(fingerprint, "fingerprint");
      if (fingerprint.length != HASH_LEN) {
        throw new IllegalArgumentException("fingerprint must be 32 bytes");
      }
      fingerprint = cloneBytes(fingerprint);
    }

    @Override
    public byte[] fingerprint() {
      return cloneBytes(fingerprint);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof X25519ChaCha20Poly1305Suite other)) {
        return false;
      }
      return bytesEquals(fingerprint, other.fingerprint);
    }

    @Override
    public int hashCode() {
      return bytesHash(fingerprint);
    }
  }

  public record Kyber768XChaCha20Poly1305Suite(byte[] fingerprint) implements EncryptionSuite {
    public Kyber768XChaCha20Poly1305Suite {
      Objects.requireNonNull(fingerprint, "fingerprint");
      if (fingerprint.length != HASH_LEN) {
        throw new IllegalArgumentException("fingerprint must be 32 bytes");
      }
      fingerprint = cloneBytes(fingerprint);
    }

    @Override
    public byte[] fingerprint() {
      return cloneBytes(fingerprint);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof Kyber768XChaCha20Poly1305Suite other)) {
        return false;
      }
      return bytesEquals(fingerprint, other.fingerprint);
    }

    @Override
    public int hashCode() {
      return bytesHash(fingerprint);
    }
  }

  public record TransportCapabilities(
      HpkeSuiteMask hpkeSuites,
      boolean supportsDatagram,
      long maxSegmentDatagramSize,
      long fecFeedbackIntervalMs,
      PrivacyBucketGranularity privacyBucketGranularity) {
    public TransportCapabilities {
      Objects.requireNonNull(hpkeSuites, "hpkeSuites");
      Objects.requireNonNull(privacyBucketGranularity, "privacyBucketGranularity");
      if (maxSegmentDatagramSize < 0 || maxSegmentDatagramSize > 0xFFFFL) {
        throw new IllegalArgumentException("maxSegmentDatagramSize must fit in u16");
      }
      if (fecFeedbackIntervalMs < 0 || fecFeedbackIntervalMs > 0xFFFFL) {
        throw new IllegalArgumentException("fecFeedbackIntervalMs must fit in u16");
      }
    }
  }

  public record StreamMetadata(
      String title,
      Optional<String> description,
      Optional<byte[]> accessPolicyId,
      List<String> tags) {
    public StreamMetadata {
      Objects.requireNonNull(title, "title");
      description = description == null ? Optional.empty() : description;
      accessPolicyId = accessPolicyId == null ? Optional.empty() : cloneOptionalBytes(accessPolicyId);
      tags = tags == null ? List.of() : List.copyOf(tags);
    }

    @Override
    public Optional<byte[]> accessPolicyId() {
      return cloneOptionalBytes(accessPolicyId);
    }

    @Override
    public List<String> tags() {
      return tags;
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof StreamMetadata other)) {
        return false;
      }
      return title.equals(other.title)
          && description.equals(other.description)
          && optionalBytesEquals(accessPolicyId, other.accessPolicyId)
          && tags.equals(other.tags);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(title, description, tags);
      result = 31 * result + optionalBytesHash(accessPolicyId);
      return result;
    }
  }

  public record PrivacyRelay(
      byte[] relayId,
      String endpoint,
      byte[] keyFingerprint,
      PrivacyCapabilities capabilities) {
    public PrivacyRelay {
      Objects.requireNonNull(relayId, "relayId");
      Objects.requireNonNull(endpoint, "endpoint");
      Objects.requireNonNull(keyFingerprint, "keyFingerprint");
      Objects.requireNonNull(capabilities, "capabilities");
      relayId = cloneBytes(relayId);
      keyFingerprint = cloneBytes(keyFingerprint);
    }

    @Override
    public byte[] relayId() {
      return cloneBytes(relayId);
    }

    @Override
    public byte[] keyFingerprint() {
      return cloneBytes(keyFingerprint);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PrivacyRelay other)) {
        return false;
      }
      return bytesEquals(relayId, other.relayId)
          && endpoint.equals(other.endpoint)
          && bytesEquals(keyFingerprint, other.keyFingerprint)
          && capabilities.equals(other.capabilities);
    }

    @Override
    public int hashCode() {
      int result = bytesHash(relayId);
      result = 31 * result + endpoint.hashCode();
      result = 31 * result + bytesHash(keyFingerprint);
      result = 31 * result + capabilities.hashCode();
      return result;
    }
  }

  public enum SoranetAccessKind {
    READ_ONLY,
    AUTHENTICATED
  }

  public record SoranetRoute(
      byte[] channelId, String exitMultiaddr, Integer paddingBudgetMs, SoranetAccessKind accessKind) {
    public SoranetRoute {
      Objects.requireNonNull(channelId, "channelId");
      Objects.requireNonNull(exitMultiaddr, "exitMultiaddr");
      Objects.requireNonNull(accessKind, "accessKind");
      if (paddingBudgetMs != null && paddingBudgetMs < 0) {
        throw new IllegalArgumentException("paddingBudgetMs must be non-negative when set");
      }
      channelId = cloneBytes(channelId);
    }

    @Override
    public byte[] channelId() {
      return cloneBytes(channelId);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof SoranetRoute other)) {
        return false;
      }
      return bytesEquals(channelId, other.channelId)
          && exitMultiaddr.equals(other.exitMultiaddr)
          && Objects.equals(paddingBudgetMs, other.paddingBudgetMs)
          && accessKind == other.accessKind;
    }

    @Override
    public int hashCode() {
      int result = bytesHash(channelId);
      result = 31 * result + exitMultiaddr.hashCode();
      result = 31 * result + Objects.hashCode(paddingBudgetMs);
      result = 31 * result + accessKind.hashCode();
      return result;
    }
  }

  public record PrivacyRoute(
      byte[] routeId,
      PrivacyRelay entry,
      PrivacyRelay exit,
      byte[] ticketEntry,
      byte[] ticketExit,
      long expirySegment,
      SoranetRoute soranet) {
    public PrivacyRoute {
      Objects.requireNonNull(routeId, "routeId");
      Objects.requireNonNull(entry, "entry");
      Objects.requireNonNull(exit, "exit");
      Objects.requireNonNull(ticketEntry, "ticketEntry");
      Objects.requireNonNull(ticketExit, "ticketExit");
      if (expirySegment < 0) {
        throw new IllegalArgumentException("expirySegment must be non-negative");
      }
      routeId = cloneBytes(routeId);
      ticketEntry = cloneBytes(ticketEntry);
      ticketExit = cloneBytes(ticketExit);
      if (soranet != null) {
        soranet =
            new SoranetRoute(
                soranet.channelId(),
                soranet.exitMultiaddr(),
                soranet.paddingBudgetMs(),
                soranet.accessKind());
      }
    }

    @Override
    public byte[] routeId() {
      return cloneBytes(routeId);
    }

    @Override
    public byte[] ticketEntry() {
      return cloneBytes(ticketEntry);
    }

    @Override
    public byte[] ticketExit() {
      return cloneBytes(ticketExit);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PrivacyRoute other)) {
        return false;
      }
      return bytesEquals(routeId, other.routeId)
          && entry.equals(other.entry)
          && exit.equals(other.exit)
          && bytesEquals(ticketEntry, other.ticketEntry)
          && bytesEquals(ticketExit, other.ticketExit)
          && expirySegment == other.expirySegment
          && Objects.equals(soranet, other.soranet);
    }

    @Override
    public int hashCode() {
      int result = bytesHash(routeId);
      result = 31 * result + entry.hashCode();
      result = 31 * result + exit.hashCode();
      result = 31 * result + bytesHash(ticketEntry);
      result = 31 * result + bytesHash(ticketExit);
      result = 31 * result + Long.hashCode(expirySegment);
      result = 31 * result + Objects.hashCode(soranet);
      return result;
    }
  }

  public record NeuralBundle(
      String bundleId,
      byte[] weightsSha256,
      List<Long> activationScale,
      List<Long> bias,
      byte[] metadataSignature,
      Optional<byte[]> metalShaderSha256,
      Optional<byte[]> cudaPtxSha256) {
    private static final long I16_MIN = -0x8000L;
    private static final long I16_MAX = 0x7FFFL;
    private static final long I32_MIN = -0x8000_0000L;
    private static final long I32_MAX = 0x7FFF_FFFFL;

    public NeuralBundle {
      Objects.requireNonNull(bundleId, "bundleId");
      Objects.requireNonNull(weightsSha256, "weightsSha256");
      Objects.requireNonNull(activationScale, "activationScale");
      Objects.requireNonNull(bias, "bias");
      Objects.requireNonNull(metadataSignature, "metadataSignature");
      weightsSha256 = cloneBytes(weightsSha256);
      metadataSignature = cloneBytes(metadataSignature);
      activationScale = List.copyOf(activationScale);
      bias = List.copyOf(bias);
      for (long value : activationScale) {
        if (value < I16_MIN || value > I16_MAX) {
          throw new IllegalArgumentException("activationScale value out of i16 range");
        }
      }
      for (long value : bias) {
        if (value < I32_MIN || value > I32_MAX) {
          throw new IllegalArgumentException("bias value out of i32 range");
        }
      }
      metalShaderSha256 = metalShaderSha256 == null ? Optional.empty() : cloneOptionalBytes(metalShaderSha256);
      cudaPtxSha256 = cudaPtxSha256 == null ? Optional.empty() : cloneOptionalBytes(cudaPtxSha256);
    }

    @Override
    public byte[] weightsSha256() {
      return cloneBytes(weightsSha256);
    }

    @Override
    public byte[] metadataSignature() {
      return cloneBytes(metadataSignature);
    }

    @Override
    public Optional<byte[]> metalShaderSha256() {
      return cloneOptionalBytes(metalShaderSha256);
    }

    @Override
    public Optional<byte[]> cudaPtxSha256() {
      return cloneOptionalBytes(cudaPtxSha256);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof NeuralBundle other)) {
        return false;
      }
      return bundleId.equals(other.bundleId)
          && bytesEquals(weightsSha256, other.weightsSha256)
          && activationScale.equals(other.activationScale)
          && bias.equals(other.bias)
          && bytesEquals(metadataSignature, other.metadataSignature)
          && optionalBytesEquals(metalShaderSha256, other.metalShaderSha256)
          && optionalBytesEquals(cudaPtxSha256, other.cudaPtxSha256);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(bundleId, activationScale, bias);
      result = 31 * result + bytesHash(weightsSha256);
      result = 31 * result + bytesHash(metadataSignature);
      result = 31 * result + optionalBytesHash(metalShaderSha256);
      result = 31 * result + optionalBytesHash(cudaPtxSha256);
      return result;
    }
  }

  public record ChunkDescriptor(long chunkId, long offset, long length, byte[] commitment, boolean parity) {
    public ChunkDescriptor {
      Objects.requireNonNull(commitment, "commitment");
      if (chunkId < 0 || chunkId > 0xFFFFL) {
        throw new IllegalArgumentException("chunkId must fit in u16");
      }
      if (offset < 0 || offset > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("offset must fit in u32");
      }
      if (length < 0 || length > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("length must fit in u32");
      }
      commitment = cloneBytes(commitment);
    }

    @Override
    public byte[] commitment() {
      return cloneBytes(commitment);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof ChunkDescriptor other)) {
        return false;
      }
      return chunkId == other.chunkId
          && offset == other.offset
          && length == other.length
          && parity == other.parity
          && bytesEquals(commitment, other.commitment);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(chunkId, offset, length, parity);
      result = 31 * result + bytesHash(commitment);
      return result;
    }
  }

  public record ResolutionCustom(int width, int height) {
    public ResolutionCustom {
      if (width < 0 || width > 0xFFFF) {
        throw new IllegalArgumentException("width must fit in u16");
      }
      if (height < 0 || height > 0xFFFF) {
        throw new IllegalArgumentException("height must fit in u16");
      }
    }
  }

  public record Resolution(ResolutionKind kind, Optional<ResolutionCustom> custom) {
    public Resolution {
      Objects.requireNonNull(kind, "kind");
      if (kind == ResolutionKind.CUSTOM) {
        if (custom == null || custom.isEmpty()) {
          throw new IllegalArgumentException("custom resolution requires parameters");
        }
      } else if (custom != null && custom.isPresent()) {
        throw new IllegalArgumentException("non-custom resolution must not carry parameters");
      }
      custom = custom == null ? Optional.empty() : custom;
    }
  }

  public record ManifestV1(
      byte[] streamId,
      long protocolVersion,
      long segmentNumber,
      long publishedAt,
      ProfileId profile,
      String daEndpoint,
      byte[] chunkRoot,
      long contentKeyId,
      byte[] nonceSalt,
      List<ChunkDescriptor> chunkDescriptors,
      byte[] transportCapabilitiesHash,
      EncryptionSuite encryptionSuite,
      FecScheme fecSuite,
      List<PrivacyRoute> privacyRoutes,
      Optional<NeuralBundle> neuralBundle,
      StreamMetadata publicMetadata,
      CapabilityFlags capabilities,
      byte[] signature) {
    public ManifestV1 {
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(profile, "profile");
      Objects.requireNonNull(daEndpoint, "daEndpoint");
      Objects.requireNonNull(chunkRoot, "chunkRoot");
      Objects.requireNonNull(nonceSalt, "nonceSalt");
      Objects.requireNonNull(chunkDescriptors, "chunkDescriptors");
      Objects.requireNonNull(transportCapabilitiesHash, "transportCapabilitiesHash");
      Objects.requireNonNull(encryptionSuite, "encryptionSuite");
      Objects.requireNonNull(fecSuite, "fecSuite");
      Objects.requireNonNull(privacyRoutes, "privacyRoutes");
      Objects.requireNonNull(publicMetadata, "publicMetadata");
      Objects.requireNonNull(capabilities, "capabilities");
      Objects.requireNonNull(signature, "signature");
      if (protocolVersion < 0 || protocolVersion > 0xFFFFL) {
        throw new IllegalArgumentException("protocolVersion must fit in u16");
      }
      if (contentKeyId < 0) {
        throw new IllegalArgumentException("contentKeyId must be non-negative");
      }
      streamId = cloneBytes(streamId);
      chunkRoot = cloneBytes(chunkRoot);
      nonceSalt = cloneBytes(nonceSalt);
      transportCapabilitiesHash = cloneBytes(transportCapabilitiesHash);
      signature = cloneBytes(signature);
      chunkDescriptors = List.copyOf(chunkDescriptors);
      privacyRoutes = List.copyOf(privacyRoutes);
      neuralBundle = neuralBundle == null ? Optional.empty() : neuralBundle;
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }

    @Override
    public byte[] chunkRoot() {
      return cloneBytes(chunkRoot);
    }

    @Override
    public byte[] nonceSalt() {
      return cloneBytes(nonceSalt);
    }

    @Override
    public List<ChunkDescriptor> chunkDescriptors() {
      return chunkDescriptors;
    }

    @Override
    public byte[] transportCapabilitiesHash() {
      return cloneBytes(transportCapabilitiesHash);
    }

    @Override
    public List<PrivacyRoute> privacyRoutes() {
      return privacyRoutes;
    }

    @Override
    public Optional<NeuralBundle> neuralBundle() {
      return neuralBundle;
    }

    @Override
    public byte[] signature() {
      return cloneBytes(signature);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof ManifestV1 other)) {
        return false;
      }
      return bytesEquals(streamId, other.streamId)
          && protocolVersion == other.protocolVersion
          && segmentNumber == other.segmentNumber
          && publishedAt == other.publishedAt
          && profile.equals(other.profile)
          && daEndpoint.equals(other.daEndpoint)
          && bytesEquals(chunkRoot, other.chunkRoot)
          && contentKeyId == other.contentKeyId
          && bytesEquals(nonceSalt, other.nonceSalt)
          && chunkDescriptors.equals(other.chunkDescriptors)
          && bytesEquals(transportCapabilitiesHash, other.transportCapabilitiesHash)
          && encryptionSuite.equals(other.encryptionSuite)
          && fecSuite == other.fecSuite
          && privacyRoutes.equals(other.privacyRoutes)
          && neuralBundle.equals(other.neuralBundle)
          && publicMetadata.equals(other.publicMetadata)
          && capabilities.equals(other.capabilities)
          && bytesEquals(signature, other.signature);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(
          protocolVersion,
          segmentNumber,
          publishedAt,
          profile,
          daEndpoint,
          contentKeyId,
          chunkDescriptors,
          encryptionSuite,
          fecSuite,
          privacyRoutes,
          neuralBundle,
          publicMetadata,
          capabilities);
      result = 31 * result + bytesHash(streamId);
      result = 31 * result + bytesHash(chunkRoot);
      result = 31 * result + bytesHash(nonceSalt);
      result = 31 * result + bytesHash(transportCapabilitiesHash);
      result = 31 * result + bytesHash(signature);
      return result;
    }
  }

  public sealed interface ControlFramePayload
      permits ManifestAnnounceFrame,
          ChunkRequestFrame,
          ChunkAcknowledgeFrame,
          TransportCapabilitiesFrame,
          CapabilityReport,
          CapabilityAck,
          FeedbackHintFrame,
          ReceiverReport,
          KeyUpdate,
          ContentKeyUpdate,
          PrivacyRouteUpdate,
          PrivacyRouteAckFrame,
          ControlErrorFrame {}

  public record ManifestAnnounceFrame(ManifestV1 manifest) implements ControlFramePayload {
    public ManifestAnnounceFrame {
      Objects.requireNonNull(manifest, "manifest");
    }
  }

  public record ChunkRequestFrame(long segment, long chunkId) implements ControlFramePayload {
    public ChunkRequestFrame {
      if (chunkId < 0 || chunkId > 0xFFFFL) {
        throw new IllegalArgumentException("chunkId must fit in u16");
      }
      if (segment < 0) {
        throw new IllegalArgumentException("segment must be non-negative");
      }
    }
  }

  public record ChunkAcknowledgeFrame(long segment, long chunkId) implements ControlFramePayload {
    public ChunkAcknowledgeFrame {
      if (chunkId < 0 || chunkId > 0xFFFFL) {
        throw new IllegalArgumentException("chunkId must fit in u16");
      }
      if (segment < 0) {
        throw new IllegalArgumentException("segment must be non-negative");
      }
    }
  }

  public record TransportCapabilitiesFrame(CapabilityRole endpointRole, TransportCapabilities capabilities)
      implements ControlFramePayload {
    public TransportCapabilitiesFrame {
      Objects.requireNonNull(endpointRole, "endpointRole");
      Objects.requireNonNull(capabilities, "capabilities");
    }
  }

  public record CapabilityReport(
      byte[] streamId,
      CapabilityRole endpointRole,
      long protocolVersion,
      Resolution maxResolution,
      boolean hdrSupported,
      boolean captureHdr,
      List<String> neuralBundles,
      AudioCapability audioCaps,
      CapabilityFlags featureBits,
      long maxDatagramSize,
      boolean dplpmtud) implements ControlFramePayload {
    public CapabilityReport {
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(endpointRole, "endpointRole");
      Objects.requireNonNull(maxResolution, "maxResolution");
      Objects.requireNonNull(neuralBundles, "neuralBundles");
      Objects.requireNonNull(audioCaps, "audioCaps");
      Objects.requireNonNull(featureBits, "featureBits");
      if (protocolVersion < 0 || protocolVersion > 0xFFFFL) {
        throw new IllegalArgumentException("protocolVersion must fit in u16");
      }
      if (maxDatagramSize < 0 || maxDatagramSize > 0xFFFFL) {
        throw new IllegalArgumentException("maxDatagramSize must fit in u16");
      }
      streamId = cloneBytes(streamId);
      neuralBundles = List.copyOf(neuralBundles);
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof CapabilityReport other)) {
        return false;
      }
      return bytesEquals(streamId, other.streamId)
          && endpointRole == other.endpointRole
          && protocolVersion == other.protocolVersion
          && maxResolution.equals(other.maxResolution)
          && hdrSupported == other.hdrSupported
          && captureHdr == other.captureHdr
          && neuralBundles.equals(other.neuralBundles)
          && audioCaps.equals(other.audioCaps)
          && featureBits.equals(other.featureBits)
          && maxDatagramSize == other.maxDatagramSize
          && dplpmtud == other.dplpmtud;
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(
          endpointRole,
          protocolVersion,
          maxResolution,
          hdrSupported,
          captureHdr,
          neuralBundles,
          audioCaps,
          featureBits,
          maxDatagramSize,
          dplpmtud);
      result = 31 * result + bytesHash(streamId);
      return result;
    }
  }

  public record CapabilityAck(
      byte[] streamId,
      long acceptedVersion,
      CapabilityFlags negotiatedFeatures,
      long maxDatagramSize,
      boolean dplpmtud) implements ControlFramePayload {
    public CapabilityAck {
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(negotiatedFeatures, "negotiatedFeatures");
      if (acceptedVersion < 0 || acceptedVersion > 0xFFFFL) {
        throw new IllegalArgumentException("acceptedVersion must fit in u16");
      }
      if (maxDatagramSize < 0 || maxDatagramSize > 0xFFFFL) {
        throw new IllegalArgumentException("maxDatagramSize must fit in u16");
      }
      streamId = cloneBytes(streamId);
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof CapabilityAck other)) {
        return false;
      }
      return bytesEquals(streamId, other.streamId)
          && acceptedVersion == other.acceptedVersion
          && negotiatedFeatures.equals(other.negotiatedFeatures)
          && maxDatagramSize == other.maxDatagramSize
          && dplpmtud == other.dplpmtud;
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(acceptedVersion, negotiatedFeatures, maxDatagramSize, dplpmtud);
      result = 31 * result + bytesHash(streamId);
      return result;
    }
  }

  public record AudioCapability(List<Long> sampleRates, boolean ambisonics, long maxChannels) {
    public AudioCapability {
      Objects.requireNonNull(sampleRates, "sampleRates");
      sampleRates = List.copyOf(sampleRates);
      if (maxChannels < 0 || maxChannels > 0xFFL) {
        throw new IllegalArgumentException("maxChannels must fit in u8");
      }
    }
  }

  public record AudioFrame(
      long sequence,
      long timestampNs,
      long fecLevel,
      AudioLayout channelLayout,
      byte[] payload) {
    public AudioFrame {
      Objects.requireNonNull(channelLayout, "channelLayout");
      Objects.requireNonNull(payload, "payload");
      payload = cloneBytes(payload);
      if (sequence < 0) {
        throw new IllegalArgumentException("sequence must be non-negative");
      }
      if (fecLevel < 0 || fecLevel > 0xFFL) {
        throw new IllegalArgumentException("fecLevel must fit in u8");
      }
    }

    @Override
    public byte[] payload() {
      return cloneBytes(payload);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof AudioFrame other)) {
        return false;
      }
      return sequence == other.sequence
          && timestampNs == other.timestampNs
          && fecLevel == other.fecLevel
          && channelLayout == other.channelLayout
          && bytesEquals(payload, other.payload);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(sequence, timestampNs, fecLevel, channelLayout);
      result = 31 * result + bytesHash(payload);
      return result;
    }
  }

  public record FeedbackHintFrame(
      byte[] streamId,
      long lossEwmaQ16,
      long latencyGradientQ16,
      long observedRttMs,
      long reportIntervalMs,
      long parityChunks) implements ControlFramePayload {
    public FeedbackHintFrame {
      Objects.requireNonNull(streamId, "streamId");
      streamId = cloneBytes(streamId);
      if (lossEwmaQ16 < 0 || lossEwmaQ16 > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("lossEwmaQ16 must fit in u32");
      }
      if (latencyGradientQ16 < Integer.MIN_VALUE || latencyGradientQ16 > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("latencyGradientQ16 must fit in i32");
      }
      if (observedRttMs < 0 || observedRttMs > 0xFFFFL) {
        throw new IllegalArgumentException("observedRttMs must fit in u16");
      }
      if (reportIntervalMs < 0 || reportIntervalMs > 0xFFFFL) {
        throw new IllegalArgumentException("reportIntervalMs must fit in u16");
      }
      if (parityChunks < 0 || parityChunks > 0xFFL) {
        throw new IllegalArgumentException("parityChunks must fit in u8");
      }
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }
  }

  public record SyncDiagnostics(
      long windowMs,
      long samples,
      long avgAudioJitterMs,
      long maxAudioJitterMs,
      long avgAvDriftMs,
      long maxAvDriftMs,
      long ewmaAvDriftMs,
      long violationCount) {
    public SyncDiagnostics {
      if (windowMs < 0 || windowMs > 0xFFFFL) {
        throw new IllegalArgumentException("windowMs must fit in u16");
      }
      if (samples < 0 || samples > 0xFFFFL) {
        throw new IllegalArgumentException("samples must fit in u16");
      }
      if (avgAudioJitterMs < 0 || avgAudioJitterMs > 0xFFFFL) {
        throw new IllegalArgumentException("avgAudioJitterMs must fit in u16");
      }
      if (maxAudioJitterMs < 0 || maxAudioJitterMs > 0xFFFFL) {
        throw new IllegalArgumentException("maxAudioJitterMs must fit in u16");
      }
      if (avgAvDriftMs < -0x8000L || avgAvDriftMs > 0x7FFFL) {
        throw new IllegalArgumentException("avgAvDriftMs must fit in i16");
      }
      if (maxAvDriftMs < 0 || maxAvDriftMs > 0xFFFFL) {
        throw new IllegalArgumentException("maxAvDriftMs must fit in u16");
      }
      if (ewmaAvDriftMs < -0x8000L || ewmaAvDriftMs > 0x7FFFL) {
        throw new IllegalArgumentException("ewmaAvDriftMs must fit in i16");
      }
      if (violationCount < 0 || violationCount > 0xFFFFL) {
        throw new IllegalArgumentException("violationCount must fit in u16");
      }
    }
  }

  public record ReceiverReport(
      byte[] streamId,
      long latestSegment,
      long layerMask,
      long measuredThroughputKbps,
      long rttMs,
      long lossPercentX100,
      long decoderBufferMs,
      Resolution activeResolution,
      boolean hdrActive,
      long ecnCeCount,
      long jitterMs,
      long deliveredSequence,
      long parityApplied,
      long fecBudget,
      Optional<SyncDiagnostics> syncDiagnostics) implements ControlFramePayload {
    public ReceiverReport {
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(activeResolution, "activeResolution");
      streamId = cloneBytes(streamId);
      if (latestSegment < 0) {
        throw new IllegalArgumentException("latestSegment must be non-negative");
      }
      if (layerMask < 0 || layerMask > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("layerMask must fit in u32");
      }
      if (measuredThroughputKbps < 0 || measuredThroughputKbps > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("measuredThroughputKbps must fit in u32");
      }
      if (rttMs < 0 || rttMs > 0xFFFFL) {
        throw new IllegalArgumentException("rttMs must fit in u16");
      }
      if (lossPercentX100 < 0 || lossPercentX100 > 0xFFFFL) {
        throw new IllegalArgumentException("lossPercentX100 must fit in u16");
      }
      if (decoderBufferMs < 0 || decoderBufferMs > 0xFFFFL) {
        throw new IllegalArgumentException("decoderBufferMs must fit in u16");
      }
      if (ecnCeCount < 0 || ecnCeCount > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("ecnCeCount must fit in u32");
      }
      if (jitterMs < 0 || jitterMs > 0xFFFFL) {
        throw new IllegalArgumentException("jitterMs must fit in u16");
      }
      if (deliveredSequence < 0) {
        throw new IllegalArgumentException("deliveredSequence must be non-negative");
      }
      if (parityApplied < 0 || parityApplied > 0xFFL) {
        throw new IllegalArgumentException("parityApplied must fit in u8");
      }
      if (fecBudget < 0 || fecBudget > 0xFFL) {
        throw new IllegalArgumentException("fecBudget must fit in u8");
      }
      syncDiagnostics = syncDiagnostics == null ? Optional.empty() : syncDiagnostics;
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }
  }

  public record TelemetryEncodeStats(long segment, long avgLatencyMs, long droppedLayers, long avgAudioJitterMs, long maxAudioJitterMs) {
    public TelemetryEncodeStats {
      if (avgLatencyMs < 0 || avgLatencyMs > 0xFFFFL) {
        throw new IllegalArgumentException("avgLatencyMs must fit in u16");
      }
      if (droppedLayers < 0 || droppedLayers > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("droppedLayers must fit in u32");
      }
      if (avgAudioJitterMs < 0 || avgAudioJitterMs > 0xFFFFL) {
        throw new IllegalArgumentException("avgAudioJitterMs must fit in u16");
      }
      if (maxAudioJitterMs < 0 || maxAudioJitterMs > 0xFFFFL) {
        throw new IllegalArgumentException("maxAudioJitterMs must fit in u16");
      }
    }
  }

  public record TelemetryDecodeStats(long segment, long bufferMs, long droppedFrames, long maxDecodeQueueMs, long avgAvDriftMs, long maxAvDriftMs) {
    public TelemetryDecodeStats {
      if (bufferMs < 0 || bufferMs > 0xFFFFL) {
        throw new IllegalArgumentException("bufferMs must fit in u16");
      }
      if (droppedFrames < 0 || droppedFrames > 0xFFFFL) {
        throw new IllegalArgumentException("droppedFrames must fit in u16");
      }
      if (maxDecodeQueueMs < 0 || maxDecodeQueueMs > 0xFFFFL) {
        throw new IllegalArgumentException("maxDecodeQueueMs must fit in u16");
      }
      if (avgAvDriftMs < -0x8000L || avgAvDriftMs > 0x7FFFL) {
        throw new IllegalArgumentException("avgAvDriftMs must fit in i16");
      }
      if (maxAvDriftMs < 0 || maxAvDriftMs > 0xFFFFL) {
        throw new IllegalArgumentException("maxAvDriftMs must fit in u16");
      }
    }
  }

  public record TelemetryNetworkStats(long rttMs, long lossPercentX100, long fecRepairs, long fecFailures, long datagramReinjects) {}

  public record TelemetrySecurityStats(
      EncryptionSuite suite,
      long rekeys,
      long gckRotations,
      Optional<Long> lastContentKeyId,
      Optional<Long> lastContentKeyValidFrom) {
    public TelemetrySecurityStats {
      Objects.requireNonNull(suite, "suite");
      Objects.requireNonNull(lastContentKeyId, "lastContentKeyId");
      Objects.requireNonNull(lastContentKeyValidFrom, "lastContentKeyValidFrom");
      if (rekeys < 0 || rekeys > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("rekeys must fit in u32");
      }
      if (gckRotations < 0 || gckRotations > 0xFFFF_FFFFL) {
        throw new IllegalArgumentException("gckRotations must fit in u32");
      }
      lastContentKeyId.ifPresent(
          value -> {
            if (value < 0 || Long.compareUnsigned(value, -1L) > 0) {
              throw new IllegalArgumentException("lastContentKeyId must fit in u64");
            }
          });
      lastContentKeyValidFrom.ifPresent(
          value -> {
            if (value < 0 || Long.compareUnsigned(value, -1L) > 0) {
              throw new IllegalArgumentException("lastContentKeyValidFrom must fit in u64");
            }
          });
    }
  }

  public record TelemetryEnergyStats(long segment, long encoderMilliwatts, long decoderMilliwatts) {}

  public record TelemetryAuditOutcome(
      String traceId,
      long slotHeight,
      String reviewer,
      String status,
      Optional<String> mitigationUrl) {
    public TelemetryAuditOutcome {
      Objects.requireNonNull(traceId, "traceId");
      Objects.requireNonNull(reviewer, "reviewer");
      Objects.requireNonNull(status, "status");
      Objects.requireNonNull(mitigationUrl, "mitigationUrl");
    }
  }

  public sealed interface TelemetryEvent
      permits TelemetryEncodeEvent,
          TelemetryDecodeEvent,
          TelemetryNetworkEvent,
          TelemetrySecurityEvent,
          TelemetryEnergyEvent,
          TelemetryAuditOutcomeEvent {}

  public record TelemetryEncodeEvent(TelemetryEncodeStats stats) implements TelemetryEvent {
    public TelemetryEncodeEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record TelemetryDecodeEvent(TelemetryDecodeStats stats) implements TelemetryEvent {
    public TelemetryDecodeEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record TelemetryNetworkEvent(TelemetryNetworkStats stats) implements TelemetryEvent {
    public TelemetryNetworkEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record TelemetrySecurityEvent(TelemetrySecurityStats stats) implements TelemetryEvent {
    public TelemetrySecurityEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record TelemetryEnergyEvent(TelemetryEnergyStats stats) implements TelemetryEvent {
    public TelemetryEnergyEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record TelemetryAuditOutcomeEvent(TelemetryAuditOutcome stats) implements TelemetryEvent {
    public TelemetryAuditOutcomeEvent {
      Objects.requireNonNull(stats, "stats");
    }
  }

  public record KeyUpdate(
      byte[] sessionId,
      EncryptionSuite suite,
      long protocolVersion,
      byte[] pubEphemeral,
      long keyCounter,
      byte[] signature) implements ControlFramePayload {
    public KeyUpdate {
      Objects.requireNonNull(sessionId, "sessionId");
      Objects.requireNonNull(suite, "suite");
      Objects.requireNonNull(pubEphemeral, "pubEphemeral");
      Objects.requireNonNull(signature, "signature");
      if (protocolVersion < 0 || protocolVersion > 0xFFFFL) {
        throw new IllegalArgumentException("protocolVersion must fit in u16");
      }
      if (keyCounter < 0) {
        throw new IllegalArgumentException("keyCounter must be non-negative");
      }
      sessionId = cloneBytes(sessionId);
      pubEphemeral = cloneBytes(pubEphemeral);
      signature = cloneBytes(signature);
    }

    @Override
    public byte[] sessionId() {
      return cloneBytes(sessionId);
    }

    @Override
    public byte[] pubEphemeral() {
      return cloneBytes(pubEphemeral);
    }

    @Override
    public byte[] signature() {
      return cloneBytes(signature);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof KeyUpdate other)) {
        return false;
      }
      return bytesEquals(sessionId, other.sessionId)
          && suite.equals(other.suite)
          && protocolVersion == other.protocolVersion
          && bytesEquals(pubEphemeral, other.pubEphemeral)
          && keyCounter == other.keyCounter
          && bytesEquals(signature, other.signature);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(suite, protocolVersion, keyCounter);
      result = 31 * result + bytesHash(sessionId);
      result = 31 * result + bytesHash(pubEphemeral);
      result = 31 * result + bytesHash(signature);
      return result;
    }
  }

  public record ContentKeyUpdate(long contentKeyId, byte[] gckWrapped, long validFromSegment)
      implements ControlFramePayload {
    public ContentKeyUpdate {
      Objects.requireNonNull(gckWrapped, "gckWrapped");
      if (contentKeyId < 0) {
        throw new IllegalArgumentException("contentKeyId must be non-negative");
      }
      if (validFromSegment < 0) {
        throw new IllegalArgumentException("validFromSegment must be non-negative");
      }
      gckWrapped = cloneBytes(gckWrapped);
    }

    @Override
    public byte[] gckWrapped() {
      return cloneBytes(gckWrapped);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof ContentKeyUpdate other)) {
        return false;
      }
      return contentKeyId == other.contentKeyId
          && validFromSegment == other.validFromSegment
          && bytesEquals(gckWrapped, other.gckWrapped);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(contentKeyId, validFromSegment);
      result = 31 * result + bytesHash(gckWrapped);
      return result;
    }
  }

  public sealed interface CryptoError
      permits CryptoError.NonMonotonicKeyCounter,
          CryptoError.SuiteChanged,
          CryptoError.ContentKeyRegression,
          CryptoError.InvalidValidFrom,
          CryptoError.InvalidWrappedKey {
    record NonMonotonicKeyCounter(long previous, long found) implements CryptoError {}

    record SuiteChanged(EncryptionSuite expected, EncryptionSuite found) implements CryptoError {
      public SuiteChanged {
        Objects.requireNonNull(expected, "expected");
        Objects.requireNonNull(found, "found");
      }
    }

    record ContentKeyRegression(long previous, long found) implements CryptoError {}

    record InvalidValidFrom(long previous, long found) implements CryptoError {}

    record InvalidWrappedKey() implements CryptoError {}
  }

  public static final class CryptoException extends RuntimeException {
    private final CryptoError error;

    public CryptoException(CryptoError error) {
      super(error.toString());
      this.error = Objects.requireNonNull(error, "error");
    }

    public CryptoError error() {
      return error;
    }
  }

  public record KeyUpdateSnapshot(EncryptionSuite suite, long lastCounter) {
    public KeyUpdateSnapshot {
      Objects.requireNonNull(suite, "suite");
      if (lastCounter < 0 || Long.compareUnsigned(lastCounter, -1L) > 0) {
        throw new IllegalArgumentException("lastCounter must fit in u64");
      }
    }
  }

  public record ContentKeySnapshot(long lastId, long lastValidFrom) {
    public ContentKeySnapshot {
      if (lastId < 0 || Long.compareUnsigned(lastId, -1L) > 0) {
        throw new IllegalArgumentException("lastId must fit in u64");
      }
      if (lastValidFrom < 0 || Long.compareUnsigned(lastValidFrom, -1L) > 0) {
        throw new IllegalArgumentException("lastValidFrom must fit in u64");
      }
    }
  }

  public static final class KeyUpdateState {
    private EncryptionSuite suite;
    private Long lastCounter;

    public void record(KeyUpdate frame) {
      Objects.requireNonNull(frame, "frame");
      if (lastCounter != null && frame.keyCounter() <= lastCounter) {
        throw new CryptoException(
            new CryptoError.NonMonotonicKeyCounter(lastCounter, frame.keyCounter()));
      }
      if (suite != null) {
        if (!suite.equals(frame.suite())) {
          throw new CryptoException(new CryptoError.SuiteChanged(suite, frame.suite()));
        }
      } else {
        suite = frame.suite();
      }
      lastCounter = frame.keyCounter();
    }

    public Optional<EncryptionSuite> suite() {
      return Optional.ofNullable(suite);
    }

    public Optional<Long> lastCounter() {
      return Optional.ofNullable(lastCounter);
    }

    public void restore(Optional<Long> lastCounter, Optional<EncryptionSuite> suite) {
      Objects.requireNonNull(lastCounter, "lastCounter");
      Objects.requireNonNull(suite, "suite");
      this.lastCounter = lastCounter.orElse(null);
      this.suite = suite.orElse(null);
    }

    public Optional<KeyUpdateSnapshot> snapshot() {
      if (suite == null || lastCounter == null) {
        return Optional.empty();
      }
      return Optional.of(new KeyUpdateSnapshot(suite, lastCounter));
    }

    public static KeyUpdateState fromSnapshot(KeyUpdateSnapshot snapshot) {
      Objects.requireNonNull(snapshot, "snapshot");
      KeyUpdateState state = new KeyUpdateState();
      state.restore(Optional.of(snapshot.lastCounter()), Optional.of(snapshot.suite()));
      return state;
    }
  }

  public static final class ContentKeyState {
    private Long lastId;
    private Long lastValidFrom;

    public void record(ContentKeyUpdate update) {
      Objects.requireNonNull(update, "update");
      if (update.gckWrapped().length == 0) {
        throw new CryptoException(new CryptoError.InvalidWrappedKey());
      }
      if (lastId != null && update.contentKeyId() <= lastId) {
        throw new CryptoException(
            new CryptoError.ContentKeyRegression(lastId, update.contentKeyId()));
      }
      if (lastValidFrom != null && update.validFromSegment() < lastValidFrom) {
        throw new CryptoException(
            new CryptoError.InvalidValidFrom(lastValidFrom, update.validFromSegment()));
      }
      lastId = update.contentKeyId();
      lastValidFrom = update.validFromSegment();
    }

    public Optional<Long> lastId() {
      return Optional.ofNullable(lastId);
    }

    public Optional<Long> lastValidFrom() {
      return Optional.ofNullable(lastValidFrom);
    }

    public void restore(Optional<Long> lastId, Optional<Long> lastValidFrom) {
      Objects.requireNonNull(lastId, "lastId");
      Objects.requireNonNull(lastValidFrom, "lastValidFrom");
      this.lastId = lastId.orElse(null);
      this.lastValidFrom = lastValidFrom.orElse(null);
    }

    public Optional<ContentKeySnapshot> snapshot() {
      if (lastId == null || lastValidFrom == null) {
        return Optional.empty();
      }
      return Optional.of(new ContentKeySnapshot(lastId, lastValidFrom));
    }

    public static ContentKeyState fromSnapshot(ContentKeySnapshot snapshot) {
      Objects.requireNonNull(snapshot, "snapshot");
      ContentKeyState state = new ContentKeyState();
      state.restore(Optional.of(snapshot.lastId()), Optional.of(snapshot.lastValidFrom()));
      return state;
    }
  }

  public record PrivacyRouteUpdate(
      byte[] routeId,
      byte[] streamId,
      long contentKeyId,
      long validFromSegment,
      long validUntilSegment,
      byte[] exitToken,
      SoranetRoute soranet) implements ControlFramePayload {
    public PrivacyRouteUpdate {
      Objects.requireNonNull(routeId, "routeId");
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(exitToken, "exitToken");
      if (contentKeyId < 0) {
        throw new IllegalArgumentException("contentKeyId must be non-negative");
      }
      if (validFromSegment < 0 || validUntilSegment < 0) {
        throw new IllegalArgumentException("segment bounds must be non-negative");
      }
      routeId = cloneBytes(routeId);
      streamId = cloneBytes(streamId);
      exitToken = cloneBytes(exitToken);
      if (soranet != null) {
        soranet =
            new SoranetRoute(
                soranet.channelId(),
                soranet.exitMultiaddr(),
                soranet.paddingBudgetMs(),
                soranet.accessKind());
      }
    }

    @Override
    public byte[] routeId() {
      return cloneBytes(routeId);
    }

    @Override
    public byte[] streamId() {
      return cloneBytes(streamId);
    }

    @Override
    public byte[] exitToken() {
      return cloneBytes(exitToken);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PrivacyRouteUpdate other)) {
        return false;
      }
      return bytesEquals(routeId, other.routeId)
          && bytesEquals(streamId, other.streamId)
          && contentKeyId == other.contentKeyId
          && validFromSegment == other.validFromSegment
          && validUntilSegment == other.validUntilSegment
          && bytesEquals(exitToken, other.exitToken)
          && Objects.equals(soranet, other.soranet);
    }

    @Override
    public int hashCode() {
      int result = Objects.hash(contentKeyId, validFromSegment, validUntilSegment, soranet);
      result = 31 * result + bytesHash(routeId);
      result = 31 * result + bytesHash(streamId);
      result = 31 * result + bytesHash(exitToken);
      return result;
    }
  }

  public record PrivacyRouteAckFrame(byte[] routeId) implements ControlFramePayload {
    public PrivacyRouteAckFrame {
      Objects.requireNonNull(routeId, "routeId");
      routeId = cloneBytes(routeId);
    }

    @Override
    public byte[] routeId() {
      return cloneBytes(routeId);
    }

    @Override
    public boolean equals(Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof PrivacyRouteAckFrame other)) {
        return false;
      }
      return bytesEquals(routeId, other.routeId);
    }

    @Override
    public int hashCode() {
      return bytesHash(routeId);
    }
  }

  public record ControlErrorFrame(ErrorCode code, String message) implements ControlFramePayload {
    public ControlErrorFrame {
      Objects.requireNonNull(code, "code");
      Objects.requireNonNull(message, "message");
    }
  }

  public record ControlFrame(ControlFrameVariant variant, ControlFramePayload payload) {
    public ControlFrame {
      Objects.requireNonNull(variant, "variant");
      Objects.requireNonNull(payload, "payload");
    }
  }

  private static <E extends Enum<E>> TypeAdapter<E> enumAdapter(Class<E> enumClass) {
    E[] values = enumClass.getEnumConstants();
    return new TypeAdapter<>() {
      @Override
      public void encode(NoritoEncoder encoder, E value) {
        encoder.writeUInt(value.ordinal(), 32);
      }

      @Override
      public E decode(NoritoDecoder decoder) {
        long raw = decoder.readUInt(32);
        if (raw < 0 || raw >= values.length) {
          throw new IllegalArgumentException(
              "Invalid " + enumClass.getSimpleName() + " discriminant: " + raw);
        }
        return values[(int) raw];
      }

      @Override
      public int fixedSize() {
        return 4;
      }
    };
  }

  private static final TypeAdapter<ProfileId> PROFILE_ID_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ProfileId value) {
      UINT16.encode(encoder, (long) value.value());
    }

    @Override
    public ProfileId decode(NoritoDecoder decoder) {
      Long raw = UINT16.decode(decoder);
      return new ProfileId(raw.intValue());
    }

    @Override
    public int fixedSize() {
      return UINT16.fixedSize();
    }
  };

  private static final TypeAdapter<CapabilityFlags> CAPABILITY_FLAGS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, CapabilityFlags value) {
      UINT32.encode(encoder, value.bits());
    }

    @Override
    public CapabilityFlags decode(NoritoDecoder decoder) {
      Long raw = UINT32.decode(decoder);
      return new CapabilityFlags(raw.longValue());
    }

    @Override
    public int fixedSize() {
      return UINT32.fixedSize();
    }
  };

  private static final TypeAdapter<PrivacyCapabilities> PRIVACY_CAPABILITIES_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, PrivacyCapabilities value) {
      UINT32.encode(encoder, value.bits());
    }

    @Override
    public PrivacyCapabilities decode(NoritoDecoder decoder) {
      Long raw = UINT32.decode(decoder);
      return new PrivacyCapabilities(raw.longValue());
    }

    @Override
    public int fixedSize() {
      return UINT32.fixedSize();
    }
  };

  private static final TypeAdapter<HpkeSuiteMask> HPKE_SUITE_MASK_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, HpkeSuiteMask value) {
      UINT16.encode(encoder, (long) value.bits());
    }

    @Override
    public HpkeSuiteMask decode(NoritoDecoder decoder) {
      Long raw = UINT16.decode(decoder);
      return new HpkeSuiteMask(raw.intValue());
    }

    @Override
    public int fixedSize() {
      return UINT16.fixedSize();
    }
  };

  private static final TypeAdapter<PrivacyBucketGranularity> PRIVACY_BUCKET_GRANULARITY_ADAPTER =
      enumAdapter(PrivacyBucketGranularity.class);

  private static final TypeAdapter<FecScheme> FEC_SCHEME_ADAPTER = enumAdapter(FecScheme.class);

  private static final TypeAdapter<CapabilityRole> CAPABILITY_ROLE_ADAPTER =
      enumAdapter(CapabilityRole.class);

  private static final TypeAdapter<AudioLayout> AUDIO_LAYOUT_ADAPTER = enumAdapter(AudioLayout.class);

  private static final TypeAdapter<ErrorCode> ERROR_CODE_ADAPTER = enumAdapter(ErrorCode.class);

  private static final TypeAdapter<ResolutionKind> RESOLUTION_KIND_ADAPTER = enumAdapter(ResolutionKind.class);

  private static final TypeAdapter<EncryptionSuite> ENCRYPTION_SUITE_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, EncryptionSuite suite) {
      if (suite instanceof X25519ChaCha20Poly1305Suite x) {
        encoder.writeUInt(0, 32);
        HASH.encode(encoder, x.fingerprint());
      } else if (suite instanceof Kyber768XChaCha20Poly1305Suite k) {
        encoder.writeUInt(1, 32);
        HASH.encode(encoder, k.fingerprint());
      } else {
        throw new IllegalArgumentException("Unsupported EncryptionSuite variant: " + suite);
      }
    }

    @Override
    public EncryptionSuite decode(NoritoDecoder decoder) {
      long tag = decoder.readUInt(32);
      if (tag == 0) {
        return new X25519ChaCha20Poly1305Suite(HASH.decode(decoder));
      } else if (tag == 1) {
        return new Kyber768XChaCha20Poly1305Suite(HASH.decode(decoder));
      }
      throw new IllegalArgumentException("Invalid EncryptionSuite discriminant: " + tag);
    }

    @Override
    public int fixedSize() {
      return 4 + HASH_LEN;
    }
  };

  private static final TypeAdapter<ResolutionCustom> RESOLUTION_CUSTOM_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ResolutionCustom value) {
      UINT16.encode(encoder, (long) value.width());
      UINT16.encode(encoder, (long) value.height());
    }

    @Override
    public ResolutionCustom decode(NoritoDecoder decoder) {
      int width = UINT16.decode(decoder).intValue();
      int height = UINT16.decode(decoder).intValue();
      return new ResolutionCustom(width, height);
    }

    @Override
    public int fixedSize() {
      return UINT16.fixedSize() * 2;
    }
  };

  private static final TypeAdapter<Resolution> RESOLUTION_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, Resolution value) {
      RESOLUTION_KIND_ADAPTER.encode(encoder, value.kind());
      if (value.kind() == ResolutionKind.CUSTOM) {
        RESOLUTION_CUSTOM_ADAPTER.encode(encoder, value.custom().orElseThrow());
      }
    }

    @Override
    public Resolution decode(NoritoDecoder decoder) {
      ResolutionKind kind = RESOLUTION_KIND_ADAPTER.decode(decoder);
      if (kind == ResolutionKind.CUSTOM) {
        return new Resolution(kind, Optional.of(RESOLUTION_CUSTOM_ADAPTER.decode(decoder)));
      }
      return new Resolution(kind, Optional.empty());
    }
  };

  private static final TypeAdapter<TransportCapabilities> TRANSPORT_CAPABILITIES_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, TransportCapabilities value) {
          HPKE_SUITE_MASK_ADAPTER.encode(encoder, value.hpkeSuites());
          BOOL.encode(encoder, value.supportsDatagram());
          UINT16.encode(encoder, value.maxSegmentDatagramSize());
          UINT16.encode(encoder, value.fecFeedbackIntervalMs());
          PRIVACY_BUCKET_GRANULARITY_ADAPTER.encode(encoder, value.privacyBucketGranularity());
        }

        @Override
        public TransportCapabilities decode(NoritoDecoder decoder) {
          HpkeSuiteMask mask = HPKE_SUITE_MASK_ADAPTER.decode(decoder);
          boolean datagram = BOOL.decode(decoder);
          long maxDatagram = UINT16.decode(decoder);
          long fecInterval = UINT16.decode(decoder);
          PrivacyBucketGranularity granularity = PRIVACY_BUCKET_GRANULARITY_ADAPTER.decode(decoder);
          return new TransportCapabilities(mask, datagram, maxDatagram, fecInterval, granularity);
        }
      };

  private static final TypeAdapter<StreamMetadata> STREAM_METADATA_ADAPTER = new TypeAdapter<>() {
    private final TypeAdapter<Optional<String>> optionalString = NoritoAdapters.option(STRING);
    private final TypeAdapter<Optional<byte[]>> optionalHash = NoritoAdapters.option(HASH);

    @Override
    public void encode(NoritoEncoder encoder, StreamMetadata value) {
      STRING.encode(encoder, value.title());
      optionalString.encode(encoder, value.description());
      optionalHash.encode(encoder, value.accessPolicyId());
      STRING_LIST.encode(encoder, value.tags());
    }

    @Override
    public StreamMetadata decode(NoritoDecoder decoder) {
      String title = STRING.decode(decoder);
      Optional<String> description = optionalString.decode(decoder);
      Optional<byte[]> accessPolicy = optionalHash.decode(decoder);
      List<String> tags = STRING_LIST.decode(decoder);
      return new StreamMetadata(title, description, accessPolicy, tags);
    }
  };

  private static final TypeAdapter<TicketCapabilities> TICKET_CAPABILITIES_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, TicketCapabilities value) {
          UINT32.encode(encoder, value.bits() & 0xFFFF_FFFFL);
        }

        @Override
        public TicketCapabilities decode(NoritoDecoder decoder) {
          long raw = UINT32.decode(decoder);
          return new TicketCapabilities((int) (raw & 0xFFFF_FFFFL));
        }
      };

  private static final TypeAdapter<TicketPolicy> TICKET_POLICY_ADAPTER = new TypeAdapter<>() {
    private final TypeAdapter<Optional<Long>> optionalUint32 = NoritoAdapters.option(UINT32);

    @Override
    public void encode(NoritoEncoder encoder, TicketPolicy value) {
      UINT16.encode(encoder, value.maxRelays() & 0xFFFFL);
      STRING_LIST.encode(encoder, value.allowedRegions());
      optionalUint32.encode(encoder, value.maxBandwidthKbps());
    }

    @Override
    public TicketPolicy decode(NoritoDecoder decoder) {
      int maxRelays = (int) (UINT16.decode(decoder) & 0xFFFFL);
      List<String> regions = STRING_LIST.decode(decoder);
      Optional<Long> maxBandwidth = optionalUint32.decode(decoder);
      return new TicketPolicy(maxRelays, regions, maxBandwidth);
    }
  };

  private static final TypeAdapter<BigInteger> UINT128_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, BigInteger value) {
      byte[] le = toLittleEndian(value, 16, UINT128_MAX);
      encoder.writeBytes(le);
    }

    @Override
    public BigInteger decode(NoritoDecoder decoder) {
      byte[] le = decoder.readBytes(16);
      return fromLittleEndian(le);
    }

    @Override
    public int fixedSize() {
      return 16;
    }
  };

  private static final TypeAdapter<StreamingTicket> STREAMING_TICKET_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<TicketPolicy>> optionalPolicy =
            NoritoAdapters.option(TICKET_POLICY_ADAPTER);

        @Override
        public void encode(NoritoEncoder encoder, StreamingTicket value) {
          HASH.encode(encoder, value.ticketId());
          STRING.encode(encoder, value.owner());
          UINT64.encode(encoder, value.dsid());
          UINT8.encode(encoder, (long) value.laneId());
          UINT64.encode(encoder, value.settlementBucket());
          UINT64.encode(encoder, value.startSlot());
          UINT64.encode(encoder, value.expireSlot());
          UINT128_ADAPTER.encode(encoder, value.prepaidTeu());
          UINT32.encode(encoder, value.chunkTeu());
          UINT16.encode(encoder, value.fanoutQuota() & 0xFFFFL);
          HASH.encode(encoder, value.keyCommitment());
          UINT64.encode(encoder, value.nonce());
          SIGNATURE.encode(encoder, value.contractSignature());
          HASH.encode(encoder, value.commitment());
          HASH.encode(encoder, value.nullifier());
          HASH.encode(encoder, value.proofId());
          UINT64.encode(encoder, value.issuedAt());
          UINT64.encode(encoder, value.expiresAt());
          optionalPolicy.encode(encoder, value.policy());
          TICKET_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities());
        }

        @Override
        public StreamingTicket decode(NoritoDecoder decoder) {
          byte[] ticketId = HASH.decode(decoder);
          String owner = STRING.decode(decoder);
          long dsid = UINT64.decode(decoder);
          int laneId = (int) (UINT8.decode(decoder) & 0xFFL);
          long settlementBucket = UINT64.decode(decoder);
          long startSlot = UINT64.decode(decoder);
          long expireSlot = UINT64.decode(decoder);
          BigInteger prepaidTeu = UINT128_ADAPTER.decode(decoder);
          long chunkTeu = UINT32.decode(decoder);
          int fanoutQuota = (int) (UINT16.decode(decoder) & 0xFFFFL);
          byte[] keyCommitment = HASH.decode(decoder);
          long nonce = UINT64.decode(decoder);
          byte[] contractSig = SIGNATURE.decode(decoder);
          byte[] commitment = HASH.decode(decoder);
          byte[] nullifier = HASH.decode(decoder);
          byte[] proofId = HASH.decode(decoder);
          long issuedAt = UINT64.decode(decoder);
          long expiresAt = UINT64.decode(decoder);
          Optional<TicketPolicy> policy = optionalPolicy.decode(decoder);
          TicketCapabilities capabilities = TICKET_CAPABILITIES_ADAPTER.decode(decoder);
          return new StreamingTicket(
              ticketId,
              owner,
              dsid,
              laneId,
              settlementBucket,
              startSlot,
              expireSlot,
              prepaidTeu,
              chunkTeu,
              fanoutQuota,
              keyCommitment,
              nonce,
              contractSig,
              commitment,
              nullifier,
              proofId,
              issuedAt,
              expiresAt,
              policy,
              capabilities);
        }
      };

  private static final TypeAdapter<TicketRevocation> TICKET_REVOCATION_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, TicketRevocation value) {
          HASH.encode(encoder, value.ticketId());
          HASH.encode(encoder, value.nullifier());
          UINT16.encode(encoder, value.reasonCode() & 0xFFFFL);
          SIGNATURE.encode(encoder, value.revocationSignature());
        }

        @Override
        public TicketRevocation decode(NoritoDecoder decoder) {
          byte[] ticketId = HASH.decode(decoder);
          byte[] nullifier = HASH.decode(decoder);
          int reasonCode = (int) (UINT16.decode(decoder) & 0xFFFFL);
          byte[] signature = SIGNATURE.decode(decoder);
          return new TicketRevocation(ticketId, nullifier, reasonCode, signature);
        }
      };

  private static final TypeAdapter<PrivacyRelay> PRIVACY_RELAY_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, PrivacyRelay value) {
      HASH.encode(encoder, value.relayId());
      STRING.encode(encoder, value.endpoint());
      HASH.encode(encoder, value.keyFingerprint());
      PRIVACY_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities());
    }

    @Override
    public PrivacyRelay decode(NoritoDecoder decoder) {
      byte[] relayId = HASH.decode(decoder);
      String endpoint = STRING.decode(decoder);
      byte[] keyFingerprint = HASH.decode(decoder);
      PrivacyCapabilities capabilities = PRIVACY_CAPABILITIES_ADAPTER.decode(decoder);
      return new PrivacyRelay(relayId, endpoint, keyFingerprint, capabilities);
    }
  };

  private static final TypeAdapter<SoranetAccessKind> SORANET_ACCESS_KIND_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, SoranetAccessKind value) {
          UINT8.encode(encoder, (long) value.ordinal());
        }

        @Override
        public SoranetAccessKind decode(NoritoDecoder decoder) {
          long ordinal = UINT8.decode(decoder);
          return switch ((int) ordinal) {
            case 0 -> SoranetAccessKind.READ_ONLY;
            case 1 -> SoranetAccessKind.AUTHENTICATED;
            default ->
                throw new IllegalArgumentException(
                    "unknown SoranetAccessKind ordinal: " + ordinal);
          };
        }
      };

  private static final TypeAdapter<SoranetRoute> SORANET_ROUTE_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<Long>> optionalUint16 = NoritoAdapters.option(UINT16);

        @Override
        public void encode(NoritoEncoder encoder, SoranetRoute value) {
          HASH.encode(encoder, value.channelId());
          STRING.encode(encoder, value.exitMultiaddr());
          Optional<Long> padding =
              value.paddingBudgetMs() == null
                  ? Optional.empty()
                  : Optional.of(value.paddingBudgetMs().longValue());
          optionalUint16.encode(encoder, padding);
          SORANET_ACCESS_KIND_ADAPTER.encode(encoder, value.accessKind());
        }

        @Override
        public SoranetRoute decode(NoritoDecoder decoder) {
          byte[] channelId = HASH.decode(decoder);
          String exitMultiaddr = STRING.decode(decoder);
          Optional<Long> padding = optionalUint16.decode(decoder);
          Integer paddingBudgetMs = padding.map(Long::intValue).orElse(null);
          SoranetAccessKind accessKind = SORANET_ACCESS_KIND_ADAPTER.decode(decoder);
          return new SoranetRoute(channelId, exitMultiaddr, paddingBudgetMs, accessKind);
        }
      };

  private static final TypeAdapter<Optional<SoranetRoute>> OPTIONAL_SORANET_ROUTE =
      NoritoAdapters.option(SORANET_ROUTE_ADAPTER);

  private static final TypeAdapter<PrivacyRoute> PRIVACY_ROUTE_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, PrivacyRoute value) {
      HASH.encode(encoder, value.routeId());
      PRIVACY_RELAY_ADAPTER.encode(encoder, value.entry());
      PRIVACY_RELAY_ADAPTER.encode(encoder, value.exit());
      BYTES.encode(encoder, value.ticketEntry());
      BYTES.encode(encoder, value.ticketExit());
      UINT64.encode(encoder, value.expirySegment());
      OPTIONAL_SORANET_ROUTE.encode(encoder, Optional.ofNullable(value.soranet()));
    }

    @Override
    public PrivacyRoute decode(NoritoDecoder decoder) {
      byte[] routeId = HASH.decode(decoder);
      PrivacyRelay entry = PRIVACY_RELAY_ADAPTER.decode(decoder);
      PrivacyRelay exit = PRIVACY_RELAY_ADAPTER.decode(decoder);
      byte[] ticketEntry = BYTES.decode(decoder);
      byte[] ticketExit = BYTES.decode(decoder);
      long expirySegment = UINT64.decode(decoder);
      SoranetRoute soranet = OPTIONAL_SORANET_ROUTE.decode(decoder).orElse(null);
      return new PrivacyRoute(routeId, entry, exit, ticketEntry, ticketExit, expirySegment, soranet);
    }
  };

  private static final TypeAdapter<NeuralBundle> NEURAL_BUNDLE_ADAPTER = new TypeAdapter<>() {
    private final TypeAdapter<Optional<byte[]>> optionalHash = NoritoAdapters.option(HASH);

    @Override
    public void encode(NoritoEncoder encoder, NeuralBundle value) {
      STRING.encode(encoder, value.bundleId());
      HASH.encode(encoder, value.weightsSha256());
      INT16_LIST.encode(encoder, value.activationScale());
      INT32_LIST.encode(encoder, value.bias());
      SIGNATURE.encode(encoder, value.metadataSignature());
      optionalHash.encode(encoder, value.metalShaderSha256());
      optionalHash.encode(encoder, value.cudaPtxSha256());
    }

    @Override
    public NeuralBundle decode(NoritoDecoder decoder) {
      String bundleId = STRING.decode(decoder);
      byte[] weights = HASH.decode(decoder);
      List<Long> activationScale = INT16_LIST.decode(decoder);
      List<Long> bias = INT32_LIST.decode(decoder);
      byte[] signature = SIGNATURE.decode(decoder);
      Optional<byte[]> metal = optionalHash.decode(decoder);
      Optional<byte[]> cuda = optionalHash.decode(decoder);
      return new NeuralBundle(bundleId, weights, activationScale, bias, signature, metal, cuda);
    }
  };

  private static final TypeAdapter<ChunkDescriptor> CHUNK_DESCRIPTOR_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ChunkDescriptor value) {
      UINT16.encode(encoder, value.chunkId());
      UINT32.encode(encoder, value.offset());
      UINT32.encode(encoder, value.length());
      HASH.encode(encoder, value.commitment());
      BOOL.encode(encoder, value.parity());
    }

    @Override
    public ChunkDescriptor decode(NoritoDecoder decoder) {
      long chunkId = UINT16.decode(decoder);
      long offset = UINT32.decode(decoder);
      long length = UINT32.decode(decoder);
      byte[] commitment = HASH.decode(decoder);
      boolean parity = BOOL.decode(decoder);
      return new ChunkDescriptor(chunkId, offset, length, commitment, parity);
    }
  };

  private static final TypeAdapter<List<ChunkDescriptor>> CHUNK_DESCRIPTOR_LIST_ADAPTER =
      NoritoAdapters.sequence(CHUNK_DESCRIPTOR_ADAPTER);

  private static final TypeAdapter<List<PrivacyRoute>> PRIVACY_ROUTE_LIST_ADAPTER =
      NoritoAdapters.sequence(PRIVACY_ROUTE_ADAPTER);

  private static final TypeAdapter<Optional<NeuralBundle>> OPTIONAL_NEURAL_BUNDLE_ADAPTER =
      NoritoAdapters.option(NEURAL_BUNDLE_ADAPTER);
  private static final TypeAdapter<Optional<Long>> OPTIONAL_UINT64_ADAPTER =
      NoritoAdapters.option(UINT64);

  private static final TypeAdapter<ManifestV1> MANIFEST_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ManifestV1 value) {
      HASH.encode(encoder, value.streamId());
      UINT16.encode(encoder, value.protocolVersion());
      UINT64.encode(encoder, value.segmentNumber());
      UINT64.encode(encoder, value.publishedAt());
      PROFILE_ID_ADAPTER.encode(encoder, value.profile());
      STRING.encode(encoder, value.daEndpoint());
      HASH.encode(encoder, value.chunkRoot());
      UINT64.encode(encoder, value.contentKeyId());
      HASH.encode(encoder, value.nonceSalt());
      CHUNK_DESCRIPTOR_LIST_ADAPTER.encode(encoder, value.chunkDescriptors());
      HASH.encode(encoder, value.transportCapabilitiesHash());
      ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.encryptionSuite());
      FEC_SCHEME_ADAPTER.encode(encoder, value.fecSuite());
      PRIVACY_ROUTE_LIST_ADAPTER.encode(encoder, value.privacyRoutes());
      OPTIONAL_NEURAL_BUNDLE_ADAPTER.encode(encoder, value.neuralBundle());
      STREAM_METADATA_ADAPTER.encode(encoder, value.publicMetadata());
      CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.capabilities());
      SIGNATURE.encode(encoder, value.signature());
    }

    @Override
    public ManifestV1 decode(NoritoDecoder decoder) {
      byte[] streamId = HASH.decode(decoder);
      long protocolVersion = UINT16.decode(decoder);
      long segmentNumber = UINT64.decode(decoder);
      long publishedAt = UINT64.decode(decoder);
      ProfileId profile = PROFILE_ID_ADAPTER.decode(decoder);
      String daEndpoint = STRING.decode(decoder);
      byte[] chunkRoot = HASH.decode(decoder);
      long contentKeyId = UINT64.decode(decoder);
      byte[] nonceSalt = HASH.decode(decoder);
      List<ChunkDescriptor> chunkDescriptors = CHUNK_DESCRIPTOR_LIST_ADAPTER.decode(decoder);
      byte[] transportHash = HASH.decode(decoder);
      EncryptionSuite encryptionSuite = ENCRYPTION_SUITE_ADAPTER.decode(decoder);
      FecScheme fecScheme = FEC_SCHEME_ADAPTER.decode(decoder);
      List<PrivacyRoute> privacyRoutes = PRIVACY_ROUTE_LIST_ADAPTER.decode(decoder);
      Optional<NeuralBundle> neuralBundle = OPTIONAL_NEURAL_BUNDLE_ADAPTER.decode(decoder);
      StreamMetadata publicMetadata = STREAM_METADATA_ADAPTER.decode(decoder);
      CapabilityFlags capabilities = CAPABILITY_FLAGS_ADAPTER.decode(decoder);
      byte[] signature = SIGNATURE.decode(decoder);
      return new ManifestV1(
          streamId,
          protocolVersion,
          segmentNumber,
          publishedAt,
          profile,
          daEndpoint,
          chunkRoot,
          contentKeyId,
          nonceSalt,
          chunkDescriptors,
          transportHash,
          encryptionSuite,
          fecScheme,
          privacyRoutes,
          neuralBundle,
          publicMetadata,
          capabilities,
          signature);
    }
  };

  private static final TypeAdapter<ManifestAnnounceFrame> MANIFEST_ANNOUNCE_FRAME_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, ManifestAnnounceFrame value) {
          MANIFEST_ADAPTER.encode(encoder, value.manifest());
        }

        @Override
        public ManifestAnnounceFrame decode(NoritoDecoder decoder) {
          return new ManifestAnnounceFrame(MANIFEST_ADAPTER.decode(decoder));
        }
      };

  private static final TypeAdapter<ChunkRequestFrame> CHUNK_REQUEST_FRAME_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, ChunkRequestFrame value) {
          UINT64.encode(encoder, value.segment());
          UINT16.encode(encoder, value.chunkId());
        }

        @Override
        public ChunkRequestFrame decode(NoritoDecoder decoder) {
          long segment = UINT64.decode(decoder);
          long chunkId = UINT16.decode(decoder);
          return new ChunkRequestFrame(segment, chunkId);
        }
      };

  private static final TypeAdapter<ChunkAcknowledgeFrame> CHUNK_ACK_FRAME_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, ChunkAcknowledgeFrame value) {
          UINT64.encode(encoder, value.segment());
          UINT16.encode(encoder, value.chunkId());
        }

        @Override
        public ChunkAcknowledgeFrame decode(NoritoDecoder decoder) {
          long segment = UINT64.decode(decoder);
          long chunkId = UINT16.decode(decoder);
          return new ChunkAcknowledgeFrame(segment, chunkId);
        }
      };

  private static final TypeAdapter<TransportCapabilitiesFrame> TRANSPORT_CAPABILITIES_FRAME_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, TransportCapabilitiesFrame value) {
          CAPABILITY_ROLE_ADAPTER.encode(encoder, value.endpointRole());
          TRANSPORT_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities());
        }

        @Override
        public TransportCapabilitiesFrame decode(NoritoDecoder decoder) {
          CapabilityRole role = CAPABILITY_ROLE_ADAPTER.decode(decoder);
          TransportCapabilities capabilities = TRANSPORT_CAPABILITIES_ADAPTER.decode(decoder);
          return new TransportCapabilitiesFrame(role, capabilities);
        }
      };

  private static final TypeAdapter<CapabilityReport> CAPABILITY_REPORT_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, CapabilityReport value) {
      HASH.encode(encoder, value.streamId());
      CAPABILITY_ROLE_ADAPTER.encode(encoder, value.endpointRole());
      UINT16.encode(encoder, value.protocolVersion());
      RESOLUTION_ADAPTER.encode(encoder, value.maxResolution());
      BOOL.encode(encoder, value.hdrSupported());
      BOOL.encode(encoder, value.captureHdr());
      STRING_LIST.encode(encoder, value.neuralBundles());
      AUDIO_CAPABILITY_ADAPTER.encode(encoder, value.audioCaps());
      CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.featureBits());
      UINT16.encode(encoder, value.maxDatagramSize());
      BOOL.encode(encoder, value.dplpmtud());
    }

    @Override
    public CapabilityReport decode(NoritoDecoder decoder) {
      byte[] streamId = HASH.decode(decoder);
      CapabilityRole role = CAPABILITY_ROLE_ADAPTER.decode(decoder);
      long protocolVersion = UINT16.decode(decoder);
      Resolution resolution = RESOLUTION_ADAPTER.decode(decoder);
      boolean hdrSupported = BOOL.decode(decoder);
      boolean captureHdr = BOOL.decode(decoder);
      List<String> neuralBundles = STRING_LIST.decode(decoder);
      AudioCapability audioCaps = AUDIO_CAPABILITY_ADAPTER.decode(decoder);
      CapabilityFlags featureBits = CAPABILITY_FLAGS_ADAPTER.decode(decoder);
      long maxDatagramSize = UINT16.decode(decoder);
      boolean dplpmtud = BOOL.decode(decoder);
      return new CapabilityReport(
          streamId,
          role,
          protocolVersion,
          resolution,
          hdrSupported,
          captureHdr,
          neuralBundles,
          audioCaps,
          featureBits,
          maxDatagramSize,
          dplpmtud);
    }
  };

  private static final TypeAdapter<CapabilityAck> CAPABILITY_ACK_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, CapabilityAck value) {
      HASH.encode(encoder, value.streamId());
      UINT16.encode(encoder, value.acceptedVersion());
      CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.negotiatedFeatures());
      UINT16.encode(encoder, value.maxDatagramSize());
      BOOL.encode(encoder, value.dplpmtud());
    }

    @Override
    public CapabilityAck decode(NoritoDecoder decoder) {
      byte[] streamId = HASH.decode(decoder);
      long acceptedVersion = UINT16.decode(decoder);
      CapabilityFlags negotiatedFeatures = CAPABILITY_FLAGS_ADAPTER.decode(decoder);
      long maxDatagramSize = UINT16.decode(decoder);
      boolean dplpmtud = BOOL.decode(decoder);
      return new CapabilityAck(streamId, acceptedVersion, negotiatedFeatures, maxDatagramSize, dplpmtud);
    }
  };

  private static final TypeAdapter<AudioCapability> AUDIO_CAPABILITY_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, AudioCapability value) {
      UINT32_LIST.encode(encoder, value.sampleRates());
      BOOL.encode(encoder, value.ambisonics());
      UINT8.encode(encoder, value.maxChannels());
    }

    @Override
    public AudioCapability decode(NoritoDecoder decoder) {
      List<Long> sampleRates = UINT32_LIST.decode(decoder);
      boolean ambisonics = BOOL.decode(decoder);
      long maxChannels = UINT8.decode(decoder);
      return new AudioCapability(sampleRates, ambisonics, maxChannels);
    }
  };

  private static final TypeAdapter<AudioFrame> AUDIO_FRAME_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, AudioFrame value) {
      UINT64.encode(encoder, value.sequence());
      UINT64.encode(encoder, value.timestampNs());
      UINT8.encode(encoder, value.fecLevel());
      AUDIO_LAYOUT_ADAPTER.encode(encoder, value.channelLayout());
      BYTES.encode(encoder, value.payload());
    }

    @Override
    public AudioFrame decode(NoritoDecoder decoder) {
      long sequence = UINT64.decode(decoder);
      long timestampNs = UINT64.decode(decoder);
      long fecLevel = UINT8.decode(decoder);
      AudioLayout layout = AUDIO_LAYOUT_ADAPTER.decode(decoder);
      byte[] payload = BYTES.decode(decoder);
      return new AudioFrame(sequence, timestampNs, fecLevel, layout, payload);
    }
  };

  private static final TypeAdapter<FeedbackHintFrame> FEEDBACK_HINT_FRAME_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, FeedbackHintFrame value) {
      HASH.encode(encoder, value.streamId());
      UINT32.encode(encoder, value.lossEwmaQ16());
      SINT32.encode(encoder, value.latencyGradientQ16());
      UINT16.encode(encoder, value.observedRttMs());
      UINT16.encode(encoder, value.reportIntervalMs());
      UINT8.encode(encoder, value.parityChunks());
    }

    @Override
    public FeedbackHintFrame decode(NoritoDecoder decoder) {
      byte[] streamId = HASH.decode(decoder);
      long lossEwma = UINT32.decode(decoder);
      long latencyGradient = SINT32.decode(decoder);
      long observedRtt = UINT16.decode(decoder);
      long reportInterval = UINT16.decode(decoder);
      long parityChunks = UINT8.decode(decoder);
      return new FeedbackHintFrame(streamId, lossEwma, latencyGradient, observedRtt, reportInterval, parityChunks);
    }
  };

  private static final TypeAdapter<SyncDiagnostics> SYNC_DIAGNOSTICS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, SyncDiagnostics value) {
      UINT16.encode(encoder, value.windowMs());
      UINT16.encode(encoder, value.samples());
      UINT16.encode(encoder, value.avgAudioJitterMs());
      UINT16.encode(encoder, value.maxAudioJitterMs());
      SINT16.encode(encoder, value.avgAvDriftMs());
      UINT16.encode(encoder, value.maxAvDriftMs());
      SINT16.encode(encoder, value.ewmaAvDriftMs());
      UINT16.encode(encoder, value.violationCount());
    }

    @Override
    public SyncDiagnostics decode(NoritoDecoder decoder) {
      long windowMs = UINT16.decode(decoder);
      long samples = UINT16.decode(decoder);
      long avgAudioJitter = UINT16.decode(decoder);
      long maxAudioJitter = UINT16.decode(decoder);
      long avgAvDrift = SINT16.decode(decoder);
      long maxAvDrift = UINT16.decode(decoder);
      long ewmaAvDrift = SINT16.decode(decoder);
      long violationCount = UINT16.decode(decoder);
      return new SyncDiagnostics(
          windowMs,
          samples,
          avgAudioJitter,
          maxAudioJitter,
          avgAvDrift,
          maxAvDrift,
          ewmaAvDrift,
          violationCount);
    }
  };

  private static final TypeAdapter<Optional<SyncDiagnostics>> OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER =
      NoritoAdapters.option(SYNC_DIAGNOSTICS_ADAPTER);

  private static final TypeAdapter<ReceiverReport> RECEIVER_REPORT_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ReceiverReport value) {
      HASH.encode(encoder, value.streamId());
      UINT64.encode(encoder, value.latestSegment());
      UINT32.encode(encoder, value.layerMask());
      UINT32.encode(encoder, value.measuredThroughputKbps());
      UINT16.encode(encoder, value.rttMs());
      UINT16.encode(encoder, value.lossPercentX100());
      UINT16.encode(encoder, value.decoderBufferMs());
      RESOLUTION_ADAPTER.encode(encoder, value.activeResolution());
      BOOL.encode(encoder, value.hdrActive());
      UINT32.encode(encoder, value.ecnCeCount());
      UINT16.encode(encoder, value.jitterMs());
      UINT64.encode(encoder, value.deliveredSequence());
      UINT8.encode(encoder, value.parityApplied());
      UINT8.encode(encoder, value.fecBudget());
      OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER.encode(encoder, value.syncDiagnostics());
    }

    @Override
    public ReceiverReport decode(NoritoDecoder decoder) {
      byte[] streamId = HASH.decode(decoder);
      long latestSegment = UINT64.decode(decoder);
      long layerMask = UINT32.decode(decoder);
      long throughput = UINT32.decode(decoder);
      long rtt = UINT16.decode(decoder);
      long loss = UINT16.decode(decoder);
      long buffer = UINT16.decode(decoder);
      Resolution resolution = RESOLUTION_ADAPTER.decode(decoder);
      boolean hdrActive = BOOL.decode(decoder);
      long ecnCe = UINT32.decode(decoder);
      long jitter = UINT16.decode(decoder);
      long delivered = UINT64.decode(decoder);
      long parityApplied = UINT8.decode(decoder);
      long fecBudget = UINT8.decode(decoder);
      Optional<SyncDiagnostics> syncDiagnostics = OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER.decode(decoder);
      return new ReceiverReport(
          streamId,
          latestSegment,
          layerMask,
          throughput,
          rtt,
          loss,
          buffer,
          resolution,
          hdrActive,
          ecnCe,
          jitter,
          delivered,
          parityApplied,
          fecBudget,
          syncDiagnostics);
    }
  };

  private static final TypeAdapter<TelemetryEncodeStats> TELEMETRY_ENCODE_STATS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, TelemetryEncodeStats value) {
      UINT64.encode(encoder, value.segment());
      UINT16.encode(encoder, value.avgLatencyMs());
      UINT32.encode(encoder, value.droppedLayers());
      UINT16.encode(encoder, value.avgAudioJitterMs());
      UINT16.encode(encoder, value.maxAudioJitterMs());
    }

    @Override
    public TelemetryEncodeStats decode(NoritoDecoder decoder) {
      long segment = UINT64.decode(decoder);
      long latency = UINT16.decode(decoder);
      long dropped = UINT32.decode(decoder);
      long avgJitter = UINT16.decode(decoder);
      long maxJitter = UINT16.decode(decoder);
      return new TelemetryEncodeStats(segment, latency, dropped, avgJitter, maxJitter);
    }
  };

  private static final TypeAdapter<TelemetryDecodeStats> TELEMETRY_DECODE_STATS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, TelemetryDecodeStats value) {
      UINT64.encode(encoder, value.segment());
      UINT16.encode(encoder, value.bufferMs());
      UINT16.encode(encoder, value.droppedFrames());
      UINT16.encode(encoder, value.maxDecodeQueueMs());
      SINT16.encode(encoder, value.avgAvDriftMs());
      UINT16.encode(encoder, value.maxAvDriftMs());
    }

    @Override
    public TelemetryDecodeStats decode(NoritoDecoder decoder) {
      long segment = UINT64.decode(decoder);
      long bufferMs = UINT16.decode(decoder);
      long droppedFrames = UINT16.decode(decoder);
      long maxQueue = UINT16.decode(decoder);
      long avgDrift = SINT16.decode(decoder);
      long maxDrift = UINT16.decode(decoder);
      return new TelemetryDecodeStats(segment, bufferMs, droppedFrames, maxQueue, avgDrift, maxDrift);
    }
  };

  private static final TypeAdapter<TelemetryNetworkStats> TELEMETRY_NETWORK_STATS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, TelemetryNetworkStats value) {
      UINT16.encode(encoder, value.rttMs());
      UINT16.encode(encoder, value.lossPercentX100());
      UINT32.encode(encoder, value.fecRepairs());
      UINT32.encode(encoder, value.fecFailures());
      UINT32.encode(encoder, value.datagramReinjects());
    }

    @Override
    public TelemetryNetworkStats decode(NoritoDecoder decoder) {
      long rtt = UINT16.decode(decoder);
      long loss = UINT16.decode(decoder);
      long repairs = UINT32.decode(decoder);
      long failures = UINT32.decode(decoder);
      long reinjects = UINT32.decode(decoder);
      return new TelemetryNetworkStats(rtt, loss, repairs, failures, reinjects);
    }
  };

  private static final TypeAdapter<TelemetrySecurityStats> TELEMETRY_SECURITY_STATS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, TelemetrySecurityStats value) {
          ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.suite());
          UINT32.encode(encoder, value.rekeys());
          UINT32.encode(encoder, value.gckRotations());
          OPTIONAL_UINT64_ADAPTER.encode(encoder, value.lastContentKeyId());
          OPTIONAL_UINT64_ADAPTER.encode(encoder, value.lastContentKeyValidFrom());
        }

        @Override
        public TelemetrySecurityStats decode(NoritoDecoder decoder) {
          EncryptionSuite suite = ENCRYPTION_SUITE_ADAPTER.decode(decoder);
          long rekeys = UINT32.decode(decoder);
          long rotations = UINT32.decode(decoder);
          Optional<Long> lastId = OPTIONAL_UINT64_ADAPTER.decode(decoder);
          Optional<Long> lastValidFrom = OPTIONAL_UINT64_ADAPTER.decode(decoder);
          return new TelemetrySecurityStats(suite, rekeys, rotations, lastId, lastValidFrom);
        }
      };

  private static final TypeAdapter<TelemetryEnergyStats> TELEMETRY_ENERGY_STATS_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, TelemetryEnergyStats value) {
      UINT64.encode(encoder, value.segment());
      UINT32.encode(encoder, value.encoderMilliwatts());
      UINT32.encode(encoder, value.decoderMilliwatts());
    }

    @Override
    public TelemetryEnergyStats decode(NoritoDecoder decoder) {
      long segment = UINT64.decode(decoder);
      long encoderMw = UINT32.decode(decoder);
      long decoderMw = UINT32.decode(decoder);
      return new TelemetryEnergyStats(segment, encoderMw, decoderMw);
    }
  };

  private static final TypeAdapter<TelemetryAuditOutcome> TELEMETRY_AUDIT_OUTCOME_ADAPTER =
      new TypeAdapter<>() {
        private final TypeAdapter<Optional<String>> optionalString = NoritoAdapters.option(STRING);

        @Override
        public void encode(NoritoEncoder encoder, TelemetryAuditOutcome value) {
          STRING.encode(encoder, value.traceId());
          UINT64.encode(encoder, value.slotHeight());
          STRING.encode(encoder, value.reviewer());
          STRING.encode(encoder, value.status());
          optionalString.encode(encoder, value.mitigationUrl());
        }

        @Override
        public TelemetryAuditOutcome decode(NoritoDecoder decoder) {
          String traceId = STRING.decode(decoder);
          long slotHeight = UINT64.decode(decoder);
          String reviewer = STRING.decode(decoder);
          String status = STRING.decode(decoder);
          Optional<String> mitigationUrl = optionalString.decode(decoder);
          return new TelemetryAuditOutcome(traceId, slotHeight, reviewer, status, mitigationUrl);
        }
      };

  private static final TypeAdapter<TelemetryEvent> TELEMETRY_EVENT_ADAPTER = new TypeAdapter<>() {
    private final List<TelemetryEntry<?>> entries = List.of(
        new TelemetryEntry<>(
            TelemetryEncodeEvent.class,
            TelemetryEncodeEvent::stats,
            payload -> new TelemetryEncodeEvent((TelemetryEncodeStats) payload),
            TELEMETRY_ENCODE_STATS_ADAPTER),
        new TelemetryEntry<>(
            TelemetryDecodeEvent.class,
            TelemetryDecodeEvent::stats,
            payload -> new TelemetryDecodeEvent((TelemetryDecodeStats) payload),
            TELEMETRY_DECODE_STATS_ADAPTER),
        new TelemetryEntry<>(
            TelemetryNetworkEvent.class,
            TelemetryNetworkEvent::stats,
            payload -> new TelemetryNetworkEvent((TelemetryNetworkStats) payload),
            TELEMETRY_NETWORK_STATS_ADAPTER),
        new TelemetryEntry<>(
            TelemetrySecurityEvent.class,
            TelemetrySecurityEvent::stats,
            payload -> new TelemetrySecurityEvent((TelemetrySecurityStats) payload),
            TELEMETRY_SECURITY_STATS_ADAPTER),
        new TelemetryEntry<>(
            TelemetryEnergyEvent.class,
            TelemetryEnergyEvent::stats,
            payload -> new TelemetryEnergyEvent((TelemetryEnergyStats) payload),
            TELEMETRY_ENERGY_STATS_ADAPTER),
        new TelemetryEntry<>(
            TelemetryAuditOutcomeEvent.class,
            TelemetryAuditOutcomeEvent::stats,
            payload -> new TelemetryAuditOutcomeEvent((TelemetryAuditOutcome) payload),
            TELEMETRY_AUDIT_OUTCOME_ADAPTER));

    @Override
    public void encode(NoritoEncoder encoder, TelemetryEvent value) {
      for (int i = 0; i < entries.size(); i++) {
        TelemetryEntry<?> entry = entries.get(i);
        if (entry.type().isInstance(value)) {
          encoder.writeUInt(i, 32);
          entry.encode(encoder, value);
          return;
        }
      }
      throw new IllegalArgumentException("Unsupported TelemetryEvent: " + value.getClass());
    }

    @Override
    public TelemetryEvent decode(NoritoDecoder decoder) {
      long tag = decoder.readUInt(32);
      if (tag < 0 || tag >= entries.size()) {
        throw new IllegalArgumentException("Invalid TelemetryEvent discriminant: " + tag);
      }
      return entries.get((int) tag).decode(decoder);
    }

    @Override
    public int fixedSize() {
      return -1;
    }
  };

  private record TelemetryEntry<T extends TelemetryEvent>(
      Class<T> type,
      Function<T, Object> extractor,
      Function<Object, T> factory,
      TypeAdapter<?> adapter) {
    void encode(NoritoEncoder encoder, TelemetryEvent value) {
      T cast = type.cast(value);
      Object payload = extractor.apply(cast);
      @SuppressWarnings("unchecked")
      TypeAdapter<Object> typedAdapter = (TypeAdapter<Object>) adapter;
      typedAdapter.encode(encoder, payload);
    }

    T decode(NoritoDecoder decoder) {
      @SuppressWarnings("unchecked")
      TypeAdapter<Object> typedAdapter = (TypeAdapter<Object>) adapter;
      Object payload = typedAdapter.decode(decoder);
      return factory.apply(payload);
    }
  }

  private static final TypeAdapter<KeyUpdate> KEY_UPDATE_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, KeyUpdate value) {
      HASH.encode(encoder, value.sessionId());
      ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.suite());
      UINT16.encode(encoder, value.protocolVersion());
      BYTES.encode(encoder, value.pubEphemeral());
      UINT64.encode(encoder, value.keyCounter());
      SIGNATURE.encode(encoder, value.signature());
    }

    @Override
    public KeyUpdate decode(NoritoDecoder decoder) {
      byte[] sessionId = HASH.decode(decoder);
      EncryptionSuite suite = ENCRYPTION_SUITE_ADAPTER.decode(decoder);
      long protocolVersion = UINT16.decode(decoder);
      byte[] pubEphemeral = BYTES.decode(decoder);
      long keyCounter = UINT64.decode(decoder);
      byte[] signature = SIGNATURE.decode(decoder);
      return new KeyUpdate(sessionId, suite, protocolVersion, pubEphemeral, keyCounter, signature);
    }
  };

  private static final TypeAdapter<ContentKeyUpdate> CONTENT_KEY_UPDATE_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ContentKeyUpdate value) {
      UINT64.encode(encoder, value.contentKeyId());
      BYTES.encode(encoder, value.gckWrapped());
      UINT64.encode(encoder, value.validFromSegment());
    }

    @Override
    public ContentKeyUpdate decode(NoritoDecoder decoder) {
      long contentKeyId = UINT64.decode(decoder);
      byte[] gckWrapped = BYTES.decode(decoder);
      long validFrom = UINT64.decode(decoder);
      return new ContentKeyUpdate(contentKeyId, gckWrapped, validFrom);
    }
  };

  private static final TypeAdapter<PrivacyRouteUpdate> PRIVACY_ROUTE_UPDATE_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, PrivacyRouteUpdate value) {
      HASH.encode(encoder, value.routeId());
      HASH.encode(encoder, value.streamId());
      UINT64.encode(encoder, value.contentKeyId());
      UINT64.encode(encoder, value.validFromSegment());
      UINT64.encode(encoder, value.validUntilSegment());
      BYTES.encode(encoder, value.exitToken());
      OPTIONAL_SORANET_ROUTE.encode(encoder, Optional.ofNullable(value.soranet()));
    }

    @Override
    public PrivacyRouteUpdate decode(NoritoDecoder decoder) {
      byte[] routeId = HASH.decode(decoder);
      byte[] streamId = HASH.decode(decoder);
      long contentKeyId = UINT64.decode(decoder);
      long validFrom = UINT64.decode(decoder);
      long validUntil = UINT64.decode(decoder);
      byte[] exitToken = BYTES.decode(decoder);
      SoranetRoute soranet = OPTIONAL_SORANET_ROUTE.decode(decoder).orElse(null);
      return new PrivacyRouteUpdate(routeId, streamId, contentKeyId, validFrom, validUntil, exitToken, soranet);
    }
  };

  private static final TypeAdapter<PrivacyRouteAckFrame> PRIVACY_ROUTE_ACK_FRAME_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(NoritoEncoder encoder, PrivacyRouteAckFrame value) {
          HASH.encode(encoder, value.routeId());
        }

        @Override
        public PrivacyRouteAckFrame decode(NoritoDecoder decoder) {
          return new PrivacyRouteAckFrame(HASH.decode(decoder));
        }
      };

  private static final TypeAdapter<ControlErrorFrame> CONTROL_ERROR_FRAME_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ControlErrorFrame value) {
      ERROR_CODE_ADAPTER.encode(encoder, value.code());
      STRING.encode(encoder, value.message());
    }

    @Override
    public ControlErrorFrame decode(NoritoDecoder decoder) {
      ErrorCode code = ERROR_CODE_ADAPTER.decode(decoder);
      String message = STRING.decode(decoder);
      return new ControlErrorFrame(code, message);
    }
  };

  private record ControlFrameEntry(
      ControlFrameVariant variant,
      TypeAdapter<? extends ControlFramePayload> adapter,
      Class<? extends ControlFramePayload> payloadClass) {}

  private static final List<ControlFrameEntry> CONTROL_FRAME_ENTRIES = List.of(
      new ControlFrameEntry(ControlFrameVariant.MANIFEST_ANNOUNCE, MANIFEST_ANNOUNCE_FRAME_ADAPTER, ManifestAnnounceFrame.class),
      new ControlFrameEntry(ControlFrameVariant.CHUNK_REQUEST, CHUNK_REQUEST_FRAME_ADAPTER, ChunkRequestFrame.class),
      new ControlFrameEntry(ControlFrameVariant.CHUNK_ACKNOWLEDGE, CHUNK_ACK_FRAME_ADAPTER, ChunkAcknowledgeFrame.class),
      new ControlFrameEntry(ControlFrameVariant.TRANSPORT_CAPABILITIES, TRANSPORT_CAPABILITIES_FRAME_ADAPTER, TransportCapabilitiesFrame.class),
      new ControlFrameEntry(ControlFrameVariant.CAPABILITY_REPORT, CAPABILITY_REPORT_ADAPTER, CapabilityReport.class),
      new ControlFrameEntry(ControlFrameVariant.CAPABILITY_ACK, CAPABILITY_ACK_ADAPTER, CapabilityAck.class),
      new ControlFrameEntry(ControlFrameVariant.FEEDBACK_HINT, FEEDBACK_HINT_FRAME_ADAPTER, FeedbackHintFrame.class),
      new ControlFrameEntry(ControlFrameVariant.RECEIVER_REPORT, RECEIVER_REPORT_ADAPTER, ReceiverReport.class),
      new ControlFrameEntry(ControlFrameVariant.KEY_UPDATE, KEY_UPDATE_ADAPTER, KeyUpdate.class),
      new ControlFrameEntry(ControlFrameVariant.CONTENT_KEY_UPDATE, CONTENT_KEY_UPDATE_ADAPTER, ContentKeyUpdate.class),
      new ControlFrameEntry(ControlFrameVariant.PRIVACY_ROUTE_UPDATE, PRIVACY_ROUTE_UPDATE_ADAPTER, PrivacyRouteUpdate.class),
      new ControlFrameEntry(ControlFrameVariant.PRIVACY_ROUTE_ACK, PRIVACY_ROUTE_ACK_FRAME_ADAPTER, PrivacyRouteAckFrame.class),
      new ControlFrameEntry(ControlFrameVariant.ERROR, CONTROL_ERROR_FRAME_ADAPTER, ControlErrorFrame.class));

  private static final TypeAdapter<ControlFrame> CONTROL_FRAME_ADAPTER = new TypeAdapter<>() {
    @Override
    public void encode(NoritoEncoder encoder, ControlFrame value) {
      for (int i = 0; i < CONTROL_FRAME_ENTRIES.size(); i++) {
        ControlFrameEntry entry = CONTROL_FRAME_ENTRIES.get(i);
        if (entry.variant() == value.variant()) {
          if (!entry.payloadClass().isInstance(value.payload())) {
            throw new IllegalArgumentException(
                "Variant " + value.variant() + " expects payload " + entry.payloadClass().getSimpleName());
          }
          encoder.writeUInt(i, 32);
          @SuppressWarnings("unchecked")
          TypeAdapter<ControlFramePayload> adapter = (TypeAdapter<ControlFramePayload>) entry.adapter();
          adapter.encode(encoder, value.payload());
          return;
        }
      }
      throw new IllegalArgumentException("Unsupported ControlFrame variant: " + value.variant());
    }

    @Override
    public ControlFrame decode(NoritoDecoder decoder) {
      long tag = decoder.readUInt(32);
      if (tag < 0 || tag >= CONTROL_FRAME_ENTRIES.size()) {
        throw new IllegalArgumentException("Invalid ControlFrame discriminant: " + tag);
      }
      ControlFrameEntry entry = CONTROL_FRAME_ENTRIES.get((int) tag);
      @SuppressWarnings("unchecked")
      TypeAdapter<ControlFramePayload> adapter = (TypeAdapter<ControlFramePayload>) entry.adapter();
      ControlFramePayload payload = adapter.decode(decoder);
      return new ControlFrame(entry.variant(), payload);
    }
  };

  public static TypeAdapter<byte[]> hashAdapter() {
    return HASH;
  }

  public static TypeAdapter<byte[]> signatureAdapter() {
    return SIGNATURE;
  }

  public static TypeAdapter<byte[]> bytesAdapter() {
    return BYTES;
  }

  public static TypeAdapter<ProfileId> profileIdAdapter() {
    return PROFILE_ID_ADAPTER;
  }

  public static TypeAdapter<CapabilityFlags> capabilityFlagsAdapter() {
    return CAPABILITY_FLAGS_ADAPTER;
  }

  public static TypeAdapter<TicketCapabilities> ticketCapabilitiesAdapter() {
    return TICKET_CAPABILITIES_ADAPTER;
  }

  public static TypeAdapter<TicketPolicy> ticketPolicyAdapter() {
    return TICKET_POLICY_ADAPTER;
  }

  public static TypeAdapter<StreamingTicket> streamingTicketAdapter() {
    return STREAMING_TICKET_ADAPTER;
  }

  public static TypeAdapter<TicketRevocation> ticketRevocationAdapter() {
    return TICKET_REVOCATION_ADAPTER;
  }

  public static TypeAdapter<PrivacyCapabilities> privacyCapabilitiesAdapter() {
    return PRIVACY_CAPABILITIES_ADAPTER;
  }

  public static TypeAdapter<HpkeSuiteMask> hpkeSuiteMaskAdapter() {
    return HPKE_SUITE_MASK_ADAPTER;
  }

  public static TypeAdapter<EncryptionSuite> encryptionSuiteAdapter() {
    return ENCRYPTION_SUITE_ADAPTER;
  }

  public static TypeAdapter<TransportCapabilities> transportCapabilitiesAdapter() {
    return TRANSPORT_CAPABILITIES_ADAPTER;
  }

  public static TypeAdapter<StreamMetadata> streamMetadataAdapter() {
    return STREAM_METADATA_ADAPTER;
  }

  public static TypeAdapter<PrivacyRelay> privacyRelayAdapter() {
    return PRIVACY_RELAY_ADAPTER;
  }

  public static TypeAdapter<PrivacyRoute> privacyRouteAdapter() {
    return PRIVACY_ROUTE_ADAPTER;
  }

  public static TypeAdapter<NeuralBundle> neuralBundleAdapter() {
    return NEURAL_BUNDLE_ADAPTER;
  }

  public static TypeAdapter<ChunkDescriptor> chunkDescriptorAdapter() {
    return CHUNK_DESCRIPTOR_ADAPTER;
  }

  public static TypeAdapter<ManifestV1> manifestAdapter() {
    return MANIFEST_ADAPTER;
  }

  public static TypeAdapter<ManifestAnnounceFrame> manifestAnnounceFrameAdapter() {
    return MANIFEST_ANNOUNCE_FRAME_ADAPTER;
  }

  public static TypeAdapter<ChunkRequestFrame> chunkRequestFrameAdapter() {
    return CHUNK_REQUEST_FRAME_ADAPTER;
  }

  public static TypeAdapter<ChunkAcknowledgeFrame> chunkAcknowledgeFrameAdapter() {
    return CHUNK_ACK_FRAME_ADAPTER;
  }

  public static TypeAdapter<TransportCapabilitiesFrame> transportCapabilitiesFrameAdapter() {
    return TRANSPORT_CAPABILITIES_FRAME_ADAPTER;
  }

  public static TypeAdapter<CapabilityReport> capabilityReportAdapter() {
    return CAPABILITY_REPORT_ADAPTER;
  }

  public static TypeAdapter<CapabilityAck> capabilityAckAdapter() {
    return CAPABILITY_ACK_ADAPTER;
  }

  public static TypeAdapter<AudioCapability> audioCapabilityAdapter() {
    return AUDIO_CAPABILITY_ADAPTER;
  }

  public static TypeAdapter<AudioFrame> audioFrameAdapter() {
    return AUDIO_FRAME_ADAPTER;
  }

  public static TypeAdapter<Resolution> resolutionAdapter() {
    return RESOLUTION_ADAPTER;
  }

  public static TypeAdapter<ResolutionCustom> resolutionCustomAdapter() {
    return RESOLUTION_CUSTOM_ADAPTER;
  }

  public static TypeAdapter<FeedbackHintFrame> feedbackHintFrameAdapter() {
    return FEEDBACK_HINT_FRAME_ADAPTER;
  }

  public static TypeAdapter<ReceiverReport> receiverReportAdapter() {
    return RECEIVER_REPORT_ADAPTER;
  }

  public static TypeAdapter<TelemetryEncodeStats> telemetryEncodeStatsAdapter() {
    return TELEMETRY_ENCODE_STATS_ADAPTER;
  }

  public static TypeAdapter<TelemetryDecodeStats> telemetryDecodeStatsAdapter() {
    return TELEMETRY_DECODE_STATS_ADAPTER;
  }

  public static TypeAdapter<TelemetryNetworkStats> telemetryNetworkStatsAdapter() {
    return TELEMETRY_NETWORK_STATS_ADAPTER;
  }

  public static TypeAdapter<TelemetrySecurityStats> telemetrySecurityStatsAdapter() {
    return TELEMETRY_SECURITY_STATS_ADAPTER;
  }

  public static TypeAdapter<TelemetryEnergyStats> telemetryEnergyStatsAdapter() {
    return TELEMETRY_ENERGY_STATS_ADAPTER;
  }

  public static TypeAdapter<TelemetryEvent> telemetryEventAdapter() {
    return TELEMETRY_EVENT_ADAPTER;
  }

  public static TypeAdapter<KeyUpdate> keyUpdateAdapter() {
    return KEY_UPDATE_ADAPTER;
  }

  public static TypeAdapter<ContentKeyUpdate> contentKeyUpdateAdapter() {
    return CONTENT_KEY_UPDATE_ADAPTER;
  }

  public static TypeAdapter<PrivacyRouteUpdate> privacyRouteUpdateAdapter() {
    return PRIVACY_ROUTE_UPDATE_ADAPTER;
  }

  public static TypeAdapter<PrivacyRouteAckFrame> privacyRouteAckFrameAdapter() {
    return PRIVACY_ROUTE_ACK_FRAME_ADAPTER;
  }

  public static TypeAdapter<ControlErrorFrame> controlErrorFrameAdapter() {
    return CONTROL_ERROR_FRAME_ADAPTER;
  }

  public static TypeAdapter<ControlFrame> controlFrameAdapter() {
    return CONTROL_FRAME_ADAPTER;
  }
}
