using System.Net;
using System.Buffers.Binary;
using System.Text.Json;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class TransactionBuilderTests
{
    private const string FixtureSeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Theory]
    [InlineData("swift_transfer_asset_basic", 761, 1379, "bbefa0a333292f0a8e910363f00cc79554edd579d83a953a4171d2faf1976445")]
    [InlineData("swift_mint_asset_basic", 651, 1269, "35da74c14f2243f1d4beaac1611911ccfbdf9f5db6252d008be87c971c88da19")]
    [InlineData("swift_burn_asset_basic", 651, 1269, "3cd19ec99c36fbf38f057a9f7fdddeb003d6838980469d0479b7910e98ca1bb5")]
    public void BuildSignedProducesDeterministicGoldenOutputs(
        string fixtureName,
        int expectedPayloadLength,
        int expectedSignedLength,
        string expectedHashHex)
    {
        using var payloadsDocument = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "swift_parity_payloads.json")));

        var payload = payloadsDocument.RootElement.EnumerateArray()
            .First(candidate => candidate.GetProperty("name").GetString() == fixtureName)
            .GetProperty("payload");

        var builder = new TransactionBuilder(
            payload.GetProperty("chain").GetString()!,
            payload.GetProperty("authority").GetString()!)
            .SetCreationTimeMilliseconds((ulong)payload.GetProperty("creation_time_ms").GetInt64())
            .SetTimeToLiveMilliseconds((ulong)payload.GetProperty("time_to_live_ms").GetInt64())
            .SetNonce((uint)payload.GetProperty("nonce").GetInt32());

        var instruction = payload.GetProperty("executable").GetProperty("Instructions")[0];
        var arguments = instruction.GetProperty("arguments");
        var action = arguments.GetProperty("action").GetString();
        var assetDefinitionId = arguments.GetProperty("asset_definition_id").GetString()!;
        var quantity = arguments.GetProperty("quantity").GetString()!;
        var destination = arguments.GetProperty("destination").GetString()!;

        _ = action switch
        {
            "TransferAsset" => builder.TransferAsset(assetDefinitionId, quantity, destination),
            "MintAsset" => builder.MintAsset(assetDefinitionId, quantity, destination),
            "BurnAsset" => builder.BurnAsset(assetDefinitionId, quantity, destination),
            _ => throw new InvalidOperationException($"Unsupported fixture action `{action}`."),
        };

        var envelope = builder.BuildSigned(Convert.FromHexString(FixtureSeedHex));
        Assert.Equal(expectedPayloadLength, envelope.PayloadBytes.Length);
        Assert.Equal(expectedSignedLength, envelope.SignedTransactionBytes.Length);
        Assert.Equal(expectedHashHex, envelope.TransactionHashHex);

        AssertSignedEnvelopeStructure(envelope, Convert.FromHexString(FixtureSeedHex));
    }

    [Fact]
    public async Task LedgerClientSubmitAndWaitPollsUntilTerminalState()
    {
        var transaction = new TransactionBuilder("00000042", "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53")
            .TransferAsset("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", "15.7500", "sorauロ1NイリウdPBeシRoクQ2ヤgシQqeカヘスチhRW2コソZ9ユヲUナRX5NJYH53")
            .SetCreationTimeMilliseconds(1736000000000)
            .SetTimeToLiveMilliseconds(3500)
            .SetNonce(17)
            .BuildSigned(Convert.FromHexString(FixtureSeedHex));

        var statusPollCount = 0;
        using var handler = new RecordingHandler(request =>
        {
            if (request.RequestUri!.AbsolutePath == "/transaction")
            {
                return new HttpResponseMessage(HttpStatusCode.Accepted)
                {
                    Content = new ByteArrayContent(Array.Empty<byte>()),
                };
            }

            statusPollCount++;
            var body = statusPollCount switch
            {
                1 => """
                    {
                      "kind": "Transaction",
                      "content": {
                        "hash": "da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b",
                        "status": { "kind": "Queued", "content": null },
                        "scope": "auto",
                        "resolved_from": "queue"
                      }
                    }
                    """,
                _ => """
                    {
                      "kind": "Transaction",
                      "content": {
                        "hash": "da01f3a369d10e6ad78f241c86f4fe2d5481ff13ace97e6fb5db5c30240bdb3b",
                        "status": { "kind": "Applied", "block_height": 11, "content": null },
                        "scope": "auto",
                        "resolved_from": "state"
                      }
                    }
                    """,
            };

            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(body),
            };
        });

        using var client = new IrohaClient(new Uri("https://torii.example"), new HttpClient(handler));
        var status = await client.Ledger.SubmitAndWaitAsync(
            transaction,
            new PipelineSubmitOptions
            {
                PollInterval = TimeSpan.FromMilliseconds(1),
                Timeout = TimeSpan.FromSeconds(1),
            });

        Assert.Equal(PipelineTransactionState.Applied, status.State);
        Assert.Equal((ulong)11, status.BlockHeight);
        Assert.True(status.IsTerminal);
    }

    private static void AssertSignedEnvelopeStructure(SignedTransactionEnvelope envelope, byte[] privateKeySeed)
    {
        var signatureField = ReadField(envelope.SignedTransactionBytes, out var offsetAfterSignature);
        var payloadField = ReadField(envelope.SignedTransactionBytes[offsetAfterSignature..], out var offsetAfterPayload);
        var attachmentsField = ReadField(envelope.SignedTransactionBytes[(offsetAfterSignature + offsetAfterPayload)..], out var offsetAfterAttachments);
        var multisigField = ReadField(envelope.SignedTransactionBytes[(offsetAfterSignature + offsetAfterPayload + offsetAfterAttachments)..], out _);

        Assert.Equal(envelope.PayloadBytes, payloadField);
        Assert.Equal(new byte[] { 0 }, attachmentsField);
        Assert.Equal(new byte[] { 0 }, multisigField);

        var signature = DecodeConstVec(signatureField);
        var payloadHash = IrohaHash.Hash(envelope.PayloadBytes);
        var publicKey = Ed25519Signer.GetPublicKey(privateKeySeed);
        Assert.True(Ed25519Signer.Verify(payloadHash, signature, publicKey));
    }

    private static byte[] ReadField(ReadOnlySpan<byte> bytes, out int consumed)
    {
        var length = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        consumed = 8 + length;
        return bytes.Slice(8, length).ToArray();
    }

    private static byte[] DecodeConstVec(ReadOnlySpan<byte> bytes)
    {
        var count = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes[..8]));
        var output = new byte[count];
        var offset = 8;
        for (var index = 0; index < count; index++)
        {
            var fieldLength = checked((int)BinaryPrimitives.ReadUInt64LittleEndian(bytes.Slice(offset, 8)));
            Assert.Equal(1, fieldLength);
            output[index] = bytes[offset + 8];
            offset += 9;
        }

        Assert.Equal(bytes.Length, offset);
        return output;
    }

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> responder;

        public RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            this.responder = responder;
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }
}
