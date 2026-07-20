package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
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
import org.hyperledger.iroha.android.alias.AccountOnboardingResponseV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingStatusV1;
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
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;
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
            "test-chain",
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
        authority, request.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0));
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
        account,
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

  /** Sponsored onboarding keeps its token in one header for plan, apply, and readiness. */
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
            "test-chain",
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
        .planSponsoredAccountOnboarding(intent, ONBOARDING_TOKEN, authority)
        .join();

    assertTokenOnlyRequest(
        planExecutor.lastRequest,
        "POST",
        "https://torii.example/api/v1/accounts/onboard/plan");

    final String applyResponse =
        "{\"account_id\":\""
            + account
            + "\",\"alias\":\"merchant@banka.paynet\",\"status\":\"Unchanged\","
            + "\"disposition\":{\"kind\":\"no_op\",\"value\":null}}";
    final CapturingExecutor applyExecutor = new CapturingExecutor(200, applyResponse);
    final AccountOnboardingResponseV1 applied =
        transport(applyExecutor)
            .applySponsoredAccountOnboarding(receipt, ONBOARDING_TOKEN, authority)
            .join();
    assertEquals(AccountOnboardingStatusV1.UNCHANGED, applied.status());
    assertTokenOnlyRequest(
        applyExecutor.lastRequest,
        "POST",
        "https://torii.example/api/v1/accounts/onboard");

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

  /** Apply responses remain bound to the receipt, semantic status, hash, and HTTP status. */
  @Test
  public void sponsoredOnboardingApplyBindsReceiptStatusHashAndDisposition()
      throws Exception {
    final OnboardingFixture fixture =
        onboardingFixture(AliasSetupModels.AliasPlanDispositionV1.CREATE);
    final String hash = "ab".repeat(32);
    final String queuedBody =
        onboardingApplyResponse(
            fixture.accountId,
            fixture.alias,
            hash,
            AccountOnboardingStatusV1.QUEUED,
            AliasSetupModels.AliasPlanDispositionV1.CREATE);
    final AccountOnboardingResponseV1 applied =
        transport(new CapturingExecutor(202, queuedBody))
            .applySponsoredAccountOnboarding(
                fixture.receipt, ONBOARDING_TOKEN, fixture.authority)
            .join();
    assertEquals(AccountOnboardingStatusV1.QUEUED, applied.status());
    assertEquals(hash, applied.transactionHashHex());

    final String unchangedBody =
        onboardingApplyResponse(
            fixture.accountId,
            fixture.alias,
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasSetupModels.AliasPlanDispositionV1.NO_OP);
    expectOnboardingApplyFailure(fixture, 200, queuedBody);
    expectOnboardingApplyFailure(fixture, 201, queuedBody);
    expectOnboardingApplyFailure(fixture, 202, unchangedBody);
    expectOnboardingApplyFailure(
        fixture,
        200,
        onboardingApplyResponse(
            TestAccountIds.ed25519Authority(0x43),
            fixture.alias,
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasSetupModels.AliasPlanDispositionV1.NO_OP));
    expectOnboardingApplyFailure(
        fixture,
        200,
        onboardingApplyResponse(
            fixture.accountId,
            "substituted@paynet",
            null,
            AccountOnboardingStatusV1.UNCHANGED,
            AliasSetupModels.AliasPlanDispositionV1.NO_OP));
    expectOnboardingApplyFailure(
        fixture,
        202,
        onboardingApplyResponse(
            fixture.accountId,
            fixture.alias,
            hash,
            AccountOnboardingStatusV1.QUEUED,
            AliasSetupModels.AliasPlanDispositionV1.REPAIR));
    expectOnboardingApplyFailure(
        fixture,
        202,
        onboardingApplyResponse(
            fixture.accountId,
            fixture.alias,
            null,
            AccountOnboardingStatusV1.QUEUED,
            AliasSetupModels.AliasPlanDispositionV1.CREATE));

    final OnboardingFixture noOpFixture =
        onboardingFixture(AliasSetupModels.AliasPlanDispositionV1.NO_OP);
    expectOnboardingApplyFailure(
        noOpFixture,
        202,
        onboardingApplyResponse(
            noOpFixture.accountId,
            noOpFixture.alias,
            hash,
            AccountOnboardingStatusV1.QUEUED,
            AliasSetupModels.AliasPlanDispositionV1.CREATE));
  }

  private static HttpClientTransport transport(final CapturingExecutor executor) {
    return HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example/api"))
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

  private static OnboardingFixture onboardingFixture(
      final AliasSetupModels.AliasPlanDispositionV1 disposition) throws Exception {
    final byte[] seed = new byte[32];
    Arrays.fill(seed, (byte) 0x53);
    final Ed25519PrivateKeyParameters signer =
        new Ed25519PrivateKeyParameters(seed, 0);
    final String authority =
        AccountAddress.fromAccount(signer.generatePublicKey().getEncoded(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String accountId = TestAccountIds.ed25519Authority(0x42);
    final ResolvedAccountAliasV1 alias =
        new ResolvedAccountAliasV1(
            AccountAliasName.parse("merchant@banka.paynet"), 7L);
    final AliasSetupModels.AccountAliasIntent intent =
        new AliasSetupModels.AccountAliasIntent(
            new AliasSetupModels.AliasAccountIntentV1(
                alias,
                accountId,
                AliasSetupModels.AccountProvisionV1.CREATE,
                AliasSetupModels.AccountAliasRoleV1.PRIMARY));
    final AliasQuoteGuardV1 guard =
        new AliasQuoteGuardV1(
            3, TestAssetDefinitionIds.PRIMARY, "5", 1_700_000_100_000L);
    final AccountOnboardingPlanBodyV1 body =
        new AccountOnboardingPlanBodyV1(
            1,
            new AccountOnboardingPlanRequestV1(
                alias.canonicalName().canonicalText(),
                accountId,
                Collections.emptyList()),
            authority,
            "test-chain",
            new AliasSetupModels.AliasPlanAnchorV1(9, "01".repeat(32)),
            new AliasSetupModels.AliasPlanResourceV1(
                intent,
                disposition,
                null,
                disposition == AliasSetupModels.AliasPlanDispositionV1.NO_OP
                    ? null
                    : 0L),
            new AliasSetupModels.AliasLeaseAcquisitionV1(1, null),
            guard,
            disposition == AliasSetupModels.AliasPlanDispositionV1.NO_OP
                ? Collections.emptyList()
                : Collections.singletonList(
                    new AliasSetupModels.AliasFramedInstructionV1(
                        EnsureAlias.WIRE_ID, new byte[] {4, 5, 6})),
            null,
            guard.validUntilMs());
    return new OnboardingFixture(
        signedOnboardingReceipt(body, signer),
        authority,
        accountId,
        alias.canonicalName().canonicalText());
  }

  private static String onboardingApplyResponse(
      final String accountId,
      final String alias,
      final String transactionHashHex,
      final AccountOnboardingStatusV1 status,
      final AliasSetupModels.AliasPlanDispositionV1 disposition) {
    return "{\"account_id\":\""
        + accountId
        + "\",\"alias\":\""
        + alias
        + "\""
        + (transactionHashHex == null
            ? ""
            : ",\"tx_hash_hex\":\"" + transactionHashHex + "\"")
        + ",\"status\":\""
        + status.wireValue()
        + "\",\"disposition\":{\"kind\":\""
        + disposition.wireValue()
        + "\",\"value\":null}}";
  }

  private static void expectOnboardingApplyFailure(
      final OnboardingFixture fixture, final int status, final String body) {
    expectCompletionFailure(
        () ->
            transport(new CapturingExecutor(status, body))
                .applySponsoredAccountOnboarding(
                    fixture.receipt, ONBOARDING_TOKEN, fixture.authority)
                .join());
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

  private static final class OnboardingFixture {
    private final AccountOnboardingPlanReceiptV1 receipt;
    private final String authority;
    private final String accountId;
    private final String alias;

    private OnboardingFixture(
        final AccountOnboardingPlanReceiptV1 receipt,
        final String authority,
        final String accountId,
        final String alias) {
      this.receipt = receipt;
      this.authority = authority;
      this.accountId = accountId;
      this.alias = alias;
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;

    private CapturingExecutor(final int status, final String body) {
      this.response =
          new TransportResponse(
              status,
              body.getBytes(StandardCharsets.UTF_8),
              "stub",
              Collections.emptyMap());
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      return CompletableFuture.completedFuture(response);
    }
  }
}
