package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.Arrays;
import java.util.Base64;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.ContractAddressValidator;
import org.hyperledger.iroha.android.model.NetworkId;

/** Recursive fail-closed admission for the exact first-release proposal wire contract. */
final class ParliamentProposalValidatorV1 {
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final BigInteger FIRST_RELEASE_MAX_EXACT_JSON_U64 =
      new BigInteger("9007199254740991");
  private static final Pattern KEBAB = Pattern.compile("[a-z0-9]+(?:-[a-z0-9]+)*");
  private static final Pattern ALPHANUMERIC_PRERELEASE =
      Pattern.compile("(?=.*[A-Za-z-])[A-Za-z0-9-]+");
  private static final Pattern UNSIGNED = Pattern.compile("0|[1-9][0-9]*");
  private static final Pattern QUANTITY =
      Pattern.compile("(?:0|[1-9][0-9]*)(?:\\.[0-9]*[1-9])?");
  private static final Pattern LOWER_HEX_32 = Pattern.compile("[0-9a-f]{64}");

  private ParliamentProposalValidatorV1() {}

  static Map<String, Object> parse(final byte[] bytes) {
    final String text = new String(bytes, StandardCharsets.UTF_8);
    if (!Arrays.equals(bytes, text.getBytes(StandardCharsets.UTF_8))) {
      throw invalid("proposal must be UTF-8 JSON");
    }
    final Map<String, Object> proposal = objectValue(JsonParser.parse(text), "proposal");
    exact(proposal, fields("kind", "payload"), "proposal");
    final String kind = text(proposal.get("kind"), "proposal.kind");
    final Map<String, Object> payload = objectValue(proposal.get("payload"), "proposal.payload");
    switch (kind) {
      case "DeployContract" -> deployContract(payload);
      case "RuntimeUpgrade" -> runtimeUpgrade(payload);
      case "SccpRouteGovernance" -> sccpRoute(payload);
      case "ValidationFeePolicy" -> validationFeePolicyProposal(payload);
      case "ValidationFeePayoutLifecycle" -> validationFeePayoutLifecycle(payload);
      case "MusubiRegistryGovernance" -> musubiAction(payload);
      case "SorafsProviderGovernance" -> sorafsProvider(payload);
      case "ContractLifecycleGovernance" -> contractLifecycle(payload);
      case "ContractEmergencyHold" -> contractEmergencyHold(payload);
      default -> throw invalid("proposal.kind is unknown or retired");
    }
    return proposal;
  }

  private static void deployContract(final Map<String, Object> value) {
    exact(
        value,
        fields(
            "contract_address",
            "code_hash",
            "abi_hash",
            "abi_version",
            "manifest_provenance"),
        "DeployContract");
    ContractAddressValidator.requireCanonicalV1(
        text(value.get("contract_address"), "contract_address"));
    lowerHex32(value.get("code_hash"), "code_hash");
    lowerHex32(value.get("abi_hash"), "abi_hash");
    if (!BigInteger.ONE.equals(uint(value.get("abi_version"), "abi_version"))) {
      throw invalid("abi_version must equal 1");
    }
    if (value.get("manifest_provenance") != null) {
      manifestProvenance(
          objectValue(value.get("manifest_provenance"), "manifest_provenance"),
          "manifest_provenance");
    }
  }

  private static void manifestProvenance(
      final Map<String, Object> value, final String label) {
    exact(value, fields("signer", "signature"), label);
    canonicalPublicKey(value.get("signer"), label + ".signer");
    canonicalSignature(value.get("signature"), label + ".signature");
  }

  private static void runtimeUpgrade(final Map<String, Object> value) {
    exact(value, fields("manifest"), "RuntimeUpgrade");
    final Map<String, Object> manifest =
        objectValue(value.get("manifest"), "RuntimeUpgrade.manifest");
    exact(
        manifest,
        fields(
            "name",
            "description",
            "abi_version",
            "abi_hash",
            "added_syscalls",
            "added_pointer_types",
            "start_height",
            "end_height",
            "sbom_digests",
            "slsa_attestation",
            "provenance"),
        "RuntimeUpgrade.manifest");
    text(manifest.get("name"), "manifest.name");
    string(manifest.get("description"), "manifest.description");
    if (!BigInteger.ONE.equals(uint(manifest.get("abi_version"), "manifest.abi_version"))) {
      throw invalid("manifest.abi_version must equal 1");
    }
    bytes(manifest.get("abi_hash"), 32, "manifest.abi_hash", false);
    final List<?> syscalls = list(manifest.get("added_syscalls"), "manifest.added_syscalls");
    final List<?> pointers =
        list(manifest.get("added_pointer_types"), "manifest.added_pointer_types");
    for (int index = 0; index < syscalls.size(); index++) {
      if (uint(syscalls.get(index), "manifest.added_syscalls[" + index + "]")
              .compareTo(BigInteger.valueOf(0xffff))
          > 0) {
        throw invalid("manifest syscall id exceeds u16");
      }
    }
    for (int index = 0; index < pointers.size(); index++) {
      if (uint(pointers.get(index), "manifest.added_pointer_types[" + index + "]")
              .compareTo(BigInteger.valueOf(0xffff))
          > 0) {
        throw invalid("manifest pointer type id exceeds u16");
      }
    }
    if (!syscalls.isEmpty() || !pointers.isEmpty()) {
      throw invalid("V1 ABI delta lists must be empty");
    }
    final BigInteger start = uint(manifest.get("start_height"), "manifest.start_height");
    final BigInteger end = uint(manifest.get("end_height"), "manifest.end_height");
    if (end.compareTo(start) <= 0) {
      throw invalid("manifest.end_height must exceed start_height");
    }
    final List<?> digests = list(manifest.get("sbom_digests"), "manifest.sbom_digests");
    for (int index = 0; index < digests.size(); index++) {
      final String label = "manifest.sbom_digests[" + index + "]";
      final Map<String, Object> digest = objectValue(digests.get(index), label);
      exact(digest, fields("algorithm", "digest"), label);
      text(digest.get("algorithm"), label + ".algorithm");
      canonicalBase64(digest.get("digest"), label + ".digest");
    }
    canonicalBase64(manifest.get("slsa_attestation"), "manifest.slsa_attestation");
    final List<?> provenance = list(manifest.get("provenance"), "manifest.provenance");
    for (int index = 0; index < provenance.size(); index++) {
      final String label = "manifest.provenance[" + index + "]";
      manifestProvenance(objectValue(provenance.get(index), label), label);
    }
  }

  private static void sccpRoute(final Map<String, Object> value) {
    exact(value, fields("anchor"), "SccpRouteGovernance");
    final Map<String, Object> anchor =
        objectValue(value.get("anchor"), "SccpRouteGovernance.anchor");
    exact(anchor, fields("network_id", "action"), "SccpRouteGovernance.anchor");
    NetworkId.parse(text(anchor.get("network_id"), "anchor.network_id"));
    SccpJsonParser.validateRouteGovernanceAction(
        objectValue(anchor.get("action"), "anchor.action"));
  }

  private static void validationFeePolicyProposal(final Map<String, Object> value) {
    exact(
        value,
        fields("proposal_operator", "policy", "payout_lifecycle_proposal_id"),
        "ValidationFeePolicy");
    account(value.get("proposal_operator"), "proposal_operator");
    final Map<String, Object> policy = objectValue(value.get("policy"), "policy");
    validationFeePolicy(policy);
    final byte[] lifecycle =
        value.get("payout_lifecycle_proposal_id") == null
            ? null
            : bytes(
                value.get("payout_lifecycle_proposal_id"),
                32,
                "payout_lifecycle_proposal_id",
                true);
    if ((policy.get("treasury_payout_binding") == null) != (lifecycle == null)) {
      throw invalid(
          "payout lifecycle id must be present exactly when the policy has a payout binding");
    }
  }

  private static void validationFeePolicy(final Map<String, Object> value) {
    exact(
        value,
        fields(
            "schema_version",
            "network_id",
            "policy_version",
            "previous_policy_hash",
            "ds_asset_id",
            "ds_scale",
            "fee",
            "treasury_account_id",
            "charging_mode",
            "effective_from_height",
            "expires_after_height",
            "exemption_classes",
            "treasury_payout_binding"),
        "validation fee policy");
    if (!BigInteger.ONE.equals(uint(value.get("schema_version"), "policy.schema_version"))) {
      throw invalid("policy.schema_version must equal 1");
    }
    NetworkId.parse(text(value.get("network_id"), "policy.network_id"));
    final BigInteger version =
        u64String(value.get("policy_version"), "policy.policy_version", true);
    final byte[] previousHash =
        value.get("previous_policy_hash") == null
            ? null
            : bytes(value.get("previous_policy_hash"), 32, "policy.previous_policy_hash", false);
    if (BigInteger.ONE.equals(version) != (previousHash == null)) {
      throw invalid("policy.previous_policy_hash does not match policy_version");
    }
    asset(value.get("ds_asset_id"), "policy.ds_asset_id");
    if (!BigInteger.valueOf(2).equals(uint(value.get("ds_scale"), "policy.ds_scale"))) {
      throw invalid("policy.ds_scale must equal 2");
    }
    final String fee = quantity(value.get("fee"), "policy.fee");
    account(value.get("treasury_account_id"), "policy.treasury_account_id");
    final String mode =
        chargingMode(objectValue(value.get("charging_mode"), "policy.charging_mode"));
    final BigInteger effective =
        u64String(value.get("effective_from_height"), "policy.effective_from_height", false);
    if (value.get("expires_after_height") != null
        && u64String(value.get("expires_after_height"), "policy.expires_after_height", false)
                .compareTo(effective)
            <= 0) {
      throw invalid("policy.expires_after_height must exceed effective_from_height");
    }
    final List<?> rawExemptions =
        list(value.get("exemption_classes"), "policy.exemption_classes");
    final Set<String> exemptions = new HashSet<>();
    for (int index = 0; index < rawExemptions.size(); index++) {
      final String exemption =
          text(rawExemptions.get(index), "policy.exemption_classes[" + index + "]");
      if (!"TREASURY_PAYOUT".equals(exemption) || !exemptions.add(exemption)) {
        throw invalid("policy.exemption_classes contains an unsupported or duplicate class");
      }
    }
    final boolean hasBinding = value.get("treasury_payout_binding") != null;
    if (hasBinding) {
      payoutBinding(
          objectValue(value.get("treasury_payout_binding"), "policy.treasury_payout_binding"));
    }
    if (hasBinding != exemptions.contains("TREASURY_PAYOUT")) {
      throw invalid("policy payout binding does not match exemption classes");
    }
    if ("DISABLED".equals(mode)) {
      if (!"0".equals(fee) || !exemptions.isEmpty() || hasBinding) {
        throw invalid("disabled validation fees require zero fee and no payout exemption");
      }
    } else if (!"0.1".equals(fee)) {
      throw invalid("enabled V1 validation fee must equal 0.1");
    }
  }

  private static String chargingMode(final Map<String, Object> value) {
    exact(value, fields("charging_mode", "value"), "charging_mode");
    final String mode = text(value.get("charging_mode"), "charging_mode.charging_mode");
    if (!"DISABLED".equals(mode)
        && !"PER_QUALIFYING_TRANSFER_INSTRUCTION".equals(mode)) {
      throw invalid("charging_mode is unsupported");
    }
    if (value.get("value") != null) {
      throw invalid("charging_mode.value must be null");
    }
    return mode;
  }

  private static void validationFeePayoutLifecycle(final Map<String, Object> value) {
    exact(
        value,
        fields("proposal_operator", "payout_binding"),
        "ValidationFeePayoutLifecycle");
    account(value.get("proposal_operator"), "proposal_operator");
    payoutBinding(objectValue(value.get("payout_binding"), "payout_binding"));
  }

  private static void payoutBinding(final Map<String, Object> value) {
    exact(
        value,
        fields(
            "contract_address",
            "code_hash",
            "entrypoint",
            "treasury_account_id",
            "ds_asset_id",
            "xor_asset_id",
            "pool_vault_account_id",
            "batch_ds",
            "min_xor_out",
            "max_xor_out",
            "recipients"),
        "payout_binding");
    ContractAddressValidator.requireCanonicalV1(
        text(value.get("contract_address"), "payout_binding.contract_address"));
    bytes(value.get("code_hash"), 32, "payout_binding.code_hash", true);
    if (!"autonomous_validation_fee_tick"
        .equals(text(value.get("entrypoint"), "payout_binding.entrypoint"))) {
      throw invalid("payout binding entrypoint is not the exact V1 entrypoint");
    }
    final String treasury =
        account(value.get("treasury_account_id"), "payout_binding.treasury_account_id");
    final String vault =
        account(value.get("pool_vault_account_id"), "payout_binding.pool_vault_account_id");
    if (treasury.equals(vault)) {
      throw invalid("treasury and pool vault accounts must differ");
    }
    final String ds = asset(value.get("ds_asset_id"), "payout_binding.ds_asset_id");
    final String xor = asset(value.get("xor_asset_id"), "payout_binding.xor_asset_id");
    if (ds.equals(xor)) {
      throw invalid("DS and XOR assets must differ");
    }
    if (!"10".equals(quantity(value.get("batch_ds"), "payout_binding.batch_ds"))
        || !"4".equals(quantity(value.get("min_xor_out"), "payout_binding.min_xor_out"))
        || !"100".equals(quantity(value.get("max_xor_out"), "payout_binding.max_xor_out"))) {
      throw invalid("payout binding quantities do not equal the fixed V1 values");
    }
    final List<?> rawRecipients = list(value.get("recipients"), "payout_binding.recipients");
    final Set<String> recipients = new HashSet<>();
    for (int index = 0; index < rawRecipients.size(); index++) {
      final String label = "payout_binding.recipients[" + index + "]";
      final Map<String, Object> recipient = objectValue(rawRecipients.get(index), label);
      exact(recipient, fields("account_id", "share"), label);
      if (!"0.25".equals(quantity(recipient.get("share"), label + ".share"))) {
        throw invalid(label + ".share must equal 0.25");
      }
      if (!recipients.add(account(recipient.get("account_id"), label + ".account_id"))) {
        throw invalid("payout recipients must be unique");
      }
    }
    if (recipients.size() != 4 || recipients.contains(treasury) || recipients.contains(vault)) {
      throw invalid("payout recipients must contain four unique non-pool accounts");
    }
  }

  private static void musubiAction(final Map<String, Object> value) {
    exact(value, fields("kind", "value"), "MusubiRegistryGovernance");
    final String kind = text(value.get("kind"), "MusubiRegistryGovernance.kind");
    final Map<String, Object> action =
        objectValue(value.get("value"), "MusubiRegistryGovernance.value");
    switch (kind) {
      case "RecoverPackageOwners" -> {
        exact(action, fields("package", "owners", "expected_revision"), kind);
        musubiPackage(objectValue(action.get("package"), kind + ".package"), kind + ".package");
        final List<?> rawOwners = list(action.get("owners"), kind + ".owners");
        final Set<String> owners = new HashSet<>();
        for (int index = 0; index < rawOwners.size(); index++) {
          if (!owners.add(account(rawOwners.get(index), kind + ".owners[" + index + "]"))) {
            throw invalid(kind + ".owners must be distinct");
          }
        }
        if (owners.isEmpty() || owners.size() > 64) {
          throw invalid(kind + ".owners must contain 1-64 accounts");
        }
        requirePositive(uint(action.get("expected_revision"), kind + ".expected_revision"), kind);
      }
      case "RetargetAlias" -> {
        exact(action, fields("alias", "target", "expected_revision"), kind);
        kebab(stringTuple(action.get("alias"), kind + ".alias"), kind + ".alias", 32);
        musubiPackage(objectValue(action.get("target"), kind + ".target"), kind + ".target");
        requirePositive(uint(action.get("expected_revision"), kind + ".expected_revision"), kind);
      }
      case "TakedownArtifact" -> {
        exact(
            action,
            fields("release", "reason", "expected_artifact_governance_revision"),
            kind);
        musubiRelease(objectValue(action.get("release"), kind + ".release"), kind + ".release");
        reason(stringTuple(action.get("reason"), kind + ".reason"), kind + ".reason");
        requirePositive(
            uint(
                action.get("expected_artifact_governance_revision"),
                kind + ".expected_artifact_governance_revision"),
            kind);
      }
      case "SetRegistryPolicy" -> {
        exact(action, fields("policy", "expected_revision"), kind);
        final BigInteger expected =
            uint(action.get("expected_revision"), kind + ".expected_revision");
        requirePositive(expected, kind);
        final BigInteger revision =
            musubiRegistryPolicy(
                objectValue(action.get("policy"), kind + ".policy"), kind + ".policy");
        if (!revision.equals(expected.add(BigInteger.ONE))) {
          throw invalid("policy revision must follow expected_revision");
        }
      }
      default -> throw invalid("Musubi governance action is unsupported");
    }
  }

  private static void musubiPackage(final Map<String, Object> value, final String label) {
    exact(value, fields("home_dataspace", "scope", "name"), label);
    uint(value.get("home_dataspace"), label + ".home_dataspace");
    final Map<String, Object> scope = objectValue(value.get("scope"), label + ".scope");
    exact(scope, fields("kind", "value"), label + ".scope");
    final String kind = text(scope.get("kind"), label + ".scope.kind");
    if ("DataspaceRoot".equals(kind)) {
      if (scope.get("value") != null) throw invalid(label + ".scope.value must be null");
    } else if ("Domain".equals(kind)) {
      canonicalName(scope.get("value"), label + ".scope.value");
    } else {
      throw invalid(label + ".scope.kind is unsupported");
    }
    kebab(stringTuple(value.get("name"), label + ".name"), label + ".name", 64);
  }

  private static void musubiRelease(final Map<String, Object> value, final String label) {
    exact(value, fields("package", "version"), label);
    musubiPackage(objectValue(value.get("package"), label + ".package"), label + ".package");
    final Map<String, Object> version = objectValue(value.get("version"), label + ".version");
    exact(version, fields("major", "minor", "patch", "prerelease"), label + ".version");
    uint(version.get("major"), label + ".version.major");
    uint(version.get("minor"), label + ".version.minor");
    uint(version.get("patch"), label + ".version.patch");
    final List<?> prerelease = list(version.get("prerelease"), label + ".version.prerelease");
    if (prerelease.size() > 16) throw invalid(label + ".version.prerelease is too long");
    for (int index = 0; index < prerelease.size(); index++) {
      final String itemLabel = label + ".version.prerelease[" + index + "]";
      final Map<String, Object> identifier = objectValue(prerelease.get(index), itemLabel);
      exact(identifier, fields("kind", "value"), itemLabel);
      final String kind = text(identifier.get("kind"), itemLabel + ".kind");
      if ("Numeric".equals(kind)) {
        uint(identifier.get("value"), itemLabel + ".value");
      } else if ("AlphaNumeric".equals(kind)) {
        final String literal = text(identifier.get("value"), itemLabel + ".value");
        if (literal.getBytes(StandardCharsets.UTF_8).length > 64
            || !ALPHANUMERIC_PRERELEASE.matcher(literal).matches()) {
          throw invalid(itemLabel + " contains an invalid alphanumeric identifier");
        }
      } else {
        throw invalid("unsupported prerelease identifier");
      }
    }
  }

  private static BigInteger musubiRegistryPolicy(
      final Map<String, Object> value, final String label) {
    exact(
        value,
        fields("version", "revision", "mode", "allowlisted_dataspaces", "alias_pricing"),
        label);
    if (!BigInteger.ONE.equals(uint(value.get("version"), label + ".version"))) {
      throw invalid(label + ".version must equal 1");
    }
    final BigInteger revision = uint(value.get("revision"), label + ".revision");
    requirePositive(revision, label);
    final Map<String, Object> mode = objectValue(value.get("mode"), label + ".mode");
    exact(mode, fields("kind", "value"), label + ".mode");
    final String modeKind = text(mode.get("kind"), label + ".mode.kind");
    if (!("Closed".equals(modeKind)
            || "Allowlisted".equals(modeKind)
            || "Open".equals(modeKind))
        || mode.get("value") != null) {
      throw invalid(label + ".mode is unsupported");
    }
    final List<?> rawAllowed =
        list(value.get("allowlisted_dataspaces"), label + ".allowlisted_dataspaces");
    BigInteger previous = null;
    for (int index = 0; index < rawAllowed.size(); index++) {
      final BigInteger current =
          uint(rawAllowed.get(index), label + ".allowlisted_dataspaces[" + index + "]");
      if (previous != null && previous.compareTo(current) >= 0) {
        throw invalid(label + ".allowlisted_dataspaces must be sorted and unique");
      }
      previous = current;
    }
    if (!"Allowlisted".equals(modeKind) && !rawAllowed.isEmpty()) {
      throw invalid(label + ".allowlisted_dataspaces does not match mode");
    }
    final Map<String, Object> pricing =
        objectValue(value.get("alias_pricing"), label + ".alias_pricing");
    final Set<String> pricingFields =
        fields(
            "revision",
            "length_1_xor",
            "length_2_xor",
            "length_3_xor",
            "length_4_xor",
            "length_5_to_32_xor");
    exact(pricing, pricingFields, label + ".alias_pricing");
    for (final String field : pricingFields) {
      requirePositive(uint(pricing.get(field), label + ".alias_pricing." + field), label);
    }
    return revision;
  }

  private static void sorafsProvider(final Map<String, Object> value) {
    exact(value, fields("action"), "SorafsProviderGovernance");
    final Map<String, Object> action =
        objectValue(value.get("action"), "SorafsProviderGovernance.action");
    exact(action, fields("action", "value"), "SorafsProviderGovernance.action");
    final String kind = text(action.get("action"), "SorafsProviderGovernance.action.action");
    final Map<String, Object> payload =
        objectValue(action.get("value"), "SorafsProviderGovernance.action.value");
    switch (kind) {
      case "establish" -> {
        exact(payload, fields("provider_id", "owner"), "provider establish");
        providerId(payload.get("provider_id"), "provider_id");
        account(payload.get("owner"), "owner");
      }
      case "rebind" -> {
        exact(
            payload,
            fields("provider_id", "expected_owner", "next_owner"),
            "provider rebind");
        providerId(payload.get("provider_id"), "provider_id");
        final String current = account(payload.get("expected_owner"), "expected_owner");
        final String next = account(payload.get("next_owner"), "next_owner");
        if (current.equals(next)) throw invalid("next_owner must differ from expected_owner");
      }
      case "remove" -> {
        exact(payload, fields("provider_id", "expected_owner"), "provider remove");
        providerId(payload.get("provider_id"), "provider_id");
        account(payload.get("expected_owner"), "expected_owner");
      }
      default -> throw invalid("Sorafs provider action is unsupported");
    }
  }

  private static void contractLifecycle(final Map<String, Object> value) {
    exact(
        value,
        fields("contract_address", "expected_revision", "action"),
        "ContractLifecycleGovernance");
    ContractAddressValidator.requireCanonicalV1(
        text(
            value.get("contract_address"),
            "ContractLifecycleGovernance.contract_address"));
    requirePositive(
        uint(
            value.get("expected_revision"),
            "ContractLifecycleGovernance.expected_revision"),
        "ContractLifecycleGovernance.expected_revision");
    final Map<String, Object> action =
        objectValue(value.get("action"), "ContractLifecycleGovernance.action");
    exact(action, fields("action", "payload"), "ContractLifecycleGovernance.action");
    final String kind =
        text(action.get("action"), "ContractLifecycleGovernance.action.action");
    if ("CancelOwnershipOffer".equals(kind) || "AcceptParliamentOwnership".equals(kind)) {
      if (action.get("payload") != null) throw invalid(kind + " payload must be null");
      return;
    }
    final Map<String, Object> payload =
        objectValue(action.get("payload"), "ContractLifecycleGovernance.action.payload");
    switch (kind) {
      case "Activate" -> {
        exact(
            payload,
            fields("code_hash", "abi_hash", "abi_version", "manifest_provenance"),
            kind);
        lowerHex32(payload.get("code_hash"), kind + ".code_hash");
        lowerHex32(payload.get("abi_hash"), kind + ".abi_hash");
        if (!BigInteger.ONE.equals(uint(payload.get("abi_version"), kind + ".abi_version"))) {
          throw invalid(kind + ".abi_version must equal 1");
        }
        if (payload.get("manifest_provenance") != null) {
          manifestProvenance(
              objectValue(
                  payload.get("manifest_provenance"), kind + ".manifest_provenance"),
              kind + ".manifest_provenance");
        }
      }
      case "Deactivate" -> {
        exact(
            payload,
            payload.containsKey("reason")
                ? fields("expected_code_hash", "reason")
                : fields("expected_code_hash"),
            kind);
        lowerHex32(payload.get("expected_code_hash"), kind + ".expected_code_hash");
        if (payload.get("reason") != null) {
          string(payload.get("reason"), kind + ".reason");
        }
      }
      case "OfferOwnership" -> {
        exact(payload, fields("new_owner"), kind);
        account(payload.get("new_owner"), kind + ".new_owner");
      }
      case "CompleteEmergencyHoldRetrospective" -> {
        exact(
            payload,
            fields(
                "hold_proposal_content_id",
                "hold_governance_attempt_id",
                "incident_digest",
                "retrospective_finding_root"),
            kind);
        bytes(
            payload.get("hold_proposal_content_id"),
            32,
            kind + ".hold_proposal_content_id",
            true);
        bytes(
            payload.get("hold_governance_attempt_id"),
            32,
            kind + ".hold_governance_attempt_id",
            true);
        bytes(payload.get("incident_digest"), 32, kind + ".incident_digest", true);
        bytes(
            payload.get("retrospective_finding_root"),
            32,
            kind + ".retrospective_finding_root",
            true);
      }
      default -> throw invalid("contract lifecycle action is unsupported");
    }
  }

  private static void contractEmergencyHold(final Map<String, Object> value) {
    exact(
        value,
        fields(
            "contract_address",
            "expected_revision",
            "expected_code_hash",
            "incident_digest",
            "reason",
            "duration_blocks"),
        "ContractEmergencyHold");
    ContractAddressValidator.requireCanonicalV1(
        text(value.get("contract_address"), "ContractEmergencyHold.contract_address"));
    requirePositive(
        uint(value.get("expected_revision"), "ContractEmergencyHold.expected_revision"),
        "ContractEmergencyHold.expected_revision");
    lowerHex32(
        value.get("expected_code_hash"), "ContractEmergencyHold.expected_code_hash");
    bytes(value.get("incident_digest"), 32, "ContractEmergencyHold.incident_digest", true);
    if (string(value.get("reason"), "ContractEmergencyHold.reason").trim().isEmpty()) {
      throw invalid("ContractEmergencyHold.reason must not be blank");
    }
    final BigInteger duration =
        uint(value.get("duration_blocks"), "ContractEmergencyHold.duration_blocks");
    if (duration.signum() == 0 || duration.compareTo(BigInteger.valueOf(3_600)) > 0) {
      throw invalid("ContractEmergencyHold.duration_blocks must be in 1..3600");
    }
  }

  private static void providerId(final Object value, final String label) {
    final List<?> tuple = list(value, label);
    if (tuple.size() != 1) throw invalid(label + " must use the exact ProviderId tuple");
    bytes(tuple.get(0), 32, label + "[0]", true);
  }

  private static String account(final Object value, final String label) {
    return AccountIdLiteral.requireCanonicalI105Address(text(value, label), label);
  }

  private static String asset(final Object value, final String label) {
    final String literal = text(value, label);
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(literal)) {
      throw invalid(label + " must be a canonical AssetDefinitionId");
    }
    return literal;
  }

  private static String quantity(final Object value, final String label) {
    final String literal = string(value, label);
    if (!QUANTITY.matcher(literal).matches()) {
      throw invalid(label + " must be a canonical non-negative quantity");
    }
    return literal;
  }

  private static void canonicalBase64(final Object value, final String label) {
    final String literal = string(value, label);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(literal);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(label + " must be canonical padded base64", ex);
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(literal)) {
      throw invalid(label + " must be canonical padded base64");
    }
  }

  private static void canonicalPublicKey(final Object value, final String label) {
    final String literal = text(value, label);
    final PublicKeyCodec.PublicKeyPayload parsed = PublicKeyCodec.decodePublicKeyLiteral(literal);
    if (parsed == null
        || !PublicKeyCodec.encodePublicKeyMultihash(parsed.curveId(), parsed.keyBytes())
            .equals(literal)) {
      throw invalid(label + " must use the canonical bare public-key spelling");
    }
  }

  private static void canonicalSignature(final Object value, final String label) {
    final String literal = string(value, label);
    if ((literal.length() & 1) != 0 || !literal.matches("[0-9A-F]+")) {
      throw invalid(label + " must be nonempty canonical uppercase hexadecimal");
    }
    boolean nonzero = false;
    for (int index = 0; index < literal.length(); index += 2) {
      nonzero |= !"00".equals(literal.substring(index, index + 2));
    }
    if (!nonzero) throw invalid(label + " must be nonzero");
  }

  private static String stringTuple(final Object value, final String label) {
    final List<?> tuple = list(value, label);
    if (tuple.size() != 1 || !(tuple.get(0) instanceof String)) {
      throw invalid(label + " must use an exact one-string tuple");
    }
    return (String) tuple.get(0);
  }

  private static void kebab(
      final String value, final String label, final int maximumBytes) {
    if (value.getBytes(StandardCharsets.UTF_8).length > maximumBytes
        || !KEBAB.matcher(value).matches()) {
      throw invalid(label + " must be canonical lowercase ASCII kebab text");
    }
  }

  private static void canonicalName(final Object value, final String label) {
    final String literal = text(value, label);
    boolean valid =
        literal.getBytes(StandardCharsets.UTF_8).length <= 255
            && Normalizer.normalize(literal, Normalizer.Form.NFC).equals(literal);
    for (int index = 0; valid && index < literal.length(); index++) {
      final char character = literal.charAt(index);
      final int code = character;
      valid =
          !Character.isWhitespace(character)
              && character != '@'
              && character != '#'
              && character != '$'
              && !Character.isISOControl(character)
              && code != 0x061c
              && !(code >= 0x200e && code <= 0x200f)
              && !(code >= 0x202a && code <= 0x202e)
              && !(code >= 0x2066 && code <= 0x2069);
    }
    if (!valid) throw invalid(label + " must be a canonical Iroha Name");
  }

  private static void reason(final String value, final String label) {
    boolean valid =
        !value.isEmpty()
            && value.equals(value.trim())
            && value.getBytes(StandardCharsets.UTF_8).length <= 1024;
    for (int index = 0; valid && index < value.length(); index++) {
      final int code = value.charAt(index);
      valid = !(code <= 0x1f || code == 0x7f);
    }
    if (!valid) throw invalid(label + " must be bounded canonical public text");
  }

  private static void lowerHex32(final Object value, final String label) {
    if (!(value instanceof String) || !LOWER_HEX_32.matcher((String) value).matches()) {
      throw invalid(label + " must contain 32 lowercase hexadecimal bytes");
    }
  }

  private static byte[] bytes(
      final Object value, final int size, final String label, final boolean nonzero) {
    final List<?> items = list(value, label);
    if (items.size() != size) throw invalid(label + " must contain exactly " + size + " bytes");
    final byte[] result = new byte[size];
    boolean allZero = true;
    for (int index = 0; index < size; index++) {
      final BigInteger parsed = uint(items.get(index), label + "[" + index + "]");
      if (parsed.compareTo(BigInteger.valueOf(255)) > 0) {
        throw invalid(label + "[" + index + "] must be a byte");
      }
      result[index] = parsed.byteValue();
      allZero &= result[index] == 0;
    }
    if (nonzero && allZero) throw invalid(label + " must be nonzero");
    return result;
  }

  private static BigInteger u64String(
      final Object value, final String label, final boolean positive) {
    final String literal = string(value, label);
    if (!UNSIGNED.matcher(literal).matches()) {
      throw invalid(label + " must be a canonical u64 decimal string");
    }
    final BigInteger parsed = new BigInteger(literal);
    if (parsed.compareTo(U64_MAX) > 0 || (positive && parsed.signum() == 0)) {
      throw invalid(label + " is outside u64");
    }
    return parsed;
  }

  private static BigInteger uint(final Object value, final String label) {
    if (!(value instanceof Number)) throw invalid(label + " must be an unsigned JSON integer");
    final String literal = value.toString();
    if (!UNSIGNED.matcher(literal).matches()) {
      throw invalid(label + " must be an unsigned JSON integer");
    }
    final BigInteger parsed = new BigInteger(literal);
    if (parsed.compareTo(FIRST_RELEASE_MAX_EXACT_JSON_U64) > 0) {
      throw invalid(label + " exceeds the first-release exact JSON integer bound");
    }
    return parsed;
  }

  private static void requirePositive(final BigInteger value, final String label) {
    if (value.signum() == 0) throw invalid(label + " must be positive");
  }

  private static String text(final Object value, final String label) {
    final String literal = string(value, label);
    if (literal.isEmpty() || !literal.equals(literal.trim())) {
      throw invalid(label + " must be canonical nonempty text");
    }
    return literal;
  }

  private static String string(final Object value, final String label) {
    if (!(value instanceof String)) throw invalid(label + " must be a string");
    return (String) value;
  }

  private static List<?> list(final Object value, final String label) {
    if (!(value instanceof List<?>)) throw invalid(label + " must be an array");
    return (List<?>) value;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> objectValue(final Object value, final String label) {
    if (!(value instanceof Map<?, ?>)) throw invalid(label + " must be an object");
    for (final Object key : ((Map<?, ?>) value).keySet()) {
      if (!(key instanceof String)) throw invalid(label + " must have string fields");
    }
    return (Map<String, Object>) value;
  }

  private static void exact(
      final Map<String, Object> value, final Set<String> expected, final String label) {
    if (!value.keySet().equals(expected)) {
      throw invalid(label + " contains unknown, aliased, or missing fields");
    }
  }

  private static Set<String> fields(final String... names) {
    return new HashSet<>(Arrays.asList(names));
  }

  private static IllegalArgumentException invalid(final String message) {
    return new IllegalArgumentException(message);
  }
}
