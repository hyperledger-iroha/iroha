package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.TypeAdapter;

final class SccpMessageProofBundles {
  private static final String SOURCE_CHAIN_PROOF_ENVELOPE_SCHEMA =
      "iroha_sccp::SccpSourceChainProofEnvelopeV1";
  private static final String SOURCE_EVENT_DIGEST_PREFIX_V1 = "sccp:source:event:v1";
  private static final int MAX_SOURCE_MERKLE_BRANCH_NODES = 64;
  private static final String MSG_PREFIX_ASSET_REGISTER_V1 = "sccp:asset:register:v1";
  private static final String MSG_PREFIX_ROUTE_ACTIVATE_V1 = "sccp:route:activate:v1";
  private static final String MSG_PREFIX_TRANSFER_V1 = "sccp:transfer:v1";
  private static final String MSG_PREFIX_TOKEN_ADD_V1 = "sccp:token:add:v1";
  private static final String MSG_PREFIX_TOKEN_PAUSE_V1 = "sccp:token:pause:v1";
  private static final String MSG_PREFIX_TOKEN_RESUME_V1 = "sccp:token:resume:v1";
  private static final String HUB_LEAF_PREFIX_V1 = "sccp:hub:leaf:v1";
  private static final String HUB_NODE_PREFIX_V1 = "sccp:hub:node:v1";
  private static final String PAYLOAD_HASH_PREFIX_V1 = "sccp:payload:v1";
  private static final int CODEC_TEXT_UTF8 = 1;
  private static final int CODEC_EVM_HEX = 2;
  private static final int CODEC_SOLANA_BASE58 = 3;
  private static final int CODEC_TON_RAW = 4;
  private static final int CODEC_TRON_BASE58CHECK = 5;
  private static final int CODEC_SORA_ASSET_ID = 6;
  private static final String BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

  private SccpMessageProofBundles() {}

  static BundleSummary requireMatchesPublicInputs(
      final int targetDomain,
      final String messageId,
      final String payloadHash,
      final String commitmentRoot,
      final String finalityHeight,
      final String finalityBlockHash,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes) {
    final BundleSummary summary = decodeMessageProofBundleSummary(bundleBytes, "bundleBytes");
    if (summary.targetDomain != targetDomain
        || !summary.messageId.equals(messageId)
        || !summary.payloadHash.equals(payloadHash)
        || !summary.commitmentRoot.equals(commitmentRoot)) {
      throw new IllegalArgumentException("bundleBytes must match publicInputs");
    }
    requireSourceProofMatchesBundle(summary, finalityHeight, finalityBlockHash, sourceProofBytes);
    return summary;
  }

  static void requireSourceProofMatchesBundle(
      final BundleSummary summary,
      final String finalityHeight,
      final String finalityBlockHash,
      final byte[] sourceProofBytes) {
    if (summary.sourceDomain == SourceSccpProofs.DOMAIN_SORA) {
      if (sourceProofBytes.length != 0) {
        throw new IllegalArgumentException("sourceProofBytes must be empty for SORA source bundle");
      }
      return;
    }
    if (sourceProofBytes.length == 0) {
      throw new IllegalArgumentException("sourceProofBytes required for non-SORA source bundle");
    }
    if (!Arrays.equals(sourceProofBytes, summary.finalityProofBytes)) {
      throw new IllegalArgumentException("sourceProofBytes must match bundleBytes finality proof");
    }
    final SourceProofSummary sourceProof =
        decodeSourceChainProofSummary(sourceProofBytes, "sourceProofBytes");
    final BigInteger normalizedFinalityHeight =
        normalizeU64(finalityHeight, "publicInputs.finalityHeight");
    final String normalizedFinalityBlockHash =
        normalizeHex32(finalityBlockHash, "publicInputs.finalityBlockHash");
    if (sourceProof.sourceDomain != summary.sourceDomain
        || sourceProof.targetDomain != summary.targetDomain
        || !sourceProof.messageId.equals(summary.messageId)
        || !sourceProof.payloadHash.equals(summary.payloadHash)
        || !sourceProof.commitmentRoot.equals(summary.commitmentRoot)
        || !sourceProof.finalityHeight.equals(normalizedFinalityHeight)
        || !sourceProof.finalityBlockHash.equals(normalizedFinalityBlockHash)) {
      throw new IllegalArgumentException("sourceProofBytes must match bundleBytes and publicInputs");
    }
  }

  private static BundleSummary decodeMessageProofBundleSummary(
      final byte[] bundleBytes, final String label) {
    int offset = 0;
    final int version = readU8At(bundleBytes, offset, label + ".version");
    offset += 1;
    if (version != 1) {
      throw new IllegalArgumentException(label + ".version must be 1");
    }
    if (offset + 32 > bundleBytes.length) {
      throw new IllegalArgumentException(label + ".commitment_root is too short");
    }
    final String commitmentRoot =
        "0x" + hexLower(Arrays.copyOfRange(bundleBytes, offset, offset + 32));
    offset += 32;
    ReadVec commitmentVec = readCanonicalVec(bundleBytes, offset, label + ".commitment");
    offset = commitmentVec.nextOffset;
    final ReadVec merkleProofVec =
        readCanonicalVec(bundleBytes, offset, label + ".merkle_proof");
    offset = merkleProofVec.nextOffset;
    final ReadVec payloadVec = readCanonicalVec(bundleBytes, offset, label + ".payload");
    offset = payloadVec.nextOffset;
    final ReadVec finalityProofVec =
        readCanonicalVec(bundleBytes, offset, label + ".finality_proof");
    offset = finalityProofVec.nextOffset;
    requireExactEnd(offset, bundleBytes, label);

    final PayloadSummary payload = decodePayloadSummary(payloadVec.bytes, label + ".payload");
    final byte[] expectedCommitmentBytes =
        canonicalCommitmentBytes(
            payload.kind, payload.targetDomain, payload.messageId, payload.payloadHash);
    if (!Arrays.equals(commitmentVec.bytes, expectedCommitmentBytes)) {
      throw new IllegalArgumentException(label + ".commitment must match payload");
    }
    final CommitmentSummary commitment = decodeCommitmentSummary(commitmentVec.bytes, label);
    if (commitment.kindCode != messageKindCode(payload.kind)) {
      throw new IllegalArgumentException(label + ".commitment kind must match payload");
    }
    final String expectedRoot =
        merkleRootFromCommitmentBytes(
            commitmentVec.bytes, merkleProofVec.bytes, label + ".merkle_proof");
    if (!commitmentRoot.equals(expectedRoot)) {
      throw new IllegalArgumentException(label + ".commitment_root must match merkle proof");
    }
    return new BundleSummary(
        payload.sourceDomain,
        commitment.targetDomain,
        commitment.messageId,
        commitment.payloadHash,
        commitmentRoot,
        finalityProofVec.bytes);
  }

  private static SourceProofSummary decodeSourceChainProofSummary(
      final byte[] sourceProofBytes, final String label) {
    final SourceProofSummary proof;
    try {
      proof =
          NoritoCodec.decode(
              sourceProofBytes, SOURCE_CHAIN_PROOF_ADAPTER, SOURCE_CHAIN_PROOF_ENVELOPE_SCHEMA);
    } catch (RuntimeException ex) {
      throw new IllegalArgumentException(
          label + " must decode as SccpSourceChainProofEnvelopeV1", ex);
    }
    if (proof.sourceDomain == SourceSccpProofs.DOMAIN_SORA) {
      throw new IllegalArgumentException(label + ".source_domain must not be SORA");
    }
    requireSupportedBundleDomain(proof.sourceDomain, label + ".source_domain");
    requireSupportedBundleDomain(proof.targetDomain, label + ".target_domain");
    if (proof.sourceDomain == proof.targetDomain) {
      throw new IllegalArgumentException(label + ".target_domain must differ from source_domain");
    }
    if (!proof.sourceChain.equals(sourceChainKeyForDomain(proof.sourceDomain))) {
      throw new IllegalArgumentException(label + ".source_chain must match source_domain");
    }
    if (proof.sourceProofPlan != sourceProofPlanCodeForDomain(proof.sourceDomain)) {
      throw new IllegalArgumentException(label + ".source_proof_plan must match source_domain");
    }
    if (proof.finalityModel != finalityModelCodeForDomain(proof.sourceDomain)) {
      throw new IllegalArgumentException(label + ".finality_model must match source_domain");
    }
    if (proof.finalityHeight.signum() <= 0) {
      throw new IllegalArgumentException(label + ".finality_height must not be zero");
    }
    if (proof.consensusProofBytes.length == 0) {
      throw new IllegalArgumentException(label + ".consensus_proof must not be empty");
    }
    if (proof.messageInclusionProofBytes.length == 0) {
      throw new IllegalArgumentException(label + ".message_inclusion_proof must not be empty");
    }
    if (proof.inclusionBranch.isEmpty()) {
      throw new IllegalArgumentException(label + ".inclusion_branch must not be empty");
    }
    if (proof.inclusionBranch.size() > MAX_SOURCE_MERKLE_BRANCH_NODES) {
      throw new IllegalArgumentException(label + ".inclusion_branch is too deep");
    }
    for (int index = 0; index < proof.inclusionBranch.size(); index++) {
      if (proof.inclusionBranch.get(index).length != 32) {
        throw new IllegalArgumentException(label + ".inclusion_branch[" + index + "] must be 32 bytes");
      }
    }
    requireNonZeroHex32(proof.messageId, label + ".message_id");
    requireNonZeroHex32(proof.payloadHash, label + ".payload_hash");
    requireNonZeroHex32(proof.sourceEventDigest, label + ".source_event_digest");
    requireNonZeroHex32(proof.commitmentRoot, label + ".commitment_root");
    requireNonZeroHex32(proof.finalityBlockHash, label + ".finality_block_hash");
    requireNonZeroHex32(proof.finalizedHeaderHash, label + ".finalized_header_hash");
    requireNonZeroHex32(proof.receiptOrMessageRoot, label + ".receipt_or_message_root");
    if (!proof.sourceEventDigest.equals(
        sourceEventDigest(
            proof.sourceDomain, proof.targetDomain, proof.messageId, proof.payloadHash))) {
      throw new IllegalArgumentException(
          label + ".source_event_digest must match source domains and message");
    }
    return proof;
  }

  private static PayloadSummary decodePayloadSummary(
      final byte[] payloadBytes, final String label) {
    if (payloadBytes.length < 2) {
      throw new IllegalArgumentException(label + " is too short");
    }
    final int discriminant = readU8At(payloadBytes, 0, label + ".kind");
    final byte[] body = Arrays.copyOfRange(payloadBytes, 1, payloadBytes.length);
    final int version = readU8At(body, 0, label + ".version");
    if (version != 1) {
      throw new IllegalArgumentException(label + ".version must be 1");
    }
    final Cursor cursor = new Cursor(1);

    switch (discriminant) {
      case 0:
        {
          final int targetDomain = readDomain(body, cursor, label, "target_domain");
          final int sourceDomain = readDomain(body, cursor, label, "home_domain");
          readU64(body, cursor, label, "nonce");
          readCodecValue(body, cursor, label, readCodec(body, cursor, label, "asset_id_codec"), "asset_id");
          readU8At(body, cursor.offset, label + ".decimals");
          cursor.offset += 1;
          requireExactEnd(cursor.offset, body, label);
          return summary(
              "AssetRegister",
              sourceDomain,
              targetDomain,
              MSG_PREFIX_ASSET_REGISTER_V1,
              body,
              payloadBytes);
        }
      case 1:
        {
          final int sourceDomain = readDomain(body, cursor, label, "source_domain");
          final int targetDomain = readDomain(body, cursor, label, "target_domain");
          if (sourceDomain == targetDomain) {
            throw new IllegalArgumentException(label + ".target_domain must differ from source_domain");
          }
          readU64(body, cursor, label, "nonce");
          readCodecValue(body, cursor, label, readCodec(body, cursor, label, "asset_id_codec"), "asset_id");
          readCodecValue(body, cursor, label, readCodec(body, cursor, label, "route_id_codec"), "route_id");
          requireExactEnd(cursor.offset, body, label);
          return summary(
              "RouteActivate",
              sourceDomain,
              targetDomain,
              MSG_PREFIX_ROUTE_ACTIVATE_V1,
              body,
              payloadBytes);
        }
      case 2:
        {
          final int sourceDomain = readDomain(body, cursor, label, "source_domain");
          final int targetDomain = readDomain(body, cursor, label, "dest_domain");
          if (sourceDomain == targetDomain) {
            throw new IllegalArgumentException(label + ".dest_domain must differ from source_domain");
          }
          readU64(body, cursor, label, "nonce");
          readDomain(body, cursor, label, "asset_home_domain");
          readCodecValue(body, cursor, label, readCodec(body, cursor, label, "asset_id_codec"), "asset_id");
          final BigInteger amount = readU128LeAt(body, cursor.offset, label + ".amount");
          cursor.offset += 16;
          if (amount.compareTo(BigInteger.ZERO) <= 0) {
            throw new IllegalArgumentException(label + ".amount must be greater than zero");
          }
          final int senderCodec = readCodec(body, cursor, label, "sender_codec");
          if (senderCodec != counterpartyAccountCodec(sourceDomain)) {
            throw new IllegalArgumentException(label + ".sender_codec must match source_domain");
          }
          readCodecValue(body, cursor, label, senderCodec, "sender");
          final int recipientCodec = readCodec(body, cursor, label, "recipient_codec");
          if (recipientCodec != counterpartyAccountCodec(targetDomain)) {
            throw new IllegalArgumentException(label + ".recipient_codec must match dest_domain");
          }
          readCodecValue(body, cursor, label, recipientCodec, "recipient");
          readCodecValue(body, cursor, label, readCodec(body, cursor, label, "route_id_codec"), "route_id");
          requireExactEnd(cursor.offset, body, label);
          return summary(
              "Transfer",
              sourceDomain,
              targetDomain,
              MSG_PREFIX_TRANSFER_V1,
              body,
              payloadBytes);
        }
      case 3:
        {
          final int targetDomain = readDomain(body, cursor, label, "target_domain");
          readU64(body, cursor, label, "nonce");
          final byte[] assetId = readFixed(body, cursor, 32, label + ".sora_asset_id");
          if (!containsNonZero(assetId)) {
            throw new IllegalArgumentException(label + ".sora_asset_id must be non-zero");
          }
          readU8At(body, cursor.offset, label + ".decimals");
          cursor.offset += 1;
          final byte[] name = readFixed(body, cursor, 32, label + ".name");
          if (!fixedAsciiFieldIsNonEmpty(name)) {
            throw new IllegalArgumentException(label + ".name must be non-empty");
          }
          final byte[] symbol = readFixed(body, cursor, 32, label + ".symbol");
          if (!fixedAsciiFieldIsNonEmpty(symbol)) {
            throw new IllegalArgumentException(label + ".symbol must be non-empty");
          }
          requireExactEnd(cursor.offset, body, label);
          return summary(
              "TokenAdd",
              SourceSccpProofs.DOMAIN_SORA,
              targetDomain,
              MSG_PREFIX_TOKEN_ADD_V1,
              body,
              payloadBytes);
        }
      case 4:
      case 5:
        {
          final int targetDomain = readDomain(body, cursor, label, "target_domain");
          readU64(body, cursor, label, "nonce");
          final byte[] assetId = readFixed(body, cursor, 32, label + ".sora_asset_id");
          if (!containsNonZero(assetId)) {
            throw new IllegalArgumentException(label + ".sora_asset_id must be non-zero");
          }
          requireExactEnd(cursor.offset, body, label);
          return summary(
              discriminant == 4 ? "TokenPause" : "TokenResume",
              SourceSccpProofs.DOMAIN_SORA,
              targetDomain,
              discriminant == 4 ? MSG_PREFIX_TOKEN_PAUSE_V1 : MSG_PREFIX_TOKEN_RESUME_V1,
              body,
              payloadBytes);
        }
      default:
        throw new IllegalArgumentException(label + " contains unsupported SCCP payload kind");
    }
  }

  private static PayloadSummary summary(
      final String kind,
      final int sourceDomain,
      final int targetDomain,
      final String prefix,
      final byte[] body,
      final byte[] payloadBytes) {
    return new PayloadSummary(
        kind,
        sourceDomain,
        targetDomain,
        "0x" + hexLower(prefixedKeccakBytes(prefix, body)),
        "0x" + hexLower(prefixedHashBytes(PAYLOAD_HASH_PREFIX_V1, payloadBytes)));
  }

  private static CommitmentSummary decodeCommitmentSummary(
      final byte[] commitmentBytes, final String label) {
    if (commitmentBytes.length != 70) {
      throw new IllegalArgumentException(label + ".commitment must be 70 bytes");
    }
    final int version = readU8At(commitmentBytes, 0, label + ".commitment.version");
    if (version != 1) {
      throw new IllegalArgumentException(label + ".commitment.version must be 1");
    }
    return new CommitmentSummary(
        readU8At(commitmentBytes, 1, label + ".commitment.kind"),
        readU32LeAt(commitmentBytes, 2, label + ".commitment.target_domain"),
        "0x" + hexLower(Arrays.copyOfRange(commitmentBytes, 6, 38)),
        "0x" + hexLower(Arrays.copyOfRange(commitmentBytes, 38, 70)));
  }

  private static String merkleRootFromCommitmentBytes(
      final byte[] commitmentBytes, final byte[] merkleProofBytes, final String label) {
    int offset = 0;
    final int stepCount = readU32LeAt(merkleProofBytes, offset, label + ".steps");
    offset += 4;
    byte[] current = prefixedHashBytes(HUB_LEAF_PREFIX_V1, commitmentBytes);
    for (int index = 0; index < stepCount; index++) {
      if (offset + 33 > merkleProofBytes.length) {
        throw new IllegalArgumentException(label + ".steps[" + index + "] is too short");
      }
      final byte[] sibling = Arrays.copyOfRange(merkleProofBytes, offset, offset + 32);
      offset += 32;
      final int siblingIsLeft =
          readU8At(merkleProofBytes, offset, label + ".steps[" + index + "].sibling_is_left");
      offset += 1;
      if (siblingIsLeft != 0 && siblingIsLeft != 1) {
        throw new IllegalArgumentException(
            label + ".steps[" + index + "].sibling_is_left must be 0 or 1");
      }
      final ByteArrayOutputStream payload = new ByteArrayOutputStream();
      if (siblingIsLeft == 1) {
        write(payload, sibling);
        write(payload, current);
      } else {
        write(payload, current);
        write(payload, sibling);
      }
      current = prefixedHashBytes(HUB_NODE_PREFIX_V1, payload.toByteArray());
    }
    requireExactEnd(offset, merkleProofBytes, label);
    return "0x" + hexLower(current);
  }

  private static byte[] canonicalCommitmentBytes(
      final String kind, final int targetDomain, final String messageId, final String payloadHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(messageKindCode(kind));
    writeU32Le(out, targetDomain);
    write(out, hex32Bytes(messageId, "commitment.messageId"));
    write(out, hex32Bytes(payloadHash, "commitment.payloadHash"));
    return out.toByteArray();
  }

  private static int messageKindCode(final String kind) {
    switch (kind) {
      case "Burn":
        return 0;
      case "TokenAdd":
        return 1;
      case "TokenPause":
        return 2;
      case "TokenResume":
        return 3;
      case "AssetRegister":
        return 4;
      case "RouteActivate":
        return 5;
      case "Transfer":
        return 6;
      default:
        throw new IllegalArgumentException("SCCP message kind is unsupported");
    }
  }

  private static int readDomain(
      final byte[] body, final Cursor cursor, final String label, final String field) {
    final int domain = readU32LeAt(body, cursor.offset, label + "." + field);
    cursor.offset += 4;
    requireSupportedBundleDomain(domain, label + "." + field);
    return domain;
  }

  private static void readU64(
      final byte[] body, final Cursor cursor, final String label, final String field) {
    readU64LeAt(body, cursor.offset, label + "." + field);
    cursor.offset += 8;
  }

  private static int readCodec(
      final byte[] body, final Cursor cursor, final String label, final String field) {
    final int codec = normalizeCodecId(readU8At(body, cursor.offset, label + "." + field));
    cursor.offset += 1;
    return codec;
  }

  private static void readCodecValue(
      final byte[] body,
      final Cursor cursor,
      final String label,
      final int codec,
      final String field) {
    final ReadVec value = readCanonicalVec(body, cursor.offset, label + "." + field);
    cursor.offset = value.nextOffset;
    validateCodecBytes(codec, value.bytes, label + "." + field);
  }

  private static void requireSupportedBundleDomain(final int domain, final String label) {
    if (domain != SourceSccpProofs.DOMAIN_SORA
        && domain != SourceSccpProofs.DOMAIN_ETH
        && domain != SourceSccpProofs.DOMAIN_BSC
        && domain != SolanaSccpProver.DOMAIN_SOLANA
        && domain != TonSccpProver.DOMAIN_TON
        && domain != TronSccpProver.DOMAIN_TRON) {
      throw new IllegalArgumentException(label + " must be a supported SCCP domain");
    }
  }

  private static int normalizeCodecId(final int value) {
    if (value != CODEC_TEXT_UTF8
        && value != CODEC_EVM_HEX
        && value != CODEC_SOLANA_BASE58
        && value != CODEC_TON_RAW
        && value != CODEC_TRON_BASE58CHECK
        && value != CODEC_SORA_ASSET_ID) {
      throw new IllegalArgumentException("SCCP codec is unsupported");
    }
    return value;
  }

  private static int counterpartyAccountCodec(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_SORA) {
      return CODEC_TEXT_UTF8;
    }
    if (domain == SourceSccpProofs.DOMAIN_ETH || domain == SourceSccpProofs.DOMAIN_BSC) {
      return CODEC_EVM_HEX;
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return CODEC_SOLANA_BASE58;
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return CODEC_TON_RAW;
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return CODEC_TRON_BASE58CHECK;
    }
    throw new IllegalArgumentException("SCCP domain must be supported");
  }

  private static String sourceChainKeyForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_SORA) {
      return "sora";
    }
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return "eth";
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return "bsc";
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return "sol";
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return "ton";
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return "tron";
    }
    throw new IllegalArgumentException("SCCP domain must be supported");
  }

  private static int sourceProofPlanCodeForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return 1;
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return 2;
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return 3;
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return 4;
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return 5;
    }
    throw new IllegalArgumentException("SCCP source domain must support source proofs");
  }

  private static int finalityModelCodeForDomain(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return 0;
    }
    if (domain == SourceSccpProofs.DOMAIN_BSC) {
      return 1;
    }
    if (domain == SolanaSccpProver.DOMAIN_SOLANA) {
      return 2;
    }
    if (domain == TonSccpProver.DOMAIN_TON) {
      return 3;
    }
    if (domain == TronSccpProver.DOMAIN_TRON) {
      return 4;
    }
    throw new IllegalArgumentException("SCCP source domain must support source proofs");
  }

  private static void validateCodecBytes(
      final int codec, final byte[] raw, final String label) {
    switch (codec) {
      case CODEC_TEXT_UTF8:
        if (decodeCanonicalUtf8Bytes(raw, label).isEmpty()) {
          throw new IllegalArgumentException(label + " must not be empty");
        }
        break;
      case CODEC_EVM_HEX:
        validateCanonicalEvmHexAddress(decodeCanonicalUtf8Bytes(raw, label), label);
        break;
      case CODEC_SOLANA_BASE58:
        decodeBase58Fixed(decodeCanonicalUtf8Bytes(raw, label), label, 32);
        break;
      case CODEC_TON_RAW:
        validateTonRawAddress(decodeCanonicalUtf8Bytes(raw, label), label);
        break;
      case CODEC_TRON_BASE58CHECK:
        tronBase58CheckPayload(decodeCanonicalUtf8Bytes(raw, label), label);
        break;
      case CODEC_SORA_ASSET_ID:
        if (raw.length != 32) {
          throw new IllegalArgumentException(label + " must be 32 bytes");
        }
        break;
      default:
        throw new IllegalArgumentException(label + " codec is unsupported");
    }
  }

  private static String decodeCanonicalUtf8Bytes(final byte[] raw, final String label) {
    final String text = new String(raw, StandardCharsets.UTF_8);
    if (!Arrays.equals(text.getBytes(StandardCharsets.UTF_8), raw)) {
      throw new IllegalArgumentException(label + " must be canonical UTF-8");
    }
    return text;
  }

  private static void validateCanonicalEvmHexAddress(final String text, final String label) {
    if (text.length() != 42 || !text.startsWith("0x")) {
      throw new IllegalArgumentException(label + " must be a 0x-prefixed 20-byte EVM address");
    }
    final String payload = text.substring(2);
    for (int index = 0; index < payload.length(); index++) {
      final char symbol = payload.charAt(index);
      if (!isEvmAddressHexDigit(symbol)) {
        throw new IllegalArgumentException(label + " must be a 0x-prefixed 20-byte EVM address");
      }
    }
    final byte[] checksum =
        keccak256(payload.toLowerCase(java.util.Locale.ROOT).getBytes(StandardCharsets.UTF_8));
    for (int index = 0; index < payload.length(); index++) {
      final char symbol = payload.charAt(index);
      if (symbol >= '0' && symbol <= '9') {
        continue;
      }
      final int checksumByte = checksum[index / 2] & 0xff;
      final int checksumNibble = index % 2 == 0 ? checksumByte >>> 4 : checksumByte & 0x0f;
      final boolean shouldBeUppercase = checksumNibble >= 8;
      if (shouldBeUppercase && symbol != Character.toUpperCase(symbol)) {
        throw new IllegalArgumentException(label + " must be a canonical EIP-55 EVM address");
      }
      if (!shouldBeUppercase && symbol != Character.toLowerCase(symbol)) {
        throw new IllegalArgumentException(label + " must be a canonical EIP-55 EVM address");
      }
    }
  }

  private static boolean isEvmAddressHexDigit(final char ch) {
    return (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F');
  }

  private static void validateTonRawAddress(final String text, final String label) {
    final String[] parts = text.split(":", -1);
    if (parts.length != 2 || !"0".equals(parts[0]) || parts[1].length() != 64) {
      throw new IllegalArgumentException(label + " must be a basechain TON raw address");
    }
    final byte[] account = hexBytes(parts[1], label);
    if (!containsNonZero(account)) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
  }

  private static ReadVec readCanonicalVec(
      final byte[] raw, final int offset, final String label) {
    final int length = readU32LeAt(raw, offset, label + ".length");
    final int start = offset + 4;
    final long end = (long) start + (long) length;
    if (end > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    return new ReadVec(Arrays.copyOfRange(raw, start, (int) end), (int) end);
  }

  private static byte[] readFixed(
      final byte[] raw, final Cursor cursor, final int length, final String label) {
    final int end = cursor.offset + length;
    if (end > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    final byte[] out = Arrays.copyOfRange(raw, cursor.offset, end);
    cursor.offset = end;
    return out;
  }

  private static int readU8At(final byte[] raw, final int offset, final String label) {
    if (offset + 1 > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    return raw[offset] & 0xff;
  }

  private static int readU32LeAt(final byte[] raw, final int offset, final String label) {
    if (offset + 4 > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    final long value =
        (raw[offset] & 0xffL)
            | ((raw[offset + 1] & 0xffL) << 8)
            | ((raw[offset + 2] & 0xffL) << 16)
            | ((raw[offset + 3] & 0xffL) << 24);
    if (value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(label + " must fit platform size");
    }
    return (int) value;
  }

  private static BigInteger readU64LeAt(final byte[] raw, final int offset, final String label) {
    if (offset + 8 > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    BigInteger value = BigInteger.ZERO;
    for (int index = 7; index >= 0; index--) {
      value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index] & 0xffL));
    }
    return value;
  }

  private static BigInteger readU128LeAt(final byte[] raw, final int offset, final String label) {
    if (offset + 16 > raw.length) {
      throw new IllegalArgumentException(label + " is too short");
    }
    BigInteger value = BigInteger.ZERO;
    for (int index = 15; index >= 0; index--) {
      value = value.shiftLeft(8).or(BigInteger.valueOf(raw[offset + index] & 0xffL));
    }
    return value;
  }

  private static void requireExactEnd(final int offset, final byte[] raw, final String label) {
    if (offset != raw.length) {
      throw new IllegalArgumentException(label + " must not contain trailing bytes");
    }
  }

  private static boolean fixedAsciiFieldIsNonEmpty(final byte[] raw) {
    int limit = raw.length;
    for (int index = 0; index < raw.length; index++) {
      if (raw[index] == 0) {
        limit = index;
        break;
      }
    }
    for (int index = 0; index < limit; index++) {
      if (raw[index] != 0) {
        return true;
      }
    }
    return false;
  }

  private static byte[] decodeBase58Fixed(
      final String value, final String field, final int byteLength) {
    final byte[] raw = decodeBase58(value, field);
    if (raw.length != byteLength) {
      throw new IllegalArgumentException(field + " must decode to " + byteLength + " bytes");
    }
    return raw;
  }

  private static byte[] decodeBase58(final String value, final String field) {
    if (!value.trim().equals(value) || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must be canonical base58");
    }
    BigInteger numeric = BigInteger.ZERO;
    for (int i = 0; i < value.length(); i++) {
      final int digit = BASE58_ALPHABET.indexOf(value.charAt(i));
      if (digit < 0) {
        throw new IllegalArgumentException(field + " must be canonical base58");
      }
      numeric = numeric.multiply(BigInteger.valueOf(58)).add(BigInteger.valueOf(digit));
    }
    byte[] encoded = numeric.equals(BigInteger.ZERO) ? new byte[0] : numeric.toByteArray();
    if (encoded.length > 0 && encoded[0] == 0) {
      encoded = Arrays.copyOfRange(encoded, 1, encoded.length);
    }
    int leadingZeroes = 0;
    while (leadingZeroes < value.length() && value.charAt(leadingZeroes) == '1') {
      leadingZeroes += 1;
    }
    final byte[] decoded = new byte[leadingZeroes + encoded.length];
    System.arraycopy(encoded, 0, decoded, leadingZeroes, encoded.length);
    return decoded;
  }

  private static byte[] tronBase58CheckPayload(final String value, final String field) {
    final byte[] raw = decodeBase58(value, field);
    if (raw.length != 25) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    final byte[] payload = Arrays.copyOfRange(raw, 0, 21);
    if ((payload[0] & 0xff) != 0x41) {
      throw new IllegalArgumentException(field + " must be a TRON mainnet address");
    }
    final byte[] checksum = Arrays.copyOfRange(sha256(sha256(payload)), 0, 4);
    if (!Arrays.equals(Arrays.copyOfRange(raw, 21, 25), checksum)) {
      throw new IllegalArgumentException(field + " must have a valid Base58Check checksum");
    }
    return payload;
  }

  private static byte[] prefixedKeccakBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return keccak256(preimage);
  }

  private static byte[] keccak256(final byte[] preimage) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(preimage, 0, preimage.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static byte[] prefixedHashBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return Blake2b.digest256(preimage);
  }

  private static String sourceEventDigest(
      final int sourceDomain,
      final int targetDomain,
      final String messageId,
      final String payloadHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, sourceDomain);
    writeU32Le(out, targetDomain);
    write(out, hex32Bytes(messageId, "sourceProofBytes.message_id"));
    write(out, hex32Bytes(payloadHash, "sourceProofBytes.payload_hash"));
    return "0x" + hexLower(prefixedHashBytes(SOURCE_EVENT_DIGEST_PREFIX_V1, out.toByteArray()));
  }

  private static byte[] sha256(final byte[] input) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(input);
    } catch (final NoSuchAlgorithmException e) {
      throw new IllegalStateException(e);
    }
  }

  private static BigInteger normalizeU64(final String value, final String label) {
    if (!("0".equals(value) || (value != null && !value.isEmpty()
        && value.charAt(0) >= '1'
        && value.charAt(0) <= '9'
        && value.chars().allMatch(ch -> ch >= '0' && ch <= '9')))) {
      throw new IllegalArgumentException(label + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(value);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(label + " must fit u64");
    }
    if (numeric.signum() == 0) {
      throw new IllegalArgumentException(label + " must not be zero");
    }
    return numeric;
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
  }

  private static void requireNonZeroHex32(final String value, final String field) {
    if (!containsNonZero(hex32Bytes(value, field))) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    String body = value;
    if (body.startsWith("0X")) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    if (body.startsWith("0x")) {
      body = body.substring(2);
    }
    if (body.length() != 64) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    return hexBytes(body, field);
  }

  private static byte[] hexBytes(final String value, final String field) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must have even hex length");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int high = hexDigit(value.charAt(index * 2));
      final int low = hexDigit(value.charAt(index * 2 + 1));
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException(field + " must be hex");
      }
      out[index] = (byte) ((high << 4) | low);
    }
    return out;
  }

  private static int hexDigit(final char ch) {
    if (ch >= '0' && ch <= '9') {
      return ch - '0';
    }
    if (ch >= 'a' && ch <= 'f') {
      return ch - 'a' + 10;
    }
    return -1;
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static boolean containsNonZero(final byte[] value) {
    for (final byte b : value) {
      if (b != 0) {
        return true;
      }
    }
    return false;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      out.append(String.format("%02x", b & 0xff));
    }
    return out.toString();
  }

  private static final TypeAdapter<SourceProofSummary> SOURCE_CHAIN_PROOF_ADAPTER =
      new TypeAdapter<SourceProofSummary>() {
        @Override
        public void encode(final NoritoEncoder encoder, final SourceProofSummary value) {
          throw new UnsupportedOperationException("source proof encoding is not supported here");
        }

        @Override
        public SourceProofSummary decode(final NoritoDecoder decoder) {
          final int version =
              readNoritoField(decoder, "sourceProofBytes.version", child -> (int) child.readUInt(8));
          if (version != 1) {
            throw new IllegalArgumentException("sourceProofBytes.version must be 1");
          }
          final int sourceDomain = readNoritoU32Field(decoder, "sourceProofBytes.source_domain");
          final int targetDomain = readNoritoU32Field(decoder, "sourceProofBytes.target_domain");
          final String sourceChain =
              readNoritoField(
                  decoder,
                  "sourceProofBytes.source_chain",
                  child -> readNoritoString(child, "sourceProofBytes.source_chain"));
          final int sourceProofPlan =
              readNoritoU32Field(decoder, "sourceProofBytes.source_proof_plan");
          final int finalityModel =
              readNoritoU32Field(decoder, "sourceProofBytes.finality_model");
          final String messageId = readNoritoHex32Field(decoder, "sourceProofBytes.message_id");
          final String payloadHash = readNoritoHex32Field(decoder, "sourceProofBytes.payload_hash");
          final String sourceEventDigest =
              readNoritoHex32Field(decoder, "sourceProofBytes.source_event_digest");
          final String commitmentRoot =
              readNoritoHex32Field(decoder, "sourceProofBytes.commitment_root");
          final BigInteger finalityHeight =
              readNoritoField(
                  decoder,
                  "sourceProofBytes.finality_height",
                  child -> readNoritoU64(child, "sourceProofBytes.finality_height"));
          final String finalityBlockHash =
              readNoritoHex32Field(decoder, "sourceProofBytes.finality_block_hash");
          final String finalizedHeaderHash =
              readNoritoHex32Field(decoder, "sourceProofBytes.finalized_header_hash");
          final String receiptOrMessageRoot =
              readNoritoHex32Field(decoder, "sourceProofBytes.receipt_or_message_root");
          final byte[] consensusProofBytes =
              readNoritoField(
                  decoder,
                  "sourceProofBytes.consensus_proof",
                  child -> readNoritoRawByteVec(child, "sourceProofBytes.consensus_proof"));
          final byte[] messageInclusionProofBytes =
              readNoritoField(
                  decoder,
                  "sourceProofBytes.message_inclusion_proof",
                  child ->
                      readNoritoRawByteVec(child, "sourceProofBytes.message_inclusion_proof"));
          final List<byte[]> inclusionBranch =
              readNoritoField(
                  decoder,
                  "sourceProofBytes.inclusion_branch",
                  child -> readNoritoRawByteVecSequence(child, "sourceProofBytes.inclusion_branch"));
          return new SourceProofSummary(
              sourceDomain,
              targetDomain,
              sourceChain,
              sourceProofPlan,
              finalityModel,
              messageId,
              payloadHash,
              sourceEventDigest,
              commitmentRoot,
              finalityHeight,
              finalityBlockHash,
              finalizedHeaderHash,
              receiptOrMessageRoot,
              consensusProofBytes,
              messageInclusionProofBytes,
              inclusionBranch);
        }
      };

  private static int readNoritoU32Field(final NoritoDecoder decoder, final String label) {
    return readNoritoField(
        decoder,
        label,
        child -> {
          final long value = child.readUInt(32);
          if (value > Integer.MAX_VALUE) {
            throw new IllegalArgumentException(label + " must fit platform size");
          }
          return (int) value;
        });
  }

  private static String readNoritoHex32Field(final NoritoDecoder decoder, final String label) {
    return readNoritoField(decoder, label, child -> "0x" + hexLower(child.readBytes(32)));
  }

  private static <T> T readNoritoField(
      final NoritoDecoder decoder, final String label, final FieldReader<T> reader) {
    final int length = checkedLength(decoder.readLength(decoder.compactLenActive()), label);
    final NoritoDecoder child =
        new NoritoDecoder(decoder.readBytes(length), decoder.flags(), decoder.flagsHint());
    final T value = reader.read(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(label + " must not contain trailing bytes");
    }
    return value;
  }

  private static String readNoritoString(final NoritoDecoder decoder, final String label) {
    final int length = checkedLength(decoder.readLength(decoder.compactLenActive()), label);
    final byte[] bytes = decoder.readBytes(length);
    final String value = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(value.getBytes(StandardCharsets.UTF_8), bytes)) {
      throw new IllegalArgumentException(label + " must be canonical UTF-8");
    }
    return value;
  }

  private static byte[] readNoritoRawByteVec(final NoritoDecoder decoder, final String label) {
    final int length = checkedLength(decoder.readLength(false), label);
    return decoder.readBytes(length);
  }

  private static List<byte[]> readNoritoRawByteVecSequence(
      final NoritoDecoder decoder, final String label) {
    final int count = checkedLength(decoder.readLength(false), label);
    final List<byte[]> out = new ArrayList<>(count);
    for (int index = 0; index < count; index++) {
      final int elementLength =
          checkedLength(decoder.readLength(decoder.compactLenActive()), label + "[" + index + "]");
      final NoritoDecoder child =
          new NoritoDecoder(decoder.readBytes(elementLength), decoder.flags(), decoder.flagsHint());
      final byte[] value = readNoritoRawByteVec(child, label + "[" + index + "]");
      if (child.remaining() != 0) {
        throw new IllegalArgumentException(label + "[" + index + "] must not contain trailing bytes");
      }
      out.add(value);
    }
    return out;
  }

  private static BigInteger readNoritoU64(final NoritoDecoder decoder, final String label) {
    return readU64LeAt(decoder.readBytes(8), 0, label);
  }

  private static int checkedLength(final long value, final String label) {
    if (value < 0) {
      throw new IllegalArgumentException(label + " must not be negative");
    }
    if (value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(label + " exceeds JVM array limit");
    }
    return (int) value;
  }

  private interface FieldReader<T> {
    T read(NoritoDecoder decoder);
  }

  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private static final class SourceProofSummary {
    final int sourceDomain;
    final int targetDomain;
    final String sourceChain;
    final int sourceProofPlan;
    final int finalityModel;
    final String messageId;
    final String payloadHash;
    final String sourceEventDigest;
    final String commitmentRoot;
    final BigInteger finalityHeight;
    final String finalityBlockHash;
    final String finalizedHeaderHash;
    final String receiptOrMessageRoot;
    final byte[] consensusProofBytes;
    final byte[] messageInclusionProofBytes;
    final List<byte[]> inclusionBranch;

    SourceProofSummary(
        final int sourceDomain,
        final int targetDomain,
        final String sourceChain,
        final int sourceProofPlan,
        final int finalityModel,
        final String messageId,
        final String payloadHash,
        final String sourceEventDigest,
        final String commitmentRoot,
        final BigInteger finalityHeight,
        final String finalityBlockHash,
        final String finalizedHeaderHash,
        final String receiptOrMessageRoot,
        final byte[] consensusProofBytes,
        final byte[] messageInclusionProofBytes,
        final List<byte[]> inclusionBranch) {
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.sourceChain = sourceChain;
      this.sourceProofPlan = sourceProofPlan;
      this.finalityModel = finalityModel;
      this.messageId = messageId;
      this.payloadHash = payloadHash;
      this.sourceEventDigest = sourceEventDigest;
      this.commitmentRoot = commitmentRoot;
      this.finalityHeight = finalityHeight;
      this.finalityBlockHash = finalityBlockHash;
      this.finalizedHeaderHash = finalizedHeaderHash;
      this.receiptOrMessageRoot = receiptOrMessageRoot;
      this.consensusProofBytes = Arrays.copyOf(consensusProofBytes, consensusProofBytes.length);
      this.messageInclusionProofBytes =
          Arrays.copyOf(messageInclusionProofBytes, messageInclusionProofBytes.length);
      this.inclusionBranch = inclusionBranch;
    }
  }

  static final class BundleSummary {
    final int sourceDomain;
    final int targetDomain;
    final String messageId;
    final String payloadHash;
    final String commitmentRoot;
    final byte[] finalityProofBytes;

    BundleSummary(
        final int sourceDomain,
        final int targetDomain,
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final byte[] finalityProofBytes) {
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.messageId = messageId;
      this.payloadHash = payloadHash;
      this.commitmentRoot = commitmentRoot;
      this.finalityProofBytes = Arrays.copyOf(finalityProofBytes, finalityProofBytes.length);
    }
  }

  private static final class ReadVec {
    final byte[] bytes;
    final int nextOffset;

    ReadVec(final byte[] bytes, final int nextOffset) {
      this.bytes = bytes;
      this.nextOffset = nextOffset;
    }
  }

  private static final class PayloadSummary {
    final String kind;
    final int sourceDomain;
    final int targetDomain;
    final String messageId;
    final String payloadHash;

    PayloadSummary(
        final String kind,
        final int sourceDomain,
        final int targetDomain,
        final String messageId,
        final String payloadHash) {
      this.kind = kind;
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.messageId = messageId;
      this.payloadHash = payloadHash;
    }
  }

  private static final class CommitmentSummary {
    final int kindCode;
    final int targetDomain;
    final String messageId;
    final String payloadHash;

    CommitmentSummary(
        final int kindCode,
        final int targetDomain,
        final String messageId,
        final String payloadHash) {
      this.kindCode = kindCode;
      this.targetDomain = targetDomain;
      this.messageId = messageId;
      this.payloadHash = payloadHash;
    }
  }

  private static final class Cursor {
    int offset;

    Cursor(final int offset) {
      this.offset = offset;
    }
  }
}
