using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

namespace Hyperledger.Iroha.Torii;

public sealed partial class ToriiClient
{
    public const int KagemushaRequiredBridgeAbiVersion = ToriiKagemushaTransport.BridgeAbiVersion;
    public const int KagemushaManifestVersion = ToriiKagemushaTransport.ManifestVersion;
    public const int KagemushaMaximumHops = ToriiKagemushaTransport.MaxHops;

    public async Task<ToriiKagemushaReadinessV4> GetKagemushaReadinessV4Async(
        string assetDefinitionId,
        CancellationToken cancellationToken = default)
    {
        var selector = RequireKagemushaSelector(assetDefinitionId, nameof(assetDefinitionId));
        var query = BuildQueryString(
        [
            new KeyValuePair<string, string?>("asset_definition_id", selector),
        ]);
        using var response = await SendAsync(
            HttpMethod.Get,
            "/v1/offline/readiness",
            query,
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha readiness response",
            cancellationToken);
        return ParseKagemushaReadiness(document.RootElement, selector);
    }

    public Task<ToriiKagemushaOperationReference> SubmitKagemushaTopUpV4Async(
        ToriiKagemushaTopUpRequestV4 request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        return SubmitKagemushaV4Async(
            "/v1/offline/top-up",
            request.Norito,
            request.OperationId,
            ToriiKagemushaOperationKind.TopUp,
            cancellationToken);
    }

    public Task<ToriiKagemushaOperationReference> SubmitKagemushaRedeemV4Async(
        ToriiKagemushaRedeemRequestV4 request,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        return SubmitKagemushaV4Async(
            "/v1/offline/redeem",
            request.Norito,
            request.OperationId,
            ToriiKagemushaOperationKind.Redeem,
            cancellationToken);
    }

    public async Task<ToriiKagemushaOperationStatus> GetKagemushaOperationStatusAsync(
        string operationId,
        CancellationToken cancellationToken = default)
    {
        var canonicalId = ToriiKagemushaTransport.RequireOperationId(operationId, nameof(operationId));
        using var response = await SendAsync(
            HttpMethod.Get,
            $"/v1/offline/operations/{canonicalId}",
            content: null,
            accept: "application/json",
            cancellationToken: cancellationToken);
        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation status response",
            cancellationToken);
        return ParseKagemushaOperationStatus(document.RootElement, canonicalId);
    }

    private async Task<ToriiKagemushaOperationReference> SubmitKagemushaV4Async(
        string path,
        byte[] norito,
        string operationId,
        ToriiKagemushaOperationKind expectedKind,
        CancellationToken cancellationToken)
    {
        using var content = new ByteArrayContent(norito);
        content.Headers.ContentType = new MediaTypeHeaderValue("application/x-norito");
        using var response = await SendAsync(
            HttpMethod.Post,
            path,
            content: content,
            accept: "application/json",
            configureRequest: message =>
            {
                message.Headers.Remove("Idempotency-Key");
                message.Headers.TryAddWithoutValidation("Idempotency-Key", operationId);
            },
            cancellationToken: cancellationToken);
        if (response.StatusCode != HttpStatusCode.Accepted)
        {
            throw new InvalidDataException($"Kagemusha command expected HTTP 202, got {(int)response.StatusCode}.");
        }

        using var document = await ReadKagemushaJsonAsync(
            response,
            "Kagemusha operation reference response",
            cancellationToken);
        var location = response.Headers.Location?.OriginalString;
        return ParseKagemushaOperationReference(
            document.RootElement,
            operationId,
            expectedKind,
            location);
    }

    private static string RequireKagemushaSelector(string? value, string parameterName)
    {
        ArgumentNullException.ThrowIfNull(value, parameterName);
        if (value.Length is 0 or > 512
            || !string.Equals(value, value.Trim(), StringComparison.Ordinal)
            || value.Any(static character => char.IsControl(character)))
        {
            throw new ArgumentException(
                "Kagemusha asset selector must be exact, non-empty text without control characters.",
                parameterName);
        }

        return value;
    }

    private static async Task<JsonDocument> ReadKagemushaJsonAsync(
        HttpResponseMessage response,
        string context,
        CancellationToken cancellationToken)
    {
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"{context} must use Content-Type application/json.");
        }

        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        return await ParseJsonDocumentRejectingDuplicatePropertiesAsync(
            stream,
            context,
            cancellationToken);
    }

    private ToriiKagemushaReadinessV4 ParseKagemushaReadiness(
        JsonElement root,
        string requestedSelector)
    {
        RequireExactFields(
            root,
            "Kagemusha readiness response",
            "required_bridge_abi_version",
            "max_hops",
            "asset_definition_id",
            "asset_scale",
            "evaluated_block_height",
            "evaluated_block_hash",
            "active_transfer_verifier",
            "active_topup_shield_verifier",
            "active_unshield_verifier",
            "active_recursive_step_eq_verifier",
            "active_recursive_step_ep_verifier",
            "artifact_set",
            "proof_backend_available",
            "recursive_lineage_supported",
            "ready",
            "blockers");
        var readiness = root.Deserialize<ToriiKagemushaReadinessV4>(serializerOptions)
            ?? throw new JsonException("Kagemusha readiness response deserialized to null.");
        if (readiness.RequiredBridgeAbiVersion != KagemushaRequiredBridgeAbiVersion)
        {
            throw new JsonException($"required_bridge_abi_version must be {KagemushaRequiredBridgeAbiVersion}.");
        }
        if (readiness.MaxHops != KagemushaMaximumHops)
        {
            throw new JsonException($"max_hops must be {KagemushaMaximumHops}.");
        }
        if (requestedSelector.Contains('#')
            && !string.Equals(readiness.AssetDefinitionId, requestedSelector, StringComparison.Ordinal))
        {
            throw new JsonException("asset_definition_id does not match the requested canonical asset.");
        }
        RequireLowerHex32(readiness.EvaluatedBlockHash, "evaluated_block_hash");

        var verifierFields = new (string Field, ToriiKagemushaActiveVerifier? Value)[]
        {
            ("active_transfer_verifier", readiness.ActiveTransferVerifier),
            ("active_topup_shield_verifier", readiness.ActiveTopUpShieldVerifier),
            ("active_unshield_verifier", readiness.ActiveUnshieldVerifier),
            ("active_recursive_step_eq_verifier", readiness.ActiveRecursiveStepEqVerifier),
            ("active_recursive_step_ep_verifier", readiness.ActiveRecursiveStepEpVerifier),
        };
        foreach (var (field, verifier) in verifierFields)
        {
            if (verifier is not null)
            {
                ValidateKagemushaVerifier(verifier, field, readiness.EvaluatedBlockHeight);
            }
        }

        var hasEq = readiness.ActiveRecursiveStepEqVerifier is not null;
        var hasEp = readiness.ActiveRecursiveStepEpVerifier is not null;
        if (hasEq != hasEp)
        {
            throw new JsonException("ABI-20 V4 recursive verifier records must be reported as an Eq/Ep pair.");
        }
        if ((readiness.ArtifactSet is not null) != hasEq)
        {
            throw new JsonException("artifact_set and the ABI-20 V4 Eq/Ep verifier pair must be reported together.");
        }
        if (readiness.ArtifactSet is { } artifactSet)
        {
            ValidateKagemushaArtifactSet(artifactSet, readiness);
        }
        if (readiness.ProofBackendAvailable && readiness.ArtifactSet is null)
        {
            throw new JsonException("proof_backend_available requires an authenticated V4 artifact_set.");
        }

        var expectedLineage = readiness.ProofBackendAvailable
            && readiness.ArtifactSet is not null
            && hasEq
            && hasEp;
        if (readiness.RecursiveLineageSupported != expectedLineage)
        {
            throw new JsonException("recursive_lineage_supported contradicts the ABI-20 runtime conjunction.");
        }
        var expectedReady = expectedLineage
            && readiness.AssetScale is <= 28
            && readiness.ActiveTransferVerifier is not null
            && readiness.ActiveTopUpShieldVerifier is not null
            && readiness.ActiveUnshieldVerifier is not null
            && readiness.Blockers.Length == 0;
        if (readiness.Ready != expectedReady)
        {
            throw new JsonException("ready contradicts the complete ABI-20 runtime conjunction.");
        }
        if (readiness.Blockers.Any(static blocker =>
                string.IsNullOrWhiteSpace(blocker.Code)
                || string.IsNullOrWhiteSpace(blocker.Message)))
        {
            throw new JsonException("Kagemusha readiness blockers require non-empty code and message fields.");
        }

        return readiness with { Blockers = readiness.Blockers.ToArray() };
    }

    private static void ValidateKagemushaVerifier(
        ToriiKagemushaActiveVerifier verifier,
        string field,
        ulong evaluatedHeight)
    {
        if (!string.Equals(verifier.Id.Backend, "halo2/ipa", StringComparison.Ordinal)
            || verifier.Version == 0
            || verifier.MaxProofBytes == 0
            || verifier.ActivationHeight > evaluatedHeight
            || verifier.WithdrawalHeight is { } withdrawalHeight && withdrawalHeight <= evaluatedHeight)
        {
            throw new JsonException($"{field} is not active under the production Kagemusha verifier contract.");
        }
        RequireLowerHex32(verifier.Commitment, $"{field}.commitment");
        RequireLowerHex32(verifier.PublicInputsSchemaHash, $"{field}.public_inputs_schema_hash");

        var expectedName = field switch
        {
            "active_recursive_step_eq_verifier" => "kagemusha_recursive_step_eq_v4_verifier_record",
            "active_recursive_step_ep_verifier" => "kagemusha_recursive_step_ep_v4_verifier_record",
            _ => null,
        };
        var expectedCircuit = field switch
        {
            "active_recursive_step_eq_verifier" => "kagemusha-recursive-spend-step-eq-authenticated-layout-v4",
            "active_recursive_step_ep_verifier" => "kagemusha-recursive-spend-step-ep-authenticated-layout-v4",
            _ => null,
        };
        if (expectedName is not null
            && (!string.Equals(verifier.Id.Name, expectedName, StringComparison.Ordinal)
                || !string.Equals(verifier.CircuitId, expectedCircuit, StringComparison.Ordinal)))
        {
            throw new JsonException($"{field} does not identify its ABI-20 V4 verifier role.");
        }
    }

    private static void ValidateKagemushaArtifactSet(
        ToriiKagemushaAuthenticatedArtifactSetV4 artifactSet,
        ToriiKagemushaReadinessV4 readiness)
    {
        RequireLowerHex32(artifactSet.ManifestSha256, "artifact_set.manifest_sha256");
        RequireLowerHex32(artifactSet.ReleasePolicySha256, "artifact_set.release_policy_sha256");
        RequireLowerHex32(artifactSet.ReleaseAttestationSha256, "artifact_set.release_attestation_sha256");
        if (string.IsNullOrWhiteSpace(artifactSet.Generation)
            || artifactSet.ActivationHeight == 0
            || artifactSet.WithdrawalHeight <= artifactSet.ActivationHeight
            || artifactSet.ActivationHeight > readiness.EvaluatedBlockHeight
            || artifactSet.WithdrawalHeight <= readiness.EvaluatedBlockHeight
            || artifactSet.MaxProofBytes == 0
            || artifactSet.AssetScale != readiness.AssetScale
            || artifactSet.MaxProofBytes != readiness.ActiveRecursiveStepEqVerifier!.MaxProofBytes
            || artifactSet.MaxProofBytes != readiness.ActiveRecursiveStepEpVerifier!.MaxProofBytes)
        {
            throw new JsonException("artifact_set does not match the authenticated active ABI-20 V4 release.");
        }
    }

    private static ToriiKagemushaOperationReference ParseKagemushaOperationReference(
        JsonElement root,
        string expectedOperationId,
        ToriiKagemushaOperationKind expectedKind,
        string? location)
    {
        RequireExactFields(
            root,
            "Kagemusha operation reference",
            "operation_id",
            "kind",
            "state",
            "transaction_hash",
            "status_uri",
            "submitted_at_ms");
        var operationId = RequireJsonOperationId(root.GetProperty("operation_id"), "operation_id");
        var kind = ParseTaggedKind(root.GetProperty("kind"), "kind");
        var state = ParseTaggedPendingState(root.GetProperty("state"));
        var transactionHash = RequireJsonHash(root.GetProperty("transaction_hash"), "transaction_hash");
        var statusUri = RequireJsonString(root.GetProperty("status_uri"), "status_uri");
        var expectedUri = $"/v1/offline/operations/{expectedOperationId}";
        if (!string.Equals(operationId, expectedOperationId, StringComparison.Ordinal)
            || kind != expectedKind
            || state != ToriiKagemushaOperationState.Pending
            || !string.Equals(statusUri, expectedUri, StringComparison.Ordinal)
            || !string.Equals(location, expectedUri, StringComparison.Ordinal))
        {
            throw new JsonException("Kagemusha operation reference does not match the submitted V4 command.");
        }

        return new ToriiKagemushaOperationReference
        {
            OperationId = operationId,
            Kind = kind,
            State = state,
            TransactionHash = transactionHash,
            StatusUri = statusUri,
            SubmittedAtMilliseconds = RequireJsonUInt64(root.GetProperty("submitted_at_ms"), "submitted_at_ms"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseKagemushaOperationStatus(
        JsonElement root,
        string expectedOperationId)
    {
        RequireExactFields(root, "Kagemusha operation status", "state", "value");
        var stateText = RequireJsonString(root.GetProperty("state"), "state");
        var value = root.GetProperty("value");
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha operation status value must be an object.");
        }
        var operationId = RequireJsonOperationId(value.GetProperty("operation_id"), "value.operation_id");
        if (!string.Equals(operationId, expectedOperationId, StringComparison.Ordinal))
        {
            throw new JsonException("Kagemusha operation status does not match the requested operation id.");
        }

        return stateText switch
        {
            "pending" => ParsePendingStatus(value, operationId),
            "applied" => ParseAppliedStatus(value, operationId),
            "rejected" => ParseRejectedStatus(value, operationId),
            _ => throw new JsonException("Kagemusha operation state must be pending, applied, or rejected."),
        };
    }

    private static ToriiKagemushaOperationStatus ParsePendingStatus(JsonElement value, string operationId)
    {
        RequireExactFields(
            value,
            "Kagemusha pending operation",
            "operation_id",
            "kind",
            "transaction_hash",
            "submitted_at_ms");
        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Pending,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            SubmittedAtMilliseconds = RequireJsonUInt64(value.GetProperty("submitted_at_ms"), "value.submitted_at_ms"),
        };
    }

    private static ToriiKagemushaOperationStatus ParseAppliedStatus(JsonElement value, string operationId)
    {
        RequireExactFields(value, "Kagemusha applied operation", "operation_id", "result");
        var resultTag = value.GetProperty("result");
        RequireExactFields(resultTag, "Kagemusha applied result", "kind", "result");
        var kindText = RequireJsonString(resultTag.GetProperty("kind"), "value.result.kind");
        var result = resultTag.GetProperty("result");
        if (kindText == "redeem")
        {
            RequireExactFields(
                result,
                "Kagemusha redeem result",
                "transaction_hash",
                "finalized_block_height",
                "server_time_ms");
            return new ToriiKagemushaOperationStatus
            {
                OperationId = operationId,
                State = ToriiKagemushaOperationState.Applied,
                Kind = ToriiKagemushaOperationKind.Redeem,
                RedeemResult = new ToriiKagemushaRedeemResultV4
                {
                    TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                    FinalizedBlockHeight = RequireJsonUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                    ServerTimeMilliseconds = RequireJsonUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
                },
            };
        }
        if (kindText != "top_up")
        {
            throw new JsonException("Kagemusha applied result kind must be top_up or redeem.");
        }

        RequireExactFields(
            result,
            "Kagemusha top-up result",
            "transaction_hash",
            "finalized_block_height",
            "server_time_ms",
            "anchor",
            "finality_proof");
        var anchor = result.GetProperty("anchor");
        if (RequireJsonUInt64(anchor.GetProperty("version"), "result.anchor.version") != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up anchor must use V4.");
        }
        var artifactBinding = anchor.GetProperty("artifact_binding");
        if (RequireJsonUInt64(artifactBinding.GetProperty("version"), "result.anchor.artifact_binding.version")
            != KagemushaManifestVersion)
        {
            throw new JsonException("Kagemusha top-up artifact binding must use V4.");
        }

        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Applied,
            Kind = ToriiKagemushaOperationKind.TopUp,
            TopUpResult = new ToriiKagemushaTopUpResultV4
            {
                TransactionHash = RequireJsonHash(result.GetProperty("transaction_hash"), "result.transaction_hash"),
                FinalizedBlockHeight = RequireJsonUInt64(result.GetProperty("finalized_block_height"), "result.finalized_block_height"),
                ServerTimeMilliseconds = RequireJsonUInt64(result.GetProperty("server_time_ms"), "result.server_time_ms"),
                Anchor = anchor.Clone(),
                FinalityProof = result.GetProperty("finality_proof").Clone(),
            },
        };
    }

    private static ToriiKagemushaOperationStatus ParseRejectedStatus(JsonElement value, string operationId)
    {
        RequireExactFields(
            value,
            "Kagemusha rejected operation",
            "operation_id",
            "kind",
            "transaction_hash",
            "error");
        var error = value.GetProperty("error");
        if (error.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException("Kagemusha rejection error must be an object.");
        }
        var details = error.TryGetProperty("details", out var detailValue) && detailValue.ValueKind != JsonValueKind.Null
            ? detailValue.Clone()
            : (JsonElement?)null;
        return new ToriiKagemushaOperationStatus
        {
            OperationId = operationId,
            State = ToriiKagemushaOperationState.Rejected,
            Kind = ParseTaggedKind(value.GetProperty("kind"), "value.kind"),
            TransactionHash = RequireJsonHash(value.GetProperty("transaction_hash"), "value.transaction_hash"),
            Error = new ToriiKagemushaOperationError
            {
                Code = RequireJsonString(error.GetProperty("code"), "value.error.code"),
                Message = RequireJsonString(error.GetProperty("message"), "value.error.message"),
                Details = details,
            },
        };
    }

    private static ToriiKagemushaOperationKind ParseTaggedKind(JsonElement element, string context)
    {
        RequireExactFields(element, context, "kind", "value");
        if (element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw new JsonException($"{context}.value must be null.");
        }
        return RequireJsonString(element.GetProperty("kind"), $"{context}.kind") switch
        {
            "top_up" => ToriiKagemushaOperationKind.TopUp,
            "redeem" => ToriiKagemushaOperationKind.Redeem,
            _ => throw new JsonException($"{context}.kind must be top_up or redeem."),
        };
    }

    private static ToriiKagemushaOperationState ParseTaggedPendingState(JsonElement element)
    {
        RequireExactFields(element, "Kagemusha pending state", "state", "value");
        if (!string.Equals(
                RequireJsonString(element.GetProperty("state"), "state.state"),
                "pending",
                StringComparison.Ordinal)
            || element.GetProperty("value").ValueKind != JsonValueKind.Null)
        {
            throw new JsonException("Kagemusha command reference state must be pending with a null value.");
        }
        return ToriiKagemushaOperationState.Pending;
    }

    private static void RequireExactFields(JsonElement element, string context, params string[] expected)
    {
        if (element.ValueKind != JsonValueKind.Object)
        {
            throw new JsonException($"{context} must be an object.");
        }
        var expectedSet = expected.ToHashSet(StringComparer.Ordinal);
        var actual = element.EnumerateObject().Select(static property => property.Name).ToArray();
        if (actual.Length != expected.Length || actual.Any(name => !expectedSet.Contains(name)))
        {
            throw new JsonException($"{context} contains missing or unknown fields.");
        }
    }

    private static string RequireJsonOperationId(JsonElement element, string context) =>
        ToriiKagemushaTransport.RequireOperationId(RequireJsonString(element, context), context);

    private static string RequireJsonHash(JsonElement element, string context)
    {
        var value = RequireJsonString(element, context);
        RequireLowerHex32(value, context);
        return value;
    }

    private static string RequireJsonString(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.String || string.IsNullOrEmpty(element.GetString()))
        {
            throw new JsonException($"{context} must be a non-empty string.");
        }
        return element.GetString()!;
    }

    private static ulong RequireJsonUInt64(JsonElement element, string context)
    {
        if (element.ValueKind != JsonValueKind.Number || !element.TryGetUInt64(out var value))
        {
            throw new JsonException($"{context} must be an unsigned 64-bit integer.");
        }
        return value;
    }

    private static void RequireLowerHex32(string? value, string context)
    {
        if (value is null
            || value.Length != 64
            || value.Any(static character => character is not (>= '0' and <= '9') and not (>= 'a' and <= 'f')))
        {
            throw new JsonException($"{context} must be lowercase 32-byte hexadecimal.");
        }
    }
}
