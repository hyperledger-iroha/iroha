using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Exercises exact NetworkId binding for account-onboarding receipts.</summary>
public sealed partial class ToriiClientTests
{
    [Fact]
    public async Task PrepareAccountOnboardingAsyncRejectsWrongNetworkBeforeDispatch()
    {
        var receipt = new ToriiAccountOnboardingPlanReceipt
        {
            Body = new ToriiAccountOnboardingPlanBody
            {
                Version = 1,
                Request = ValidAccountOnboardingPlanRequest(),
                Authority = OnboardingFixtureAuthority,
                NetworkId = OnboardingFixtureNetworkId,
            },
            PlanHash = "unreached",
            Signature = "unreached",
        };
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("wrong-network onboarding receipt reached HTTP dispatch"));
        using var client = new ToriiClient(new Uri("https://torii.example"), new HttpClient(handler));

        await Assert.ThrowsAnyAsync<ArgumentException>(() => client.PrepareAccountOnboardingAsync(
            receipt.Body.Request,
            receipt,
            new ToriiTairaPublicResetMutationBindingV1
            {
                AuthorizationSha256 = new string('a', 64),
                AuthorizationNonce = new string('n', 32),
                Kind = ToriiAccountOnboardingPreparedTransactionV1.OperationV1,
                Phase = "pre_edge",
                IdempotencyKey = new string('b', 64),
                ExecutionExpiresAtUnixMilliseconds = ulong.MaxValue,
            },
            PreparedAccountFeePayment,
            AccountOnboardingToken,
            OnboardingFixtureAuthority,
            NetworkId.Parse(AlternateNetworkId),
            SharedOnboardingBodyEncoder,
            cancellationToken: TestContext.Current.CancellationToken));

        Assert.Null(handler.LastRequest);
    }
}
