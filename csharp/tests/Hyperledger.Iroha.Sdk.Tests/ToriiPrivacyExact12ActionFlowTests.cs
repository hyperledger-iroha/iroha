using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Privacy;
using Hyperledger.Iroha.Torii;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class ToriiPrivacyExact12ActionFlowTests
{
    private const string AccountId =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
    private const string ExactNetworkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
    private const string ForeignNetworkIdLiteral =
        "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94";
    private static readonly byte[] PrivateKeySeed = Convert.FromHexString(
        "616e64726f69642d666978747572652d7369676e696e672d6b65792d30313032");

    public static IEnumerable<object[]> Exact12NativeEntrypoints()
    {
        yield return
        [
            "NativeInspectSignedExact12Action",
            "iroha_privacy_inspect_signed_exact12_action_v1",
        ];
        yield return
        [
            "NativeAuthenticatedTransactionDetailsPrepare",
            "iroha_privacy_authenticated_transaction_details_prepare_v1",
        ];
        yield return
        [
            "NativeAuthenticatedTransactionDetailsFinalize",
            "iroha_privacy_authenticated_transaction_details_finalize_v1",
        ];
        yield return
        [
            "NativeAuthenticatedTransactionDetailsProjectResult",
            "iroha_privacy_authenticated_transaction_details_project_result_v1",
        ];
        yield return
        [
            "NativeAuthenticatedActionReceiptPrepare",
            "iroha_privacy_authenticated_action_receipt_prepare_v1",
        ];
        yield return
        [
            "NativeAuthenticatedActionReceiptFinalize",
            "iroha_privacy_authenticated_action_receipt_finalize_v1",
        ];
        yield return
        [
            "NativeAuthenticatedActionReceiptProjectResult",
            "iroha_privacy_authenticated_action_receipt_project_result_v1",
        ];
    }

    [Theory]
    [MemberData(nameof(Exact12NativeEntrypoints))]
    public void Exact12FlowPinsEveryAbi22Entrypoint(
        string methodName,
        string entrypoint)
    {
        var method = typeof(PrivacyNative).GetMethod(
            methodName,
            BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        var import = method!.GetCustomAttribute<DllImportAttribute>();
        Assert.NotNull(import);
        Assert.Equal(entrypoint, import!.EntryPoint);
        Assert.Equal(CallingConvention.Cdecl, import.CallingConvention);
    }

    [Fact]
    public async Task SubmitRequiresCanonicalCredentialsBeforeNativeOrHttp()
    {
        using var handler = new RejectDispatchHandler();
        using var client = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            new ToriiClientOptions
            {
                LocalSigningContext = new ToriiLocalSigningContext(
                    NetworkId.Parse(ExactNetworkIdLiteral)),
            });

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.SubmitSignedPrivacyActionV1Async(
                DummyRequest(),
                new PrivacyActionSubmitOptionsV1
                {
                    NetworkId = NetworkId.Parse(ExactNetworkIdLiteral),
                    Wait = false,
                },
                TestContext.Current.CancellationToken));

        Assert.Contains(nameof(ToriiClientOptions.CanonicalRequestCredentials), error.Message);
        Assert.Equal(0, handler.DispatchCount);
    }

    [Fact]
    public async Task SubmitRequiresExactPinnedNetworkBeforeNativeOrHttp()
    {
        using var handler = new RejectDispatchHandler();
        using var client = AuthenticatedClient(handler);

        var error = await Assert.ThrowsAsync<ArgumentException>(() =>
            client.SubmitSignedPrivacyActionV1Async(
                DummyRequest(),
                new PrivacyActionSubmitOptionsV1
                {
                    NetworkId = NetworkId.Parse(ForeignNetworkIdLiteral),
                    Wait = false,
                },
                TestContext.Current.CancellationToken));

        Assert.Contains(nameof(ToriiClientOptions.LocalSigningContext), error.Message);
        Assert.Equal(0, handler.DispatchCount);
    }

    [Fact]
    public async Task SubmitRejectsInsecureToriiBeforeNativeOrHttp()
    {
        using var handler = new RejectDispatchHandler();
        using var client = new ToriiClient(
            new Uri("http://torii.example"),
            new HttpClient(handler),
            AuthenticatedOptions());

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.SubmitSignedPrivacyActionV1Async(
                DummyRequest(),
                new PrivacyActionSubmitOptionsV1
                {
                    NetworkId = NetworkId.Parse(ExactNetworkIdLiteral),
                    Wait = false,
                },
                TestContext.Current.CancellationToken));

        Assert.Contains("HTTPS", error.Message);
        Assert.Equal(0, handler.DispatchCount);
    }

    [Fact]
    public async Task StatusRejectsDetachedOperationViewBeforeHttp()
    {
        using var handler = new RejectDispatchHandler();
        using var client = AuthenticatedClient(handler);
        var operation = PrivacyOperationSchemaV1.OrchardNoteActionV1;
        var detached = new PrivacyActionOperationViewV1(
            operation.ProtocolId(),
            operation,
            Enumerable.Repeat((byte)0x11, 32).ToArray(),
            Enumerable.Repeat((byte)0x22, 32).ToArray(),
            Enumerable.Repeat((byte)0x33, 32).ToArray(),
            Enumerable.Repeat((byte)0x44, 32).ToArray(),
            PrivacyActionLocalStateV1.Submitted,
            terminalChainState: null,
            committedHeight: null,
            rejectionReason: null,
            operation.LedgerEffectKind(),
            Enumerable.Repeat((byte)0x55, 32).ToArray(),
            capabilityCommittedHeight: 7);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.GetPrivacyActionStatusV1Async(
                detached,
                new PrivacyActionStatusOptionsV1
                {
                    NetworkId = NetworkId.Parse(ExactNetworkIdLiteral),
                },
                TestContext.Current.CancellationToken));

        Assert.Contains("authenticated submission", error.Message);
        Assert.Equal(0, handler.DispatchCount);
    }

    [Fact]
    public void AuthenticatedResultProjectionRequiresExactEightFieldContract()
    {
        var success = PrivacyNative.ParseAuthenticatedTransactionDetailsResult(
            Encoding.UTF8.GetBytes(ResultJson("true", "null", "17")),
            new string('1', 64),
            AccountId);
        Assert.True(success.ResultOk);
        Assert.Null(success.RejectionMessage);
        Assert.Equal(17UL, success.CommittedBlockHeight);

        var rejected = PrivacyNative.ParseAuthenticatedTransactionDetailsResult(
            Encoding.UTF8.GetBytes(ResultJson("false", "\"proof rejected\"", "18")),
            new string('1', 64),
            AccountId);
        Assert.False(rejected.ResultOk);
        Assert.Equal("proof rejected", rejected.RejectionMessage);
        Assert.Equal(18UL, rejected.CommittedBlockHeight);

        foreach (var invalid in new[]
        {
            ResultJson("true", "\"unexpected\"", "17"),
            ResultJson("false", "null", "17"),
            ResultJson("true", "null", "017"),
            ResultJson("true", "null", "17").Replace(
                "\"version\":1,",
                "\"version\":1,\"version\":1,",
                StringComparison.Ordinal),
            ResultJson("true", "null", "17").Replace(
                "\"version\":1,",
                "\"version\":1,\"extra\":0,",
                StringComparison.Ordinal),
        })
        {
            Assert.Throws<InvalidDataException>(() =>
                PrivacyNative.ParseAuthenticatedTransactionDetailsResult(
                    Encoding.UTF8.GetBytes(invalid),
                    new string('1', 64),
                    AccountId));
        }
    }

    [Fact]
    public void AuthenticatedReceiptProjectionRequiresExactBoundFifteenFieldContract()
    {
        var query = ReceiptQuery();
        var receipt = PrivacyNative.ParseAuthenticatedPrivacyActionReceiptResult(
            Encoding.UTF8.GetBytes(ReceiptJson()),
            query);
        Assert.Equal(12UL, receipt.CapabilityCommittedHeight);
        Assert.Equal(17UL, receipt.AdmittedAtHeight);
        Assert.Equal(18UL, receipt.FinalizedHeight);
        Assert.Equal(Fixed32(0x66), receipt.CapabilityManifestDigest);
        Assert.Equal(Fixed32(0x77), receipt.FinalizedBlockHash);

        foreach (var invalid in new[]
        {
            ReceiptJson().Replace(
                "\"version\":1,",
                "\"version\":1,\"version\":1,",
                StringComparison.Ordinal),
            ReceiptJson().Replace(
                "\"version\":1,",
                "\"version\":1,\"extra\":0,",
                StringComparison.Ordinal),
            ReceiptJson().Replace("\"action_index\":0", "\"action_index\":1", StringComparison.Ordinal),
            ReceiptJson().Replace(new string('6', 64), new string('0', 64), StringComparison.Ordinal),
            ReceiptJson().Replace(new string('3', 64), new string('9', 64), StringComparison.Ordinal),
            ReceiptJson().Replace("\"12\"", "\"012\"", StringComparison.Ordinal),
            ReceiptJson().Replace("\"12\"", "\"19\"", StringComparison.Ordinal),
            ReceiptJson().Replace("\"18\"", "\"16\"", StringComparison.Ordinal),
            ReceiptJson().Replace(
                "orchard-halo2-actions-v1",
                "monero-fcmp-plus-plus-v1",
                StringComparison.Ordinal),
        })
        {
            Assert.Throws<InvalidDataException>(() =>
                PrivacyNative.ParseAuthenticatedPrivacyActionReceiptResult(
                    Encoding.UTF8.GetBytes(invalid),
                    query));
        }
    }

    [Fact]
    public void CommittedAndCacheExpiryRemainNonterminal()
    {
        var submitted = SubmittedView();
        var committed = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Committed, 17),
            SuccessfulDetails(17),
            Receipt(17));
        Assert.Same(submitted, committed);
        Assert.Equal(PrivacyActionLocalStateV1.Submitted, committed.LocalState);

        var cacheExpired = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Expired, null, "cache"),
            details: null,
            receipt: null);
        Assert.Same(submitted, cacheExpired);
    }

    [Fact]
    public void DurableStateExpiryIsTerminalButLagging404EvidenceRetries()
    {
        var submitted = SubmittedView();
        var expired = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Expired, null, "state"),
            details: null,
            receipt: null);
        Assert.Equal(PrivacyActionTerminalChainStateV1.Expired, expired.TerminalChainState);

        var detailsLag = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Applied, 17),
            details: null,
            Receipt(17));
        Assert.Same(submitted, detailsLag);
        var receiptLag = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Applied, 17),
            SuccessfulDetails(17),
            receipt: null);
        Assert.Same(submitted, receiptLag);
        var rejectionDetailsLag = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Rejected, 17),
            details: null,
            receipt: null);
        Assert.Same(submitted, rejectionDetailsLag);
    }

    [Fact]
    public void AppliedUsesExecutionReceiptEvidenceWithoutEquatingItToPreflight()
    {
        var submitted = SubmittedView();
        var applied = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Applied, 17),
            SuccessfulDetails(17),
            Receipt(17, capabilityManifestByte: 0x99));

        Assert.Equal(PrivacyActionTerminalChainStateV1.Applied, applied.TerminalChainState);
        Assert.Equal(Fixed32(0x55), applied.CapabilityManifestDigest);
        Assert.Equal(Fixed32(0x99), applied.ExecutionCapabilityManifestDigest);
        Assert.Equal(12UL, applied.ExecutionCapabilityCommittedHeight);
        Assert.Equal(18UL, applied.ExecutionReceiptFinalizedHeight);
        Assert.Equal(Fixed32(0x77), applied.ExecutionReceiptFinalizedBlockHash);
    }

    [Fact]
    public void ContradictoryTerminalEvidenceFailsClosedAndTerminalEvidenceIsStable()
    {
        var submitted = SubmittedView();
        Assert.Throws<InvalidOperationException>(() =>
            ToriiClient.ResolvePrivacyActionEvidenceV1(
                submitted,
                Status(PipelineTransactionState.Rejected, 17),
                RejectedDetails(17),
                Receipt(17)));
        Assert.Throws<InvalidOperationException>(() =>
            ToriiClient.ResolvePrivacyActionEvidenceV1(
                submitted,
                Status(PipelineTransactionState.Applied, 17),
                RejectedDetails(17),
                receipt: null));
        Assert.Throws<InvalidOperationException>(() =>
            ToriiClient.ResolvePrivacyActionEvidenceV1(
                submitted,
                Status(PipelineTransactionState.Applied, 17),
                SuccessfulDetails(17),
                Receipt(18)));

        var applied = ToriiClient.ResolvePrivacyActionEvidenceV1(
            submitted,
            Status(PipelineTransactionState.Applied, 17),
            SuccessfulDetails(17),
            Receipt(17));
        Assert.Same(
            applied,
            ToriiClient.ResolvePrivacyActionEvidenceV1(
                applied,
                Status(PipelineTransactionState.Applied, 17),
                SuccessfulDetails(17),
                Receipt(17)));
        Assert.Throws<InvalidOperationException>(() =>
            ToriiClient.ResolvePrivacyActionEvidenceV1(
                applied,
                Status(PipelineTransactionState.Applied, 17),
                SuccessfulDetails(17),
                Receipt(17, finalizedBlockByte: 0x78)));
    }

    private static PrivacyActionOperationViewV1 SubmittedView()
    {
        const PrivacyOperationSchemaV1 operation = PrivacyOperationSchemaV1.OrchardNoteActionV1;
        return new PrivacyActionOperationViewV1(
            operation.ProtocolId(),
            operation,
            Fixed32(0x11),
            Fixed32(0x22),
            Fixed32(0x33),
            Fixed32(0x44),
            PrivacyActionLocalStateV1.Submitted,
            null,
            null,
            null,
            operation.LedgerEffectKind(),
            Fixed32(0x55),
            10)
            .BindAuthenticatedSubmissionV1(
                new object(),
                NetworkId.Parse(ExactNetworkIdLiteral));
    }

    private static PipelineTransactionStatus Status(
        PipelineTransactionState state,
        ulong? blockHeight,
        string resolvedFrom = "state") =>
        new()
        {
            HashHex = new string('1', 64),
            State = state,
            RawKind = state.ToString(),
            BlockHeight = blockHeight,
            Scope = "global",
            ResolvedFrom = resolvedFrom,
        };

    private static PrivacyAuthenticatedCommittedResultV1 SuccessfulDetails(ulong height) =>
        new(
            new string('1', 64),
            AccountId,
            new string('2', 64),
            new string('3', 64),
            true,
            null,
            height);

    private static PrivacyAuthenticatedCommittedResultV1 RejectedDetails(ulong height) =>
        new(
            new string('1', 64),
            AccountId,
            new string('2', 64),
            new string('3', 64),
            false,
            "proof rejected",
            height);

    private static PrivacyAuthenticatedActionExecutionReceiptV1 Receipt(
        ulong admittedAtHeight,
        byte capabilityManifestByte = 0x66,
        byte finalizedBlockByte = 0x77) =>
        new(
            NetworkIdHex(),
            PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
            PrivacyOperationSchemaV1.OrchardNoteActionV1,
            PrivacyLedgerEffectKindV1.OrchardNoteStateTransition,
            new string('1', 64),
            0,
            Fixed32(capabilityManifestByte),
            12,
            admittedAtHeight,
            Math.Max(18UL, admittedAtHeight),
            Fixed32(finalizedBlockByte));

    private static PrivacyAuthenticatedActionReceiptQueryV1 ReceiptQuery() =>
        new(
            [1],
            [2],
            NetworkIdHex(),
            PrivacyProtocolIdV1.OrchardHalo2ActionsV1,
            PrivacyOperationSchemaV1.OrchardNoteActionV1,
            PrivacyLedgerEffectKindV1.OrchardNoteStateTransition,
            new string('1', 64),
            0,
            Fixed32(0x22),
            Fixed32(0x33),
            Fixed32(0x44));

    private static string ReceiptJson() =>
        $$"""
        {"version":1,"network_id":"{{NetworkIdHex()}}","protocol_id":"orchard-halo2-actions-v1","operation_schema":"orchard_note_action_v1","ledger_effect_kind":"orchard_note_state_transition","transaction_hash":"{{new string('1', 64)}}","action_index":0,"transaction_intent_digest":"{{new string('2', 64)}}","statement_digest":"{{new string('3', 64)}}","proof_envelope_hash":"{{new string('4', 64)}}","capability_manifest_digest":"{{new string('6', 64)}}","capability_committed_height":"12","admitted_at_height":"17","finalized_height":"18","finalized_block_hash":"{{new string('7', 64)}}"}
        """;

    private static string NetworkIdHex() =>
        Convert.ToHexString(NetworkId.Parse(ExactNetworkIdLiteral).ToBytes()).ToLowerInvariant();

    private static byte[] Fixed32(byte value) => Enumerable.Repeat(value, 32).ToArray();

    private static PrivacyExact12ActionRequestV1 DummyRequest() =>
        new(PrivacyOperationSchemaV1.OrchardNoteActionV1, [1]);

    private static ToriiClient AuthenticatedClient(HttpMessageHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            AuthenticatedOptions());

    private static ToriiClientOptions AuthenticatedOptions() =>
        new()
        {
            CanonicalRequestCredentials = new CanonicalRequestCredentials(
                AccountId,
                PrivateKeySeed),
            LocalSigningContext = new ToriiLocalSigningContext(
                NetworkId.Parse(ExactNetworkIdLiteral)),
        };

    private static string ResultJson(
        string resultOk,
        string rejectionMessage,
        string height) =>
        $$"""
        {"version":1,"transaction_hash_hex":"{{new string('1', 64)}}","transaction_authority":"{{AccountId}}","block_hash_hex":"{{new string('2', 64)}}","result_hash_hex":"{{new string('3', 64)}}","result_ok":{{resultOk}},"rejection_message":{{rejectionMessage}},"committed_block_height":"{{height}}"}
        """;

    private sealed class RejectDispatchHandler : HttpMessageHandler
    {
        internal int DispatchCount { get; private set; }

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            DispatchCount++;
            return Task.FromException<HttpResponseMessage>(
                new InvalidOperationException("Unexpected HTTP dispatch."));
        }
    }
}
