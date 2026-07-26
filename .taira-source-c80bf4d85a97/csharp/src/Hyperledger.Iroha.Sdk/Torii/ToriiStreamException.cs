namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Reports a terminal error emitted after a Torii server-sent event stream has started.
/// </summary>
public sealed class ToriiStreamException : Exception
{
    internal ToriiStreamException(
        string code,
        string message,
        ulong? droppedMessages,
        bool replayAvailable)
        : base(message)
    {
        Code = code;
        DroppedMessages = droppedMessages;
        ReplayAvailable = replayAvailable;
    }

    /// <summary>Stable machine-readable stream error code.</summary>
    public string Code { get; }

    /// <summary>Number of broadcast messages skipped before termination, when reported.</summary>
    public ulong? DroppedMessages { get; }

    /// <summary>Whether the server can replay the missing portion of this stream.</summary>
    public bool ReplayAvailable { get; }
}
