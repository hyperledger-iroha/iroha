using System.Text.Json;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Norito;
using System.Text.Json.Nodes;

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

    private static ToriiPreparedOperationBindingV1 ValidPreparedMutationBinding(string operation)
    {
        // These are public immutable fixture identities, never runtime authorization material.
        var semanticHash = operation == ToriiAccountOnboardingPreparedTransactionV1.OperationV1
            ? PreparedOnboardingReceipt().PlanHash.Substring(5, 64).ToLowerInvariant()
            : PreparedFaucetClaimHash(CanonicalAccountId, 68, "00");
        return new ToriiPreparedOperationBindingV1
        {
            SemanticHashHex = semanticHash,
            Kind = operation,
            RequestId = new string('b', 64),
            ExecutionExpiresAtUnixMilliseconds = PreparedOnboardingReceipt().Body.ValidUntilMilliseconds,
        };
    }

    private static ToriiAccountOnboardingPlanReceipt PreparedOnboardingReceipt() =>
        DeserializePreparedFixture<ToriiAccountOnboardingPlanReceipt>(
            PreparedTransactionSignatureVector("onboarding_prepared").GetProperty("response").GetProperty("receipt"));

    // This fixture encoder follows routing.rs::onboarding_receipt_fixture: retain the
    // immutable alias fixture fields, update both guards/deadline and its framed instruction.
    // It is deliberately restricted to the captured shape, with request fields encoded
    // from the supplied body so request-substitution tests exercise the real receipt hash.
    private static byte[] PreparedOnboardingBodyEncoder(ToriiAccountOnboardingPlanBody body)
    {
        var expected = JsonSerializer.SerializeToNode(PreparedOnboardingReceipt().Body)!.AsObject();
        var actual = JsonSerializer.SerializeToNode(body)!.AsObject();
        expected["request"] = actual["request"]!.DeepClone();
        if (!JsonNode.DeepEquals(expected, actual))
        {
            throw new ArgumentException("Prepared fixture encoder received unsupported non-request fields.", nameof(body));
        }
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "alias_setup_v1.json")));
        var original = Convert.FromHexString(fixture.RootElement.GetProperty("account_onboarding_receipt_vector")
            .GetProperty("canonical_body_norito_hex").GetString()!);
        var fields = PreparedFixtureFields(original, 11);
        var guard = PreparedFixtureFields(fields[7], 4);
        Assert.Equal(PreparedU64(50_000), guard[3]);
        guard[3] = PreparedU64(body.ValidUntilMilliseconds);
        var resource = PreparedFixtureFields(fields[5], 4);
        Assert.Equal(1, resource[2][0]);
        var quote = PreparedFixtureFields(PreparedFixtureFields(resource[2][1..], 1)[0], 7);
        Assert.Equal(fields[7], quote[3]);
        fields[7] = PreparedFixturePack(guard);
        quote[3] = fields[7];
        resource[2] = [1, .. PreparedFixturePack([PreparedFixturePack(quote)])];
        fields[5] = PreparedFixturePack(resource);
        var instructions = new CanonicalNoritoWriter();
        instructions.WriteSequenceLength((ulong)body.Instructions.GetArrayLength());
        foreach (var instruction in body.Instructions.EnumerateArray())
        {
            var payload = instruction.GetProperty("framed_payload").EnumerateArray().Select(value => value.GetByte()).ToArray();
            var vector = new CanonicalNoritoWriter();
            vector.WriteSequenceLength((ulong)payload.Length);
            vector.WriteBytes(payload);
            instructions.WriteField(PreparedFixturePack([
                PreparedString(instruction.GetProperty("wire_id").GetString()!), vector.ToArray()]));
        }
        fields[8] = instructions.ToArray();
        fields[10] = PreparedU64(body.ValidUntilMilliseconds);
        var permissions = new CanonicalNoritoWriter();
        permissions.WriteSequenceLength((ulong)body.Request.Permissions.Count);
        foreach (var permission in body.Request.Permissions) permissions.WriteField(PreparedString(permission));
        fields[1] = PreparedFixturePack([[body.Request.Version], PreparedString(body.Request.Alias),
            PreparedString(body.Request.AccountId), permissions.ToArray()]);
        return PreparedFixturePack(fields);
    }

    private static byte[][] PreparedFixtureFields(byte[] bytes, int count)
    {
        var reader = new CanonicalNoritoReader(bytes, "prepared receipt fixture", nameof(bytes));
        var fields = new byte[count][];
        for (var index = 0; index < count; index++) fields[index] = reader.ReadField($"field[{index}]").ToArray();
        reader.RequireEnd();
        return fields;
    }

    private static byte[] PreparedFixturePack(byte[][] fields)
    {
        var writer = new CanonicalNoritoWriter();
        foreach (var field in fields) writer.WriteField(field);
        return writer.ToArray();
    }
}
