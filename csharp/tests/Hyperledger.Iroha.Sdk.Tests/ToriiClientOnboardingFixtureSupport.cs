using System.Text.Json;

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
}
