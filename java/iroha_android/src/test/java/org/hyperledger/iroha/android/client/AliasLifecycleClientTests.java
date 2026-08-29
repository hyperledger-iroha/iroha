package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;
import static org.hyperledger.iroha.android.client.CanonicalRequestSigningTestSupport.canonicalAccountHeader;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.alias.AccountAliasName;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanBodyV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanReceiptV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanRequestV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingReceiptVerifier;
import org.hyperledger.iroha.android.alias.AccountOnboardingBlockHashV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingCurrentStateResponseV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingCurrentStateV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingProofRequiredPrepareResponseV1;
import org.hyperledger.iroha.android.alias.PreparedTransactionSignatureV1;
import org.hyperledger.iroha.android.alias.TairaPublicResetMutationBindingV1;
import org.hyperledger.iroha.android.alias.AliasLeaseRenewPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleOperationV1;
import org.hyperledger.iroha.android.alias.AliasLifecyclePlanDispositionV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleTransactionPlanBodyV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleTransactionPlanV1;
import org.hyperledger.iroha.android.alias.AliasQuoteGuardV1;
import org.hyperledger.iroha.android.alias.AliasSetupModels;
import org.hyperledger.iroha.android.alias.EnsureAlias;
import org.hyperledger.iroha.android.alias.RenewAliasLease;
import org.hyperledger.iroha.android.alias.ResolvedAccountAliasV1;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;
import org.hyperledger.iroha.android.testing.TestNetworkIds;
import org.hyperledger.iroha.android.model.NetworkId;
import org.junit.Test;

/** HTTP parity tests for safe alias lifecycle, typed reads, and sponsored onboarding. */
public final class AliasLifecycleClientTests {
  private static final String ONBOARDING_TOKEN =
      "onboarding-token-value-1234567890abcd";

  /** Renewal planning uses only the signed read-only planner route. */
  @Test
  public void renewalPlanningUsesCanonicalHeadersAndNoSecretFields() {
    final String authority = TestAccountIds.ed25519Authority(0x41);
    final ResolvedAccountAliasV1 alias =
        new ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L);
    final AliasSetupModels.AliasTargetV1 target =
        new AliasSetupModels.AccountAliasTarget(alias);
    final AliasQuoteGuardV1 guard =
        new AliasQuoteGuardV1(
            3, TestAssetDefinitionIds.PRIMARY, "5", 1_700_000_100_000L);
    final RenewAliasLease renewal =
        new RenewAliasLease(target, 1_800_000_000_000L, 1_900_000_000_000L, guard);
    final AliasLeaseRenewPlanRequestV1 requestBody =
        new AliasLeaseRenewPlanRequestV1(renewal);
    final AliasSetupModels.AliasLeaseQuoteV1 quote =
        new AliasSetupModels.AliasLeaseQuoteV1(
            target,
            1,
            "3",
            guard,
            1_900_000_000_000L,
            1_900_000_100_000L,
            1_900_000_200_000L);
    final AliasLifecycleTransactionPlanBodyV1 body =
        new AliasLifecycleTransactionPlanBodyV1(
            1,
            authority,
            TestNetworkIds.canonical(),
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            new AliasLifecycleOperationV1.RenewLease(renewal),
            AliasLifecyclePlanDispositionV1.APPLY,
            new AliasSetupModels.AliasFramedInstructionV1(
                RenewAliasLease.WIRE_ID, new byte[] {1, 2, 3}),
            quote,
            Collections.singletonList(
                new AliasSetupModels.AliasAssetTotalV1(
                    TestAssetDefinitionIds.PRIMARY, "3")),
            Collections.emptyList(),
            Collections.emptyList(),
            guard.validUntilMs());
    final AliasLifecycleTransactionPlanV1 responsePlan =
        new AliasLifecycleTransactionPlanV1(body, "03".repeat(32));
    final CapturingExecutor executor =
        new CapturingExecutor(200, JsonEncoder.encode(responsePlan.toJsonMap()));
    final HttpClientTransport transport = transport(executor);
    final ToriiCanonicalRequestAuth auth = canonicalAuth(authority);

    transport.planAliasLeaseRenewal(requestBody, auth).join();

    final TransportRequest request = executor.lastRequest;
    assertNotNull(request);
    assertEquals("POST", request.method());
    assertEquals(
        "https://torii.example/api/v1/aliases/lease/renew/plan",
        request.uri().toString());
    assertEquals(
        canonicalAccountHeader(authority),
        request.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0));
    assertEquals(
        "1700000000000",
        request.headers().get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS).get(0));
    assertEquals(
        "alias-safe-nonce",
        request.headers().get(CanonicalRequestSigner.HEADER_NONCE).get(0));
    assertNotNull(request.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE));
    final String json = new String(request.body(), StandardCharsets.UTF_8);
    assertFalse(json.contains("private_key"));
    assertFalse(json.contains("payment_proof"));
  }

  /** Restricted typed reads carry canonical request headers. */
  @Test
  public void typedByAccountReadUsesCanonicalHeaders() {
    final String account = TestAccountIds.ed25519Authority(0x45);
    final String response =
        "{\"account_id\":\""
            + account
            + "\",\"total\":1,\"items\":[{\"alias\":\"merchant@banka.paynet\","
            + "\"dataspace\":\"paynet\",\"domain\":\"banka\",\"is_primary\":true}],"
            + "\"source\":\"on_chain\"}";
    final CapturingExecutor executor = new CapturingExecutor(200, response);
    final HttpClientTransport transport = transport(executor);

    final Optional<AccountAliasesByAccount> result =
        transport
            .listAccountAliases(
                new AccountAliasesByAccountRequest(account, "paynet", "banka"),
                canonicalAuth(account))
            .join();

    assertTrue(result.isPresent());
    assertEquals("merchant@banka.paynet", result.get().items().get(0).alias());
    assertEquals(
        "https://torii.example/api/v1/aliases/by-account",
        executor.lastRequest.uri().toString());
    assertEquals(
        canonicalAccountHeader(account),
        executor.lastRequest.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0));
    assertNotNull(
        executor.lastRequest.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE));
  }

  /** Typed read responses remain pinned to the exact requested selector. */
  @Test
  public void typedReadsRejectSubstitutedSelectors() {
    final String account = TestAccountIds.ed25519Authority(0x45);
    final String otherAccount = TestAccountIds.ed25519Authority(0x46);

    final CapturingExecutor indexExecutor =
        new CapturingExecutor(
            200,
            "{\"index\":8,\"alias\":\"merchant@paynet\",\"account_id\":\""
                + account
                + "\"}");
    expectCompletionFailure(
        () -> transport(indexExecutor).resolveAccountAliasIndex(BigInteger.valueOf(7)).join());

    final CapturingExecutor accountExecutor =
        new CapturingExecutor(
            200,
            "{\"account_id\":\""
                + otherAccount
                + "\",\"total\":0,\"items\":[]}");
    expectCompletionFailure(
        () ->
            transport(accountExecutor)
                .listAccountAliases(new AccountAliasesByAccountRequest(account))
                .join());

    final CapturingExecutor aliasExecutor =
        new CapturingExecutor(
            200,
            "{\"alias\":\"other@paynet\",\"account_id\":\""
                + account
                + "\"}");
    expectCompletionFailure(
        () -> transport(aliasExecutor).resolveAccountAlias("merchant@paynet").join());
  }

  /** Sponsored onboarding keeps its token in one header and rejects the retired one-shot shape. */
  @Test
  public void sponsoredOnboardingUsesOnlyTheDedicatedTokenHeader() throws Exception {
    final byte[] seed = new byte[32];
    Arrays.fill(seed, (byte) 0x53);
    final Ed25519PrivateKeyParameters onboardingSigner =
        new Ed25519PrivateKeyParameters(seed, 0);
    final String authority =
        AccountAddress.fromAccount(
                onboardingSigner.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String account = TestAccountIds.ed25519Authority(0x42);
    final ResolvedAccountAliasV1 alias =
        new ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L);
    final AccountOnboardingPlanRequestV1 intent =
        new AccountOnboardingPlanRequestV1(
            alias.canonicalName().canonicalText(), account, Collections.emptyList());
    final AliasSetupModels.AccountAliasIntent resourceIntent =
        new AliasSetupModels.AccountAliasIntent(
            new AliasSetupModels.AliasAccountIntentV1(
                alias,
                account,
                AliasSetupModels.AccountProvisionV1.CREATE,
                AliasSetupModels.AccountAliasRoleV1.PRIMARY));
    final AliasQuoteGuardV1 guard =
        new AliasQuoteGuardV1(
            3, TestAssetDefinitionIds.PRIMARY, "5", 1_700_000_100_000L);
    final AccountOnboardingPlanBodyV1 body =
        new AccountOnboardingPlanBodyV1(
            1,
            intent,
            authority,
            TestNetworkIds.canonical(),
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            new AliasSetupModels.AliasPlanResourceV1(
                resourceIntent, AliasSetupModels.AliasPlanDispositionV1.CREATE, null, 0L),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard,
            Collections.singletonList(
                new AliasSetupModels.AliasFramedInstructionV1(
                    EnsureAlias.WIRE_ID, new byte[] {4, 5, 6})),
            null,
            guard.validUntilMs());
    final AccountOnboardingPlanReceiptV1 receipt =
        signedOnboardingReceipt(body, onboardingSigner);
    final CapturingExecutor planExecutor =
        new CapturingExecutor(200, JsonEncoder.encode(receipt.toJsonMap()));

    transport(planExecutor)
        .planSponsoredAccountOnboarding(
            intent, ONBOARDING_TOKEN, authority, TestNetworkIds.canonical())
        .join();

    assertTokenOnlyRequest(
        planExecutor.lastRequest,
        "POST",
        "https://torii.example/api/v1/accounts/onboard/plan");

    final TairaPublicResetMutationBindingV1 binding =
        new TairaPublicResetMutationBindingV1(
            "11".repeat(32),
            "onboarding-fixture-nonce-0000001",
            TairaPublicResetMutationBindingV1.ONBOARDING,
            "onboarding",
            "22".repeat(32),
            4_102_444_800_000L);
    final String retiredApplyResponse =
        "{\"account_id\":\""
            + account
            + "\",\"alias\":\"merchant@banka.paynet\",\"status\":\"Unchanged\","
            + "\"disposition\":{\"kind\":\"no_op\",\"value\":null}}";
    final CapturingExecutor prepareExecutor =
        new CapturingExecutor(200, retiredApplyResponse);
    expectCompletionFailure(
        () ->
            transport(prepareExecutor)
                .prepareSponsoredAccountOnboarding(
                    intent,
                    receipt,
                    binding,
                    FeePaymentIntent.authority(Collections.emptyList()),
                    ONBOARDING_TOKEN,
                    authority,
                    TestNetworkIds.canonical())
                .join());
    assertTokenOnlyRequest(
        prepareExecutor.lastRequest,
        "POST",
        "https://torii.example/api/v1/accounts/onboard/prepare");

    final CapturingExecutor readinessExecutor =
        new CapturingExecutor(
            200,
            "{\"version\":1,\"status\":{\"status\":\"ready\",\"value\":null},"
                + "\"diagnostics\":[]}");
    final AliasSetupModels.AliasSetupReportV1 readiness =
        transport(readinessExecutor).getAccountOnboardingReadiness(ONBOARDING_TOKEN).join();
    assertEquals(AliasSetupModels.AliasSetupStatusV1.READY, readiness.status());
    assertTokenOnlyRequest(
        readinessExecutor.lastRequest,
        "GET",
        "https://torii.example/api/v1/accounts/onboarding/readiness");
    assertEquals(0, readinessExecutor.lastRequest.body().length);
  }

  /** ProofRequired reopens through one atomic POST and returns all three closed outcomes. */
  @Test
  public void proofRequiredCurrentStateUsesOneAtomicPostAndClassifiesSnapshot() throws Exception {
    final AtomicOnboardingProofFixture fixture = atomicOnboardingProofFixture();
    final ToriiCanonicalRequestAuth canonicalAuth = canonicalAuth(fixture.authority);
    final AccountOnboardingBlockHashV1 blockHash =
        new AccountOnboardingBlockHashV1(TestNetworkIds.canonical().literal());
    final String[] targets = {fixture.accountId, null, TestAccountIds.ed25519Authority(0x49)};
    final AccountOnboardingCurrentStateV1.Outcome[] outcomes = {
      AccountOnboardingCurrentStateV1.Outcome.APPLIED,
      AccountOnboardingCurrentStateV1.Outcome.ALIAS_ABSENT,
      AccountOnboardingCurrentStateV1.Outcome.ALIAS_CONFLICT
    };
    for (int index = 0; index < targets.length; index++) {
      final AccountOnboardingCurrentStateResponseV1 response =
          new AccountOnboardingCurrentStateResponseV1(
              AccountOnboardingCurrentStateResponseV1.VERSION,
              TestNetworkIds.canonical(),
              fixture.accountId,
              fixture.alias,
              true,
              targets[index],
              BigInteger.valueOf(41),
              blockHash);
      final CapturingExecutor executor =
          new CapturingExecutor(200, JsonEncoder.encode(response.toJsonMap()));

      final AccountOnboardingCurrentStateV1 result =
          transport(executor)
              .verifyAccountOnboardingCurrentState(
                  fixture.proofRequired,
                  fixture.request,
                  fixture.receipt,
                  fixture.binding,
                  fixture.authority,
                  TestNetworkIds.canonical(),
                  canonicalAuth)
              .join();

      assertEquals(outcomes[index], result.outcome());
      assertEquals(BigInteger.valueOf(41), result.blockHeight());
      assertEquals(blockHash, result.blockHash());
      assertEquals(1, executor.requestCount);
      assertEquals("POST", executor.lastRequest.method());
      assertEquals(
          "https://torii.example/api/v1/accounts/onboarding/current-state",
          executor.lastRequest.uri().toString());
      assertFalse(executor.lastRequest.headers().containsKey("X-Iroha-Onboarding-Token"));
      assertTrue(executor.lastRequest.headers().containsKey("X-Iroha-Account"));
      assertTrue(executor.lastRequest.headers().containsKey("X-Iroha-Signature"));
      assertTrue(executor.lastRequest.headers().containsKey("X-Iroha-Timestamp-Ms"));
      assertTrue(executor.lastRequest.headers().containsKey("X-Iroha-Nonce"));
      final String sent = new String(executor.lastRequest.body(), StandardCharsets.UTF_8);
      assertEquals(
          "{\"account_id\":\""
              + fixture.accountId
              + "\",\"alias\":\""
              + fixture.alias
              + "\",\"version\":1}",
          sent);
    }
  }

  /** Atomic onboarding state rejects substitutions, noncanonical anchors, and open shapes. */
  @Test
  public void proofRequiredCurrentStateRejectsStrictNegativeCases() throws Exception {
    final AtomicOnboardingProofFixture fixture = atomicOnboardingProofFixture();
    final ToriiCanonicalRequestAuth canonicalAuth = canonicalAuth(fixture.authority);
    final Map<String, Object> exact = new LinkedHashMap<>();
    exact.put("version", Integer.valueOf(1));
    exact.put("network_id", TestNetworkIds.canonical().literal());
    exact.put("account_id", fixture.accountId);
    exact.put("alias", fixture.alias);
    exact.put("account_exists", Boolean.TRUE);
    exact.put("alias_target_account_id", fixture.accountId);
    exact.put("observed_block_height", Integer.valueOf(51));
    exact.put("observed_block_hash", TestNetworkIds.canonical().literal());
    final byte[] foreignBytes = new byte[32];
    Arrays.fill(foreignBytes, (byte) 0x25);
    final List<Map<String, Object>> invalid = new java.util.ArrayList<>();
    invalid.add(changed(exact, "version", Integer.valueOf(2)));
    invalid.add(changed(exact, "network_id", NetworkId.fromBytes(foreignBytes).literal()));
    invalid.add(changed(exact, "account_id", TestAccountIds.ed25519Authority(0x4a)));
    invalid.add(changed(exact, "alias", "other@banka.paynet"));
    final Map<String, Object> absent = changed(exact, "account_exists", Boolean.FALSE);
    absent.put("alias_target_account_id", null);
    invalid.add(absent);
    invalid.add(changed(exact, "observed_block_height", Integer.valueOf(0)));
    invalid.add(
        changed(
            exact,
            "observed_block_hash",
            TestNetworkIds.canonical().literal().toLowerCase(java.util.Locale.ROOT)));
    invalid.add(changed(exact, "alias_target_account_id", " " + fixture.accountId));
    final Map<String, Object> open = new LinkedHashMap<>(exact);
    open.put("legacy_account_state", "Applied");
    invalid.add(open);
    final Map<String, Object> missingTarget = new LinkedHashMap<>(exact);
    missingTarget.remove("alias_target_account_id");
    invalid.add(missingTarget);

    for (final Map<String, Object> body : invalid) {
      final CapturingExecutor executor =
          new CapturingExecutor(200, JsonEncoder.encode(body));
      expectCompletionFailure(
          () ->
              transport(executor)
                  .verifyAccountOnboardingCurrentState(
                      fixture.proofRequired,
                      fixture.request,
                      fixture.receipt,
                      fixture.binding,
                      fixture.authority,
                      TestNetworkIds.canonical(),
                      canonicalAuth)
                  .join());
      assertEquals(
          "https://torii.example/api/v1/accounts/onboarding/current-state",
          executor.lastRequest.uri().toString());
      assertEquals(1, executor.requestCount);
    }
  }

  private static Map<String, Object> changed(
      final Map<String, Object> source, final String field, final Object value) {
    final Map<String, Object> changed = new LinkedHashMap<>(source);
    changed.put(field, value);
    return changed;
  }

  private static AtomicOnboardingProofFixture atomicOnboardingProofFixture() throws Exception {
    final byte[] seed = new byte[32];
    Arrays.fill(seed, (byte) 0x53);
    final Ed25519PrivateKeyParameters privateKey =
        new Ed25519PrivateKeyParameters(seed, 0);
    final String authority =
        AccountAddress.fromAccount(privateKey.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String accountId = TestAccountIds.ed25519Authority(0x48);
    final String alias = "merchant@banka.paynet";
    final ResolvedAccountAliasV1 resolvedAlias =
        new ResolvedAccountAliasV1(AccountAliasName.parse(alias), 7L);
    final AccountOnboardingPlanRequestV1 request =
        new AccountOnboardingPlanRequestV1(alias, accountId, Collections.emptyList());
    final AliasSetupModels.AccountAliasIntent intent =
        new AliasSetupModels.AccountAliasIntent(
            new AliasSetupModels.AliasAccountIntentV1(
                resolvedAlias,
                accountId,
                AliasSetupModels.AccountProvisionV1.CREATE,
                AliasSetupModels.AccountAliasRoleV1.PRIMARY));
    final AliasQuoteGuardV1 guard =
        new AliasQuoteGuardV1(
            3, TestAssetDefinitionIds.PRIMARY, "5", 4_102_444_800_000L);
    final AccountOnboardingPlanBodyV1 body =
        new AccountOnboardingPlanBodyV1(
            1,
            request,
            authority,
            TestNetworkIds.canonical(),
            new AliasSetupModels.AliasPlanAnchorV1(
                9, TestNetworkIds.canonical().literal()),
            new AliasSetupModels.AliasPlanResourceV1(
                intent, AliasSetupModels.AliasPlanDispositionV1.NO_OP, null, null),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard,
            Collections.emptyList(),
            null,
            guard.validUntilMs());
    final AccountOnboardingPlanReceiptV1 receipt =
        signedOnboardingReceipt(body, privateKey);
    final TairaPublicResetMutationBindingV1 binding =
        new TairaPublicResetMutationBindingV1(
            "11".repeat(32),
            "onboarding-fixture-nonce-0000001",
            TairaPublicResetMutationBindingV1.ONBOARDING,
            "onboarding",
            "22".repeat(32),
            4_102_444_800_000L);
    final AccountOnboardingProofRequiredPrepareResponseV1 unsigned =
        new AccountOnboardingProofRequiredPrepareResponseV1(
            AccountOnboardingProofRequiredPrepareResponseV1.SCHEMA,
            binding,
            "onboarding",
            AccountOnboardingProofRequiredPrepareResponseV1.OUTCOME,
            AccountOnboardingProofRequiredPrepareResponseV1.PROOF_KIND,
            receipt.planHash().toLowerCase(java.util.Locale.ROOT),
            accountId,
            alias,
            AliasSetupModels.AliasPlanDispositionV1.NO_OP,
            "00");
    final byte[] digest =
        PreparedTransactionSignatureV1.digest(
            PreparedTransactionSignatureV1.onboardingProofRequired(unsigned));
    final Ed25519Signer signer = new Ed25519Signer();
    signer.init(true, privateKey);
    signer.update(digest, 0, digest.length);
    final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired =
        new AccountOnboardingProofRequiredPrepareResponseV1(
            AccountOnboardingProofRequiredPrepareResponseV1.SCHEMA,
            binding,
            "onboarding",
            AccountOnboardingProofRequiredPrepareResponseV1.OUTCOME,
            AccountOnboardingProofRequiredPrepareResponseV1.PROOF_KIND,
            receipt.planHash().toLowerCase(java.util.Locale.ROOT),
            accountId,
            alias,
            AliasSetupModels.AliasPlanDispositionV1.NO_OP,
            hex(signer.generateSignature()));
    return new AtomicOnboardingProofFixture(
        request, receipt, binding, proofRequired, authority, accountId, alias);
  }

  private static HttpClientTransport transport(final CapturingExecutor executor) {
    return HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example/api"))
            .setLocalSigningContext(new LocalSigningContext(TestNetworkIds.canonical()))
            .build());
  }

  private static ToriiCanonicalRequestAuth canonicalAuth(final String account) {
    return new ToriiCanonicalRequestAuth(
        account,
        message -> {
          final byte[] signature = new byte[64];
          java.util.Arrays.fill(signature, (byte) 1);
          return signature;
        },
        1_700_000_000_000L,
        "alias-safe-nonce");
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

  private static String hex(final byte[] bytes) {
    final StringBuilder result = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) result.append(String.format("%02x", value & 0xff));
    return result.toString();
  }

  private static void assertTokenOnlyRequest(
      final TransportRequest request, final String method, final String uri) {
    assertNotNull(request);
    assertEquals(method, request.method());
    assertEquals(uri, request.uri().toString());
    assertEquals(
        Collections.singletonList(ONBOARDING_TOKEN),
        request.headers().get("X-Iroha-Onboarding-Token"));
    final String json = new String(request.body(), StandardCharsets.UTF_8);
    assertFalse(json.contains(ONBOARDING_TOKEN));
    assertFalse(json.contains("private_key"));
    assertFalse(json.contains("payment_proof"));
  }

  private static void expectCompletionFailure(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("response substitution must complete exceptionally");
    } catch (final CompletionException expected) {
      // Expected.
    }
  }

  private static final class AtomicOnboardingProofFixture {
    private final AccountOnboardingPlanRequestV1 request;
    private final AccountOnboardingPlanReceiptV1 receipt;
    private final TairaPublicResetMutationBindingV1 binding;
    private final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired;
    private final String authority;
    private final String accountId;
    private final String alias;

    private AtomicOnboardingProofFixture(
        final AccountOnboardingPlanRequestV1 request,
        final AccountOnboardingPlanReceiptV1 receipt,
        final TairaPublicResetMutationBindingV1 binding,
        final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired,
        final String authority,
        final String accountId,
        final String alias) {
      this.request = request;
      this.receipt = receipt;
      this.binding = binding;
      this.proofRequired = proofRequired;
      this.authority = authority;
      this.accountId = accountId;
      this.alias = alias;
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;
    private int requestCount;

    private CapturingExecutor(final int status, final String body) {
      this.response =
          new TransportResponse(
              status,
              body.getBytes(StandardCharsets.UTF_8),
              "stub",
              Collections.singletonMap(
                  "Content-Type", Collections.singletonList("application/json")),
              null,
              false);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requestCount++;
      lastRequest = request;
      return CompletableFuture.completedFuture(response);
    }
  }
}
