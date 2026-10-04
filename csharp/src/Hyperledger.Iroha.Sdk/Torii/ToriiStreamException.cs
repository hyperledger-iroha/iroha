namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Reports a terminal error emitted after a Torii server-sent event stream has started.
/// </summary>
public sealed class ToriiStreamException : IrohaException
{
    internal ToriiStreamException(
        string code,
        string message,
        ulong? droppedMessages,
        bool replayAvailable)
        : base(code, message)
    {
        DroppedMessages = droppedMessages;
        ReplayAvailable = replayAvailable;
    }

    /// <summary>Number of broadcast messages skipped before termination, when reported.</summary>
    public ulong? DroppedMessages { get; }

    /// <summary>Whether the server can replay the missing portion of this stream.</summary>
    public bool ReplayAvailable { get; }
}
