using System.Text.Json;
using Hyperledger.Iroha.Sccp;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class SccpExactTests
{
    [Fact]
    public void SharedNativeTransferEventVectorsMatchExactly()
    {
        using var document = JsonDocument.Parse(File.ReadAllBytes(
            Path.Combine(AppContext.BaseDirectory, "Fixtures", "sccp", "native_transfer_event_v1.json")));
        foreach (var vector in document.RootElement.GetProperty("vectors").EnumerateArray())
        {
            var lane = new SccpLaneIdV1(
                SccpNetworkV1Extensions.ParseProfileKey(vector.GetProperty("source_profile").GetString()!),
                SccpNetworkV1Extensions.ParseProfileKey(vector.GetProperty("target_profile").GetString()!));
            var payload = SccpV1.DecodeLowerHex(vector.GetProperty("canonical_payload_hex").GetString()!);
            var decoded = SccpV1.DecodeCanonicalPayload(payload);
            var payloadHash = SccpV1.PayloadHash(payload);
            var messageId = SccpV1.MessageId(lane, payload);
            Assert.Equal(1U, decoded.RouteRevision);
            Assert.Equal(payload, decoded.CanonicalBytes());
            Assert.Equal(vector.GetProperty("canonical_lane_hex").GetString(), SccpV1.LowerHex(SccpV1.CanonicalLaneBytes(lane)));
            Assert.Equal(vector.GetProperty("lane_hash_hex").GetString(), SccpV1.LowerHex(SccpV1.LaneHash(lane)));
            Assert.Equal(vector.GetProperty("payload_hash_hex").GetString(), SccpV1.LowerHex(payloadHash));
            Assert.Equal(vector.GetProperty("message_id_hex").GetString(), SccpV1.LowerHex(messageId));
            Assert.Equal(
                vector.GetProperty("source_event_digest_hex").GetString(),
                SccpV1.LowerHex(SccpV1.SourceEventDigest(lane, messageId, payloadHash)));
        }
    }

    [Fact]
    public void Keccak256MatchesKnownAnswersAcrossTheRateBoundary()
    {
        Assert.Equal(
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
            SccpV1.LowerHex(SccpV1.Keccak256([])));
        Assert.Equal(
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45",
            SccpV1.LowerHex(SccpV1.Keccak256("abc"u8)));
        foreach (var (length, expected) in new (int Length, string Expected)[]
        {
            (135, "29e3704feeca7fb9ba229f0fa04d9b36449cf3ad6e1d85d9cfff3a10df9abc3e"),
            (136, "3a5912a7c5faa06ee4fe906253e339467a9ce87d533c65be3c15cb231cdb25f9"),
            (137, "bee7fbb405cb0d91a8775e338c4a5e4b5d6b2d051f687fa942043cffdc73bd28"),
            (272, "a8005c7a3125b6c3629b4181eca54d18721e41fef639718d205beb00b366ed7d"),
        })
        {
            Assert.Equal(expected, SccpV1.LowerHex(SccpV1.Keccak256(new byte[length])));
        }
    }
}
