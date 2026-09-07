using Hyperledger.Iroha.Kaigi;

namespace Hyperledger.Iroha.Transactions;

/// <summary>Create a permanent call with complete private host authorization when requested.</summary>
public sealed record CreateKaigiInstruction : TransactionInstruction
{
    public CreateKaigiInstruction(NewKaigi call, KaigiAuthorizationArtifactsV1? authorization = null)
    {
        Call = call ?? throw new ArgumentNullException(nameof(call));
        if ((call.PrivacyMode == KaigiPrivacyMode.ZkRosterV1) != (authorization is not null))
            throw new ArgumentException("Private calls require complete authorization; transparent calls carry none.", nameof(authorization));
        Authorization = authorization;
    }
    public NewKaigi Call { get; }
    public KaigiAuthorizationArtifactsV1? Authorization { get; }
    internal override string WireId => "iroha.instruction.v1::kaigi::CreateKaigi";
    internal override string TypeName => "iroha_data_model::isi::kaigi::CreateKaigi";
    internal override byte[] EncodePayload(TransactionEncodingContext context) => KaigiWireV1.Struct([Call.Encode(context), .. KaigiWireV1.Authorization(Authorization)]);
}

/// <summary>Join a call; private authorization binds the original account and ledger-owned sequence.</summary>
public sealed record JoinKaigiInstruction : TransactionInstruction
{
    public JoinKaigiInstruction(KaigiId callId, string participant, KaigiAuthorizationArtifactsV1? authorization = null)
    {
        CallId = callId ?? throw new ArgumentNullException(nameof(callId));
        Participant = TransactionEncodingContext.CanonicalizeAccountId(participant, nameof(participant));
        Authorization = authorization;
    }
    public KaigiId CallId { get; }
    public string Participant { get; }
    public KaigiAuthorizationArtifactsV1? Authorization { get; }
    internal override string WireId => "iroha.instruction.v1::kaigi::JoinKaigi";
    internal override string TypeName => "iroha_data_model::isi::kaigi::JoinKaigi";
    internal override byte[] EncodePayload(TransactionEncodingContext context) => KaigiWireV1.Struct([CallId.Encode(context), context.EncodeAccountId(Participant), .. KaigiWireV1.Authorization(Authorization)]);
}

/// <summary>Leave a call using the stored private commitment, exact leave nullifier and current roster root.</summary>
public sealed record LeaveKaigiInstruction : TransactionInstruction
{
    public LeaveKaigiInstruction(KaigiId callId, string participant, KaigiAuthorizationArtifactsV1? authorization = null)
    {
        CallId = callId ?? throw new ArgumentNullException(nameof(callId));
        Participant = TransactionEncodingContext.CanonicalizeAccountId(participant, nameof(participant));
        Authorization = authorization;
    }
    public KaigiId CallId { get; }
    public string Participant { get; }
    public KaigiAuthorizationArtifactsV1? Authorization { get; }
    internal override string WireId => "iroha.instruction.v1::kaigi::LeaveKaigi";
    internal override string TypeName => "iroha_data_model::isi::kaigi::LeaveKaigi";
    internal override byte[] EncodePayload(TransactionEncodingContext context) => KaigiWireV1.Struct([CallId.Encode(context), context.EncodeAccountId(Participant), .. KaigiWireV1.Authorization(Authorization)]);
}

/// <summary>End a call using its original host authorization.</summary>
public sealed record EndKaigiInstruction : TransactionInstruction
{
    public EndKaigiInstruction(KaigiId callId, ulong? endedAtMs = null, KaigiAuthorizationArtifactsV1? authorization = null)
    {
        CallId = callId ?? throw new ArgumentNullException(nameof(callId)); EndedAtMs = endedAtMs; Authorization = authorization;
    }
    public KaigiId CallId { get; }
    public ulong? EndedAtMs { get; }
    public KaigiAuthorizationArtifactsV1? Authorization { get; }
    internal override string WireId => "iroha.instruction.v1::kaigi::EndKaigi";
    internal override string TypeName => "iroha_data_model::isi::kaigi::EndKaigi";
    internal override byte[] EncodePayload(TransactionEncodingContext context) => KaigiWireV1.Struct([CallId.Encode(context), context.EncodeOption(EndedAtMs, context.EncodeUInt64), .. KaigiWireV1.Authorization(Authorization)]);
}

/// <summary>Record a nonempty usage segment. Private proof context and segment index come from trusted retained state.</summary>
public sealed record RecordKaigiUsageInstruction : TransactionInstruction
{
    public RecordKaigiUsageInstruction(KaigiId callId, ulong durationMs, ulong billedGas, KaigiUsageArtifactsV1? authorization = null)
    {
        CallId = callId ?? throw new ArgumentNullException(nameof(callId));
        if (durationMs == 0) throw new ArgumentOutOfRangeException(nameof(durationMs));
        DurationMs = durationMs; BilledGas = billedGas; Authorization = authorization;
    }
    public KaigiId CallId { get; }
    public ulong DurationMs { get; }
    public ulong BilledGas { get; }
    public KaigiUsageArtifactsV1? Authorization { get; }
    internal override string WireId => "iroha.instruction.v1::kaigi::RecordKaigiUsage";
    internal override string TypeName => "iroha_data_model::isi::kaigi::RecordKaigiUsage";
    internal override byte[] EncodePayload(TransactionEncodingContext context) => KaigiWireV1.Struct(
        CallId.Encode(context), context.EncodeUInt64(DurationMs), context.EncodeUInt64(BilledGas),
        KaigiWireV1.Option(Authorization?.UsageCommitment.ToLittleEndianBytes()),
        KaigiWireV1.Option(Authorization is null ? null : KaigiWireV1.BytesVector(Authorization.ProofBytes)));
}
