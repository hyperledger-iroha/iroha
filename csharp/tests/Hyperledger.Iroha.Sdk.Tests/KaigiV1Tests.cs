using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Kaigi;
using Hyperledger.Iroha.Address;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KaigiV1Tests
{
    private static readonly byte[] Modulus = Convert.FromHexString("01000000ED302D991BF94C09FC98462200000000000000000000000000000040");
    private static readonly KaigiId CallId = new("wonderland.sora", "weekly-sync");
    private static readonly string[] Accounts = Fixture()["accounts"]!.AsArray().Select(static item => item!.GetValue<string>()).ToArray();

    [Fact]
    public void ScalarPreservesZeroAndModulusMinusOneWithoutHashMarkerOrReduction()
    {
        var zero = new byte[32]; var max = (byte[])Modulus.Clone(); max[0]--;
        foreach (var bytes in new[] { zero, max, Enumerable.Repeat((byte)0x22, 32).ToArray() })
        {
            var scalar = new KaigiAuthorizationScalarV1(bytes);
            Assert.Equal(bytes, scalar.ToLittleEndianBytes());
            Assert.Equal(scalar, new KaigiAuthorizationScalarV1(bytes));
            var original = scalar.ToString(); bytes[0] ^= 0xff;
            var returned = scalar.ToLittleEndianBytes(); returned[^1] ^= 0xff;
            Assert.Equal(original, scalar.ToString());
        }
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationScalarV1(Modulus));
        var over = (byte[])Modulus.Clone(); over[0]++;
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationScalarV1(over));
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationScalarV1(new byte[31]));
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationScalarV1(new byte[33]));
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationScalarV1(Enumerable.Repeat((byte)0xff, 32).ToArray()));
    }

    [Theory]
    [InlineData("CreateKaigi")]
    [InlineData("JoinKaigi")]
    [InlineData("LeaveKaigi")]
    [InlineData("EndKaigi")]
    [InlineData("RecordKaigiUsage")]
    public void EveryTransparentInstructionMatchesRustVerifiedInstructionBox(string name)
    {
        TransactionInstruction instruction = name switch
        {
            "CreateKaigi" => TransactionInstruction.CreateKaigi(new NewKaigi(CallId, Accounts[0])),
            "JoinKaigi" => TransactionInstruction.JoinKaigi(CallId, Accounts[0]),
            "LeaveKaigi" => TransactionInstruction.LeaveKaigi(CallId, Accounts[0]),
            "EndKaigi" => TransactionInstruction.EndKaigi(CallId),
            "RecordKaigiUsage" => TransactionInstruction.RecordKaigiUsage(CallId, 1, 2),
            _ => throw new InvalidOperationException(),
        };
        var vector = Fixture()["vectors"]!.AsArray().Single(item => item!["name"]!.GetValue<string>() == name)!;
        Assert.Equal(vector["wire_id"]!.GetValue<string>(), instruction.WireId);
        Assert.Equal(vector["inner_type_name"]!.GetValue<string>(), instruction.TypeName);
        Assert.Equal(Convert.FromBase64String(vector["instruction_box_base64"]!.GetValue<string>()), instruction.EncodeInstructionBox(Accounts[0]));
    }

    [Fact]
    public void ComplexPrivateCreateMatchesRustVerifiedFullWidthFixture()
    {
        var c = Enumerable.Repeat((byte)0x44, 32).ToArray(); c[^1] = 4;
        var n = Enumerable.Repeat((byte)0x55, 32).ToArray(); n[^1] = 5;
        var root = Enumerable.Repeat((byte)0x66, 32).ToArray(); root[^1] = 0x67;
        var metadata = new Dictionary<string, JsonNode?> { ["z"] = JsonNode.Parse("[true,null,7]"), ["a"] = JsonNode.Parse("{\"nested\":\"value\"}") };
        var relay = new KaigiRelayManifest([
            new(Accounts[0], [0x10, 0x20], 1), new(Accounts[1], [0x30], 2), new(Accounts[2], [0x40, 0x50, 0x60], 255)], 1ul << 63);
        var call = new NewKaigi(CallId, Accounts[0], "Roadmap 🛰", "exact", 7, 9_007_199_254_740_993,
            metadata, 1_234_567_890_123, Accounts[0], KaigiPrivacyMode.ZkRosterV1, KaigiRoomPolicy.Public, relay);
        var artifacts = new KaigiAuthorizationArtifactsV1(new(new(c)), new(new(n)), root, [1, 2, 3]);
        var instruction = TransactionInstruction.CreateKaigi(call, artifacts);
        var expected = Convert.FromBase64String(Fixture()["complex_create"]!["instruction_box_base64"]!.GetValue<string>());
        Assert.Equal(872, expected.Length);
        Assert.Equal(expected, instruction.EncodeInstructionBox(Accounts[0]));
        metadata["a"]!["nested"] = "changed"; call.Metadata["a"]!["nested"] = "changed again";
        root[0] = 0; artifacts.RosterRoot[0] = 0; artifacts.Proof[0] = 0; relay.Hops[0].HpkePublicKey[0] = 0;
        Assert.Equal(expected, instruction.EncodeInstructionBox(Accounts[0]));
    }

    [Fact]
    public void EveryPrivateActionUsesOneFieldScalarWrappersAndExactProofVector()
    {
        var maximum = (byte[])Modulus.Clone(); maximum[0]--;
        var artifacts = new KaigiAuthorizationArtifactsV1(new(new(new byte[32])), new(new(maximum)), Root(), [1, 2, 3]);
        var context = new TransactionEncodingContext(Accounts[0]);
        TransactionInstruction[] instructions = [
            TransactionInstruction.CreateKaigi(new NewKaigi(CallId, Accounts[0], privacyMode: KaigiPrivacyMode.ZkRosterV1), artifacts),
            TransactionInstruction.JoinKaigi(CallId, Accounts[0], artifacts),
            TransactionInstruction.LeaveKaigi(CallId, Accounts[0], artifacts),
            TransactionInstruction.EndKaigi(CallId, ulong.MaxValue, artifacts)];
        for (var index = 0; index < instructions.Length; index++)
        {
            var fields = Fields(instructions[index].EncodePayload(context));
            var offset = index == 0 ? 1 : 2;
            Assert.Equal(offset + 4, fields.Count);
            Assert.Equal(new byte[32], Assert.Single(Fields(Some(fields[offset]))));
            Assert.Equal(maximum, Assert.Single(Fields(Some(fields[offset + 1]))));
            Assert.Equal(Root(), Some(fields[offset + 2]));
            Assert.Equal(new byte[] { 3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3 }, Some(fields[offset + 3]));
            Assert.NotEmpty(instructions[index].EncodeInstructionBox(Accounts[0]));
        }
        var usage = TransactionInstruction.RecordKaigiUsage(CallId, ulong.MaxValue, ulong.MaxValue, new(new(maximum), [1, 2, 3]));
        var usageFields = Fields(usage.EncodePayload(context));
        Assert.Equal(5, usageFields.Count);
        Assert.Equal(ulong.MaxValue, BinaryPrimitives.ReadUInt64LittleEndian(usageFields[1]));
        Assert.Equal(ulong.MaxValue, BinaryPrimitives.ReadUInt64LittleEndian(usageFields[2]));
        Assert.Equal(maximum, Some(usageFields[3]));
        Assert.Equal(new byte[] { 3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3 }, Some(usageFields[4]));
    }

    [Fact]
    public void AllFivePrivateInstructionBoxesMatchRustVerifiedFinalScalarFixtures()
    {
        var maximum = (byte[])Modulus.Clone(); maximum[0]--;
        var artifacts = new KaigiAuthorizationArtifactsV1(new(new(Enumerable.Repeat((byte)0x22, 32).ToArray())), new(new(maximum)), Root(), Encoding.UTF8.GetBytes("wire-fixture-only"));
        TransactionInstruction[] instructions = [
            TransactionInstruction.CreateKaigi(new NewKaigi(CallId, Accounts[0], privacyMode: KaigiPrivacyMode.ZkRosterV1), artifacts),
            TransactionInstruction.JoinKaigi(CallId, Accounts[0], artifacts),
            TransactionInstruction.LeaveKaigi(CallId, Accounts[0], artifacts),
            TransactionInstruction.EndKaigi(CallId, authorization: artifacts),
            TransactionInstruction.RecordKaigiUsage(CallId, 1, 0, new(new(maximum), Encoding.UTF8.GetBytes("wire-fixture-only")))];
        var fixture = JsonNode.Parse(File.ReadAllBytes(RepositoryFile("csharp/tests/Hyperledger.Iroha.Sdk.Tests/Fixtures/kaigi_private_instruction_wire_v1.json")))!;
        var vectors = fixture["vectors"]!.AsArray();
        Assert.Equal(5, vectors.Count);
        for (var index = 0; index < instructions.Length; index++)
        {
            Assert.EndsWith("::" + vectors[index]!["action"]!.GetValue<string>(), instructions[index].WireId);
            var expected = Convert.FromBase64String(vectors[index]!["instruction_box_base64"]!.GetValue<string>());
            Assert.Equal(vectors[index]!["sha256"]!.GetValue<string>(), Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(expected)).ToLowerInvariant());
            Assert.Equal(expected, instructions[index].EncodeInstructionBox(Accounts[0]));
        }
    }

    [Fact]
    public void TypedConstructionRejectsPartialPrivateModesAndInvalidConfiguration()
    {
        var artifacts = new KaigiAuthorizationArtifactsV1(new(new(new byte[32])), new(new(new byte[32])), Root(), [1]);
        Assert.Throws<ArgumentException>(() => TransactionInstruction.CreateKaigi(new NewKaigi(CallId, Accounts[0], privacyMode: KaigiPrivacyMode.ZkRosterV1)));
        Assert.Throws<ArgumentException>(() => TransactionInstruction.CreateKaigi(new NewKaigi(CallId, Accounts[0]), artifacts));
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationArtifactsV1(artifacts.Commitment, artifacts.Nullifier, new byte[32], [1]));
        Assert.Throws<ArgumentException>(() => new KaigiAuthorizationArtifactsV1(artifacts.Commitment, artifacts.Nullifier, Root(), []));
        Assert.Throws<ArgumentException>(() => new KaigiUsageArtifactsV1(new(new byte[32]), []));
        Assert.Throws<ArgumentOutOfRangeException>(() => TransactionInstruction.RecordKaigiUsage(CallId, 0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => new NewKaigi(CallId, Accounts[0], maxParticipants: 4097));
        Assert.Throws<ArgumentOutOfRangeException>(() => new NewKaigi(CallId, Accounts[0], maxParticipants: 0));
        Assert.Throws<ArgumentException>(() => new NewKaigi(CallId, Accounts[0], billingAccount: Accounts[1]));
        Assert.Throws<ArgumentException>(() => new NewKaigi(CallId, Accounts[0], privacyMode: (KaigiPrivacyMode)2));
        Assert.Throws<ArgumentException>(() => new KaigiRelayManifest([new(Accounts[0], [1], 1)], 1));
        Assert.Throws<ArgumentException>(() => new KaigiRelayManifest([new(Accounts[0], [1], 1), new(Accounts[0], [2], 1), new(Accounts[1], [3], 1)], 1));
        Assert.Throws<ArgumentException>(() => new KaigiRelayHop(Accounts[0], [], 1));
        Assert.Throws<ArgumentOutOfRangeException>(() => new KaigiRelayHop(Accounts[0], [1], 0));
        Assert.Throws<ArgumentException>(() => new KaigiId("Wonderland.sora", "valid"));
        Assert.Throws<ArgumentException>(() => new KaigiId("wonderland.sora", "bad name"));
        Assert.Throws<ArgumentException>(() => new KaigiId("wonderland.sora", "e\u0301"));
        Assert.Throws<EncoderFallbackException>(() => new NewKaigi(CallId, Accounts[0], title: "\ud800"));
    }

    [Fact]
    public void RustRetainedRecordPreservesOriginalHostParticipationAndRawPublicScalars()
    {
        var record = KaigiRecordV1.FromJson(RecordBytes());
        Assert.Equal(CallId, record.Id); Assert.Equal(Accounts[0], record.Host);
        Assert.Equal(KaigiPrivacyMode.ZkRosterV1, record.Call.PrivacyMode);
        Assert.Equal(Accounts[1], Assert.Single(record.PrivateParticipation.Entries).OriginalAccount);
        Assert.Equal(1ul, record.PrivateParticipation.Entries[0].Sequence);
        Assert.Equal(record.RosterCommitments[0].Commitment, record.PrivateParticipation.Entries[0].ActiveCommitment);
        Assert.Equal(2, record.NullifierLog.Count);
        Assert.Equal(1u, record.SegmentsRecorded); Assert.Single(record.UsageCommitments);
        Assert.Equal(KaigiStatus.Active, record.Status); Assert.Null(record.EndedAtMs);
        var root = record.RosterRoot; root[0] ^= 0xff; Assert.NotEqual(root, record.RosterRoot);
        Assert.Equal(Enumerable.Repeat((byte)0x22, 32), record.HostCommitment!.Commitment.ToLittleEndianBytes());
    }

    [Theory]
    [InlineData("missing_host")]
    [InlineData("missing_participation")]
    [InlineData("old_alias_tag")]
    [InlineData("old_issued_at_ms")]
    [InlineData("scalar_modulus")]
    [InlineData("scalar_short")]
    [InlineData("scalar_long")]
    [InlineData("scalar_hex")]
    [InlineData("duplicate_subject")]
    [InlineData("zero_sequence")]
    [InlineData("active_max_sequence")]
    [InlineData("roster_mismatch")]
    [InlineData("host_participant")]
    [InlineData("host_commitment_in_roster")]
    [InlineData("duplicate_usage")]
    [InlineData("empty_private_nullifiers")]
    [InlineData("unknown_enum")]
    [InlineData("missing_enum_state")]
    [InlineData("unmarked_root")]
    [InlineData("string_integer")]
    [InlineData("extra_field")]
    public void RetainedRecordRejectsRetiredOrMalformedState(string mutation)
    {
        var root = JsonNode.Parse(RecordBytes())!.AsObject();
        var entry = root["private_participation"]!["entries"]![0]!;
        switch (mutation)
        {
            case "missing_host": root.Remove("host"); break;
            case "missing_participation": root.Remove("private_participation"); break;
            case "old_alias_tag": root["host_commitment"]!["alias_tag"] = null; break;
            case "old_issued_at_ms": root["nullifier_log"]![0]!["issued_at_ms"] = 0; break;
            case "scalar_modulus": root["host_commitment"]!["commitment"] = BytesNode(Modulus); break;
            case "scalar_short": root["host_commitment"]!["commitment"] = BytesNode(new byte[31]); break;
            case "scalar_long": root["host_commitment"]!["commitment"] = BytesNode(new byte[33]); break;
            case "scalar_hex": root["host_commitment"]!["commitment"] = new string('0', 64); break;
            case "duplicate_subject": root["private_participation"]!["entries"]!.AsArray().Add(entry.DeepClone()); break;
            case "zero_sequence": entry["sequence"] = 0; break;
            case "active_max_sequence": entry["sequence"] = ulong.MaxValue; break;
            case "roster_mismatch": entry["active_commitment"] = BytesNode(new byte[32]); break;
            case "host_participant": entry["original_account"] = Accounts[0]; break;
            case "host_commitment_in_roster":
                entry["active_commitment"] = root["host_commitment"]!["commitment"]!.DeepClone();
                root["roster_commitments"]![0]!["commitment"] = root["host_commitment"]!["commitment"]!.DeepClone(); break;
            case "duplicate_usage": root["usage_commitments"]!.AsArray().Add(root["usage_commitments"]![0]!.DeepClone()); root["segments_recorded"] = 2; break;
            case "empty_private_nullifiers": root["nullifier_log"] = new JsonArray(); break;
            case "unknown_enum": root["privacy_mode"]!["mode"] = "ZkRosterV2"; break;
            case "missing_enum_state": root["privacy_mode"]!.AsObject().Remove("state"); break;
            case "unmarked_root": root["roster_root"] = new string('0', 64); break;
            case "string_integer": root["segments_recorded"] = "1"; break;
            case "extra_field": root["original_host"] = Accounts[0]; break;
        }
        var encoded = Encoding.UTF8.GetBytes(root.ToJsonString());
        if (mutation == "unmarked_root") Assert.Throws<FormatException>(() => KaigiRecordV1.FromJson(encoded));
        else Assert.ThrowsAny<ArgumentException>(() => KaigiRecordV1.FromJson(encoded));
    }

    [Fact]
    public void RecordDecoderRejectsDuplicateKeysTrailingValuesAndOversizedJson()
    {
        var text = Encoding.UTF8.GetString(RecordBytes());
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(text.Replace("\"sequence\":1", "\"sequence\":1,\"sequence\":1", StringComparison.Ordinal))));
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(text.Replace("\"metadata\":{}", "\"metadata\":{\"a\":{\"x\":1,\"x\":2}}", StringComparison.Ordinal))));
        Assert.ThrowsAny<JsonException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(text + "{}")));
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(new byte[1_048_577]));
    }

    [Theory]
    [InlineData("scalar", "3.0")]
    [InlineData("scalar", "3e0")]
    [InlineData("sequence", "1.0")]
    [InlineData("sequence", "1e0")]
    public void RetainedRecordRejectsFractionalAndExponentIntegerSpellings(string field, string replacement)
    {
        var text = Encoding.UTF8.GetString(RecordBytes());
        text = field == "scalar"
            ? text.Replace("\"usage_commitments\":[[3,", "\"usage_commitments\":[[" + replacement + ",", StringComparison.Ordinal)
            : text.Replace("\"sequence\":1", "\"sequence\":" + replacement, StringComparison.Ordinal);
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(text)));
    }

    [Fact]
    public void RetainedLedgerAllowsZeroCommitmentAndInactiveExhaustedSequence()
    {
        var root = JsonNode.Parse(RecordBytes())!;
        var entry = root["private_participation"]!["entries"]![0]!;
        entry["active_commitment"] = BytesNode(new byte[32]);
        root["roster_commitments"]![0]!["commitment"] = BytesNode(new byte[32]);
        var zero = KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString()));
        Assert.Equal(new byte[32], zero.PrivateParticipation.Entries[0].ActiveCommitment!.ToLittleEndianBytes());
        entry["active_commitment"] = null; entry["sequence"] = ulong.MaxValue;
        root["roster_commitments"] = new JsonArray();
        var left = KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString()));
        Assert.Null(left.PrivateParticipation.Entries[0].ActiveCommitment);
        Assert.Equal(ulong.MaxValue, left.PrivateParticipation.Entries[0].Sequence);
    }

    [Fact]
    public void FullOriginalControllerIdentityIgnoresOnlyNetworkDisplayPrefix()
    {
        var subject = AccountAddress.Parse(Accounts[1]);
        var alternate = subject.ToI105(42);
        Assert.NotEqual(Accounts[1], alternate);
        var root = JsonNode.Parse(RecordBytes())!;
        var entries = root["private_participation"]!["entries"]!.AsArray();
        var duplicate = entries[0]!.DeepClone();
        duplicate["original_account"] = alternate; duplicate["active_commitment"] = null;
        entries.Add(duplicate);
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString())));
        entries.RemoveAt(1);
        entries[0]!["original_account"] = AccountAddress.Parse(Accounts[0]).ToI105(42);
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString())));
        Assert.Throws<ArgumentException>(() => new KaigiRelayManifest([
            new(Accounts[1], [1], 1), new(alternate, [2], 1), new(Accounts[2], [3], 1)], 1));
        Assert.Equal(AccountAddress.Parse(Accounts[0]).ToI105(42),
            new NewKaigi(CallId, Accounts[0], billingAccount: AccountAddress.Parse(Accounts[0]).ToI105(42)).BillingAccount);

        // Two valid multisig controllers with one signer but different weights
        // are distinct original accounts. A public-key-only projection loses this.
        var key = subject.PublicKey;
        var controller = new byte[12 + key.Length];
        controller[0] = 0x0a; controller[1] = 1; controller[2] = 1;
        controller[4] = 1; controller[6] = 1; controller[7] = 1;
        controller[9] = 1; controller[11] = checked((byte)key.Length); key.CopyTo(controller, 12);
        var first = AccountAddress.FromCanonicalBytes(controller).ToI105();
        controller[9] = 2;
        var second = AccountAddress.FromCanonicalBytes(controller).ToI105();
        entries[0]!["original_account"] = first;
        var retained = entries[0]!.DeepClone(); retained["original_account"] = second; retained["active_commitment"] = null;
        entries.Add(retained);
        var record = KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString()));
        Assert.Equal(new[] { first, second }, record.PrivateParticipation.Entries.Select(static entry => entry.OriginalAccount));
    }

    [Fact]
    public void RecordEnforcesEffectiveCapacityMetadataOwnershipAndLifecycle()
    {
        foreach (var mutation in new[] { "end_missing", "active_timestamp", "ended_before_create", "orphan_metadata", "transparent_participant" })
        {
            var root = JsonNode.Parse(RecordBytes())!;
            switch (mutation)
            {
                case "end_missing": root["status"]!["status"] = "Ended"; break;
                case "active_timestamp": root["ended_at_ms"] = 1; break;
                case "ended_before_create": root["status"]!["status"] = "Ended"; root["ended_at_ms"] = 0; root["created_at_ms"] = 1; break;
                case "orphan_metadata": root["participant_metadata"]![Accounts[2]] = new JsonObject(); break;
                case "transparent_participant": root["participants"]!.AsArray().Add(Accounts[1]); break;
            }
            Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString())));
        }
        var capacity = JsonNode.Parse(RecordBytes())!;
        capacity["max_participants"] = 1;
        var c = new byte[32]; c[0] = 7;
        capacity["roster_commitments"]!.AsArray().Add(new JsonObject { ["commitment"] = BytesNode(c) });
        capacity["private_participation"]!["entries"]!.AsArray().Add(new JsonObject {
            ["original_account"] = Accounts[2], ["sequence"] = 1, ["active_commitment"] = BytesNode(c) });
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(capacity.ToJsonString())));
    }

    [Fact]
    public void ActivePrivateRecordReservesLiveLeaveAndHostEndInNullifierBound()
    {
        var root = JsonNode.Parse(RecordBytes())!;
        var history = new JsonArray();
        for (var index = 0; index < 8193; index++)
        {
            var bytes = new byte[32]; BinaryPrimitives.WriteUInt32LittleEndian(bytes, checked((uint)index));
            history.Add(new JsonObject { ["digest"] = BytesNode(bytes) });
        }
        root["nullifier_log"] = history;
        Assert.Throws<ArgumentException>(() => KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString())));
        root["status"]!["status"] = "Ended"; root["ended_at_ms"] = root["created_at_ms"]!.DeepClone();
        Assert.Equal(8193, KaigiRecordV1.FromJson(Encoding.UTF8.GetBytes(root.ToJsonString())).NullifierLog.Count);
    }

    private static JsonArray BytesNode(byte[] bytes) => new(bytes.Select(static b => (JsonNode?)JsonValue.Create(b)).ToArray());
    private static byte[] Root() => Enumerable.Repeat((byte)0x55, 32).ToArray();
    private static JsonNode Fixture() => JsonNode.Parse(File.ReadAllBytes(RepositoryFile("python/iroha_python/tests/fixtures/kaigi_instruction_wire_v1.json")))!;
    private static byte[] RecordBytes() => File.ReadAllBytes(RepositoryFile("csharp/tests/Hyperledger.Iroha.Sdk.Tests/Fixtures/kaigi_record_v1.json"));
    private static string RepositoryFile(string relative)
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory); directory is not null; directory = directory.Parent)
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml"))) return Path.Combine(directory.FullName, relative);
        throw new InvalidOperationException("Kaigi fixture tests require the repository checkout.");
    }
    private static List<byte[]> Fields(byte[] payload)
    {
        var reader = new CanonicalNoritoReader(payload, "test", nameof(payload)); var fields = new List<byte[]>();
        while (!reader.IsFinished) fields.Add(reader.ReadField("field").ToArray());
        reader.RequireEnd(); return fields;
    }
    private static byte[] Some(byte[] payload)
    {
        var reader = new CanonicalNoritoReader(payload, "option", nameof(payload));
        Assert.Equal(1, reader.ReadByte("tag")); var inner = reader.ReadField("some").ToArray(); reader.RequireEnd(); return inner;
    }
}
