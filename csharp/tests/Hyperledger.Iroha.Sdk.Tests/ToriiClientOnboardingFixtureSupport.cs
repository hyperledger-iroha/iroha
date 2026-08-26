using System.Text.Json;
using System.Text.Json.Nodes;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    private static string SharedOnboardingReceiptJson()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "alias_setup_v1.json")));
        var receipt = JsonNode.Parse(fixture.RootElement
            .GetProperty("account_onboarding_receipt_vector")
            .GetProperty("receipt_json")
            .GetRawText())!.AsObject();
        var body = receipt["body"]!.AsObject();
        _ = body.Remove("chain_id");
        body["network_id"] =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
        receipt["plan_hash"] =
            "hash:B470C0FC328FE6BFF36A20946DBDC531FE67CC3A04B1E8F95CE03697C13466F7#1D51";
        receipt["signature"] =
            "EC8AA4FB1140B7685F3D1E7F6649C31A8454F92867285488F2F0AB1CC89FE905F15A6C9A44DBB31013172BBBDBB76D7C83F25F0988CE13E0FB97FDB1F291400E";
        return receipt.ToJsonString();
    }
}
