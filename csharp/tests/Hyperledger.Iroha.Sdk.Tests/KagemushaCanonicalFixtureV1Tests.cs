using System.Text.Json;
using Hyperledger.Iroha.Kagemusha;
using KagemushaCodec = Hyperledger.Iroha.Kagemusha.Kagemusha;

namespace Hyperledger.Iroha.Sdk.Tests;

/// <summary>Cross-language canonical fixture gates for the sole KAGEMUSHA V1 protocol.</summary>
public sealed class KagemushaCanonicalFixtureV1Tests
{
    [Fact]
    public void RustGeneratedDeviceMintStageFixtureIsByteIdentical()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "kagemusha_device_mint_stage_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal("KAGEMUSHA", root.GetProperty("protocol").GetString());
        Assert.Equal(21, root.GetProperty("operation").GetInt32());

        var authorizationRaw = Raw(root.GetProperty("authorization"));
        var authorization = KagemushaCodec.DecodeMintAuthorization(authorizationRaw);
        Assert.Equal(authorizationRaw, KagemushaCodec.EncodeMintAuthorization(authorization));
        var creditRaw = Raw(root.GetProperty("mint_credit"));
        var credit = KagemushaCodec.DecodeMintCredit(creditRaw, authorization);
        Assert.Equal(creditRaw, KagemushaCodec.EncodeMintCredit(credit, authorization));

        var commandRaw = Raw(root.GetProperty("command"));
        var command = KagemushaCodec.DecodeDeviceMintStageCommandShapeExact(commandRaw);
        Assert.Equal(commandRaw, KagemushaCodec.EncodeDeviceMintStageCommandShape(command));
        Assert.Equal(authorizationRaw, command.CanonicalAuthorization.ToArray());
        Assert.Equal(creditRaw, command.CanonicalMintCredit.ToArray());

        foreach (var sectionName in new[] { "staged_result", "exact_duplicate_result" })
        {
            var resultRaw = Raw(root.GetProperty(sectionName));
            var result = KagemushaCodec.DecodeDeviceMintStageResultShapeExact(resultRaw, command);
            Assert.Equal(resultRaw, KagemushaCodec.EncodeDeviceMintStageResultShape(result));
        }
    }

    [Fact]
    public void RustGeneratedThreeMessageFixtureIsByteIdentical()
    {
        using var fixture = JsonDocument.Parse(File.ReadAllText(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "kagemusha_v1.json")));
        var root = fixture.RootElement;
        Assert.Equal(1, root.GetProperty("fixture_version").GetInt32());
        Assert.Equal("KAGEMUSHA", root.GetProperty("protocol").GetString());
        Assert.Equal("kgm1:", root.GetProperty("text_prefix").GetString());

        var order = root.GetProperty("ipm1_message_order").EnumerateArray().ToArray();
        Assert.Equal(new[] { "request", "payment", "acknowledgement" },
            order.Select(static item => item.GetProperty("kind").GetString()).ToArray());
        Assert.Equal(new[] { 1, 2, 3 },
            order.Select(static item => item.GetProperty("tag").GetInt32()).ToArray());

        var requestRaw = Raw(root.GetProperty("payment_request"));
        var request = KagemushaCodec.DecodePaymentRequest(requestRaw);
        var paymentRaw = Raw(root.GetProperty("payment"));
        var payment = KagemushaCodec.DecodePayment(paymentRaw, request);
        var acknowledgementRaw = Raw(root.GetProperty("acknowledgement"));
        var acknowledgement = KagemushaCodec.DecodeAcknowledgement(
            acknowledgementRaw, request, payment);

        Assert.Equal(requestRaw, KagemushaCodec.EncodePaymentRequest(request));
        Assert.Equal(paymentRaw, KagemushaCodec.EncodePayment(payment, request));
        Assert.Equal(acknowledgementRaw,
            KagemushaCodec.EncodeAcknowledgement(acknowledgement, request, payment));
        Assert.Equal(requestRaw.Length + paymentRaw.Length + acknowledgementRaw.Length,
            KagemushaCodec.ValidateCompleteExchange(request, payment, acknowledgement));
    }

    private static byte[] Raw(JsonElement section)
    {
        var hex = section.TryGetProperty("norito_hex", out var noritoHex)
            ? noritoHex.GetString()
            : section.GetProperty("hex").GetString();
        var bytes = Convert.FromHexString(hex!);
        Assert.Equal(section.GetProperty("raw_bytes").GetInt32(), bytes.Length);
        return bytes;
    }
}
