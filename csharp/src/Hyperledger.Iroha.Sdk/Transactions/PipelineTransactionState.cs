namespace Hyperledger.Iroha.Transactions;

public enum PipelineTransactionState
{
    Unknown = 0,
    Queued,
    Approved,
    Committed,
    Applied,
    Rejected,
    Expired,
}
