using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class DomainTransactionFixtureTests
{
    private const string Authority = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";

    public static IEnumerable<object[]> NativeCases()
    {
        var domains = new[] { "banka.universal", "xn--bcher-kva.universal", "bank_a.universal", new string('a', 63) + "." + new string('b', 63) };
        foreach (var operation in new[] { "TransferDomain", "SetDomainKeyValue", "RemoveDomainKeyValue" })
            foreach (var domain in domains)
                yield return new object[] { operation, domain };
    }

    [Theory]
    [MemberData(nameof(NativeCases))]
    public void DomainInstructionFramesMatchNative(string operation, string domain)
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "domain_transaction_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal(1, root.GetProperty("norito_layout_version").GetInt32());
        Assert.Equal(0x02, root.GetProperty("norito_layout_flags").GetInt32());
        Assert.Equal("iroha_data_model/examples/domain_transaction_fixture.rs", root.GetProperty("generator").GetString());
        var cases = root.GetProperty("cases").EnumerateArray().ToArray();
        Assert.Equal(12, cases.Length);
        Assert.Equal(12, cases.Select(item => (item.GetProperty("name").GetString(), item.GetProperty("domain_id").GetString())).Distinct().Count());
        var item = Assert.Single(cases, candidate => candidate.GetProperty("name").GetString() == operation && candidate.GetProperty("domain_id").GetString() == domain);
        var authority = root.GetProperty("authority").GetString()!;
        var destination = root.GetProperty("destination").GetString()!;
        var key = root.GetProperty("metadata_key").GetString()!;
        TransactionInstruction instruction = operation switch
        {
            "TransferDomain" => TransactionInstruction.TransferDomain(domain, destination),
            "SetDomainKeyValue" => TransactionInstruction.SetDomainKeyValue(domain, key, JsonValue.Create(root.GetProperty("metadata_value").GetString())),
            "RemoveDomainKeyValue" => TransactionInstruction.RemoveDomainKeyValue(domain, key),
            _ => throw new InvalidOperationException("Unexpected native operation."),
        };
        var actual = instruction.EncodeInstructionBox(authority);
        Assert.Equal(Convert.FromHexString(item.GetProperty("instruction_box_frame_hex").GetString()!), actual);
        var (payload, flags) = NoritoCodec.DecodeWithSchemaHash(actual.AsSpan(6, 16), actual);
        Assert.Equal(0x02, flags);
        Assert.Equal(Convert.FromHexString(item.GetProperty("instruction_box_payload_hex").GetString()!), payload);
    }

    [Theory]
    [InlineData("banka")]
    [InlineData("BANKA.universal")]
    [InlineData("banka.UNIVERSAL")]
    [InlineData("banka.universal.extra")]
    [InlineData(" banka.universal")]
    [InlineData("banka.universal ")]
    [InlineData("bank a.universal")]
    [InlineData("-banka.universal")]
    [InlineData("banka-.universal")]
    [InlineData("xn--a.universal")]
    [InlineData("bücher.universal")]
    [InlineData("banka.\u0000universal")]
    public void DomainInstructionsRejectNoncanonicalInputBeforeEncoding(string domain)
    {
        Assert.Throws<ArgumentException>(() => new TransferDomainInstruction(domain, Authority));
        Assert.Throws<ArgumentException>(() => new SetDomainKeyValueInstruction(domain, "memo", JsonValue.Create("value")));
        Assert.Throws<ArgumentException>(() => new RemoveDomainKeyValueInstruction(domain, "memo"));
        var transfer = TransactionInstruction.TransferDomain("banka.universal", Authority);
        var set = TransactionInstruction.SetDomainKeyValue("banka.universal", "memo", JsonValue.Create("value"));
        var remove = TransactionInstruction.RemoveDomainKeyValue("banka.universal", "memo");
        Assert.Throws<ArgumentException>(() => transfer with { DomainId = domain });
        Assert.Throws<ArgumentException>(() => set with { DomainId = domain });
        Assert.Throws<ArgumentException>(() => remove with { DomainId = domain });
    }

    [Fact]
    public void DomainInstructionsRejectOverlongLabels()
    {
        foreach (var domain in new[] { new string('a', 64) + ".universal", "banka." + new string('a', 64) })
            DomainInstructionsRejectNoncanonicalInputBeforeEncoding(domain);
    }
}
