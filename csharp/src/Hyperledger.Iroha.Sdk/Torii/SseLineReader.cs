using System.Buffers;
using System.Text;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Reads server-sent-event lines (terminated by LF, CRLF or CR) as strict UTF-8 with a hard
/// per-line byte limit, so a peer cannot make the client buffer an unbounded line.
/// </summary>
internal sealed class SseLineReader : IDisposable
{
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private readonly Stream stream;
    private readonly int maxLineBytes;
    private byte[] buffer;
    private int start;
    private int end;
    private bool endOfStream;
    private bool skipLineFeed;

    internal SseLineReader(Stream stream, int maxLineBytes)
    {
        ArgumentNullException.ThrowIfNull(stream);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(maxLineBytes);
        this.stream = stream;
        this.maxLineBytes = maxLineBytes;
        buffer = ArrayPool<byte>.Shared.Rent(Math.Min(16 * 1024, maxLineBytes + 2));
    }

    /// <summary>Returns the next line without its terminator, or <see langword="null"/> at the end.</summary>
    /// <exception cref="InvalidDataException">A line exceeds the byte limit.</exception>
    /// <exception cref="DecoderFallbackException">A line is not valid UTF-8.</exception>
    internal async ValueTask<string?> ReadLineAsync(CancellationToken cancellationToken)
    {
        while (true)
        {
            if (TryTakeLine(out var line))
            {
                return line;
            }

            if (endOfStream)
            {
                return TakeRemainder();
            }

            await FillAsync(cancellationToken).ConfigureAwait(false);
        }
    }

    private bool TryTakeLine(out string? line)
    {
        if (skipLineFeed && start < end)
        {
            if (buffer[start] == (byte)'\n')
            {
                start++;
            }

            skipLineFeed = false;
        }

        var available = buffer.AsSpan(start, end - start);
        var terminator = available.IndexOfAny((byte)'\n', (byte)'\r');
        if (terminator < 0)
        {
            if (available.Length > maxLineBytes)
            {
                throw LineTooLong();
            }

            line = null;
            return false;
        }

        if (terminator > maxLineBytes)
        {
            throw LineTooLong();
        }

        line = StrictUtf8.GetString(available[..terminator]);
        var consumed = terminator + 1;
        if (available[terminator] == (byte)'\r')
        {
            if (terminator + 1 < available.Length)
            {
                if (available[terminator + 1] == (byte)'\n')
                {
                    consumed++;
                }
            }
            else
            {
                skipLineFeed = true;
            }
        }

        start += consumed;
        return true;
    }

    private string? TakeRemainder()
    {
        if (start == end)
        {
            return null;
        }

        var last = StrictUtf8.GetString(buffer, start, end - start);
        start = end;
        return last;
    }

    public void Dispose()
    {
        var rented = buffer;
        buffer = Array.Empty<byte>();
        if (rented.Length > 0)
        {
            ArrayPool<byte>.Shared.Return(rented, clearArray: true);
        }
    }

    private async ValueTask FillAsync(CancellationToken cancellationToken)
    {
        if (start > 0)
        {
            Buffer.BlockCopy(buffer, start, buffer, 0, end - start);
            end -= start;
            start = 0;
        }

        if (end == buffer.Length)
        {
            var grown = ArrayPool<byte>.Shared.Rent(Math.Min(buffer.Length * 2, maxLineBytes + 2));
            Buffer.BlockCopy(buffer, 0, grown, 0, end);
            ArrayPool<byte>.Shared.Return(buffer, clearArray: true);
            buffer = grown;
        }

        var read = await stream.ReadAsync(buffer.AsMemory(end), cancellationToken).ConfigureAwait(false);
        if (read == 0)
        {
            endOfStream = true;
        }
        else
        {
            end += read;
        }
    }

    private InvalidDataException LineTooLong() =>
        new($"SSE line exceeds the {maxLineBytes}-byte limit.");
}
