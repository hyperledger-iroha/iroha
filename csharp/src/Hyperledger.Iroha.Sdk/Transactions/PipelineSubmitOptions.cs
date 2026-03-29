namespace Hyperledger.Iroha.Transactions;

public sealed record class PipelineSubmitOptions
{
    public static PipelineSubmitOptions Default { get; } = new();

    public TimeSpan PollInterval { get; init; } = TimeSpan.FromSeconds(1);

    public TimeSpan Timeout { get; init; } = TimeSpan.FromSeconds(30);

    public string Scope { get; init; } = "auto";
}
