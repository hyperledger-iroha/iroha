using System.Buffers.Binary;
using System.IO.Compression;

namespace Hyperledger.Iroha.Petal;

/// <summary>A tiny dependency-free PNG writer for inspecting renders and captures.</summary>
/// <remarks>
/// With <c>compress: false</c> the output uses stored deflate blocks and is
/// byte-identical to the Rust reference <c>png::encode</c>; with
/// <c>compress: true</c> the scanlines go through
/// <see cref="ZLibStream"/>.
/// </remarks>
public static class PetalPng
{
    private static readonly uint[] CrcTable = BuildCrcTable();

    /// <summary>Encodes interleaved 8-bit pixels as a PNG file.</summary>
    /// <param name="width">Width in pixels, at least one.</param>
    /// <param name="height">Height in pixels, at least one.</param>
    /// <param name="channels">1 (gray), 3 (RGB) or 4 (RGBA).</param>
    /// <param name="pixels">Exactly <c>width * height * channels</c> bytes, row-major.</param>
    /// <param name="compress">Deflate the scanlines instead of storing them.</param>
    /// <returns>The PNG file bytes.</returns>
    /// <exception cref="ArgumentException">The channel count or pixel buffer size is invalid.</exception>
    public static byte[] Encode(int width, int height, int channels, ReadOnlySpan<byte> pixels, bool compress = false)
    {
        if (channels is not (1 or 3 or 4))
            throw new ArgumentOutOfRangeException(nameof(channels), "PNG channels must be 1, 3 or 4.");
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(width);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(height);
        var rowBytes = (long)width * channels;
        if (pixels.Length != rowBytes * height)
            throw new ArgumentException("Pixel buffer size does not match width * height * channels.", nameof(pixels));
        var raw = new byte[checked((int)((rowBytes + 1) * height))];
        for (var row = 0; row < height; row++)
        {
            var offset = (int)(row * (rowBytes + 1));
            raw[offset] = 0; // filter: none
            pixels.Slice((int)(row * rowBytes), (int)rowBytes).CopyTo(raw.AsSpan(offset + 1));
        }

        var zlib = compress ? Deflate(raw) : Store(raw);
        using var output = new MemoryStream();
        output.Write([0x89, (byte)'P', (byte)'N', (byte)'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        Span<byte> header = stackalloc byte[13];
        BinaryPrimitives.WriteUInt32BigEndian(header, (uint)width);
        BinaryPrimitives.WriteUInt32BigEndian(header[4..], (uint)height);
        header[8] = 8; // bit depth
        header[9] = channels switch
        {
            1 => 0, // gray
            3 => 2, // RGB
            _ => 6, // RGBA
        };
        header[10] = 0;
        header[11] = 0;
        header[12] = 0;
        WriteChunk(output, "IHDR"u8, header);
        WriteChunk(output, "IDAT"u8, zlib);
        WriteChunk(output, "IEND"u8, []);
        return output.ToArray();
    }

    /// <summary>CRC-32 (IEEE 802.3) as used by PNG chunks.</summary>
    internal static uint Crc32(ReadOnlySpan<byte> bytes, uint crc = 0xFFFF_FFFF)
    {
        foreach (var value in bytes)
            crc = CrcTable[(crc ^ value) & 0xFF] ^ (crc >> 8);
        return crc;
    }

    /// <summary>Adler-32 as used by the zlib trailer.</summary>
    internal static uint Adler32(ReadOnlySpan<byte> bytes)
    {
        uint a = 1;
        uint b = 0;
        foreach (var value in bytes)
        {
            a = (a + value) % 65_521;
            b = (b + a) % 65_521;
        }

        return (b << 16) | a;
    }

    private static byte[] Store(byte[] raw)
    {
        using var zlib = new MemoryStream();
        zlib.WriteByte(0x78);
        zlib.WriteByte(0x01);
        Span<byte> blockHeader = stackalloc byte[5];
        var offset = 0;
        do
        {
            var length = Math.Min(65_535, raw.Length - offset);
            var last = offset + length >= raw.Length;
            blockHeader[0] = last ? (byte)1 : (byte)0;
            BinaryPrimitives.WriteUInt16LittleEndian(blockHeader[1..], (ushort)length);
            BinaryPrimitives.WriteUInt16LittleEndian(blockHeader[3..], (ushort)~length);
            zlib.Write(blockHeader);
            zlib.Write(raw, offset, length);
            offset += length;
        }
        while (offset < raw.Length);
        Span<byte> trailer = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(trailer, Adler32(raw));
        zlib.Write(trailer);
        return zlib.ToArray();
    }

    private static byte[] Deflate(byte[] raw)
    {
        using var output = new MemoryStream();
        using (var zlib = new ZLibStream(output, CompressionLevel.Optimal, leaveOpen: true))
            zlib.Write(raw);
        return output.ToArray();
    }

    private static void WriteChunk(Stream output, ReadOnlySpan<byte> kind, ReadOnlySpan<byte> body)
    {
        Span<byte> word = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(word, (uint)body.Length);
        output.Write(word);
        output.Write(kind);
        output.Write(body);
        var crc = ~Crc32(body, Crc32(kind));
        BinaryPrimitives.WriteUInt32BigEndian(word, crc);
        output.Write(word);
    }

    private static uint[] BuildCrcTable()
    {
        var table = new uint[256];
        for (uint i = 0; i < 256; i++)
        {
            var crc = i;
            for (var bit = 0; bit < 8; bit++)
                crc = (crc & 1) == 1 ? (crc >> 1) ^ 0xEDB8_8320u : crc >> 1;
            table[i] = crc;
        }

        return table;
    }
}
