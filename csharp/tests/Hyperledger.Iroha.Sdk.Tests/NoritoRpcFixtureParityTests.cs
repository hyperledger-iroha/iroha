using System.Net;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class NoritoRpcFixtureParityTests
{
    private const int CanonicalFixtureCount = 27;
    private const string TransactionPayloadSchemaName =
        "iroha_data_model::transaction::signed::model::TransactionPayload";
    private const string SignedTransactionSchemaName =
        "iroha_data_model::transaction::signed::model::SignedTransaction";

    private static readonly IReadOnlySet<string> ManifestFields =
        new HashSet<string>(StringComparer.Ordinal)
        {
            "fixtures",
        };

    private static readonly IReadOnlySet<string> ManifestFixtureFields =
        new HashSet<string>(StringComparer.Ordinal)
        {
            "authority",
            "creation_time_ms",
            "encoded_file",
            "encoded_len",
            "name",
            "network_id",
            "nonce",
            "payload_base64",
            "payload_hash",
            "signed_base64",
            "signed_hash",
            "signed_len",
            "time_to_live_ms",
        };

    private static readonly IReadOnlySet<string> SourceFixtureFields =
        new HashSet<string>(StringComparer.Ordinal)
        {
            "authority",
            "creation_time_ms",
            "name",
            "network_id",
            "nonce",
            "payload",
            "payload_base64",
            "payload_hash",
            "signed_base64",
            "signed_hash",
            "time_to_live_ms",
        };

    private static readonly IReadOnlySet<string> PayloadFields =
        new HashSet<string>(StringComparer.Ordinal)
        {
            "admission_intent",
            "authority",
            "creation_time_ms",
            "executable",
            "fee_payment",
            "metadata",
            "network_id",
            "nonce",
            "time_to_live_ms",
        };

    [Fact]
    public void CanonicalNoritoRpcCorpusIsClosedSchemaAndByteExact()
    {
        var fixtureDirectory = Path.Combine(AppContext.BaseDirectory, "Fixtures", "norito_rpc");
        using var manifest = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(fixtureDirectory, "transaction_fixtures.manifest.json")));
        using var sources = JsonDocument.Parse(
            File.ReadAllText(Path.Combine(fixtureDirectory, "transaction_payloads.json")));

        RequireExactProperties(manifest.RootElement, ManifestFields, "fixture manifest");
        Assert.Equal(JsonValueKind.Array, sources.RootElement.ValueKind);

        var sourceFixtures = new Dictionary<string, JsonElement>(StringComparer.Ordinal);
        foreach (var source in sources.RootElement.EnumerateArray())
        {
            ValidateSourceFixture(source);
            var name = RequireString(source, "name", "source fixture");
            Assert.True(sourceFixtures.TryAdd(name, source), $"duplicate source fixture name: {name}");
        }

        var descriptors = manifest.RootElement.GetProperty("fixtures");
        Assert.Equal(JsonValueKind.Array, descriptors.ValueKind);
        Assert.Equal(CanonicalFixtureCount, descriptors.GetArrayLength());
        Assert.Equal(CanonicalFixtureCount, sourceFixtures.Count);

        var manifestNames = new HashSet<string>(StringComparer.Ordinal);
        var encodedFiles = new HashSet<string>(StringComparer.Ordinal);
        foreach (var descriptor in descriptors.EnumerateArray())
        {
            RequireExactProperties(descriptor, ManifestFixtureFields, "manifest fixture");
            var name = RequireString(descriptor, "name", "manifest fixture");
            Assert.True(manifestNames.Add(name), $"duplicate manifest fixture name: {name}");
            Assert.True(sourceFixtures.TryGetValue(name, out var source), $"missing source fixture: {name}");

            var encodedFile = RequireString(descriptor, "encoded_file", name);
            Assert.Equal($"{name}.norito", encodedFile);
            Assert.Equal(encodedFile, Path.GetFileName(encodedFile));
            Assert.True(encodedFiles.Add(encodedFile), $"duplicate encoded_file: {encodedFile}");

            var payloadBase64 = RequireString(descriptor, "payload_base64", name);
            var payloadBytes = DecodeCanonicalBase64(payloadBase64, $"{name}.payload_base64");
            Assert.Equal(
                payloadBase64,
                RequireString(source, "payload_base64", $"source fixture {name}"));

            var encodedPath = Path.Combine(fixtureDirectory, encodedFile);
            var encodedBytes = File.ReadAllBytes(encodedPath);
            Assert.Equal(payloadBytes, encodedBytes);
            Assert.Equal(
                descriptor.GetProperty("encoded_len").GetInt64(),
                encodedBytes.LongLength);

            var payloadHash = RequireLowerHex64(descriptor, "payload_hash", name);
            Assert.Equal(
                payloadHash,
                Convert.ToHexString(IrohaHash.Hash(encodedBytes)).ToLowerInvariant());
            Assert.Equal(payloadHash, RequireString(source, "payload_hash", $"source fixture {name}"));

            var decoded = NoritoCodec.Decode(TransactionPayloadSchemaName, encodedBytes);
            Assert.Equal((byte)0x02, decoded.Flags);
            Assert.NotEmpty(decoded.Payload);

            var signedBase64 = RequireString(descriptor, "signed_base64", name);
            var signedBytes = DecodeCanonicalBase64(signedBase64, $"{name}.signed_base64");
            Assert.Equal(
                signedBase64,
                RequireString(source, "signed_base64", $"source fixture {name}"));
            Assert.Equal(descriptor.GetProperty("signed_len").GetInt64(), signedBytes.LongLength);
            var signedHash = RequireLowerHex64(descriptor, "signed_hash", name);
            Assert.Equal(signedHash, RequireString(source, "signed_hash", $"source fixture {name}"));
            var decodedSigned = NoritoCodec.Decode(SignedTransactionSchemaName, signedBytes);
            Assert.Equal((byte)0x02, decodedSigned.Flags);
            Assert.NotEmpty(decodedSigned.Payload);
            var embeddedPayload = SignedTransactionPayload(decodedSigned.Payload, name);
            Assert.Equal(decoded.Payload, embeddedPayload);
            Assert.Equal(signedHash, ExternalTransactionHashHex(embeddedPayload));
            Assert.NotEqual(
                signedHash,
                Convert.ToHexString(IrohaHash.Hash(signedBytes)).ToLowerInvariant());

            foreach (var field in new[]
            {
                "authority",
                "creation_time_ms",
                "network_id",
                "nonce",
                "time_to_live_ms",
            })
            {
                Assert.Equal(
                    descriptor.GetProperty(field).GetRawText(),
                    source.GetProperty(field).GetRawText());
            }
        }

        Assert.Equal(sourceFixtures.Keys.Order(StringComparer.Ordinal), manifestNames.Order(StringComparer.Ordinal));
        Assert.Equal(
            encodedFiles.Order(StringComparer.Ordinal),
            Directory.EnumerateFiles(fixtureDirectory, "*.norito")
                .Select(Path.GetFileName)
                .Order(StringComparer.Ordinal));
    }

    [Fact]
    public async Task SharedKotodamaArgumentRecordUsesTheProductionContractCallBoundary()
    {
        var fixturePath = Path.Combine(
            AppContext.BaseDirectory,
            "Fixtures",
            "kotodama",
            "entrypoint_argument_record_v1.json");
        using var fixture = JsonDocument.Parse(File.ReadAllText(fixturePath));
        RequireExactProperties(
            fixture.RootElement,
            new HashSet<string>(StringComparer.Ordinal)
            {
                "codec",
                "contract",
                "entrypoint_argument_record_v1",
                "entrypoint_argument_schema_v1",
                "fixture_version",
                "generator",
                "torii_boundary",
            },
            "Kotodama argument-record fixture");
        Assert.Equal(
            "EntrypointArgumentRecordV1",
            RequireString(fixture.RootElement, "codec", "Kotodama argument-record fixture"));
        Assert.Equal(
            "ivm::encode_argument_record_from_json",
            RequireString(fixture.RootElement, "generator", "Kotodama argument-record fixture"));

        var contract = fixture.RootElement.GetProperty("contract");
        RequireExactProperties(
            contract,
            new HashSet<string>(StringComparer.Ordinal) { "entrypoint", "parameters", "source" },
            "contract");
        Assert.Equal("quote", RequireString(contract, "entrypoint", "contract"));
        _ = RequireString(contract, "source", "contract");
        var parameters = contract.GetProperty("parameters");
        Assert.Equal(JsonValueKind.Array, parameters.ValueKind);
        var expectedParameters = new (string Name, string Type)[]
        {
            ("count", "int"),
            ("exact_int", "int"),
            ("exact_decimal", "decimal"),
            ("exact_quantity", "quantity"),
            ("active", "bool"),
            ("memo", "string"),
            ("digest", "bytes"),
        };
        Assert.Equal(expectedParameters.Length, parameters.GetArrayLength());
        foreach (var (parameter, expected) in parameters.EnumerateArray().Zip(expectedParameters))
        {
            RequireExactProperties(
                parameter,
                new HashSet<string>(StringComparer.Ordinal) { "name", "type" },
                $"contract parameter {expected.Name}");
            Assert.Equal(expected.Name, RequireString(parameter, "name", "contract parameter"));
            Assert.Equal(expected.Type, RequireString(parameter, "type", "contract parameter"));
        }

        var schema = fixture.RootElement.GetProperty("entrypoint_argument_schema_v1");
        RequireExactProperties(
            schema,
            new HashSet<string>(StringComparer.Ordinal) { "norito_hex", "schema_hash_hex" },
            "entrypoint argument schema");
        _ = RequireLowerHex64(schema, "schema_hash_hex", "entrypoint argument schema");
        RequireLowerHexBytes(schema, "norito_hex", "entrypoint argument schema");

        var record = fixture.RootElement.GetProperty("entrypoint_argument_record_v1");
        RequireExactProperties(
            record,
            new HashSet<string>(StringComparer.Ordinal) { "norito_hex" },
            "entrypoint argument record");
        RequireLowerHexBytes(record, "norito_hex", "entrypoint argument record");

        var boundary = fixture.RootElement.GetProperty("torii_boundary");
        RequireExactProperties(
            boundary,
            new HashSet<string>(StringComparer.Ordinal)
            {
                "authority",
                "contract_alias",
                "entrypoint",
                "fee_payment",
                "payload",
            },
            "torii_boundary");
        var boundaryPayload = boundary.GetProperty("payload");
        Assert.Equal(JsonValueKind.Object, boundaryPayload.ValueKind);
        RequireExactProperties(
            boundaryPayload,
            new HashSet<string>(StringComparer.Ordinal)
            {
                "active",
                "count",
                "digest",
                "exact_decimal",
                "exact_int",
                "exact_quantity",
                "memo",
            },
            "torii_boundary.payload");
        Assert.True(boundaryPayload.GetProperty("active").GetBoolean());
        Assert.Equal("-7", RequireString(boundaryPayload, "count", "torii_boundary.payload"));
        Assert.Equal(
            "0x000102feff",
            RequireString(boundaryPayload, "digest", "torii_boundary.payload"));
        Assert.Equal(
            "1606938044258990275541962092341162602522202993782792835301376",
            RequireString(boundaryPayload, "exact_int", "torii_boundary.payload"));
        Assert.Equal(
            "-12345678901234567890.125",
            RequireString(boundaryPayload, "exact_decimal", "torii_boundary.payload"));
        Assert.Equal(
            "12345678901234567890.0000000000000000000000000001",
            RequireString(boundaryPayload, "exact_quantity", "torii_boundary.payload"));
        Assert.Equal(
            "kotodama-v1",
            RequireString(boundaryPayload, "memo", "torii_boundary.payload"));
        var boundaryFeePayment = boundary.GetProperty("fee_payment");
        ValidateFeePayment(boundaryFeePayment, "torii_boundary.fee_payment");
        var feePayment = JsonSerializer.Deserialize<FeePaymentIntent>(boundaryFeePayment.GetRawText())
            ?? throw new InvalidDataException("torii_boundary.fee_payment decoded to null");
        _ = feePayment.GasLimit
            ?? throw new InvalidDataException("torii_boundary.fee_payment is missing gas_limit");

        JsonDocument? submittedBody = null;
        using var handler = new RecordingHandler(request =>
        {
            submittedBody = JsonDocument.Parse(
                request.Content!.ReadAsStringAsync().GetAwaiter().GetResult());
            return new HttpResponseMessage(HttpStatusCode.ServiceUnavailable)
            {
                Content = new StringContent(
                    "fixture boundary reached",
                    Encoding.UTF8,
                    "text/plain"),
            };
        });
        using var client = new ToriiClient(
            new Uri("https://fixture.invalid"),
            new HttpClient(handler));

        await Assert.ThrowsAnyAsync<Exception>(async () =>
            await client.CallContractAsync(
                new ToriiContractCallRequest
                {
                    Authority = RequireString(boundary, "authority", "torii_boundary"),
                    ContractAlias = RequireString(boundary, "contract_alias", "torii_boundary"),
                    Entrypoint = RequireString(boundary, "entrypoint", "torii_boundary"),
                    Payload = JsonNode.Parse(boundaryPayload.GetRawText()),
                    FeePayment = feePayment,
                },
                TestContext.Current.CancellationToken));

        Assert.Equal("/v1/contracts/call", handler.LastRequest!.RequestUri!.AbsolutePath);
        using var capturedBody = submittedBody
            ?? throw new InvalidDataException("contract call did not reach the HTTP boundary");
        var submitted = capturedBody.RootElement;
        Assert.Equal(
            RequireString(boundary, "authority", "torii_boundary"),
            RequireString(submitted, "authority", "submitted contract call"));
        Assert.Equal(
            RequireString(boundary, "contract_alias", "torii_boundary"),
            RequireString(submitted, "contract_alias", "submitted contract call"));
        Assert.Equal(
            RequireString(boundary, "entrypoint", "torii_boundary"),
            RequireString(submitted, "entrypoint", "submitted contract call"));
        Assert.True(
            JsonNode.DeepEquals(
                JsonNode.Parse(boundaryPayload.GetRawText()),
                JsonNode.Parse(submitted.GetProperty("payload").GetRawText())),
            "shared argument payload changed at the C# contract-call boundary");
        Assert.True(
            JsonNode.DeepEquals(
                JsonNode.Parse(boundaryFeePayment.GetRawText()),
                JsonNode.Parse(submitted.GetProperty("fee_payment").GetRawText())),
            "shared fee payment changed at the C# contract-call boundary");
        Assert.False(submitted.TryGetProperty("argument_record", out _));
        Assert.False(submitted.TryGetProperty("argument_record_norito_hex", out _));
    }

    [Fact]
    public void ClosedSchemaGuardRejectsUnknownAndDuplicateFields()
    {
        using var unknown = JsonDocument.Parse("""{"fixtures":[],"legacy":[]}""");
        Assert.Throws<InvalidDataException>(
            () => RequireExactProperties(unknown.RootElement, ManifestFields, "fixture manifest"));

        using var duplicate = JsonDocument.Parse("""{"fixtures":[],"fixtures":[]}""");
        Assert.Throws<InvalidDataException>(
            () => RequireExactProperties(duplicate.RootElement, ManifestFields, "fixture manifest"));
    }

    private static void ValidateSourceFixture(JsonElement source)
    {
        RequireExactProperties(source, SourceFixtureFields, "source fixture");
        var name = RequireString(source, "name", "source fixture");
        var payload = source.GetProperty("payload");
        RequireExactProperties(payload, PayloadFields, $"{name}.payload");
        RequireExactProperties(
            payload.GetProperty("admission_intent"),
            new HashSet<string>(StringComparer.Ordinal) { "intent", "value" },
            $"{name}.payload.admission_intent");
        ValidateFeePayment(payload.GetProperty("fee_payment"), $"{name}.payload.fee_payment");
        Assert.Equal(JsonValueKind.Object, payload.GetProperty("metadata").ValueKind);
        ValidateExecutable(payload.GetProperty("executable"), $"{name}.payload.executable");

        foreach (var field in new[]
        {
            "authority",
            "creation_time_ms",
            "network_id",
            "nonce",
            "time_to_live_ms",
        })
        {
            Assert.Equal(source.GetProperty(field).GetRawText(), payload.GetProperty(field).GetRawText());
        }
    }

    private static void ValidateExecutable(JsonElement executable, string context)
    {
        Assert.Equal(JsonValueKind.Object, executable.ValueKind);
        var variants = executable.EnumerateObject().ToArray();
        if (variants.Length != 1)
        {
            throw new InvalidDataException($"{context} must contain exactly one variant");
        }

        var variant = variants[0];
        switch (variant.Name)
        {
            case "Ivm":
                Assert.Equal(JsonValueKind.String, variant.Value.ValueKind);
                break;
            case "Instructions":
                ValidateInstructions(variant.Value, $"{context}.Instructions");
                break;
            case "ContractCall":
                ValidateContractCall(variant.Value, $"{context}.ContractCall");
                break;
            case "Batch":
                Assert.Equal(JsonValueKind.Array, variant.Value.ValueKind);
                foreach (var (item, index) in variant.Value.EnumerateArray().Select((item, index) => (item, index)))
                {
                    Assert.Equal(JsonValueKind.Object, item.ValueKind);
                    var entries = item.EnumerateObject().ToArray();
                    if (entries.Length != 1)
                    {
                        throw new InvalidDataException($"{context}.Batch[{index}] must contain exactly one variant");
                    }
                    if (entries[0].Name == "Instruction")
                    {
                        ValidateInstruction(entries[0].Value, $"{context}.Batch[{index}].Instruction");
                    }
                    else if (entries[0].Name == "ContractCall")
                    {
                        ValidateContractCall(entries[0].Value, $"{context}.Batch[{index}].ContractCall");
                    }
                    else
                    {
                        throw new InvalidDataException($"{context}.Batch[{index}] has an unknown variant");
                    }
                }
                break;
            default:
                throw new InvalidDataException($"{context} has unknown variant {variant.Name}");
        }
    }

    private static void ValidateInstructions(JsonElement instructions, string context)
    {
        Assert.Equal(JsonValueKind.Array, instructions.ValueKind);
        foreach (var (instruction, index) in instructions.EnumerateArray().Select((item, index) => (item, index)))
        {
            ValidateInstruction(instruction, $"{context}[{index}]");
        }
    }

    private static void ValidateInstruction(JsonElement instruction, string context)
    {
        RequireExactProperties(
            instruction,
            new HashSet<string>(StringComparer.Ordinal) { "payload_base64", "wire_name" },
            context);
        _ = DecodeCanonicalBase64(RequireString(instruction, "payload_base64", context), $"{context}.payload_base64");
        _ = RequireString(instruction, "wire_name", context);
    }

    private static void ValidateContractCall(JsonElement call, string context)
    {
        RequireExactProperties(
            call,
            new HashSet<string>(StringComparer.Ordinal)
            {
                "arguments",
                "contract_address",
                "entrypoint",
                "expected_code_hash",
            },
            context);
        _ = RequireString(call, "contract_address", context);
        _ = RequireString(call, "entrypoint", context);
        _ = RequireString(call, "expected_code_hash", context);
        var arguments = call.GetProperty("arguments");
        Assert.True(
            arguments.ValueKind is JsonValueKind.Array or JsonValueKind.Null,
            $"{context}.arguments must be an array or null");
    }

    private static void ValidateFeePayment(JsonElement feePayment, string context)
    {
        RequireExactProperties(
            feePayment,
            new HashSet<string>(StringComparer.Ordinal) { "payer", "value" },
            context);
        _ = RequireString(feePayment, "payer", context);
        var value = feePayment.GetProperty("value");
        RequireExactProperties(
            value,
            new HashSet<string>(StringComparer.Ordinal) { "charge_limits", "gas_limit" },
            $"{context}.value");
        var limits = value.GetProperty("charge_limits");
        Assert.Equal(JsonValueKind.Array, limits.ValueKind);
        foreach (var (limit, index) in limits.EnumerateArray().Select((item, index) => (item, index)))
        {
            var limitContext = $"{context}.value.charge_limits[{index}]";
            RequireExactProperties(
                limit,
                new HashSet<string>(StringComparer.Ordinal)
                {
                    "asset_definition_id",
                    "kind",
                    "max_amount",
                },
                limitContext);
            _ = RequireString(limit, "asset_definition_id", limitContext);
            Assert.Equal(JsonValueKind.String, limit.GetProperty("max_amount").ValueKind);
            RequireExactProperties(
                limit.GetProperty("kind"),
                new HashSet<string>(StringComparer.Ordinal) { "kind", "value" },
                $"{limitContext}.kind");
        }
        Assert.True(
            value.GetProperty("gas_limit").ValueKind is JsonValueKind.Number or JsonValueKind.Null,
            $"{context}.value.gas_limit must be a number or null");
    }

    private static void RequireExactProperties(
        JsonElement value,
        IReadOnlySet<string> expected,
        string context)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new InvalidDataException($"{context} must be an object");
        }

        var actual = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!actual.Add(property.Name))
            {
                throw new InvalidDataException($"{context} contains duplicate field {property.Name}");
            }
        }
        var missing = expected.Except(actual, StringComparer.Ordinal).Order(StringComparer.Ordinal).ToArray();
        var unexpected = actual.Except(expected, StringComparer.Ordinal).Order(StringComparer.Ordinal).ToArray();
        if (missing.Length != 0 || unexpected.Length != 0)
        {
            throw new InvalidDataException(
                $"{context} has invalid fields: missing=[{string.Join(',', missing)}], "
                + $"unexpected=[{string.Join(',', unexpected)}]");
        }
    }

    private static string RequireString(JsonElement value, string field, string context)
    {
        if (!value.TryGetProperty(field, out var encoded)
            || encoded.ValueKind != JsonValueKind.String
            || encoded.GetString() is not { Length: > 0 } text)
        {
            throw new InvalidDataException($"{context}.{field} must be a non-empty string");
        }
        return text;
    }

    private static byte[] DecodeCanonicalBase64(string value, string context)
    {
        byte[] decoded;
        try
        {
            decoded = Convert.FromBase64String(value);
        }
        catch (FormatException error)
        {
            throw new InvalidDataException($"{context} must be base64", error);
        }
        if (!string.Equals(Convert.ToBase64String(decoded), value, StringComparison.Ordinal))
        {
            throw new InvalidDataException($"{context} must be canonical base64");
        }
        return decoded;
    }

    private static byte[] SignedTransactionPayload(byte[] value, string context)
    {
        var reader = new CanonicalNoritoReader(value, $"{context}.signed", nameof(value));
        _ = reader.ReadField("signature");
        var payload = reader.ReadField("payload").ToArray();
        _ = reader.ReadField("multisig_signatures");
        reader.RequireEnd();
        return payload;
    }

    private static string ExternalTransactionHashHex(ReadOnlySpan<byte> canonicalPayload)
    {
        var entrypoint = new CanonicalNoritoWriter();
        entrypoint.WriteUInt32LittleEndian(0);
        entrypoint.WriteField(canonicalPayload);
        return Convert.ToHexString(IrohaHash.Hash(entrypoint.ToArray())).ToLowerInvariant();
    }

    private static string RequireLowerHex64(JsonElement value, string field, string context)
    {
        var text = RequireString(value, field, context);
        if (text.Length != 64 || text.Any(static character => !IsLowerHex(character)))
        {
            throw new InvalidDataException($"{context}.{field} must be 32-byte lowercase hex");
        }
        return text;
    }

    private static void RequireLowerHexBytes(JsonElement value, string field, string context)
    {
        var text = RequireString(value, field, context);
        if (text.Length == 0 || text.Length % 2 != 0 || text.Any(static character => !IsLowerHex(character)))
        {
            throw new InvalidDataException($"{context}.{field} must be lowercase hex bytes");
        }
    }

    private static bool IsLowerHex(char value) =>
        value is >= '0' and <= '9' or >= 'a' and <= 'f';

    private sealed class RecordingHandler : HttpMessageHandler
    {
        private readonly Func<HttpRequestMessage, HttpResponseMessage> responder;

        public RecordingHandler(Func<HttpRequestMessage, HttpResponseMessage> responder)
        {
            this.responder = responder;
        }

        public HttpRequestMessage? LastRequest { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            LastRequest = request;
            var response = responder(request);
            response.RequestMessage ??= request;
            return Task.FromResult(response);
        }
    }
}
