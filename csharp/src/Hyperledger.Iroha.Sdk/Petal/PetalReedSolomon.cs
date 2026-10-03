namespace Hyperledger.Iroha.Petal;

/// <summary>Why a Reed–Solomon decode failed.</summary>
public enum PetalReedSolomonError
{
    /// <summary>No error: the word was decoded.</summary>
    None,

    /// <summary>The codeword length, parity count or erasure list is invalid.</summary>
    InvalidShape,

    /// <summary>More errata than the code can correct, or the word is not decodable.</summary>
    Uncorrectable,
}

/// <summary>
/// Reed–Solomon over GF(2^8) with errors-and-erasures decoding.
/// </summary>
/// <remarks>
/// The field uses the primitive polynomial <c>x^8 + x^4 + x^3 + x^2 + 1</c>
/// (<c>0x11D</c>, the QR Code field) with <c>α = 2</c>. A codeword is
/// <c>data || parity</c>, systematic, and the generator is
/// <c>g(x) = (x - α^0)(x - α^1)…(x - α^(nsym-1))</c>, so the first consecutive
/// root is <c>α^0</c>. The first byte of a codeword is the highest-degree
/// coefficient. Every Petal lane is one codeword (<c>n &lt;= 255</c>);
/// low-confidence cells are passed as erasures, which cost one parity symbol
/// each instead of two. Decoding is Berlekamp–Massey on Forney syndromes,
/// Chien search and Forney's formula, ported step by step from the reference.
/// </remarks>
public sealed class PetalReedSolomon
{
    /// <summary>Longest codeword the field supports.</summary>
    public const int MaxCodewordLength = 255;

    private const int Primitive = 0x11D;
    private static readonly byte[] ExpTable = new byte[512];
    private static readonly byte[] LogTable = new byte[256];

    private readonly byte[] generator;

    static PetalReedSolomon()
    {
        var x = 1;
        for (var i = 0; i < 255; i++)
        {
            ExpTable[i] = (byte)x;
            LogTable[x] = (byte)i;
            x <<= 1;
            if ((x & 0x100) != 0)
                x ^= Primitive;
        }

        for (var j = 255; j < 512; j++)
            ExpTable[j] = ExpTable[j - 255];
    }

    /// <summary>Creates a code with <paramref name="parityLength"/> parity bytes.</summary>
    /// <param name="parityLength">Parity bytes, 1 to 254.</param>
    /// <exception cref="ArgumentOutOfRangeException">The parity count is outside 1–254.</exception>
    public PetalReedSolomon(int parityLength)
    {
        if (parityLength is < 1 or > 254)
            throw new ArgumentOutOfRangeException(nameof(parityLength), "Parity byte count must be within 1..=254.");
        ParityLength = parityLength;
        // Highest-degree-first monic generator.
        var current = new byte[] { 1 };
        for (var i = 0; i < parityLength; i++)
        {
            var root = GfExp(i);
            var next = new byte[current.Length + 1];
            for (var k = 0; k < current.Length; k++)
            {
                next[k] ^= current[k];
                next[k + 1] ^= GfMul(current[k], root);
            }

            current = next;
        }

        generator = current;
    }

    /// <summary>Number of parity bytes.</summary>
    public int ParityLength { get; }

    /// <summary>Multiplies two field elements.</summary>
    /// <param name="a">First factor.</param>
    /// <param name="b">Second factor.</param>
    /// <returns>The product in GF(256).</returns>
    public static byte GfMul(byte a, byte b) =>
        a == 0 || b == 0 ? (byte)0 : ExpTable[LogTable[a] + LogTable[b]];

    /// <summary>Returns <c>α^exponent</c>.</summary>
    /// <param name="exponent">Non-negative exponent; reduced modulo 255.</param>
    /// <returns>The field element.</returns>
    /// <exception cref="ArgumentOutOfRangeException">The exponent is negative.</exception>
    public static byte GfExp(int exponent)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(exponent);
        return ExpTable[exponent % 255];
    }

    private static byte GfDiv(byte a, byte b) =>
        a == 0 ? (byte)0 : ExpTable[LogTable[a] + 255 - LogTable[b]];

    private static byte GfInv(byte a) => ExpTable[255 - LogTable[a]];

    /// <summary>Encodes <paramref name="data"/>, returning <c>data || parity</c>.</summary>
    /// <param name="data">Message bytes.</param>
    /// <returns>The systematic codeword.</returns>
    /// <exception cref="ArgumentException">The codeword would exceed 255 bytes.</exception>
    public byte[] Encode(ReadOnlySpan<byte> data)
    {
        if (data.Length + ParityLength > MaxCodewordLength)
            throw new ArgumentException("Reed-Solomon codeword longer than 255 bytes.", nameof(data));
        var word = new byte[data.Length + ParityLength];
        data.CopyTo(word);
        EncodeParity(data, word.AsSpan(data.Length));
        return word;
    }

    /// <summary>Writes the parity of <paramref name="data"/> into <paramref name="parity"/>.</summary>
    internal void EncodeParity(ReadOnlySpan<byte> data, Span<byte> parity)
    {
        var nsym = ParityLength;
        var remainder = parity[..nsym];
        remainder.Clear();
        foreach (var value in data)
        {
            var feedback = (byte)(value ^ remainder[0]);
            for (var j = 0; j < nsym; j++)
            {
                var next = j + 1 < nsym ? remainder[j + 1] : (byte)0;
                remainder[j] = (byte)(next ^ GfMul(feedback, generator[j + 1]));
            }
        }
    }

    /// <summary>Whether every syndrome of <paramref name="word"/> is zero.</summary>
    /// <param name="word">Candidate codeword.</param>
    /// <returns><see langword="true"/> for a valid codeword of this code.</returns>
    public bool IsCodeword(ReadOnlySpan<byte> word)
    {
        Span<byte> syndromes = stackalloc byte[ParityLength];
        Syndromes(word, syndromes);
        return !syndromes.ContainsAnyExcept((byte)0);
    }

    /// <summary>
    /// Corrects <paramref name="word"/> in place, treating <paramref name="erasures"/>
    /// as known-bad positions.
    /// </summary>
    /// <param name="word">Received codeword; rewritten only on success.</param>
    /// <param name="erasures">Distinct byte positions the caller distrusts.</param>
    /// <param name="corrected">Number of corrected positions on success.</param>
    /// <returns><see langword="true"/> when the word decoded.</returns>
    public bool TryDecode(Span<byte> word, ReadOnlySpan<int> erasures, out int corrected) =>
        TryDecode(word, erasures, out corrected, out _);

    /// <summary>
    /// Corrects <paramref name="word"/> in place, treating <paramref name="erasures"/>
    /// as known-bad positions.
    /// </summary>
    /// <remarks>
    /// Succeeds when <c>2 * errors + erasures &lt;= ParityLength</c>. The corrected
    /// word is re-checked against zero syndromes before it is written back, so a
    /// success always yields a valid codeword.
    /// </remarks>
    /// <param name="word">Received codeword; rewritten only on success.</param>
    /// <param name="erasures">Distinct byte positions the caller distrusts.</param>
    /// <param name="corrected">Number of corrected positions on success.</param>
    /// <param name="error">Why decoding failed, or <see cref="PetalReedSolomonError.None"/>.</param>
    /// <returns><see langword="true"/> when the word decoded.</returns>
    public bool TryDecode(
        Span<byte> word,
        ReadOnlySpan<int> erasures,
        out int corrected,
        out PetalReedSolomonError error)
    {
        corrected = 0;
        var nsym = ParityLength;
        var n = word.Length;
        if (n <= nsym || n > MaxCodewordLength || erasures.Length > nsym)
        {
            error = PetalReedSolomonError.InvalidShape;
            return false;
        }

        Span<bool> seen = stackalloc bool[MaxCodewordLength];
        foreach (var position in erasures)
        {
            if ((uint)position >= (uint)n || seen[position])
            {
                error = PetalReedSolomonError.InvalidShape;
                return false;
            }

            seen[position] = true;
        }

        Span<byte> syndromes = stackalloc byte[nsym];
        Syndromes(word, syndromes);
        if (!syndromes.ContainsAnyExcept((byte)0))
        {
            error = PetalReedSolomonError.None;
            return true;
        }

        var f = erasures.Length;
        // Erasure locator Γ(x) = Π (1 + X_e x), lowest degree first.
        Span<byte> gamma = stackalloc byte[f + 1];
        Span<byte> product = stackalloc byte[f + 1];
        gamma[0] = 1;
        var gammaLength = 1;
        Span<byte> factor = stackalloc byte[2];
        foreach (var position in erasures)
        {
            factor[0] = 1;
            factor[1] = GfExp(n - 1 - position);
            var length = PolyMul(gamma[..gammaLength], factor, product);
            product[..length].CopyTo(gamma);
            gammaLength = length;
        }

        // Forney syndromes: the coefficients of S(x)Γ(x) from index f upward are
        // the syndromes of the error-only word.
        Span<byte> forney = stackalloc byte[nsym + gammaLength - 1];
        PolyMul(syndromes, gamma[..gammaLength], forney);
        var errorsOnly = forney[f..nsym];
        Span<byte> lambda = stackalloc byte[errorsOnly.Length + 1];
        var lambdaLength = BerlekampMassey(errorsOnly, lambda);
        var errorCount = lambdaLength - 1;
        if (2 * errorCount + f > nsym)
        {
            error = PetalReedSolomonError.Uncorrectable;
            return false;
        }

        Span<byte> psi = stackalloc byte[lambdaLength + gammaLength - 1];
        PolyMul(lambda[..lambdaLength], gamma[..gammaLength], psi);
        var degree = psi.Length - 1;
        // Chien search over all positions.
        Span<int> positions = stackalloc int[n];
        var found = 0;
        for (var i = 0; i < n; i++)
        {
            var xInverse = GfExp(255 - ((n - 1 - i) % 255));
            if (PolyEval(psi, xInverse) == 0)
                positions[found++] = i;
        }

        if (found != degree)
        {
            error = PetalReedSolomonError.Uncorrectable;
            return false;
        }

        // Ω(x) = S(x)Ψ(x) mod x^nsym.
        Span<byte> omegaFull = stackalloc byte[nsym + psi.Length - 1];
        PolyMul(syndromes, psi, omegaFull);
        var omega = omegaFull[..nsym];
        // Formal derivative of Ψ in characteristic 2 keeps odd-degree terms.
        Span<byte> derivative = stackalloc byte[psi.Length - 1];
        for (var k = 1; k < psi.Length; k++)
            derivative[k - 1] = k % 2 == 1 ? psi[k] : (byte)0;
        Span<byte> fixedWord = stackalloc byte[n];
        word.CopyTo(fixedWord);
        foreach (var i in positions[..found])
        {
            var x = GfExp(n - 1 - i);
            var xInverse = GfInv(x);
            var numerator = PolyEval(omega, xInverse);
            var denominator = PolyEval(derivative, xInverse);
            if (denominator == 0)
            {
                error = PetalReedSolomonError.Uncorrectable;
                return false;
            }

            fixedWord[i] ^= GfMul(x, GfDiv(numerator, denominator));
        }

        Syndromes(fixedWord, syndromes);
        if (syndromes.ContainsAnyExcept((byte)0))
        {
            error = PetalReedSolomonError.Uncorrectable;
            return false;
        }

        fixedWord.CopyTo(word);
        corrected = found;
        error = PetalReedSolomonError.None;
        return true;
    }

    private void Syndromes(ReadOnlySpan<byte> word, Span<byte> syndromes)
    {
        for (var j = 0; j < ParityLength; j++)
        {
            var root = GfExp(j);
            byte acc = 0;
            foreach (var value in word)
                acc = (byte)(GfMul(acc, root) ^ value);
            syndromes[j] = acc;
        }
    }

    /// <summary>Multiplies two lowest-degree-first polynomials into <paramref name="output"/>.</summary>
    private static int PolyMul(ReadOnlySpan<byte> a, ReadOnlySpan<byte> b, Span<byte> output)
    {
        var length = a.Length + b.Length - 1;
        output[..length].Clear();
        for (var i = 0; i < a.Length; i++)
        {
            var x = a[i];
            if (x == 0)
                continue;
            for (var j = 0; j < b.Length; j++)
                output[i + j] ^= GfMul(x, b[j]);
        }

        return length;
    }

    /// <summary>Evaluates a lowest-degree-first polynomial at <paramref name="x"/> (Horner).</summary>
    private static byte PolyEval(ReadOnlySpan<byte> poly, byte x)
    {
        byte acc = 0;
        for (var i = poly.Length - 1; i >= 0; i--)
            acc = (byte)(GfMul(acc, x) ^ poly[i]);
        return acc;
    }

    /// <summary>Berlekamp–Massey over GF(256); writes the lowest-degree-first locator.</summary>
    /// <returns>The locator length (<c>L + 1</c>).</returns>
    private static int BerlekampMassey(ReadOnlySpan<byte> syndromes, Span<byte> locator)
    {
        var n = syndromes.Length;
        Span<byte> c = locator[..(n + 1)];
        Span<byte> b = stackalloc byte[n + 1];
        Span<byte> snapshot = stackalloc byte[n + 1];
        c.Clear();
        c[0] = 1;
        b[0] = 1;
        var l = 0;
        var m = 1;
        byte previousDiscrepancy = 1;
        for (var i = 0; i < n; i++)
        {
            var d = syndromes[i];
            for (var j = 1; j <= l; j++)
                d ^= GfMul(c[j], syndromes[i - j]);
            if (d == 0)
            {
                m++;
                continue;
            }

            var scale = GfDiv(d, previousDiscrepancy);
            var span = Math.Max(n + 1 - m, 0);
            if (2 * l <= i)
            {
                c.CopyTo(snapshot);
                for (var j = 0; j < span; j++)
                    c[j + m] ^= GfMul(scale, b[j]);
                l = i + 1 - l;
                snapshot.CopyTo(b);
                previousDiscrepancy = d;
                m = 1;
            }
            else
            {
                for (var j = 0; j < span; j++)
                    c[j + m] ^= GfMul(scale, b[j]);
                m++;
            }
        }

        return l + 1;
    }
}
