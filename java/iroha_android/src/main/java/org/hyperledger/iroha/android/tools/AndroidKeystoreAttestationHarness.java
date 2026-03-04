package org.hyperledger.iroha.android.tools;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.cert.Certificate;
import java.security.cert.CertificateException;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Objects;
import java.util.Set;
import java.util.stream.Collectors;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationResult;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerificationException;
import org.hyperledger.iroha.android.crypto.keystore.attestation.AttestationVerifier;

/**
 * Command-line harness that validates Android Keystore attestation bundles using {@link
 * AttestationVerifier}. It is intended for use with physical StrongBox-capable devices so labs can
 * archive attestation bundles and confirm firmware coverage before onboarding pilot networks.
 *
 * <p>The harness accepts DER or PEM encoded certificate chains and requires at least one trusted
 * root. Challenges can be supplied via command-line flag or a {@code challenge.hex} file located in
 * the bundle directory. Results are printed to stdout and may optionally be written to a JSON file
 * for archival.
 */
public final class AndroidKeystoreAttestationHarness {

  /** Summary of the attestation verification. */
  public static final class Result {
    private final String alias;
    private final AttestationResult.SecurityLevel attestationLevel;
    private final AttestationResult.SecurityLevel keymasterLevel;
    private final boolean strongBoxAttestation;
    private final String challengeHex;
    private final int chainLength;

    Result(
        final String alias,
        final AttestationResult.SecurityLevel attestationLevel,
        final AttestationResult.SecurityLevel keymasterLevel,
        final boolean strongBoxAttestation,
        final String challengeHex,
        final int chainLength) {
      this.alias = alias;
      this.attestationLevel = attestationLevel;
      this.keymasterLevel = keymasterLevel;
      this.strongBoxAttestation = strongBoxAttestation;
      this.challengeHex = challengeHex;
      this.chainLength = chainLength;
    }

    public String alias() {
      return alias;
    }

    public AttestationResult.SecurityLevel attestationSecurityLevel() {
      return attestationLevel;
    }

    public AttestationResult.SecurityLevel keymasterSecurityLevel() {
      return keymasterLevel;
    }

    public boolean strongBoxAttestation() {
      return strongBoxAttestation;
    }

    public String challengeHex() {
      return challengeHex;
    }

    public int chainLength() {
      return chainLength;
    }

    String toJson() {
      final StringBuilder builder = new StringBuilder();
      builder.append("{\n");
      builder
          .append("  \"alias\": \"")
          .append(escapeJson(alias))
          .append("\",\n");
      builder
          .append("  \"attestation_security_level\": \"")
          .append(attestationLevel)
          .append("\",\n");
      builder
          .append("  \"keymaster_security_level\": \"")
          .append(keymasterLevel)
          .append("\",\n");
      builder
          .append("  \"strongbox_attestation\": ")
          .append(strongBoxAttestation)
          .append(",\n");
      builder
          .append("  \"challenge_hex\": \"")
          .append(challengeHex)
          .append("\",\n");
      builder
          .append("  \"chain_length\": ")
          .append(chainLength)
          .append('\n');
      builder.append("}\n");
      return builder.toString();
    }
  }

  private AndroidKeystoreAttestationHarness() {}

  public static void main(final String[] args) {
    try {
      final Result result = run(args);
      System.out.printf(
          Locale.US,
          "[attestation] alias=%s attestation=%s keymaster=%s strongbox=%s chain=%d challenge=%s%n",
          result.alias(),
          result.attestationSecurityLevel(),
          result.keymasterSecurityLevel(),
          result.strongBoxAttestation(),
          result.chainLength(),
          result.challengeHex());
    } catch (final IllegalArgumentException ex) {
      System.err.println("[attestation] " + ex.getMessage());
      System.exit(1);
    } catch (final Exception ex) {
      System.err.println("[attestation] verification failed: " + ex.getMessage());
      ex.printStackTrace(System.err);
      System.exit(1);
    }
  }

  /**
   * Executes the harness. Exposed for unit testing so callers can assert on the resulting summary.
   */
  public static Result run(final String[] args)
      throws IOException, CertificateException, AttestationVerificationException {
    final Arguments arguments = Arguments.parse(args);

    final List<X509Certificate> chain = loadCertificateChain(arguments);
    if (chain.isEmpty()) {
      throw new IllegalArgumentException("No attestation certificates found");
    }

    final AttestationVerifier.Builder verifierBuilder = AttestationVerifier.builder();
    for (final X509Certificate root :
        loadTrustedRoots(
            arguments.trustedRoots, arguments.trustedRootDirs, arguments.trustedRootBundles)) {
      verifierBuilder.addTrustedRoot(root);
    }
    verifierBuilder.requireStrongBox(arguments.requireStrongBox);

    final KeyAttestation.Builder attestationBuilder = KeyAttestation.builder();
    attestationBuilder.setAlias(arguments.alias);
    for (final X509Certificate certificate : chain) {
      attestationBuilder.addCertificate(certificate);
    }
    final KeyAttestation attestation = attestationBuilder.build();
    final AttestationVerifier verifier = verifierBuilder.build();
    final AttestationResult attestationResult = verifier.verify(attestation, arguments.challenge);

    final Result result =
        new Result(
            arguments.alias,
            attestationResult.attestationSecurityLevel(),
            attestationResult.keymasterSecurityLevel(),
            attestationResult.isStrongBoxAttestation(),
            toHex(attestationResult.attestationChallenge()),
            attestationResult.certificateChain().size());

    if (arguments.output != null) {
      Files.createDirectories(arguments.output.getParent());
      Files.writeString(arguments.output, result.toJson(), StandardCharsets.UTF_8);
    }

    return result;
  }

  private static List<X509Certificate> loadCertificateChain(final Arguments arguments)
      throws IOException, CertificateException {
    final List<X509Certificate> certificates = new ArrayList<>();
    if (arguments.chainFile != null) {
      certificates.addAll(readCertificates(arguments.chainFile));
      return certificates;
    }

    final Path bundleDir =
        Objects.requireNonNull(arguments.bundleDir, "bundle directory must be specified");
    final Path chainPem = bundleDir.resolve("chain.pem");
    if (Files.isRegularFile(chainPem)) {
      certificates.addAll(readCertificates(chainPem));
    } else {
      certificates.addAll(readCertificatesFromDirectory(bundleDir));
    }
    return certificates;
  }

  private static List<X509Certificate> readCertificatesFromDirectory(final Path directory)
      throws IOException, CertificateException {
    final List<Path> candidates = new ArrayList<>();
    try (DirectoryStream<Path> stream = Files.newDirectoryStream(directory)) {
      for (Path path : stream) {
        if (!Files.isRegularFile(path)) {
          continue;
        }
        final String lower = path.getFileName().toString().toLowerCase(Locale.US);
        if (lower.endsWith(".pem") || lower.endsWith(".crt") || lower.endsWith(".cer")
            || lower.endsWith(".der")) {
          candidates.add(path);
        }
      }
    }
    Collections.sort(candidates);
    final List<X509Certificate> certificates = new ArrayList<>();
    for (Path candidate : candidates) {
      certificates.addAll(readCertificates(candidate));
    }
    return certificates;
  }

  private static List<X509Certificate> readCertificates(final Path file)
      throws IOException, CertificateException {
    try (InputStream in = Files.newInputStream(file)) {
      final byte[] bytes = in.readAllBytes();
      return readCertificates(bytes);
    }
  }

  private static List<X509Certificate> readCertificates(final byte[] data)
      throws CertificateException, IOException {
    final CertificateFactory factory = CertificateFactory.getInstance("X.509");
    final List<X509Certificate> certificates = new ArrayList<>();
    try (InputStream in = new ByteArrayInputStream(data)) {
      final Collection<? extends Certificate> decoded = factory.generateCertificates(in);
      for (Certificate certificate : decoded) {
        certificates.add((X509Certificate) certificate);
      }
      if (!certificates.isEmpty()) {
        return certificates;
      }
    } catch (final CertificateException ex) {
      // Fall back to single-certificate decoding below.
    }

    try (InputStream single = new ByteArrayInputStream(data)) {
      final X509Certificate certificate = (X509Certificate) factory.generateCertificate(single);
      return List.of(certificate);
    }
  }

  private static Set<X509Certificate> loadTrustedRoots(
      final List<Path> rootPaths,
      final List<Path> rootDirectories,
      final List<Path> rootBundles)
      throws IOException, CertificateException {
    if (rootPaths.isEmpty() && rootDirectories.isEmpty() && rootBundles.isEmpty()) {
      throw new IllegalArgumentException(
          "At least one --trust-root, --trust-root-dir, or --trust-root-bundle must be supplied");
    }
    final Set<X509Certificate> roots = new LinkedHashSet<>();
    for (Path path : rootPaths) {
      if (!Files.isRegularFile(path)) {
        throw new IllegalArgumentException("--trust-root does not refer to a file: " + path);
      }
      roots.addAll(readCertificates(path));
    }
    for (Path directory : rootDirectories) {
      if (!Files.isDirectory(directory)) {
        throw new IllegalArgumentException(
            "--trust-root-dir does not refer to a directory: " + directory);
      }
      collectCertificatesFromDirectory(directory, roots);
    }
    for (Path bundle : rootBundles) {
      loadCertificatesFromBundle(bundle, roots);
    }
    if (roots.isEmpty()) {
      throw new IllegalArgumentException(
          "No trust roots were loaded; verify the provided inputs contain certificates.");
    }
    return roots;
  }

  private static void loadCertificatesFromBundle(
      final Path bundle, final Set<X509Certificate> roots) throws IOException, CertificateException {
    if (!Files.isRegularFile(bundle)) {
      throw new IllegalArgumentException("--trust-root-bundle does not refer to a file: " + bundle);
    }
    final String filename = bundle.getFileName().toString().toLowerCase(Locale.US);
    if (!filename.endsWith(".zip")) {
      throw new IllegalArgumentException(
          "--trust-root-bundle must reference a .zip archive: " + bundle);
    }
    boolean added = false;
    try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(bundle))) {
      ZipEntry entry;
      while ((entry = zip.getNextEntry()) != null) {
        if (entry.isDirectory()) {
          continue;
        }
        if (!isCertificateFilename(entry.getName())) {
          continue;
        }
        final ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        zip.transferTo(buffer);
        roots.addAll(readCertificates(buffer.toByteArray()));
        added = true;
      }
    }
    if (!added) {
      throw new IllegalArgumentException(
          "No certificates were found inside bundle: " + bundle.toAbsolutePath());
    }
  }

  private static void collectCertificatesFromDirectory(
      final Path directory, final Set<X509Certificate> roots)
      throws IOException, CertificateException {
    try (DirectoryStream<Path> entries = Files.newDirectoryStream(directory)) {
      for (Path entry : entries) {
        if (Files.isDirectory(entry)) {
          collectCertificatesFromDirectory(entry, roots);
        } else if (isCertificateFilename(entry.getFileName().toString())) {
          roots.addAll(readCertificates(entry));
        } else if (isZipFilename(entry.getFileName().toString())) {
          loadCertificatesFromBundle(entry, roots);
        }
      }
    }
  }

  private static boolean isCertificateFilename(final String filename) {
    final String lower = filename.toLowerCase(Locale.US);
    return lower.endsWith(".pem")
        || lower.endsWith(".der")
        || lower.endsWith(".crt")
        || lower.endsWith(".cer");
  }

  private static boolean isZipFilename(final String filename) {
    return filename.toLowerCase(Locale.US).endsWith(".zip");
  }

  private static byte[] parseHex(final String value) {
    if (value == null || value.isEmpty()) {
      return null;
    }
    final String normalized = value.replaceAll("\\s+", "");
    if ((normalized.length() & 1) != 0) {
      throw new IllegalArgumentException("Challenge hex must contain an even number of digits");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < normalized.length(); i += 2) {
      final String slice = normalized.substring(i, i + 2);
      out[i / 2] = (byte) Integer.parseInt(slice, 16);
    }
    return out;
  }

  private static String toHex(final byte[] data) {
    if (data == null || data.length == 0) {
      return "";
    }
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (byte b : data) {
      builder.append(String.format(Locale.US, "%02X", b));
    }
    return builder.toString();
  }

  private static String escapeJson(final String value) {
    final StringBuilder builder = new StringBuilder();
    for (char ch : value.toCharArray()) {
      switch (ch) {
        case '"':
          builder.append("\\\"");
          break;
        case '\\':
          builder.append("\\\\");
          break;
        case '\b':
          builder.append("\\b");
          break;
        case '\f':
          builder.append("\\f");
          break;
        case '\n':
          builder.append("\\n");
          break;
        case '\r':
          builder.append("\\r");
          break;
        case '\t':
          builder.append("\\t");
          break;
        default:
          if (ch < 0x20) {
            builder.append(String.format(Locale.US, "\\u%04X", (int) ch));
          } else {
            builder.append(ch);
          }
      }
    }
    return builder.toString();
  }

  private static final class Arguments {
    final Path bundleDir;
    final Path chainFile;
    final List<Path> trustedRoots;
    final List<Path> trustedRootDirs;
    final List<Path> trustedRootBundles;
    final boolean requireStrongBox;
    final String alias;
    final byte[] challenge;
    final Path output;

    private Arguments(
        final Path bundleDir,
        final Path chainFile,
        final List<Path> trustedRoots,
        final List<Path> trustedRootDirs,
        final List<Path> trustedRootBundles,
        final boolean requireStrongBox,
        final String alias,
        final byte[] challenge,
        final Path output) {
      this.bundleDir = bundleDir;
      this.chainFile = chainFile;
      this.trustedRoots = trustedRoots;
      this.trustedRootDirs = trustedRootDirs;
      this.trustedRootBundles = trustedRootBundles;
      this.requireStrongBox = requireStrongBox;
      this.alias = alias;
      this.challenge = challenge;
      this.output = output;
    }

    static Arguments parse(final String[] args) throws IOException {
      Path bundleDir = null;
      Path chainFile = null;
      final List<Path> trustedRoots = new ArrayList<>();
      final List<Path> trustedRootDirs = new ArrayList<>();
      final List<Path> trustedRootBundles = new ArrayList<>();
      boolean requireStrongBox = false;
      String alias = "android-keystore-alias";
      byte[] challenge = null;
      Path output = null;

      for (int i = 0; i < args.length; ++i) {
        final String arg = args[i];
        switch (arg) {
          case "--bundle-dir":
            bundleDir = Paths.get(requireValue(args, ++i, "--bundle-dir"));
            break;
          case "--chain":
            chainFile = Paths.get(requireValue(args, ++i, "--chain"));
            break;
          case "--trust-root":
            trustedRoots.add(Paths.get(requireValue(args, ++i, "--trust-root")));
            break;
          case "--trust-root-dir":
            trustedRootDirs.add(Paths.get(requireValue(args, ++i, "--trust-root-dir")));
            break;
          case "--trust-root-bundle":
            trustedRootBundles.add(Paths.get(requireValue(args, ++i, "--trust-root-bundle")));
            break;
          case "--require-strongbox":
            requireStrongBox = true;
            break;
          case "--alias":
            alias = requireValue(args, ++i, "--alias");
            break;
          case "--challenge-hex":
            challenge = parseHex(requireValue(args, ++i, "--challenge-hex"));
            break;
          case "--challenge-file":
            challenge =
                parseHex(
                    Files.readString(Paths.get(requireValue(args, ++i, "--challenge-file")))
                        .trim());
            break;
          case "--output":
            output = Paths.get(requireValue(args, ++i, "--output"));
            break;
          case "--help":
          case "-h":
            throw new IllegalArgumentException(usage());
          default:
            throw new IllegalArgumentException("Unknown argument: " + arg);
        }
      }

      if (chainFile == null && bundleDir == null) {
        throw new IllegalArgumentException(
            "Either --chain or --bundle-dir must be provided (bundle-dir is recommended).");
      }

      if (bundleDir != null) {
        if (!Files.isDirectory(bundleDir)) {
          throw new IllegalArgumentException(
              "--bundle-dir does not refer to a directory: " + bundleDir);
        }
        if (challenge == null) {
          final Path challengeFile = bundleDir.resolve("challenge.hex");
          if (Files.isRegularFile(challengeFile)) {
            challenge = parseHex(Files.readString(challengeFile).trim());
          }
        }
        final Path aliasFile = bundleDir.resolve("alias.txt");
        if (Files.isRegularFile(aliasFile)) {
          alias = Files.readString(aliasFile).trim();
        }
        collectTrustRootsFromBundle(bundleDir, trustedRoots, trustedRootBundles);
      }

      if (challenge == null) {
        challenge = new byte[0];
      }

      final List<Path> normalizedRoots =
          trustedRoots.stream().map(Path::toAbsolutePath).collect(Collectors.toUnmodifiableList());
      final List<Path> normalizedRootDirs =
          trustedRootDirs.stream()
              .map(Path::toAbsolutePath)
              .collect(Collectors.toUnmodifiableList());

      return new Arguments(
          bundleDir == null ? null : bundleDir.toAbsolutePath(),
          chainFile == null ? null : chainFile.toAbsolutePath(),
          normalizedRoots,
          normalizedRootDirs,
          trustedRootBundles.stream()
              .map(Path::toAbsolutePath)
              .collect(Collectors.toUnmodifiableList()),
          requireStrongBox,
          alias,
          challenge,
          output == null ? null : output.toAbsolutePath());
    }

    private static String requireValue(final String[] args, final int index, final String flag) {
      if (index >= args.length) {
        throw new IllegalArgumentException(flag + " requires a value");
      }
      return args[index];
    }

    private static String usage() {
      return String.join(
          System.lineSeparator(),
          "Usage: android_keystore_attestation --bundle-dir <path> --trust-root <root.pem> [options]",
          "",
          "Required:",
          "  --bundle-dir <path>      Directory containing chain.pem/alias.txt/challenge.hex.",
          "  --trust-root <path>      Trusted root certificate (PEM/DER). Repeat as needed.",
          "                             Files named trust_root_*.pem in the bundle directory are detected",
          "                             automatically.",
          "",
          "Optional:",
          "  --trust-root-dir <path>  Directory containing PEM/DER/CRT trust anchors (recursively scanned).",
          "  --trust-root-bundle <zip>  ZIP archive containing trusted roots. Repeat as needed.",
          "                             Bundles named trust_root_bundle_*.zip inside the bundle directory",
          "                             are detected automatically.",
          "  --chain <path>           Explicit attestation chain file (PEM/DER). Overrides bundle.",
          "  --alias <alias>          Override alias (falls back to alias.txt).",
          "  --challenge-hex <hex>    Verification challenge (hex encoded).",
          "  --challenge-file <path>  File containing hex-encoded challenge.",
          "  --require-strongbox      Enforce StrongBox attestation.",
          "  --output <path>          Write result JSON to <path>.",
          "  --help                   Print this message.");
    }

    private static void collectTrustRootsFromBundle(
        final Path bundleDir,
        final List<Path> trustedRoots,
        final List<Path> trustedRootBundles)
        throws IOException {
      try (DirectoryStream<Path> entries = Files.newDirectoryStream(bundleDir)) {
        for (Path entry : entries) {
          if (!Files.isRegularFile(entry)) {
            continue;
          }
          final String filename = entry.getFileName().toString();
          final String lower = filename.toLowerCase(Locale.US);
          if (lower.startsWith("trust_root_bundle_") && lower.endsWith(".zip")) {
            trustedRootBundles.add(entry);
          } else if (lower.startsWith("trust_root_") && isCertificateFilename(lower)) {
            trustedRoots.add(entry);
          }
        }
      }
    }
  }
}
