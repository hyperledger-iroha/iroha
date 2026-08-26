package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.junit.Test;

/** Tests catalog-free alias names, planner shapes, and local plan verification. */
public final class AliasSetupModelsTests {
  private static final NetworkId TEST_NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
  private static final NetworkId OTHER_NETWORK_ID =
      NetworkId.parse(
          "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22");

  @Test
  public void parsesCatalogFreeAccountAliasForms() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> cases =
        (List<Map<String, Object>>) fixture.get("account_alias_cases");
    for (final Map<String, Object> value : cases) {
      final AccountAliasName parsed = AccountAliasName.parse((String) value.get("input"));
      assert value.get("canonical").equals(parsed.canonicalText());
      assert value.get("label").equals(parsed.label());
      assert java.util.Objects.equals(value.get("domain"), parsed.domain());
      assert value.get("dataspace").equals(parsed.dataspace());
    }

    final AccountAliasName qualified = AccountAliasName.parse("Merchant@Banka.Paynet");
    assert "merchant".equals(qualified.label());
    assert "banka".equals(qualified.domain());
    assert "paynet".equals(qualified.dataspace());
    assert "merchant@banka.paynet".equals(qualified.canonicalText());

    final AccountAliasName root = AccountAliasName.parse("Merchant@Paynet");
    assert root.domain() == null;
    assert "merchant@paynet".equals(root.canonicalText());
    assert "merchant@xn--r8jz45g".equals(AccountAliasName.parse("merchant@例え").canonicalText());
    assert "{\"dataspace\":\"paynet\",\"domain\":\"banka\",\"label\":\"merchant\"}"
        .equals(JsonEncoder.encode(qualified.toJsonMap()));
  }

  @Test
  public void rejectsAmbiguousAccountAliases() {
    final String[] invalid = {
        "", " merchant@paynet", "merchant", "merchant@", "@paynet",
        "merchant@@paynet", "merchant@a.b.c", "merchant@.paynet", "merchant@paynet."
    };
    for (final String literal : invalid) {
      try {
        AccountAliasName.parse(literal);
        throw new AssertionError("expected invalid alias to fail: " + literal);
      } catch (final IllegalArgumentException expected) {
        // Expected.
      }
    }
  }

  @Test
  public void pinsFullUnsignedDataspaceIds() {
    final BigInteger max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
    final ResolvedDataSpaceV1 dataspace = new ResolvedDataSpaceV1("Paynet", max);
    final ResolvedDomainV1 domain = new ResolvedDomainV1("Banka.Paynet", max);
    final ResolvedAccountAliasV1 alias =
        new ResolvedAccountAliasV1("Merchant@Banka.Paynet", max);
    assert "paynet".equals(dataspace.canonicalName());
    assert domain.parentDataspace().equals(dataspace);
    assert alias.parentDomain().equals(domain);
    try {
      new ResolvedDataSpaceV1("paynet", max.add(BigInteger.ONE));
      throw new AssertionError("expected u64 overflow to fail");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  @Test
  public void usesVersionedEnsureAliasJsonShape() throws Exception {
    final EnsureAlias ensure =
        new EnsureAlias(
            accountIntent(),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard());
    final String json = JsonEncoder.encode(ensure.toJsonMap());
    assert json.contains("\"kind\":\"account_alias\"");
    assert json.contains("\"provision\":{\"kind\":\"create\",\"value\":null}");
    assert json.contains("\"quote_guard\"");
    assert "iroha.alias.ensure".equals(EnsureAlias.WIRE_ID);
  }

  @Test
  public void lifecycleBuildersUseCasAndNeverCarryBindingLeaseExpiry() throws Exception {
    final ResolvedAccountAliasV1 alias = resolvedAlias();
    final AliasSetupModels.AliasTargetV1 target =
        new AliasSetupModels.AccountAliasTarget(alias);
    final RenewAliasLease renewal = new RenewAliasLease(target, 1_000, 2_000, guard());
    assert "iroha.alias.lease.renew".equals(RenewAliasLease.WIRE_ID);
    assert Long.valueOf(1_000).equals(renewal.toJsonMap().get("expected_current_expiry_ms"));
    assert Long.valueOf(2_000).equals(renewal.toJsonMap().get("target_expiry_ms"));

    final AliasAutoRenewConfigV1 config =
        new AliasAutoRenewConfigV1(1, 3, asset(), "5", 86_400_000, 60_000, 5);
    final ConfigureAliasAutoRenew enabled = new ConfigureAliasAutoRenew(target, 4, config);
    final ConfigureAliasAutoRenew disabled = new ConfigureAliasAutoRenew(target, 5, null);
    assert config.toJsonMap().equals(enabled.toJsonMap().get("config"));
    assert disabled.toJsonMap().get("config") == null;
    assert "iroha.alias.auto_renew.configure".equals(ConfigureAliasAutoRenew.WIRE_ID);

    final RebindAccountAlias rebind =
        new RebindAccountAlias(alias, account(0x22), account(0x33));
    final CompareAndSetPrimaryAccountAlias primary =
        new CompareAndSetPrimaryAccountAlias(account(0x33), alias, null);
    assert !rebind.toJsonMap().containsKey("lease_expiry_ms");
    assert !primary.toJsonMap().containsKey("lease_expiry_ms");
    assert "iroha.account.alias.rebind".equals(RebindAccountAlias.WIRE_ID);
    assert "iroha.account.alias.primary.compare_and_set"
        .equals(CompareAndSetPrimaryAccountAlias.WIRE_ID);
  }

  @Test
  public void verifiesPlanHashAndExactFrames() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> hashVectors =
        (List<Map<String, Object>>) fixture.get("plan_hash_vectors");
    final Map<String, Object> hashVector = vectorNamed(hashVectors, "setup_account_alias_create");
    final byte[] bodyBytes = decodeHex((String) hashVector.get("canonical_body_norito_hex"));
    final AliasTransactionPlanV1 plan = createPlan(bodyBytes);
    assert hashVector.get("canonical_plan_hash_hex")
        .equals(hex(AliasPlanVerifier.canonicalHash(bodyBytes)));
    assert AliasPlanVerifier.verifyHash(plan, bodyBytes);
    assert !AliasPlanVerifier.verifyHash(plan, new byte[] {1, 3, 3, 7, 8});
    assert AliasPlanVerifier.validateExecutable(plan).isEmpty();
    assert AliasPlanVerifier.verifyExactFrames(plan, (wireId, payload) -> payload.clone());
    assert !AliasPlanVerifier.verifyExactFrames(
        plan,
        (wireId, payload) -> {
          final byte[] changed = payload.clone();
          changed[0] ^= 1;
          return changed;
        });
    AliasPlanVerifier.requireExecutable(plan, bodyBytes, (wireId, payload) -> payload.clone());
  }

  @Test
  public void validatesEverySharedRustFrameAndLifecycleHash() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> frames =
        (List<Map<String, Object>>) fixture.get("instruction_frame_vectors");
    assert frames.size() == 6;
    for (final Map<String, Object> vector : frames) {
      final String name = (String) vector.get("name");
      assert expectedWireId(name).equals(vector.get("wire_id"));
      final byte[] original = decodeHex((String) vector.get("framed_payload_hex"));
      final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(original, null);
      decoded.header().validateChecksum(decoded.payload());
      final int paddingLength =
          original.length - NoritoHeader.HEADER_LENGTH - decoded.payload().length;
      assert paddingLength >= 0;
      final byte[] reencoded = new byte[original.length];
      final byte[] header = decoded.header().encode();
      System.arraycopy(header, 0, reencoded, 0, header.length);
      System.arraycopy(
          decoded.payload(),
          0,
          reencoded,
          NoritoHeader.HEADER_LENGTH + paddingLength,
          decoded.payload().length);
      assert Arrays.equals(original, reencoded);
    }

    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> hashes =
        (List<Map<String, Object>>) fixture.get("plan_hash_vectors");
    final Map<String, Object> lifecycle = vectorNamed(hashes, "renew_account_alias");
    final byte[] lifecycleBody =
        decodeHex((String) lifecycle.get("canonical_body_norito_hex"));
    assert lifecycle.get("canonical_plan_hash_hex")
        .equals(hex(AliasPlanVerifier.canonicalLifecycleHash(lifecycleBody)));
  }

  @Test
  public void sharedRustPlanBodiesAndTypedFramesDecodeAndReencodeCanonically()
      throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> hashes =
        (List<Map<String, Object>>) fixture.get("plan_hash_vectors");

    final Map<String, Object> setup = vectorNamed(hashes, "setup_account_alias_create");
    final byte[] setupBytes = decodeHex((String) setup.get("canonical_body_norito_hex"));
    final AliasSetupModels.AliasTransactionPlanBodyV1 setupBody =
        AliasNoritoCodec.decodePlanBody(
            setupBytes, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert Arrays.equals(setupBytes, AliasNoritoCodec.encodePlanBody(setupBody));
    assert setup.get("canonical_plan_hash_hex")
        .equals(hex(AliasPlanVerifier.canonicalHash(setupBytes)));

    final Map<String, Object> lifecycle = vectorNamed(hashes, "renew_account_alias");
    final byte[] lifecycleBytes =
        decodeHex((String) lifecycle.get("canonical_body_norito_hex"));
    final AliasLifecycleTransactionPlanBodyV1 lifecycleBody =
        AliasNoritoCodec.decodeLifecyclePlanBody(
            lifecycleBytes, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert Arrays.equals(
        lifecycleBytes, AliasNoritoCodec.encodeLifecyclePlanBody(lifecycleBody));
    assert lifecycle.get("canonical_plan_hash_hex")
        .equals(hex(AliasPlanVerifier.canonicalLifecycleHash(lifecycleBytes)));

    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> frames =
        (List<Map<String, Object>>) fixture.get("instruction_frame_vectors");
    for (final Map<String, Object> vector : frames) {
      final byte[] original = decodeHex((String) vector.get("framed_payload_hex"));
      final String name = (String) vector.get("name");
      final byte[] reencoded;
      switch (name) {
        case "ensure_account_alias":
          reencoded =
              AliasNoritoCodec.encodeEnsureAliasFrame(
                  AliasNoritoCodec.decodeEnsureAliasFrame(
                      original, AccountAddress.DEFAULT_I105_DISCRIMINANT));
          break;
        case "renew_account_alias":
          reencoded =
              AliasNoritoCodec.encodeRenewAliasLeaseFrame(
                  AliasNoritoCodec.decodeRenewAliasLeaseFrame(
                      original, AccountAddress.DEFAULT_I105_DISCRIMINANT));
          break;
        case "configure_auto_renew_enable":
        case "configure_auto_renew_disable":
          reencoded =
              AliasNoritoCodec.encodeConfigureAutoRenewFrame(
                  AliasNoritoCodec.decodeConfigureAutoRenewFrame(
                      original, AccountAddress.DEFAULT_I105_DISCRIMINANT));
          break;
        case "rebind_account_alias":
          reencoded =
              AliasNoritoCodec.encodeRebindAccountAliasFrame(
                  AliasNoritoCodec.decodeRebindAccountAliasFrame(
                      original, AccountAddress.DEFAULT_I105_DISCRIMINANT));
          break;
        case "compare_and_set_primary_account_alias":
          reencoded =
              AliasNoritoCodec.encodeCompareAndSetPrimaryAliasFrame(
                  AliasNoritoCodec.decodeCompareAndSetPrimaryAliasFrame(
                      original, AccountAddress.DEFAULT_I105_DISCRIMINANT));
          break;
        default:
          throw new AssertionError("unexpected shared alias frame: " + name);
      }
      assert Arrays.equals(original, reencoded) : name;
    }

    final EnsureAlias ensure =
        AliasNoritoCodec.decodeEnsureAliasFrame(
            setupBody.instructions().get(0).framedPayload(),
            AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final AliasTransactionPlanV1 setupPlan =
        new AliasTransactionPlanV1(
            setupBody, (String) setup.get("canonical_plan_hash_hex"));
    final TransactionPayload setupTransaction =
        AliasPlanApply.buildTransactionPayload(
            new AliasSetupPlanRequestV1(Collections.singletonList(ensure)),
            setupPlan,
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(Collections.emptyList()),
            40_000,
            null,
            Collections.emptyMap());
    assert setupTransaction.executable().instructions().size()
        == setupBody.instructions().size();

    final AliasLifecycleOperationV1.RenewLease renewal =
        (AliasLifecycleOperationV1.RenewLease) lifecycleBody.operation();
    final AliasLifecycleTransactionPlanV1 lifecyclePlan =
        new AliasLifecycleTransactionPlanV1(
            lifecycleBody, (String) lifecycle.get("canonical_plan_hash_hex"));
    final TransactionPayload lifecycleTransaction =
        AliasLifecyclePlanApply.buildTransactionPayload(
            new AliasLeaseRenewPlanRequestV1(renewal.renewal()),
            lifecyclePlan,
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(Collections.emptyList()),
            40_000,
            null,
            Collections.emptyMap());
    assert lifecycleTransaction.executable().instructions().size() == 1;
  }

  @Test
  public void sharedBlockedReportUsesTypedSecretFreeShape() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final Map<String, Object> expected =
        (Map<String, Object>) fixture.get("report_json_vector");
    final AliasSetupModels.AliasSetupDiagnosticV1 diagnostic =
        new AliasSetupModels.AliasSetupDiagnosticV1(
            AliasSetupModels.AliasSetupValidationPhaseV1.CATALOG,
            "alias.catalog.mapping_conflict",
            AliasSetupModels.AliasSetupSeverityV1.ERROR,
            "dataspace:paynet",
            null,
            "7",
            "9",
            "Make the static catalog and active SNS record map paynet to the same dataspace ID.");
    final AliasSetupModels.AliasSetupReportV1 report =
        new AliasSetupModels.AliasSetupReportV1(
            AliasSetupModels.AliasSetupStatusV1.BLOCKED,
            Collections.singletonList(diagnostic));
    assert JsonEncoder.encode(expected).equals(JsonEncoder.encode(report.toJsonMap()));
  }

  @Test
  public void sharedResolvedNamesQuoteGuardAndExactPermissionMatchTypedJson() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final Map<String, Object> expectedNames =
        (Map<String, Object>) fixture.get("resolved_name_json_vectors");
    final Map<String, Object> typedNames = new java.util.LinkedHashMap<>();
    typedNames.put("dataspace", new ResolvedDataSpaceV1("paynet", 7).toJsonMap());
    typedNames.put("domain", new ResolvedDomainV1("banka.paynet", 7).toJsonMap());
    typedNames.put("account_alias", resolvedAlias().toJsonMap());
    assert JsonEncoder.encode(expectedNames).equals(JsonEncoder.encode(typedNames));

    @SuppressWarnings("unchecked")
    final Map<String, Object> expectedQuote =
        (Map<String, Object>) fixture.get("quote_guard_json_vector");
    final AliasQuoteGuardV1 quote =
        new AliasQuoteGuardV1(2, "4rPeAP6jAjiLVZThZYwwPRBuQagt", "10", 50_000);
    assert JsonEncoder.encode(expectedQuote).equals(JsonEncoder.encode(quote.toJsonMap()));

    @SuppressWarnings("unchecked")
    final Map<String, Object> expectedPermission =
        (Map<String, Object>) fixture.get("permission_scope_json_vector");
    final AccountAliasPermissionScope permission =
        AccountAliasPermissionScope.alias(resolvedAlias());
    assert JsonEncoder.encode(expectedPermission)
        .equals(JsonEncoder.encode(permission.toJsonMap()));
  }

  @Test
  public void parsesTypedPlanAndBuildsOneOrdinaryRequestBoundTransaction() throws Exception {
    final byte[] bodyBytes = {1, 3, 3, 7};
    final AliasTransactionPlanV1 original = createPlan(bodyBytes);
    final AliasTransactionPlanV1 checksummed =
        new AliasTransactionPlanV1(
            original.body(), HashLiteral.canonicalize(AliasPlanVerifier.canonicalHash(bodyBytes)));
    final String canonicalJson = JsonEncoder.encode(checksummed.toJsonMap());
    assert canonicalJson.contains("\"network_id\":\"" + TEST_NETWORK_ID.literal() + "\"");
    assert !canonicalJson.contains("\"chain_id\"");
    final AliasTransactionPlanV1 parsed =
        AliasTransactionPlanJsonParser.parse(canonicalJson.getBytes(StandardCharsets.UTF_8));
    try {
      AliasTransactionPlanJsonParser.parse(
          canonicalJson.replace("\"network_id\"", "\"chain_id\"")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("retired chain_id plan JSON must fail closed");
    } catch (final IllegalStateException expected) {
      // Expected.
    }
    assert checksummed.equals(parsed);
    assert AliasPlanVerifier.verifyHash(parsed, bodyBytes);

    final EnsureAlias ensure =
        new EnsureAlias(
            parsed.body().resources().get(0).intent(),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard());
    final AliasSetupPlanRequestV1 request =
        new AliasSetupPlanRequestV1(Collections.singletonList(ensure));
    final TransactionPayload payload =
        AliasPlanApply.buildTransactionPayload(
            request,
            parsed,
            ignored -> bodyBytes.clone(),
            (wireId, frame, chainDiscriminant) -> {
              assert EnsureAlias.WIRE_ID.equals(wireId);
              assert chainDiscriminant == AccountAddress.DEFAULT_I105_DISCRIMINANT;
              return new DecodedEnsureAliasFrame(ensure, frame.clone());
            },
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(Collections.emptyList()),
            40_000,
            7L,
            Collections.emptyMap());
    assert TEST_NETWORK_ID.equals(payload.networkId());
    assert TEST_NETWORK_ID.equals(parsed.body().networkId());
    assert parsed.body().authority().equals(payload.authority());
    assert payload.timeToLiveMs().orElseThrow() == 9_000L;
    assert payload.executable().isInstructions();
    assert payload.executable().instructions().size() == 1;
    assert EnsureAlias.WIRE_ID.equals(payload.executable().instructions().get(0).name());
    try {
      AliasPlanApply.buildTransactionPayload(
          request,
          parsed,
          ignored -> bodyBytes.clone(),
          (wireId, frame, chainDiscriminant) ->
              new DecodedEnsureAliasFrame(ensure, frame.clone()),
          OTHER_NETWORK_ID,
          AccountAddress.DEFAULT_I105_DISCRIMINANT,
          FeePaymentIntent.authority(Collections.emptyList()),
          40_000,
          null,
          Collections.emptyMap());
      throw new AssertionError("a setup plan for another exact NetworkId must fail");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("NetworkId");
    }
  }

  @Test
  public void requestBoundVerificationRejectsSubstitutedAcquisitionTerms() throws Exception {
    final byte[] bodyBytes = {4, 2};
    final AliasTransactionPlanV1 plan = createPlan(bodyBytes);
    final EnsureAlias intended =
        new EnsureAlias(
            plan.body().resources().get(0).intent(),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard());
    final EnsureAlias substituted =
        new EnsureAlias(
            plan.body().resources().get(0).intent(),
            new AliasSetupModels.AliasLeaseAcquisitionV1(2, null),
            guard());
    try {
      AliasPlanVerifier.requireExecutableForRequest(
          new AliasSetupPlanRequestV1(Collections.singletonList(intended)),
          plan,
          bodyBytes,
          (wireId, frame, chainDiscriminant) ->
              new DecodedEnsureAliasFrame(substituted, frame),
          AccountAddress.DEFAULT_I105_DISCRIMINANT);
      throw new AssertionError("substituted acquisition terms must fail");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("signed_request_mismatch");
    }
  }

  @Test
  public void parsesVerifiesAndBuildsLifecyclePlansWithoutMutationRoutes() throws Exception {
    final byte[] bodyBytes = {7, 1, 9, 4};
    final AliasSetupModels.AliasTargetV1 target =
        new AliasSetupModels.AccountAliasTarget(resolvedAlias());
    final RenewAliasLease renewal = new RenewAliasLease(target, 1_000, 2_000, guard());
    final AliasLeaseRenewPlanRequestV1 request = new AliasLeaseRenewPlanRequestV1(renewal);
    final AliasSetupModels.AliasFramedInstructionV1 frame =
        new AliasSetupModels.AliasFramedInstructionV1(
            RenewAliasLease.WIRE_ID, new byte[] {1, 2, 3});
    final AliasSetupModels.AliasLeaseQuoteV1 quote =
        new AliasSetupModels.AliasLeaseQuoteV1(
            target, 1, "3", guard(), 2_000, 2_100, 2_200);
    final AliasLifecycleTransactionPlanBodyV1 body =
        new AliasLifecycleTransactionPlanBodyV1(
            AliasLifecycleTransactionPlanBodyV1.VERSION,
            account(0x11),
            TEST_NETWORK_ID,
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            request.operation(),
            AliasLifecyclePlanDispositionV1.APPLY,
            frame,
            quote,
            Collections.singletonList(new AliasSetupModels.AliasAssetTotalV1(asset(), "3")),
            Collections.emptyList(),
            Collections.emptyList(),
            50_000);
    final AliasLifecycleTransactionPlanV1 plan =
        new AliasLifecycleTransactionPlanV1(
            body, hex(AliasPlanVerifier.canonicalLifecycleHash(bodyBytes)));
    final String canonicalJson = JsonEncoder.encode(plan.toJsonMap());
    assert canonicalJson.contains("\"network_id\":\"" + TEST_NETWORK_ID.literal() + "\"");
    assert !canonicalJson.contains("\"chain_id\"");
    final AliasLifecycleTransactionPlanV1 parsed =
        AliasLifecycleTransactionPlanJsonParser.parse(
            canonicalJson.getBytes(StandardCharsets.UTF_8));
    try {
      AliasLifecycleTransactionPlanJsonParser.parse(
          canonicalJson.replace("\"network_id\"", "\"chain_id\"")
              .getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("retired chain_id lifecycle-plan JSON must fail closed");
    } catch (final IllegalStateException expected) {
      // Expected.
    }
    assert plan.equals(parsed);
    assert AliasPlanVerifier.validateLifecycleExecutable(parsed).isEmpty();
    AliasPlanVerifier.requireLifecycleExecutableForRequest(
        request,
        parsed,
        bodyBytes,
        (wireId, payload, chainDiscriminant) -> {
          assert RenewAliasLease.WIRE_ID.equals(wireId);
          assert chainDiscriminant == AccountAddress.DEFAULT_I105_DISCRIMINANT;
          return new DecodedAliasLifecycleFrame(request.operation(), payload.clone());
        },
        AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final TransactionPayload transaction =
        AliasLifecyclePlanApply.buildTransactionPayload(
            request,
            parsed,
            ignored -> bodyBytes.clone(),
            (wireId, payload, chainDiscriminant) ->
                new DecodedAliasLifecycleFrame(request.operation(), payload.clone()),
            TEST_NETWORK_ID,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            FeePaymentIntent.authority(Collections.emptyList()),
            40_000,
            null,
            Collections.emptyMap());
    assert RenewAliasLease.WIRE_ID
        .equals(transaction.executable().instructions().get(0).name());
    try {
      AliasLifecyclePlanApply.buildTransactionPayload(
          request,
          parsed,
          ignored -> bodyBytes.clone(),
          (wireId, payload, chainDiscriminant) ->
              new DecodedAliasLifecycleFrame(request.operation(), payload.clone()),
          OTHER_NETWORK_ID,
          AccountAddress.DEFAULT_I105_DISCRIMINANT,
          FeePaymentIntent.authority(Collections.emptyList()),
          40_000,
          null,
          Collections.emptyMap());
      throw new AssertionError("a lifecycle plan for another exact NetworkId must fail");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("NetworkId");
    }
  }

  @Test
  public void autoRenewNoOpHasNoFrameChargeOrSubmission() throws Exception {
    final byte[] bodyBytes = {2, 4, 6};
    final AliasAutoRenewPlanRequestV1 request =
        new AliasAutoRenewPlanRequestV1(
            new ConfigureAliasAutoRenew(
                new AliasSetupModels.AccountAliasTarget(resolvedAlias()), 5, null));
    final AliasLifecycleTransactionPlanBodyV1 body =
        new AliasLifecycleTransactionPlanBodyV1(
            1,
            account(0x11),
            TEST_NETWORK_ID,
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            request.operation(),
            AliasLifecyclePlanDispositionV1.NO_OP,
            null,
            null,
            Collections.emptyList(),
            Collections.emptyList(),
            Collections.emptyList(),
            50_000);
    final AliasLifecycleTransactionPlanV1 plan =
        new AliasLifecycleTransactionPlanV1(
            body, hex(AliasPlanVerifier.canonicalLifecycleHash(bodyBytes)));
    assert AliasPlanVerifier.validateLifecycleExecutable(plan).isEmpty();
    AliasPlanVerifier.requireLifecycleExecutableForRequest(
        request,
        plan,
        bodyBytes,
        (wireId, payload, chainDiscriminant) -> {
          throw new AssertionError("no-op must not decode an instruction");
        },
        AccountAddress.DEFAULT_I105_DISCRIMINANT);
    try {
      AliasLifecyclePlanApply.buildTransactionPayload(
          request,
          plan,
          ignored -> bodyBytes,
          (wireId, payload, chainDiscriminant) -> {
            throw new AssertionError("unreachable");
          },
          TEST_NETWORK_ID,
          AccountAddress.DEFAULT_I105_DISCRIMINANT,
          FeePaymentIntent.authority(Collections.emptyList()),
          40_000,
          null,
          Collections.emptyMap());
      throw new AssertionError("no-op plan must not be submitted");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("no-op");
    }
  }

  @Test
  public void sponsoredOnboardingReceiptAndPrepareRequestAreTypedAndSecretFree()
      throws Exception {
    final AliasSetupModels.AccountAliasIntent intent = accountIntent();
    final AccountOnboardingPlanRequestV1 request =
        new AccountOnboardingPlanRequestV1(
            "Merchant@Banka.Paynet",
            account(0x22),
            Arrays.asList("CanSetMetadata", "CanSetMetadata"));
    final AccountOnboardingPlanBodyV1 body =
        new AccountOnboardingPlanBodyV1(
            1,
            request,
            account(0x11),
            TEST_NETWORK_ID,
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            new AliasSetupModels.AliasPlanResourceV1(
                intent, AliasSetupModels.AliasPlanDispositionV1.CREATE, null, 0L),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard(),
            Collections.singletonList(
                new AliasSetupModels.AliasFramedInstructionV1(
                    EnsureAlias.WIRE_ID, new byte[] {1, 2, 3})),
            null,
            50_000);
    final AccountOnboardingPlanReceiptV1 receipt =
        new AccountOnboardingPlanReceiptV1(body, "03".repeat(32), "AA");
    final byte[] encoded =
        JsonEncoder.encode(receipt.toJsonMap()).getBytes(StandardCharsets.UTF_8);
    assert receipt.equals(AccountOnboardingJsonParser.parseReceipt(encoded));
    final TairaPublicResetMutationBindingV1 binding =
        new TairaPublicResetMutationBindingV1(
            "11".repeat(32),
            "onboarding-fixture-nonce-0000001",
            TairaPublicResetMutationBindingV1.ONBOARDING,
            "onboarding",
            "22".repeat(32),
            50_000);
    final String prepare =
        JsonEncoder.encode(new AccountOnboardingPrepareRequestV1(binding, receipt).toJsonMap());
    assert prepare.contains(AccountOnboardingPrepareRequestV1.SCHEMA);
    assert prepare.contains(TairaPublicResetMutationBindingV1.SCHEMA);
    assert !prepare.contains("token");
    assert !prepare.contains("private_key");

    final AliasSetupModels.AliasSetupReportV1 readiness =
        AccountOnboardingJsonParser.parseReadiness(
            "{\"version\":1,\"status\":{\"status\":\"ready\",\"value\":null},\"diagnostics\":[]}"
                .getBytes(StandardCharsets.UTF_8));
    assert readiness.status() == AliasSetupModels.AliasSetupStatusV1.READY;
    assert readiness.diagnostics().isEmpty();
  }

  @Test
  public void onboardingReceiptVerifiesCanonicalBodyAndRejectsTamperOrWrongAuthority()
      throws Exception {
    final Ed25519PrivateKeyParameters signer =
        new Ed25519PrivateKeyParameters(filled(0x51), 0);
    final String authority =
        AccountAddress.fromAccount(signer.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final AccountOnboardingPlanBodyV1 body = onboardingBody(authority, TEST_NETWORK_ID);
    final byte[] encoded = AliasNoritoCodec.encodeOnboardingPlanBody(body);
    assert Arrays.equals(
        encoded,
        AliasNoritoCodec.encodeOnboardingPlanBody(
            AliasNoritoCodec.decodeOnboardingPlanBody(
                encoded, AccountAddress.DEFAULT_I105_DISCRIMINANT)));

    final AccountOnboardingPlanReceiptV1 receipt = signedOnboardingReceipt(body, signer);
    assert AccountOnboardingReceiptVerifier.verify(receipt, body.networkId(), authority);
    assert receipt.equals(
        AccountOnboardingReceiptVerifier.requireValidForRequest(
            body.request(), receipt, body.networkId(), authority));

    final AccountOnboardingPlanReceiptV1 tampered =
        new AccountOnboardingPlanReceiptV1(
            onboardingBody(authority, OTHER_NETWORK_ID),
            receipt.planHash(),
            receipt.signature());
    assert !AccountOnboardingReceiptVerifier.verify(tampered, body.networkId(), authority);

    final Ed25519PrivateKeyParameters wrongSigner =
        new Ed25519PrivateKeyParameters(filled(0x52), 0);
    final String wrongAuthority =
        AccountAddress.fromAccount(wrongSigner.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final AccountOnboardingPlanReceiptV1 wrongAuthorityReceipt =
        signedOnboardingReceipt(onboardingBody(wrongAuthority, TEST_NETWORK_ID), signer);
    assert !AccountOnboardingReceiptVerifier.verify(
        wrongAuthorityReceipt, body.networkId(), authority);

    final AccountOnboardingPlanReceiptV1 substitutedSelfSignedReceipt =
        signedOnboardingReceipt(onboardingBody(wrongAuthority, TEST_NETWORK_ID), wrongSigner);
    assert !AccountOnboardingReceiptVerifier.verify(
        substitutedSelfSignedReceipt, body.networkId(), authority);
    try {
      AccountOnboardingReceiptVerifier.requireValid(
          substitutedSelfSignedReceipt, body.networkId(), authority);
      throw new AssertionError("expected configured onboarding authority mismatch to fail");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  @Test
  public void sharedRustOnboardingReceiptReencodesAndVerifiesExactly() throws Exception {
    final Map<String, Object> fixture = sharedAliasFixture();
    @SuppressWarnings("unchecked")
    final Map<String, Object> vector =
        (Map<String, Object>) fixture.get("account_onboarding_receipt_vector");
    final byte[] bodyBytes =
        decodeHex((String) vector.get("canonical_body_norito_hex"));
    final AccountOnboardingPlanBodyV1 body =
        AliasNoritoCodec.decodeOnboardingPlanBody(
            bodyBytes, AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert Arrays.equals(bodyBytes, AliasNoritoCodec.encodeOnboardingPlanBody(body));
    assert vector.get("canonical_plan_hash_hex")
        .equals(hex(AccountOnboardingReceiptVerifier.canonicalHash(body)));

    @SuppressWarnings("unchecked")
    final Map<String, Object> receiptJson =
        (Map<String, Object>) vector.get("receipt_json");
    final AccountOnboardingPlanReceiptV1 receipt =
        AccountOnboardingJsonParser.parseReceipt(
            JsonEncoder.encode(receiptJson).getBytes(StandardCharsets.UTF_8));
    assert Arrays.equals(
        bodyBytes, AliasNoritoCodec.encodeOnboardingPlanBody(receipt.body()));
    assert vector.get("authority").equals(receipt.body().authority());
    assert vector.get("signature_hex").equals(receipt.signature());
    assert AccountOnboardingReceiptVerifier.verify(
        receipt, receipt.body().networkId(), (String) vector.get("authority"));

    final char[] tampered = receipt.signature().toCharArray();
    tampered[0] = tampered[0] == '0' ? '1' : '0';
    assert !AccountOnboardingReceiptVerifier.verify(
        new AccountOnboardingPlanReceiptV1(
            receipt.body(), receipt.planHash(), new String(tampered)),
        receipt.body().networkId(),
        receipt.body().authority());
  }

  @Test
  public void framePayloadIsDefensivelyCopied() {
    final byte[] source = {1, 2, 3};
    final AliasSetupModels.AliasFramedInstructionV1 frame =
        new AliasSetupModels.AliasFramedInstructionV1(EnsureAlias.WIRE_ID, source);
    source[0] = 9;
    assert Arrays.equals(new byte[] {1, 2, 3}, frame.framedPayload());
    final byte[] read = frame.framedPayload();
    read[1] = 8;
    assert Arrays.equals(new byte[] {1, 2, 3}, frame.framedPayload());
  }

  private static AliasTransactionPlanV1 createPlan(final byte[] canonicalBodyBytes)
      throws Exception {
    final ResolvedAccountAliasV1 alias = resolvedAlias();
    final AliasSetupModels.AccountAliasIntent intent = accountIntent();
    final AliasSetupModels.AliasLeaseQuoteV1 quote =
        new AliasSetupModels.AliasLeaseQuoteV1(
            new AliasSetupModels.AccountAliasTarget(alias),
            1,
            "3",
            guard(),
            1_000,
            2_000,
            3_000);
    final AliasSetupModels.AliasTransactionPlanBodyV1 body =
        new AliasSetupModels.AliasTransactionPlanBodyV1(
            AliasSetupModels.AliasTransactionPlanBodyV1.VERSION,
            account(0x11),
            TEST_NETWORK_ID,
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            Collections.singletonList(
                new AliasSetupModels.AliasPlanResourceV1(
                    intent, AliasSetupModels.AliasPlanDispositionV1.CREATE, quote, 0L)),
            Collections.singletonList(
                new AliasSetupModels.AliasFramedInstructionV1(
                    EnsureAlias.WIRE_ID, sharedEnsureFrame())),
            Collections.singletonList(
                new AliasSetupModels.AliasAssetTotalV1(asset(), "3")),
            Collections.emptyList(),
            Collections.emptyList(),
            49_000);
    return new AliasTransactionPlanV1(
        body, hex(AliasPlanVerifier.canonicalHash(canonicalBodyBytes)));
  }

  private static AccountOnboardingPlanBodyV1 onboardingBody(
      final String authority, final NetworkId networkId) throws Exception {
    return onboardingBody(
        authority, networkId, AliasSetupModels.AliasPlanDispositionV1.CREATE);
  }

  private static AccountOnboardingPlanBodyV1 onboardingBody(
      final String authority,
      final NetworkId networkId,
      final AliasSetupModels.AliasPlanDispositionV1 disposition)
      throws Exception {
    final AliasSetupModels.AccountAliasIntent intent = accountIntent();
    return new AccountOnboardingPlanBodyV1(
        1,
        new AccountOnboardingPlanRequestV1(
            resolvedAlias().canonicalName().canonicalText(),
            account(0x22),
            Collections.emptyList()),
        authority,
        networkId,
        new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
        new AliasSetupModels.AliasPlanResourceV1(
            intent,
            disposition,
            null,
            disposition == AliasSetupModels.AliasPlanDispositionV1.NO_OP ? null : 0L),
        new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
        guard(),
        disposition == AliasSetupModels.AliasPlanDispositionV1.NO_OP
            ? Collections.emptyList()
            : Collections.singletonList(
                new AliasSetupModels.AliasFramedInstructionV1(
                    EnsureAlias.WIRE_ID, sharedEnsureFrame())),
        null,
        50_000);
  }

  private static AccountOnboardingPlanReceiptV1 signedOnboardingReceipt(
      final AccountOnboardingPlanBodyV1 body,
      final Ed25519PrivateKeyParameters privateKey) {
    final byte[] hash = AccountOnboardingReceiptVerifier.canonicalHash(body);
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, privateKey);
    signer.update(hash, 0, hash.length);
    return new AccountOnboardingPlanReceiptV1(
        body, hex(hash), hex(signer.generateSignature()));
  }

  private static byte[] filled(final int value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static AliasSetupModels.AccountAliasIntent accountIntent() throws Exception {
    return new AliasSetupModels.AccountAliasIntent(
        new AliasSetupModels.AliasAccountIntentV1(
            resolvedAlias(),
            account(0x22),
            AliasSetupModels.AccountProvisionV1.CREATE,
            AliasSetupModels.AccountAliasRoleV1.PRIMARY));
  }

  private static AliasQuoteGuardV1 guard() {
    return new AliasQuoteGuardV1(3, asset(), "5", 50_000);
  }

  private static ResolvedAccountAliasV1 resolvedAlias() {
    return new ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L);
  }

  private static String account(final int fill) throws Exception {
    return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static String asset() {
    final byte[] bytes = new byte[16];
    for (int index = 0; index < bytes.length; index++) bytes[index] = (byte) index;
    bytes[6] = 0x46;
    bytes[8] = (byte) 0x88;
    return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder result = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) result.append(String.format("%02x", value & 0xff));
    return result.toString();
  }

  private static byte[] decodeHex(final String value) {
    if ((value.length() & 1) != 0) throw new IllegalArgumentException("fixture hex length");
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static String expectedWireId(final String name) {
    return switch (name) {
      case "ensure_account_alias" -> EnsureAlias.WIRE_ID;
      case "renew_account_alias" -> RenewAliasLease.WIRE_ID;
      case "configure_auto_renew_enable", "configure_auto_renew_disable" ->
          ConfigureAliasAutoRenew.WIRE_ID;
      case "rebind_account_alias" -> RebindAccountAlias.WIRE_ID;
      case "compare_and_set_primary_account_alias" ->
          CompareAndSetPrimaryAccountAlias.WIRE_ID;
      default -> throw new IllegalArgumentException("unknown shared alias frame: " + name);
    };
  }

  private static Map<String, Object> vectorNamed(
      final List<Map<String, Object>> vectors, final String name) {
    for (final Map<String, Object> vector : vectors) {
      if (name.equals(vector.get("name"))) return vector;
    }
    throw new IllegalArgumentException("shared alias vector not found: " + name);
  }

  @SuppressWarnings("unchecked")
  private static byte[] sharedEnsureFrame() throws Exception {
    final List<Map<String, Object>> frames =
        (List<Map<String, Object>>) sharedAliasFixture().get("instruction_frame_vectors");
    return decodeHex(
        (String) vectorNamed(frames, "ensure_account_alias").get("framed_payload_hex"));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> sharedAliasFixture() throws Exception {
    Path current = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
    for (int depth = 0; depth < 8 && current != null; depth++, current = current.getParent()) {
      final Path candidate =
          current.resolve("fixtures/norito_rpc/alias_setup_v1/alias_setup_v1.json");
      if (Files.isRegularFile(candidate)) {
        final String json =
            new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
        return (Map<String, Object>) JsonParser.parse(json);
      }
    }
    throw new IllegalStateException("shared alias setup fixture not found");
  }
}
