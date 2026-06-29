using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Crypto;
using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class OfflineNoteWalletNoteTests
{
    private const string AssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    private const string SeedHex = "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032";

    [Fact]
    public void WalletNoteDefensivelyCopiesOpaquePersistenceBytes()
    {
        var keyCertificate = Bytes(0x11, 48);
        var noteCommitment = Fixed32(0x21);
        var noteSecret = Fixed32(0x31);
        var audit = Bytes(0x41, 24);

        var note = ValidNote(
            keyCertificateNorito: keyCertificate,
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            audits: new[] { audit });

        keyCertificate[0] ^= 0xff;
        noteCommitment[0] ^= 0xff;
        noteSecret[0] ^= 0xff;
        audit[0] ^= 0xff;

        Assert.NotEqual(keyCertificate[0], note.KeyCertificateNorito[0]);
        Assert.NotEqual(noteCommitment[0], note.NoteCommitment[0]);
        Assert.NotEqual(noteSecret[0], note.NoteSecret[0]);
        Assert.NotEqual(audit[0], note.BearerAuditTrailNorito[0][0]);

        var returnedAudit = note.BearerAuditTrailNorito[0];
        returnedAudit[1] ^= 0xff;
        Assert.NotEqual(returnedAudit[1], note.BearerAuditTrailNorito[0][1]);

        Assert.Equal(Convert.ToHexString(note.NoteCommitment).ToLowerInvariant(), note.NoteCommitmentHex);
        Assert.Equal(OfflineNoteWalletNoteState.RedeemPending, note.WithState(
            OfflineNoteWalletNoteState.RedeemPending,
            1_706_000_000_250).State);
    }

    [Fact]
    public void WalletNoteScopeIdsRejectNonExactWhitespace()
    {
        var note = ValidNote(spentPaymentRequestId: "payment-request-7");

        Assert.Equal("payment-request-7", note.SpentPaymentRequestId);
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(chainId: " iroha-mainnet"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(chainId: "iroha mainnet"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(accountId: "merchant@sora"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(accountId: AccountId() + "\n"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(accountId: AccountId().Insert(8, " ")));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(accountId: AccountId() + "\u0000"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(assetId: " " + AssetId()));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(assetId: AssetDefinitionId + "#" + OtherAccountId()));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(assetId: AssetId("007")));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(amount: "10 00"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(spentPaymentRequestId: "payment-request-7 "));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(spentPaymentRequestId: "payment request-7"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(spentPaymentRequestId: "payment-request-7\u0000"));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(spentPaymentRequestId: ""));
        Assert.ThrowsAny<ArgumentException>(() => note.WithSpentPaymentRequestId("\tpayment-request-8", 300));
        Assert.ThrowsAny<ArgumentException>(() => note.WithSpentPaymentRequestId("payment request-8", 300));
        Assert.ThrowsAny<ArgumentException>(() => note.WithSpentPaymentRequestId("payment-request-8\u0000", 300));
    }

    [Theory]
    [InlineData("1.2300")]
    [InlineData("0")]
    [InlineData("-1.25")]
    public void WalletNoteAmountAcceptsCanonicalNumericText(string amount)
    {
        Assert.Equal(amount, ValidNote(amount: amount).Amount);
    }

    [Theory]
    [InlineData("not-number")]
    [InlineData("+1000")]
    [InlineData("001000")]
    [InlineData("1000.")]
    [InlineData(".1000")]
    [InlineData("-0")]
    [InlineData("1.00000000000000000000000000000")]
    public void WalletNoteAmountRejectsNonCanonicalNumericText(string amount)
    {
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(amount: amount));
    }

    [Fact]
    public void WalletNoteRejectsUnsupportedStatesAndBackwardTimestamps()
    {
        var note = ValidNote(createdAtMs: 200, updatedAtMs: 250);

        Assert.ThrowsAny<ArgumentException>(() => ValidNote(state: (OfflineNoteWalletNoteState)999));
        Assert.ThrowsAny<ArgumentException>(() => note.WithState((OfflineNoteWalletNoteState)999, 300));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(createdAtMs: 0, updatedAtMs: 0));
        Assert.ThrowsAny<ArgumentException>(() => ValidNote(createdAtMs: 200, updatedAtMs: 199));
        Assert.ThrowsAny<ArgumentException>(() => note.WithState(OfflineNoteWalletNoteState.RedeemPending, 199));
    }

    [Fact]
    public void JsonCodecRoundTripsWithCrossSdkFieldNames()
    {
        var note = ValidNote(
            origin: new OfflineNoteCommitmentOrigin.P2pOutput("payment-request-7", 2),
            state: OfflineNoteWalletNoteState.ReceivePending,
            spentPaymentRequestId: "payment-request-7",
            audits: new[] { Bytes(0x61, 16), Bytes(0x71, 17) });

        var encoded = OfflineNoteWalletNoteJsonCodec.Encode(note);
        var json = Encoding.UTF8.GetString(encoded);
        Assert.Contains("\"chain_id\":\"iroha-mainnet\"", json);
        var root = AsObject(encoded);
        Assert.Equal(AccountId(), root["account_id"]?.GetValue<string>());
        Assert.Equal(AssetId(), root["asset_id"]?.GetValue<string>());
        Assert.Contains("\"spent_payment_request_id\":\"payment-request-7\"", json);
        Assert.Contains("\"key_certificate_norito_base64\"", json);
        Assert.Contains("\"bearer_audit_trail_norito_base64\"", json);

        var decoded = OfflineNoteWalletNoteJsonCodec.Decode(encoded);
        Assert.Equal(note.ChainId, decoded.ChainId);
        Assert.Equal(note.AccountId, decoded.AccountId);
        Assert.Equal(note.AssetId, decoded.AssetId);
        Assert.Equal(note.Amount, decoded.Amount);
        Assert.Equal(note.KeyCertificateNorito, decoded.KeyCertificateNorito);
        Assert.Equal(note.NoteCommitment, decoded.NoteCommitment);
        Assert.Equal(note.NoteSecret, decoded.NoteSecret);
        Assert.Equal(note.State, decoded.State);
        Assert.Equal(note.SpentPaymentRequestId, decoded.SpentPaymentRequestId);
        Assert.Equal(2, decoded.BearerAuditTrailNorito.Count);
        var origin = Assert.IsType<OfflineNoteCommitmentOrigin.P2pOutput>(decoded.Origin);
        Assert.Equal("payment-request-7", origin.PaymentRequestId);
        Assert.Equal(2u, origin.OutputIndex);
    }

    [Fact]
    public void JsonCodecRejectsPaddedPersistedScopeFields()
    {
        var encoded = OfflineNoteWalletNoteJsonCodec.Encode(ValidNote(spentPaymentRequestId: "payment-request-7"));

        AssertRejects(JsonWith(encoded, "chain_id", " iroha-mainnet"));
        AssertRejects(JsonWith(encoded, "chain_id", "iroha mainnet"));
        AssertRejects(JsonWith(encoded, "account_id", "merchant@sora"));
        AssertRejects(JsonWith(encoded, "account_id", AccountId() + "\n"));
        AssertRejects(JsonWith(encoded, "account_id", AccountId().Insert(8, " ")));
        AssertRejects(JsonWith(encoded, "account_id", AccountId() + "\u0000"));
        AssertRejects(JsonWith(encoded, "asset_id", " " + AssetId()));
        AssertRejects(JsonWith(encoded, "asset_id", AssetDefinitionId + "#" + OtherAccountId()));
        AssertRejects(JsonWith(encoded, "asset_id", AssetId("007")));
        AssertRejects(JsonWith(encoded, "spent_payment_request_id", "payment-request-7 "));
        AssertRejects(JsonWith(encoded, "spent_payment_request_id", "payment request-7"));
        AssertRejects(JsonWith(encoded, "spent_payment_request_id", "payment-request-7\u0000"));
        AssertRejects(JsonWith(encoded, "spent_payment_request_id", ""));
    }

    [Fact]
    public void JsonCodecRejectsDuplicatePersistenceProperties()
    {
        var encodedText = Encoding.UTF8.GetString(OfflineNoteWalletNoteJsonCodec.Encode(ValidNote()));

        AssertRejects(Encoding.UTF8.GetBytes(encodedText.Replace(
            "\"chain_id\":\"iroha-mainnet\"",
            "\"chain_id\":\"iroha-mainnet\",\"chain_id\":\"evil-mainnet\"",
            StringComparison.Ordinal)));
        AssertRejects(Encoding.UTF8.GetBytes(encodedText.Replace(
            "\"type\":\"issuer_load\"",
            "\"type\":\"issuer_load\",\"type\":\"p2p_output\"",
            StringComparison.Ordinal)));
    }

    [Fact]
    public void JsonCodecRejectsUnknownPersistenceProperties()
    {
        var encoded = OfflineNoteWalletNoteJsonCodec.Encode(ValidNote());

        var unknownRoot = AsObject(encoded);
        unknownRoot["issuer_hint"] = "malicious-overlay";
        AssertRejects(JsonBytes(unknownRoot));

        var unknownOrigin = AsObject(encoded);
        unknownOrigin["origin"]!.AsObject()["payment_request_id"] = "payment-request-7";
        AssertRejects(JsonBytes(unknownOrigin));

        var unknownP2pOrigin = AsObject(encoded);
        unknownP2pOrigin["origin"] = new JsonObject
        {
            ["type"] = "p2p_output",
            ["payment_request_id"] = "payment-request-7",
            ["output_index"] = 2,
            ["lineage_id"] = "lineage-1",
        };
        AssertRejects(JsonBytes(unknownP2pOrigin));
    }

    [Fact]
    public void JsonCodecRejectsMalformedPersistenceEnvelope()
    {
        var encoded = OfflineNoteWalletNoteJsonCodec.Encode(ValidNote());

        AssertRejects("null"u8.ToArray());
        AssertRejects("[1,2,3]"u8.ToArray());
        AssertRejects([0xff, 0xfe, 0xfd]);
        AssertRejects(JsonWith(encoded, "version", 2));
        AssertRejects(JsonWith(encoded, "version", "01"));
        AssertRejects(JsonWith(encoded, "note_secret_base64", Convert.ToBase64String(Bytes(0x7a, 31))));
        AssertRejects(JsonWith(encoded, "note_commitment_hex", "0x" + Convert.ToHexString(Fixed32(0x7b))));
        AssertRejects(JsonWith(encoded, "note_commitment_hex", Convert.ToHexString(Fixed32(0x20))));
        AssertRejects(JsonWith(encoded, "key_certificate_norito_base64", " " + Convert.ToBase64String(Bytes(0x7c, 8))));
        AssertRejects(JsonWith(encoded, "key_certificate_norito_base64", "AR=="));
        AssertRejects(JsonWith(encoded, "state", "pending"));
        AssertRejects(JsonWith(encoded, "created_at_ms", 0));
        AssertRejects(JsonWith(encoded, "created_at_ms", "01706000000000"));
        AssertRejects(JsonWith(encoded, "updated_at_ms", "+1706000000100"));
        AssertRejects(JsonWith(encoded, "updated_at_ms", "18446744073709551616"));
        AssertRejects(JsonWith(encoded, "updated_at_ms", 1_706_000_000_000 - 1));
        AssertRejects(JsonWith(encoded, "amount", "not-number"));
        AssertRejects(JsonWith(encoded, "amount", "+1000"));
        AssertRejects(JsonWith(encoded, "amount", "001000"));
        AssertRejects(JsonWith(encoded, "amount", "1000."));
        AssertRejects(JsonWith(encoded, "amount", ".1000"));
        AssertRejects(JsonWith(encoded, "amount", "-0"));
        AssertRejects(JsonWith(encoded, "amount", "1.00000000000000000000000000000"));

        var originUnknown = AsObject(encoded);
        originUnknown["origin"] = new JsonObject { ["type"] = "unknown" };
        AssertRejects(JsonBytes(originUnknown));

        var originPaddedRevision = AsObject(encoded);
        originPaddedRevision["origin"]!.AsObject()["local_revision"] = "03";
        AssertRejects(JsonBytes(originPaddedRevision));

        var originOverflow = AsObject(encoded);
        originOverflow["origin"] = new JsonObject
        {
            ["type"] = "p2p_output",
            ["payment_request_id"] = "payment-request-7",
            ["output_index"] = (ulong)uint.MaxValue + 1,
        };
        AssertRejects(JsonBytes(originOverflow));

        var originPaddedOutputIndex = AsObject(encoded);
        originPaddedOutputIndex["origin"] = new JsonObject
        {
            ["type"] = "p2p_output",
            ["payment_request_id"] = "payment-request-7",
            ["output_index"] = "02",
        };
        AssertRejects(JsonBytes(originPaddedOutputIndex));

        AssertRejects(JsonWith(encoded, "bearer_audit_trail_norito_base64", "not-array"));
        var invalidAudit = AsObject(encoded);
        invalidAudit["bearer_audit_trail_norito_base64"] = new JsonArray("AA==", "A A=");
        AssertRejects(JsonBytes(invalidAudit));
        invalidAudit = AsObject(encoded);
        invalidAudit["bearer_audit_trail_norito_base64"] = new JsonArray("AA==", "AR==");
        AssertRejects(JsonBytes(invalidAudit));
    }

    [Theory]
    [InlineData("SPENDABLE", OfflineNoteWalletNoteState.Spendable)]
    [InlineData("receivePending", OfflineNoteWalletNoteState.ReceivePending)]
    [InlineData("RECEIVE_PENDING", OfflineNoteWalletNoteState.ReceivePending)]
    [InlineData("spendPending", OfflineNoteWalletNoteState.Spent)]
    [InlineData("SPEND_PENDING", OfflineNoteWalletNoteState.Spent)]
    [InlineData("CHANGE_PENDING", OfflineNoteWalletNoteState.Spendable)]
    public void JsonCodecDecodesCrossPlatformStateNames(
        string state,
        OfflineNoteWalletNoteState expected)
    {
        var encoded = JsonWith(OfflineNoteWalletNoteJsonCodec.Encode(ValidNote()), "state", state);

        Assert.Equal(expected, OfflineNoteWalletNoteJsonCodec.Decode(encoded).State);
    }

    private static OfflineNoteWalletNote ValidNote(
        string chainId = "iroha-mainnet",
        string? accountId = null,
        string? assetId = null,
        string amount = "1000",
        byte[]? keyCertificateNorito = null,
        byte[]? noteCommitment = null,
        byte[]? noteSecret = null,
        OfflineNoteCommitmentOrigin? origin = null,
        OfflineNoteWalletNoteState state = OfflineNoteWalletNoteState.Spendable,
        ulong createdAtMs = 1_706_000_000_000,
        ulong updatedAtMs = 1_706_000_000_100,
        string? spentPaymentRequestId = null,
        IEnumerable<byte[]>? audits = null)
    {
        return new OfflineNoteWalletNote(
            chainId,
            accountId ?? AccountId(),
            assetId ?? AssetId(),
            amount,
            keyCertificateNorito ?? Bytes(0x10, 48),
            noteCommitment ?? Fixed32(0x20),
            noteSecret ?? Fixed32(0x30),
            origin ?? new OfflineNoteCommitmentOrigin.IssuerLoad("load-op-1", "lineage-1", 3),
            state,
            createdAtMs,
            updatedAtMs,
            spentPaymentRequestId,
            audits);
    }

    private static string AccountId()
    {
        return Ed25519KeyPair.FromSeed(Convert.FromHexString(SeedHex))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static string OtherAccountId()
    {
        return Ed25519KeyPair.FromSeed(Bytes(0x55, 32))
            .ToAccountAddress()
            .ToI105(AccountAddress.DefaultChainDiscriminant);
    }

    private static string AssetId(string? dataspaceId = null)
    {
        var baseId = AssetDefinitionId + "#" + AccountId();
        return dataspaceId is null ? baseId : baseId + "#dataspace:" + dataspaceId;
    }

    private static void AssertRejects(byte[] payload)
    {
        Assert.ThrowsAny<ArgumentException>(() => OfflineNoteWalletNoteJsonCodec.Decode(payload));
    }

    private static byte[] JsonWith(byte[] encoded, string field, object? value)
    {
        var root = AsObject(encoded);
        root[field] = JsonValue.Create(value);
        return JsonBytes(root);
    }

    private static JsonObject AsObject(byte[] encoded)
    {
        return JsonNode.Parse(Encoding.UTF8.GetString(encoded))!.AsObject();
    }

    private static byte[] JsonBytes(JsonObject root)
    {
        return JsonSerializer.SerializeToUtf8Bytes(root);
    }

    private static byte[] Bytes(byte seed, int length)
    {
        var bytes = new byte[length];
        for (var index = 0; index < bytes.Length; index++)
        {
            bytes[index] = (byte)(seed + index);
        }

        return bytes;
    }

    private static byte[] Fixed32(byte seed) => Bytes(seed, 32);
}
