namespace Hyperledger.Iroha.Transactions;

public sealed record class PipelineTransactionStatus
{
    public string HashHex { get; init; } = string.Empty;

    public PipelineTransactionState State { get; init; }

    public string RawKind { get; init; } = string.Empty;

    public ulong? BlockHeight { get; init; }

    public string Scope { get; init; } = string.Empty;

    public string ResolvedFrom { get; init; } = string.Empty;

    public bool IsSuccess =>
        Scope == "global"
        && ResolvedFrom == "state"
        && State == PipelineTransactionState.Applied
        && BlockHeight is > 0;

    public bool IsFailure =>
        Scope == "global"
        && ResolvedFrom == "state"
        && State is PipelineTransactionState.Rejected or PipelineTransactionState.Expired;

    public bool IsTerminal => IsSuccess || IsFailure;
}
