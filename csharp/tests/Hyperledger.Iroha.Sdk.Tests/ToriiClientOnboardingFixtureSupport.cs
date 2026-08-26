using System.Text.Json;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static string SharedOnboardingReceiptJson()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "alias_setup_v1.json")));
        return fixture.RootElement
            .GetProperty("account_onboarding_receipt_vector")
            .GetProperty("receipt_json")
            .GetRawText();
    }

    private static ToriiTairaPublicResetMutationBindingV1 ValidPreparedMutationBinding(
        string operation) =>
        new()
        {
            AuthorizationSha256 = new string('a', 64),
            AuthorizationNonce = new string('n', 32),
            Kind = operation,
            Phase = "pre_edge",
            IdempotencyKey = new string('b', 64),
            ExecutionExpiresAtUnixMilliseconds = ulong.MaxValue,
        };
}
