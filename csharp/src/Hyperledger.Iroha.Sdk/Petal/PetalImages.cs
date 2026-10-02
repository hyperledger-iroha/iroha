namespace Hyperledger.Iroha.Petal;

/// <summary>
/// A single-channel 8-bit image, typically a camera luma (Y) plane.
/// </summary>
/// <remarks>
/// Pixel <c>(i, j)</c> covers <c>[i, i+1) × [j, j+1)</c> and its centre is at
/// <c>(i + 0.5, j + 0.5)</c>. Every homography in this namespace maps into
/// these pixel-edge coordinates.
/// </remarks>
public sealed class PetalLuma
{
    /// <summary>Creates a black image.</summary>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <exception cref="ArgumentOutOfRangeException">A dimension is negative or the image is too large.</exception>
    public PetalLuma(int width, int height)
    {
        Data = new byte[CheckedArea(width, height)];
        Width = width;
        Height = height;
    }

    /// <summary>Wraps an existing row-major buffer without copying.</summary>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="data">Exactly <c>width * height</c> samples.</param>
    /// <exception cref="ArgumentException">The buffer size does not match the dimensions.</exception>
    public PetalLuma(int width, int height, byte[] data)
    {
        ArgumentNullException.ThrowIfNull(data);
        if (data.Length != CheckedArea(width, height))
            throw new ArgumentException("Luma buffer size does not match width * height.", nameof(data));
        Width = width;
        Height = height;
        Data = data;
    }

    /// <summary>Width in pixels.</summary>
    public int Width { get; }

    /// <summary>Height in pixels.</summary>
    public int Height { get; }

    /// <summary>Row-major samples, <c>Width * Height</c> bytes.</summary>
    public byte[] Data { get; }

    /// <summary>
    /// Copies a strided luma plane, for example the Y plane of an Android
    /// <c>YUV_420_888</c>/NV21 image or of an iOS bi-planar <c>CVPixelBuffer</c>.
    /// </summary>
    /// <param name="plane">Plane bytes starting at pixel <c>(0, 0)</c>.</param>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="rowStride">Bytes between the starts of consecutive rows.</param>
    /// <param name="pixelStride">Bytes between horizontally adjacent samples (1 for planar Y).</param>
    /// <returns>A new image.</returns>
    /// <exception cref="ArgumentException">The geometry is inconsistent or the plane is too short.</exception>
    public static PetalLuma FromYPlane(ReadOnlySpan<byte> plane, int width, int height, int rowStride, int pixelStride = 1)
    {
        var luma = new PetalLuma(width, height);
        luma.LoadYPlane(plane, rowStride, pixelStride);
        return luma;
    }

    /// <summary>
    /// Refills this image from a strided luma plane of the same size, so a
    /// camera loop can reuse one buffer per resolution.
    /// </summary>
    /// <param name="plane">Plane bytes starting at pixel <c>(0, 0)</c>.</param>
    /// <param name="rowStride">Bytes between the starts of consecutive rows.</param>
    /// <param name="pixelStride">Bytes between horizontally adjacent samples (1 for planar Y).</param>
    /// <exception cref="ArgumentException">The geometry is inconsistent or the plane is too short.</exception>
    public void LoadYPlane(ReadOnlySpan<byte> plane, int rowStride, int pixelStride = 1)
    {
        ValidatePlane(plane.Length, Width, Height, rowStride, pixelStride, 1, nameof(plane));
        if (pixelStride == 1)
        {
            for (var row = 0; row < Height; row++)
                plane.Slice(row * rowStride, Width).CopyTo(Data.AsSpan(row * Width, Width));
            return;
        }

        for (var row = 0; row < Height; row++)
        {
            var source = row * rowStride;
            var target = row * Width;
            for (var column = 0; column < Width; column++)
                Data[target + column] = plane[source + column * pixelStride];
        }
    }

    /// <summary>Rec. 601 luma of interleaved 8-bit RGB pixels.</summary>
    /// <param name="pixels">Pixel bytes.</param>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="rowStride">Bytes per row; <c>0</c> means tightly packed.</param>
    /// <returns>A new image.</returns>
    public static PetalLuma FromRgb(ReadOnlySpan<byte> pixels, int width, int height, int rowStride = 0) =>
        FromInterleaved(pixels, width, height, rowStride, 3, 0, 1, 2);

    /// <summary>Rec. 601 luma of interleaved 8-bit RGBA pixels (alpha ignored).</summary>
    /// <param name="pixels">Pixel bytes.</param>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="rowStride">Bytes per row; <c>0</c> means tightly packed.</param>
    /// <returns>A new image.</returns>
    public static PetalLuma FromRgba(ReadOnlySpan<byte> pixels, int width, int height, int rowStride = 0) =>
        FromInterleaved(pixels, width, height, rowStride, 4, 0, 1, 2);

    /// <summary>Rec. 601 luma of interleaved 8-bit BGRA pixels (alpha ignored).</summary>
    /// <param name="pixels">Pixel bytes.</param>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="rowStride">Bytes per row; <c>0</c> means tightly packed.</param>
    /// <returns>A new image.</returns>
    public static PetalLuma FromBgra(ReadOnlySpan<byte> pixels, int width, int height, int rowStride = 0) =>
        FromInterleaved(pixels, width, height, rowStride, 4, 2, 1, 0);

    /// <summary>Reads pixel <c>(x, y)</c>.</summary>
    /// <param name="x">Column.</param>
    /// <param name="y">Row.</param>
    /// <returns>The sample.</returns>
    public byte At(int x, int y)
    {
        if ((uint)x >= (uint)Width || (uint)y >= (uint)Height)
            throw new ArgumentOutOfRangeException(nameof(x), "Pixel outside the image.");
        return Data[y * Width + x];
    }

    /// <summary>
    /// Bilinear sample at continuous pixel-edge coordinates, clamped to the
    /// image border. Returns <c>0</c> for an empty image and NaN for NaN input.
    /// </summary>
    /// <param name="x">Pixel-edge x.</param>
    /// <param name="y">Pixel-edge y.</param>
    /// <returns>The interpolated value.</returns>
    public double Sample(double x, double y)
    {
        if (Width == 0 || Height == 0)
            return 0.0;
        double maxX = Width - 1;
        double maxY = Height - 1;
        var fx = x - 0.5;
        if (fx < 0.0)
            fx = 0.0;
        if (fx > maxX)
            fx = maxX;
        var fy = y - 0.5;
        if (fy < 0.0)
            fy = 0.0;
        if (fy > maxY)
            fy = maxY;
        // Saturating like Rust `as usize`: NaN becomes 0 (and stays NaN below).
        var x0 = fx > 0.0 ? (int)fx : 0;
        var y0 = fy > 0.0 ? (int)fy : 0;
        var x1 = Math.Min(x0 + 1, Width - 1);
        var y1 = Math.Min(y0 + 1, Height - 1);
        var tx = fx - x0;
        var ty = fy - y0;
        var data = Data;
        var row0 = y0 * Width;
        var row1 = y1 * Width;
        var top = data[row0 + x0] * (1.0 - tx) + data[row0 + x1] * tx;
        var bottom = data[row1 + x0] * (1.0 - tx) + data[row1 + x1] * tx;
        return top * (1.0 - ty) + bottom * ty;
    }

    /// <summary>Encodes the image as a grayscale PNG for inspection.</summary>
    /// <param name="compress">Deflate with <see cref="System.IO.Compression.ZLibStream"/> instead of stored blocks.</param>
    /// <returns>PNG file bytes.</returns>
    public byte[] ToPng(bool compress = true) => PetalPng.Encode(Width, Height, 1, Data, compress);

    internal static int CheckedArea(int width, int height)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(width);
        ArgumentOutOfRangeException.ThrowIfNegative(height);
        var area = (long)width * height;
        if (area > Array.MaxLength)
            throw new ArgumentOutOfRangeException(nameof(width), "Image is too large.");
        return (int)area;
    }

    private static PetalLuma FromInterleaved(
        ReadOnlySpan<byte> pixels,
        int width,
        int height,
        int rowStride,
        int bytesPerPixel,
        int red,
        int green,
        int blue)
    {
        var luma = new PetalLuma(width, height);
        if (rowStride == 0)
            rowStride = width * bytesPerPixel;
        ValidatePlane(pixels.Length, width, height, rowStride, bytesPerPixel, bytesPerPixel, nameof(pixels));
        for (var row = 0; row < height; row++)
        {
            var source = pixels.Slice(row * rowStride, width * bytesPerPixel);
            var target = luma.Data.AsSpan(row * width, width);
            for (var column = 0; column < width; column++)
            {
                var at = column * bytesPerPixel;
                target[column] = Rec601(source[at + red], source[at + green], source[at + blue]);
            }
        }

        return luma;
    }

    /// <summary>Integer Rec. 601 luma, identical to the reference <c>Rgb::to_luma</c>.</summary>
    internal static byte Rec601(byte r, byte g, byte b) =>
        (byte)((299u * r + 587u * g + 114u * b + 500u) / 1000u);

    private static void ValidatePlane(
        int length,
        int width,
        int height,
        int rowStride,
        int pixelStride,
        int pixelBytes,
        string paramName)
    {
        if (pixelStride < 1)
            throw new ArgumentOutOfRangeException(nameof(pixelStride));
        if (width == 0 || height == 0)
            return;
        var rowSpan = (long)(width - 1) * pixelStride + pixelBytes;
        if (rowStride < rowSpan)
            throw new ArgumentOutOfRangeException(nameof(rowStride), "Row stride is shorter than one row.");
        var required = (long)rowStride * (height - 1) + rowSpan;
        if (length < required)
            throw new ArgumentException("Plane is too short for the given geometry.", paramName);
    }
}

/// <summary>An interleaved 8-bit RGB image, as produced by <see cref="PetalRenderer"/>.</summary>
public sealed class PetalRgbImage
{
    /// <summary>Wraps an existing RGB buffer without copying.</summary>
    /// <param name="width">Width in pixels.</param>
    /// <param name="height">Height in pixels.</param>
    /// <param name="data">Exactly <c>width * height * 3</c> bytes.</param>
    /// <exception cref="ArgumentException">The buffer size does not match the dimensions.</exception>
    public PetalRgbImage(int width, int height, byte[] data)
    {
        ArgumentNullException.ThrowIfNull(data);
        if (data.Length != (long)PetalLuma.CheckedArea(width, height) * 3)
            throw new ArgumentException("RGB buffer size does not match width * height * 3.", nameof(data));
        Width = width;
        Height = height;
        Data = data;
    }

    /// <summary>Width in pixels.</summary>
    public int Width { get; }

    /// <summary>Height in pixels.</summary>
    public int Height { get; }

    /// <summary>Row-major <c>r, g, b</c> triples.</summary>
    public byte[] Data { get; }

    /// <summary>Rec. 601 luma of the image (integer weights 299/587/114).</summary>
    /// <returns>A new luma image.</returns>
    public PetalLuma ToLuma() => PetalLuma.FromRgb(Data, Width, Height);

    /// <summary>Converts to interleaved RGBA, for bitmaps such as Android <c>ARGB_8888</c> buffers.</summary>
    /// <param name="alpha">Alpha of every pixel.</param>
    /// <returns><c>Width * Height * 4</c> bytes.</returns>
    public byte[] ToRgba(byte alpha = 255) => Expand(0, 2, alpha);

    /// <summary>Converts to interleaved BGRA, for bitmaps such as Windows/Apple 32-bit BGRA buffers.</summary>
    /// <param name="alpha">Alpha of every pixel.</param>
    /// <returns><c>Width * Height * 4</c> bytes.</returns>
    public byte[] ToBgra(byte alpha = 255) => Expand(2, 0, alpha);

    /// <summary>Encodes the image as an RGB PNG for inspection.</summary>
    /// <param name="compress">Deflate with <see cref="System.IO.Compression.ZLibStream"/> instead of stored blocks.</param>
    /// <returns>PNG file bytes.</returns>
    public byte[] ToPng(bool compress = true) => PetalPng.Encode(Width, Height, 3, Data, compress);

    private byte[] Expand(int first, int third, byte alpha)
    {
        var count = Width * Height;
        var output = new byte[count * 4];
        for (var i = 0; i < count; i++)
        {
            output[4 * i + first] = Data[3 * i];
            output[4 * i + 1] = Data[3 * i + 1];
            output[4 * i + third] = Data[3 * i + 2];
            output[4 * i + 3] = alpha;
        }

        return output;
    }
}
