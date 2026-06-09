package org.hyperledger.iroha.android.offline;

import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Helpers for embedding native Kagemusha instruction archives in signed transactions. */
public final class KagemushaInstructionArchives {

  private KagemushaInstructionArchives() {}

  public enum InstructionType {
    TRANSFER(
        "KagemushaTransfer",
        "iroha_data_model::isi::offline::KagemushaTransfer"),
    REDEEM_RECURSIVE(
        "RedeemKagemushaRecursive",
        "iroha_data_model::isi::offline::RedeemKagemushaRecursive");

    private final String archiveTypeName;
    private final String wireName;

    InstructionType(final String archiveTypeName, final String wireName) {
      this.archiveTypeName = archiveTypeName;
      this.wireName = wireName;
    }

    public String archiveTypeName() {
      return archiveTypeName;
    }

    public String wireName() {
      return wireName;
    }
  }

  public static InstructionBox instructionBox(
      final InstructionType instructionType, final byte[] instructionArchive) {
    final byte[] archive = copyAndValidateInstructionArchive(instructionType, instructionArchive);
    return InstructionBox.fromWirePayload(instructionType.wireName(), archive);
  }

  public static InstructionBox recursiveRedeemInstructionBox(final byte[] instructionArchive) {
    return instructionBox(InstructionType.REDEEM_RECURSIVE, instructionArchive);
  }

  public static InstructionBox recursiveRedeemInstructionBoxFromRequest(
      final byte[] redeemRequestArchive) {
    return recursiveRedeemInstructionBox(
        KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive));
  }

  public static TransactionPayload transactionPayload(
      final InstructionType instructionType,
      final byte[] instructionArchive,
      final String chainId,
      final String authority,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Integer nonce,
      final Map<String, String> metadata) {
    return TransactionPayload.builder()
        .setChainId(chainId)
        .setAuthority(authority)
        .setCreationTimeMs(creationTimeMs)
        .setExecutable(Executable.instructions(List.of(instructionBox(instructionType, instructionArchive))))
        .setTimeToLiveMs(timeToLiveMs)
        .setNonce(nonce)
        .setMetadata(metadata)
        .build();
  }

  public static TransactionPayload recursiveRedeemTransactionPayload(
      final byte[] instructionArchive,
      final String chainId,
      final String authority,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Integer nonce,
      final Map<String, String> metadata) {
    return transactionPayload(
        InstructionType.REDEEM_RECURSIVE,
        instructionArchive,
        chainId,
        authority,
        creationTimeMs,
        timeToLiveMs,
        nonce,
        metadata);
  }

  public static TransactionPayload recursiveRedeemTransactionPayloadFromRequest(
      final byte[] redeemRequestArchive,
      final String chainId,
      final String authority,
      final long creationTimeMs,
      final Long timeToLiveMs,
      final Integer nonce,
      final Map<String, String> metadata) {
    return recursiveRedeemTransactionPayload(
        KagemushaRecursiveSpendProver.redeemSpend(redeemRequestArchive),
        chainId,
        authority,
        creationTimeMs,
        timeToLiveMs,
        nonce,
        metadata);
  }


  private static byte[] copyAndValidateInstructionArchive(
      final InstructionType instructionType, final byte[] instructionArchive) {
    Objects.requireNonNull(instructionType, "instructionType");
    Objects.requireNonNull(instructionArchive, "instructionArchive");
    if (instructionArchive.length == 0) {
      throw new IllegalArgumentException("Kagemusha instruction archive must not be empty.");
    }
    if (instructionArchive.length > KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalArgumentException(
          "Kagemusha instruction archive must not exceed "
              + KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES
              + " bytes.");
    }

    final byte[] archive = instructionArchive.clone();
    final NoritoHeader.DecodeResult decoded;
    try {
      decoded = NoritoHeader.decode(archive, SchemaHash.hash16(instructionType.archiveTypeName()));
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(
          "Kagemusha instruction archive must be a valid "
              + instructionType.archiveTypeName()
              + " Norito archive.",
          ex);
    }
    if (decoded.header().compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException("Kagemusha instruction archive must not be compressed.");
    }
    if (decoded.header().payloadLength() == 0) {
      throw new IllegalArgumentException(
          "Kagemusha instruction archive must contain a non-empty Norito payload.");
    }
    try {
      decoded.header().validateChecksum(decoded.payload());
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("Kagemusha instruction archive checksum is invalid.", ex);
    }
    return archive;
  }
}
