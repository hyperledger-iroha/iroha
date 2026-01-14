package org.hyperledger.iroha.android.tx;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.regex.Pattern;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.testing.SimpleJson;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

/**
 * Validates the Norito fixture manifest emitted by {@code scripts/export_norito_fixtures}.
 *
 * <p>The manifest advertises deterministic hashes, base64 payloads, and encoded blob lengths used
 * by downstream Android fixtures. This test ensures the entries stay in sync with the checked-in
 * {@code transaction_payloads.json} file and the generated {@code *.norito} blobs.
 */
public final class TransactionFixtureManifestTests {

  private static final Pattern HEX_64 = Pattern.compile("^[0-9a-fA-F]{64}$");
  private static final String HASH_ALGORITHM = "BLAKE2B-256";
  private static final String PAYLOAD_SCHEMA = "iroha.android.transaction.Payload.v1";
  private static final String SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1";
  private static final int CONTROLLER_SINGLE = 0x00;
  private static final int CURVE_ED25519 = 0x01;
  private static final byte VERSION_BYTE = 0x01;
  private static final NoritoJavaCodecAdapter PAYLOAD_CODEC = new NoritoJavaCodecAdapter();
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT64_ADAPTER = NoritoAdapters.uint(64);
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);
  private static final TypeAdapter<Optional<Long>> TTL_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(64));
  private static final TypeAdapter<Optional<Long>> NONCE_ADAPTER =
      NoritoAdapters.option(NoritoAdapters.uint(32));
  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<List<InstructionEnvelope>> INSTRUCTION_LIST_ADAPTER =
      NoritoAdapters.sequence(new InstructionEnvelopeAdapter());
  private static final TypeAdapter<List<MetadataEntry>> METADATA_ENTRY_LIST_ADAPTER =
      NoritoAdapters.sequence(new MetadataEntryAdapter());
  private static int compatChecked = 0;

  @Test
  public void validateManifest() throws Exception {
    runValidation();
  }

  @Test
  public void validateManifestSchema() throws Exception {
    final Path manifestPath = resolveFixturePath("transaction_fixtures.manifest.json");
    final Map<String, Object> manifest = loadManifest(manifestPath);
    assertSchemaMatches(manifest);
  }

  public static void main(final String[] args) throws Exception {
    runValidation();
  }

  private static void runValidation() throws Exception {
    compatChecked = 0;
    final Path manifestPath = resolveFixturePath("transaction_fixtures.manifest.json");
    final Map<String, Object> manifest = loadManifest(manifestPath);
    assertSchemaMatches(manifest);
    assertSigningKeyFormat(manifest);

    final Object fixturesValue = manifest.get("fixtures");
    final List<Object> fixtures = asList(fixturesValue, "fixtures");
    if (fixtures.isEmpty()) {
      throw new IllegalStateException("Manifest must contain at least one fixture");
    }

    // Map fixtures from transaction_payloads.json by name for cross checks.
    final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures =
        loadPayloadFixtures(manifestPath.getParent());

    for (Object entry : fixtures) {
      validateFixture(entry, manifestPath, payloadFixtures);
    }

    assertEquals(
        "All fixtures must be validated for SDK compatibility",
        fixtures.size(),
        compatChecked);
    System.out.println("[IrohaAndroid] Transaction fixture manifest tests passed.");
    System.out.println("[IrohaAndroid] Compatibility checks: " + compatChecked + " checked.");
  }

  private static void validateFixture(
      final Object entry,
      final Path manifestPath,
      final Map<String, TransactionPayloadFixtures.Fixture> payloadFixtures)
      throws Exception {
    final Map<String, Object> map = asMap(entry, "fixture");
    final String name = requireString(map.get("name"), "fixture.name");
    final String payloadBase64 = requireString(map.get("payload_base64"), name + ".payload_base64");
    final String signedBase64 = requireString(map.get("signed_base64"), name + ".signed_base64");
    final String encodedFile = requireString(map.get("encoded_file"), name + ".encoded_file");

    final long encodedLen = requireNumber(map.get("encoded_len"), name + ".encoded_len");
    final long signedLen = requireNumber(map.get("signed_len"), name + ".signed_len");
    final String chain = requireString(map.get("chain"), name + ".chain");
    final String authority = requireString(map.get("authority"), name + ".authority");
    final AuthorityKey authorityKey = parseAuthorityKey(authority, name + ".authority");
    final Long ttl = optionalNumber(map.get("time_to_live_ms"));
    final Long nonce = optionalNumber(map.get("nonce"));
    final String payloadHash = requireString(map.get("payload_hash"), name + ".payload_hash");
    final String signedHash = requireString(map.get("signed_hash"), name + ".signed_hash");

    final Path baseDir = manifestPath != null ? manifestPath.getParent() : null;
    final Path encodedPath = (baseDir == null)
        ? Paths.get(encodedFile)
        : baseDir.resolve(encodedFile).normalize();
    if (!Files.exists(encodedPath)) {
      throw new IllegalStateException(name + ": encoded file not found: " + encodedPath);
    }
    final byte[] encodedBytes = Files.readAllBytes(encodedPath);
    assertEquals(
        name + ": encoded_len mismatch (expected " + encodedLen + ", found " + encodedBytes.length + ")",
        encodedLen,
        encodedBytes.length);
    final String actualBase64 = Base64.getEncoder().encodeToString(encodedBytes);
    assertEquals(
        name + ": payload_base64 does not match encoded file",
        payloadBase64,
        actualBase64);

    // Validate the signed payload looks well-formed base64 and matches the advertised length.
    final byte[] payloadBytes = decodeBase64(payloadBase64, name + ".payload_base64");
    final byte[] signedBytes = decodeBase64(signedBase64, name + ".signed_base64");
    assertEquals(
        name + ": signed_len mismatch (expected " + signedLen + ", found " + signedBytes.length + ")",
        signedLen,
        signedBytes.length);

    assertTrue(name + ": payload_hash must be a 64-character hex string",
        HEX_64.matcher(payloadHash).matches());
    assertTrue(name + ": signed_hash must be a 64-character hex string",
        HEX_64.matcher(signedHash).matches());
    final String computedPayloadHash = canonicalHashHex(payloadBytes);
    assertEquals(
        name + ": payload hash mismatch (expected " + payloadHash + ", computed " + computedPayloadHash + ")",
        payloadHash,
        computedPayloadHash);
    final String computedSignedHash = SignedTransactionHasher.hashCanonicalHex(signedBytes);
    assertEquals(
        name + ": signed hash mismatch (expected " + signedHash + ", computed " + computedSignedHash + ")",
        signedHash,
        computedSignedHash);

    final TransactionPayloadFixtures.Fixture payloadFixture = payloadFixtures.get(name);
    if (payloadFixture != null) {
      payloadFixture.encoded().ifPresent(encoded -> {
        assertEquals(
            name + ": manifest payload mismatch vs transaction_payloads entry",
            payloadBase64,
            encoded);
      });
      if (payloadFixture.isDecodable()) {
        final TransactionPayload payload = payloadFixture.toPayload();
        assertEquals(
            name + ": chain mismatch vs transaction_payloads",
            chain,
            payload.chainId());
        assertEquals(
            name + ": authority mismatch vs transaction_payloads",
            authority,
            payload.authority());
        assertTrue(
            name + ": TTL mismatch vs transaction_payloads",
            optionalLongEquals(payload.timeToLiveMs(), ttl));
        assertTrue(
            name + ": nonce mismatch vs transaction_payloads",
            optionalIntEquals(payload.nonce(), nonce));
      }
    }

    validateCompatibility(name, payloadBytes, signedBytes, chain, authority, ttl, nonce, authorityKey);
  }

  private static byte[] decodeBase64(final String value, final String fieldName) {
    try {
      return Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException(fieldName + " is not valid base64", ex);
    }
  }

  private static String canonicalHashHex(final byte[] data) {
    final byte[] digest = canonicalHashBytes(data);
    final StringBuilder builder = new StringBuilder(digest.length * 2);
    for (byte b : digest) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static byte[] canonicalHashBytes(final byte[] data) {
    byte[] digest;
    try {
      final MessageDigest md = MessageDigest.getInstance(HASH_ALGORITHM);
      digest = md.digest(data);
    } catch (final NoSuchAlgorithmException ex) {
      digest = Blake2b.digest(data);
    }
    digest[digest.length - 1] |= 1;
    return digest;
  }

  private static Map<String, Object> loadManifest(final Path manifestPath) throws Exception {
    final String json = new String(Files.readAllBytes(manifestPath), StandardCharsets.UTF_8);
    return asMap(SimpleJson.parse(json), "manifest");
  }

  private static void assertSchemaMatches(final Map<String, Object> manifest) {
    final Map<String, Object> schema = asMap(manifest.get("schema"), "schema");
    assertEquals(
        "schema.payload mismatch",
        PAYLOAD_SCHEMA,
        requireString(schema.get("payload"), "schema.payload"));
    assertEquals(
        "schema.signed mismatch",
        SIGNED_SCHEMA,
        requireString(schema.get("signed"), "schema.signed"));
  }

  private static void assertSigningKeyFormat(final Map<String, Object> manifest) {
    final Map<String, Object> signingKey = asMap(manifest.get("signing_key"), "signing_key");
    final String algorithm =
        requireString(signingKey.get("algorithm"), "signing_key.algorithm").toLowerCase();
    if (!"ed25519".equals(algorithm)) {
      throw new IllegalStateException("Unsupported signing key algorithm: " + algorithm);
    }
    final String publicKeyHex =
        requireString(signingKey.get("public_key_hex"), "signing_key.public_key_hex");
    final byte[] publicKey = hexToBytes(publicKeyHex, "signing_key.public_key_hex");
    if (publicKey.length != 32) {
      throw new IllegalStateException(
          "signing_key.public_key_hex must be 32 bytes (found " + publicKey.length + ")");
    }
    final String seedHex = requireString(signingKey.get("seed_hex"), "signing_key.seed_hex");
    hexToBytes(seedHex, "signing_key.seed_hex");
  }

  private static void verifySignature(
      final String name,
      final AuthorityKey authorityKey,
      final byte[] payloadBytes,
      final byte[] signature) {
    if (authorityKey.curveId() != CURVE_ED25519) {
      throw new IllegalStateException(
          name + ": unsupported authority curve id " + authorityKey.curveId());
    }
    final byte[] publicKey = authorityKey.publicKey();
    if (publicKey.length != 32) {
      throw new IllegalStateException(
          name + ": authority public key must be 32 bytes (found " + publicKey.length + ")");
    }
    if (signature.length != 64) {
      throw new IllegalStateException(
          name + ": signature length mismatch (expected 64, found " + signature.length + ")");
    }
    final byte[] hash = canonicalHashBytes(payloadBytes);
    final Ed25519Signer verifier = new Ed25519Signer();
    verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
    verifier.update(hash, 0, hash.length);
    if (!verifier.verifySignature(signature)) {
      throw new IllegalStateException(name + ": signature verification failed");
    }
  }

  private static void validateCompatibility(
      final String name,
      final byte[] payloadBytes,
      final byte[] signedBytes,
      final String chain,
      final String authority,
      final Long ttl,
      final Long nonce,
      final AuthorityKey authorityKey) {
    final RawPayload raw = decodePayloadRaw(name, payloadBytes);
    assertEquals(
        name + ": chain mismatch vs payload bytes",
        chain,
        raw.chainId());
    assertEquals(
        name + ": authority mismatch vs payload bytes",
        authority,
        raw.authority());
    assertTrue(
        name + ": TTL mismatch vs payload bytes",
        optionalLongEquals(raw.timeToLiveMs(), ttl));
    assertTrue(
        name + ": nonce mismatch vs payload bytes",
        optionalLongEquals(raw.nonce(), nonce));
    if (raw.executable().isIvm()) {
      final TransactionPayload payload;
      try {
        payload = PAYLOAD_CODEC.decodeTransaction(payloadBytes);
      } catch (final Exception ex) {
        throw new IllegalStateException(name + ": failed to decode payload", ex);
      }
      assertEquals(
          name + ": chain mismatch vs decoded payload",
          chain,
          payload.chainId());
      assertEquals(
          name + ": authority mismatch vs decoded payload",
          authority,
          payload.authority());
      assertTrue(
          name + ": TTL mismatch vs decoded payload",
          optionalLongEquals(payload.timeToLiveMs(), ttl));
      assertTrue(
          name + ": nonce mismatch vs decoded payload",
          optionalIntEquals(payload.nonce(), nonce));
      assertArrayEquals(
          name + ": IVM bytes mismatch vs decoded payload",
          raw.executable().ivmBytes(),
          payload.executable().ivmBytes());
      final byte[] reencoded;
      try {
        reencoded = PAYLOAD_CODEC.encodeTransaction(payload);
      } catch (final Exception ex) {
        throw new IllegalStateException(name + ": failed to re-encode payload", ex);
      }
      assertArrayEquals(
          name + ": payload bytes differ after Android re-encoding",
          payloadBytes,
          reencoded);
    } else {
      // TODO: Re-encode instruction payloads once Android SDK can build instruction transactions.
    }

    final SignedParts signedParts = decodeSignedParts(name, signedBytes);
    assertArrayEquals(
        name + ": signed payload mismatch vs manifest payload bytes",
        payloadBytes,
        signedParts.payloadBytes());
    verifySignature(name, authorityKey, payloadBytes, signedParts.signature());
    if (raw.executable().isIvm()) {
      final SignedTransaction signed =
          new SignedTransaction(payloadBytes, signedParts.signature(), new byte[0], SIGNED_SCHEMA);
      final byte[] encodedSigned;
      try {
        encodedSigned = SignedTransactionEncoder.encode(signed);
      } catch (final Exception ex) {
        throw new IllegalStateException(name + ": failed to encode signed transaction", ex);
      }
      assertArrayEquals(
          name + ": signed bytes differ after Android re-encoding",
          signedBytes,
          encodedSigned);

      final byte[] versioned;
      try {
        versioned = SignedTransactionEncoder.encodeVersioned(signed);
      } catch (final Exception ex) {
        throw new IllegalStateException(name + ": failed to encode versioned signed transaction", ex);
      }
      assertEquals(
          name + ": versioned length mismatch",
          signedBytes.length + 1,
          versioned.length);
      assertEquals(name + ": versioned prefix mismatch", VERSION_BYTE, versioned[0]);
      assertArrayEquals(
          name + ": versioned payload mismatch",
          signedBytes,
          Arrays.copyOfRange(versioned, 1, versioned.length));
    }

    compatChecked++;
  }

  private static SignedParts decodeSignedParts(final String name, final byte[] signedBytes) {
    final NoritoDecoder decoder = new NoritoDecoder(signedBytes, NoritoHeader.MINOR_VERSION);
    final byte[] signatureField = readField(decoder, name + ".signed.signature");
    final byte[] payloadField = readField(decoder, name + ".signed.payload");
    final byte[] attachmentsField = readField(decoder, name + ".signed.attachments");
    final byte[] multisigField = readField(decoder, name + ".signed.multisig_signatures");
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signed transaction payload has trailing bytes");
    }
    final byte[] signature = decodeSignature(name, signatureField);
    decodeOptionField(name + ".signed.attachments", attachmentsField);
    decodeOptionField(name + ".signed.multisig_signatures", multisigField);
    return new SignedParts(signature, payloadField);
  }

  private static byte[] decodeSignature(final String name, final byte[] signatureField) {
    final NoritoDecoder fieldDecoder = new NoritoDecoder(signatureField, NoritoHeader.MINOR_VERSION);
    final byte[] inner = readField(fieldDecoder, name + ".signed.signature.inner");
    if (fieldDecoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signature field has trailing bytes");
    }
    final NoritoDecoder decoder = new NoritoDecoder(inner, NoritoHeader.MINOR_VERSION);
    final byte[] signature = BYTE_VECTOR_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": signature payload has trailing bytes");
    }
    return signature;
  }

  private static Optional<byte[]> decodeOptionField(final String name, final byte[] fieldBytes) {
    final NoritoDecoder decoder = new NoritoDecoder(fieldBytes, NoritoHeader.MINOR_VERSION);
    final int tag = decoder.readByte();
    if (tag == 0) {
      if (decoder.remaining() != 0) {
        throw new IllegalStateException(name + ": Option::None has trailing bytes");
      }
      return Optional.empty();
    }
    if (tag != 1) {
      throw new IllegalStateException(name + ": invalid Option tag " + tag);
    }
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalStateException(name + ": Option payload too large");
    }
    final byte[] payload = decoder.readBytes((int) length);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": Option payload has trailing bytes");
    }
    return Optional.of(payload);
  }

  private static AuthorityKey parseAuthorityKey(final String authority, final String context) {
    final int atIndex = authority.indexOf('@');
    if (atIndex <= 0 || atIndex == authority.length() - 1) {
      throw new IllegalStateException(context + " must be in address@domain form");
    }
    final String addressPart = authority.substring(0, atIndex);
    final AccountAddress.ParseResult parsed;
    try {
      parsed = AccountAddress.parseAny(addressPart, null);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException(context + " is not a valid account address", ex);
    }
    final byte[] canonical = parsed.address.canonicalBytes();
    return extractAuthorityKey(canonical, context);
  }

  private static AuthorityKey extractAuthorityKey(final byte[] canonical, final String context) {
    if (canonical.length < 4) {
      throw new IllegalStateException(context + ": canonical address too short");
    }
    int cursor = 1;
    final int domainTag = canonical[cursor++] & 0xFF;
    switch (domainTag) {
      case 0x00 -> {}
      case 0x01 -> cursor += 12;
      case 0x02 -> cursor += 4;
      default -> throw new IllegalStateException(context + ": unknown domain tag " + domainTag);
    }
    if (cursor >= canonical.length) {
      throw new IllegalStateException(context + ": missing controller tag");
    }
    final int controllerTag = canonical[cursor++] & 0xFF;
    if (controllerTag != CONTROLLER_SINGLE) {
      throw new IllegalStateException(
          context + ": unsupported controller tag " + controllerTag);
    }
    if (cursor + 2 > canonical.length) {
      throw new IllegalStateException(context + ": missing curve id/key length");
    }
    final int curveId = canonical[cursor++] & 0xFF;
    final int keyLen = canonical[cursor++] & 0xFF;
    if (keyLen <= 0) {
      throw new IllegalStateException(context + ": invalid key length " + keyLen);
    }
    final int end = cursor + keyLen;
    if (end != canonical.length) {
      throw new IllegalStateException(context + ": unexpected trailing bytes in authority");
    }
    final byte[] publicKey = Arrays.copyOfRange(canonical, cursor, end);
    return new AuthorityKey(curveId, publicKey);
  }

  private static byte[] readField(final NoritoDecoder decoder, final String field) {
    final boolean compact = decoder.compactLenActive();
    final long length = decoder.readLength(compact);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalStateException(field + " length too large: " + length);
    }
    return decoder.readBytes((int) length);
  }

  private static RawPayload decodePayloadRaw(final String name, final byte[] payloadBytes) {
    final NoritoDecoder decoder = new NoritoDecoder(payloadBytes, NoritoHeader.MINOR_VERSION);
    final byte[] chainField = readField(decoder, name + ".payload.chain_id");
    final byte[] authorityField = readField(decoder, name + ".payload.authority");
    final byte[] creationField = readField(decoder, name + ".payload.creation_time_ms");
    final byte[] executableField = readField(decoder, name + ".payload.executable");
    final byte[] ttlField = readField(decoder, name + ".payload.time_to_live_ms");
    final byte[] nonceField = readField(decoder, name + ".payload.nonce");
    final byte[] metadataField = readField(decoder, name + ".payload.metadata");
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(name + ": payload has trailing bytes");
    }
    final String chainId = decodeFieldPayload(chainField, STRING_ADAPTER, name + ".payload.chain_id");
    final String authority =
        decodeFieldPayload(authorityField, STRING_ADAPTER, name + ".payload.authority");
    decodeFieldPayload(creationField, UINT64_ADAPTER, name + ".payload.creation_time_ms");
    final ExecutableEnvelope executable =
        decodeExecutableEnvelope(name, executableField);
    final Optional<Long> ttl =
        decodeFieldPayload(ttlField, TTL_ADAPTER, name + ".payload.time_to_live_ms");
    final Optional<Long> nonce =
        decodeFieldPayload(nonceField, NONCE_ADAPTER, name + ".payload.nonce");
    validateMetadataField(metadataField, name + ".payload.metadata");
    return new RawPayload(chainId, authority, ttl, nonce, executable);
  }

  private static ExecutableEnvelope decodeExecutableEnvelope(
      final String name,
      final byte[] executableField) {
    final NoritoDecoder decoder = new NoritoDecoder(executableField, NoritoHeader.MINOR_VERSION);
    final long tag = UINT32_ADAPTER.decode(decoder);
    if (tag == 1L) {
      final byte[] bytecodeField =
          readField(decoder, name + ".payload.executable.ivm");
      final byte[] ivmBytes =
          decodeFieldPayload(bytecodeField, BYTE_VECTOR_ADAPTER, name + ".payload.executable.ivm");
      if (decoder.remaining() != 0) {
        throw new IllegalStateException(name + ": executable has trailing bytes");
      }
      return ExecutableEnvelope.forIvm(ivmBytes);
    }
    if (tag == 0L) {
      final byte[] instructionsField =
          readField(decoder, name + ".payload.executable.instructions");
      final List<InstructionEnvelope> instructions =
          decodeInstructionEnvelopes(instructionsField, name + ".payload.executable.instructions");
      if (decoder.remaining() != 0) {
        throw new IllegalStateException(name + ": executable has trailing bytes");
      }
      return ExecutableEnvelope.forInstructions(instructions);
    }
    throw new IllegalStateException(name + ": unknown executable tag " + tag);
  }

  private static List<InstructionEnvelope> decodeInstructionEnvelopes(
      final byte[] payload,
      final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.MINOR_VERSION);
    final List<InstructionEnvelope> instructions = INSTRUCTION_LIST_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(field + ": instruction list has trailing bytes");
    }
    for (final InstructionEnvelope envelope : instructions) {
      if (envelope.wireName().isBlank()) {
        throw new IllegalStateException(field + ": instruction name must not be blank");
      }
      final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(envelope.payload(), null);
      decoded.header().validateChecksum(decoded.payload());
    }
    return instructions;
  }

  private static void validateMetadataField(final byte[] payload, final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.MINOR_VERSION);
    final List<MetadataEntry> entries = METADATA_ENTRY_LIST_ADAPTER.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(field + ": metadata has trailing bytes");
    }
    final Map<String, String> seen = new LinkedHashMap<>();
    String previousKey = null;
    for (final MetadataEntry entry : entries) {
      final String key = entry.key();
      if (key.isBlank()) {
        throw new IllegalStateException(field + ": metadata key must not be blank");
      }
      if (seen.put(key, entry.value()) != null) {
        throw new IllegalStateException(field + ": duplicate metadata key " + key);
      }
      if (previousKey != null && previousKey.compareTo(key) > 0) {
        throw new IllegalStateException(field + ": metadata keys must be sorted");
      }
      previousKey = key;
    }
  }

  private static <T> T decodeFieldPayload(
      final byte[] payload,
      final TypeAdapter<T> adapter,
      final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.MINOR_VERSION);
    final T value = adapter.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalStateException(field + ": trailing bytes after field payload");
    }
    return value;
  }

  private static final class SignedParts {
    private final byte[] signature;
    private final byte[] payloadBytes;

    private SignedParts(final byte[] signature, final byte[] payloadBytes) {
      this.signature = signature;
      this.payloadBytes = payloadBytes;
    }

    private byte[] signature() {
      return Arrays.copyOf(signature, signature.length);
    }

    private byte[] payloadBytes() {
      return Arrays.copyOf(payloadBytes, payloadBytes.length);
    }
  }

  private static final class RawPayload {
    private final String chainId;
    private final String authority;
    private final Optional<Long> timeToLiveMs;
    private final Optional<Long> nonce;
    private final ExecutableEnvelope executable;

    private RawPayload(
        final String chainId,
        final String authority,
        final Optional<Long> timeToLiveMs,
        final Optional<Long> nonce,
        final ExecutableEnvelope executable) {
      this.chainId = chainId;
      this.authority = authority;
      this.timeToLiveMs = timeToLiveMs;
      this.nonce = nonce;
      this.executable = executable;
    }

    private String chainId() {
      return chainId;
    }

    private String authority() {
      return authority;
    }

    private Optional<Long> timeToLiveMs() {
      return timeToLiveMs;
    }

    private Optional<Long> nonce() {
      return nonce;
    }

    private ExecutableEnvelope executable() {
      return executable;
    }
  }

  private static final class ExecutableEnvelope {
    private final byte[] ivmBytes;
    private final List<InstructionEnvelope> instructions;

    private ExecutableEnvelope(final byte[] ivmBytes, final List<InstructionEnvelope> instructions) {
      this.ivmBytes = ivmBytes;
      this.instructions = instructions;
    }

    private static ExecutableEnvelope forIvm(final byte[] ivmBytes) {
      return new ExecutableEnvelope(Arrays.copyOf(ivmBytes, ivmBytes.length), null);
    }

    private static ExecutableEnvelope forInstructions(final List<InstructionEnvelope> instructions) {
      return new ExecutableEnvelope(new byte[0], new ArrayList<>(instructions));
    }

    private boolean isIvm() {
      return instructions == null;
    }

    private byte[] ivmBytes() {
      return Arrays.copyOf(ivmBytes, ivmBytes.length);
    }
  }

  private static final class InstructionEnvelope {
    private final String wireName;
    private final byte[] payload;

    private InstructionEnvelope(final String wireName, final byte[] payload) {
      this.wireName = wireName;
      this.payload = payload;
    }

    private String wireName() {
      return wireName;
    }

    private byte[] payload() {
      return Arrays.copyOf(payload, payload.length);
    }
  }

  private static final class InstructionEnvelopeAdapter implements TypeAdapter<InstructionEnvelope> {
    @Override
    public void encode(final NoritoEncoder encoder, final InstructionEnvelope value) {
      throw new UnsupportedOperationException("Instruction envelope encoding is not supported");
    }

    @Override
    public InstructionEnvelope decode(final NoritoDecoder decoder) {
      final String name = STRING_ADAPTER.decode(decoder);
      final byte[] payload = BYTE_VECTOR_ADAPTER.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Instruction envelope has trailing bytes");
      }
      return new InstructionEnvelope(name, payload);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class MetadataEntry {
    private final String key;
    private final String value;

    private MetadataEntry(final String key, final String value) {
      this.key = key;
      this.value = value;
    }

    private String key() {
      return key;
    }

    private String value() {
      return value;
    }
  }

  private static final class MetadataEntryAdapter implements TypeAdapter<MetadataEntry> {
    @Override
    public void encode(final NoritoEncoder encoder, final MetadataEntry value) {
      throw new UnsupportedOperationException("Metadata entry encoding is not supported");
    }

    @Override
    public MetadataEntry decode(final NoritoDecoder decoder) {
      final String key = STRING_ADAPTER.decode(decoder);
      final String value = STRING_ADAPTER.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException("Metadata entry has trailing bytes");
      }
      return new MetadataEntry(key, value);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class AuthorityKey {
    private final int curveId;
    private final byte[] publicKey;

    private AuthorityKey(final int curveId, final byte[] publicKey) {
      this.curveId = curveId;
      this.publicKey = publicKey;
    }

    private int curveId() {
      return curveId;
    }

    private byte[] publicKey() {
      return Arrays.copyOf(publicKey, publicKey.length);
    }
  }

  private static Map<String, TransactionPayloadFixtures.Fixture> loadPayloadFixtures(final Path baseDir)
      throws Exception {
    Path payloadPath = resolveFixturePath("transaction_payloads.json");
    if (baseDir != null) {
      final Path local = baseDir.resolve("transaction_payloads.json");
      if (Files.exists(local)) {
        payloadPath = local;
      }
    }
    final List<TransactionPayloadFixtures.Fixture> fixtures = TransactionPayloadFixtures.load(payloadPath);
    final Map<String, TransactionPayloadFixtures.Fixture> map = new LinkedHashMap<>();
    for (TransactionPayloadFixtures.Fixture fixture : fixtures) {
      map.put(fixture.name(), fixture);
    }
    return map;
  }

  private static Path resolveFixturePath(final String filename) {
    final String[] candidates =
        new String[] {
          "java/iroha_android/src/test/resources/" + filename,
          "src/test/resources/" + filename,
          "../src/test/resources/" + filename,
          "../../src/test/resources/" + filename
        };
    for (final String candidate : candidates) {
      final Path path = Paths.get(candidate);
      if (Files.exists(path)) {
        return path;
      }
    }
    throw new IllegalStateException(
        "Fixture not found: " + Paths.get(candidates[0]).toAbsolutePath());
  }

  private static boolean optionalLongEquals(final Optional<Long> optional, final Long expected) {
    if (!optional.isPresent() && expected == null) {
      return true;
    }
    return optional.isPresent() && expected != null && Objects.equals(optional.get(), expected);
  }

  private static boolean optionalIntEquals(final Optional<Integer> optional, final Long expected) {
    if (!optional.isPresent() && expected == null) {
      return true;
    }
    return optional.isPresent()
        && expected != null
        && Objects.equals(optional.get().longValue(), expected.longValue());
  }

  private static Map<String, Object> asMap(final Object value, final String field) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException("Expected object for " + field);
    }
    @SuppressWarnings("unchecked")
    final Map<?, ?> raw = (Map<?, ?>) value;
    final Map<String, Object> copy = new LinkedHashMap<>();
    raw.forEach((key, v) -> copy.put(Objects.toString(key), v));
    return copy;
  }

  private static List<Object> asList(final Object value, final String field) {
    if (!(value instanceof List)) {
      throw new IllegalStateException("Expected array for " + field);
    }
    @SuppressWarnings("unchecked")
    final List<Object> list = (List<Object>) value;
    return new ArrayList<>(list);
  }

  private static String requireString(final Object value, final String field) {
    if (!(value instanceof String) || ((String) value).trim().isEmpty()) {
      throw new IllegalStateException(field + " must be a non-empty string");
    }
    return (String) value;
  }

  private static long requireNumber(final Object value, final String field) {
    if (!(value instanceof Number)) {
      throw new IllegalStateException(field + " must be a number");
    }
    return ((Number) value).longValue();
  }

  private static Long optionalNumber(final Object value) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Number)) {
      throw new IllegalStateException("Expected number or null");
    }
    return ((Number) value).longValue();
  }

  private static byte[] hexToBytes(final String hex, final String field) {
    if (hex == null) {
      throw new IllegalArgumentException(field + " must not be null");
    }
    final String normalized = hex.trim();
    if (normalized.length() % 2 != 0) {
      throw new IllegalArgumentException(field + " must have even length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(normalized.charAt(i * 2), 16);
      final int lo = Character.digit(normalized.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " has invalid hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

}
