using System.Net;
using System.Net.Http.Headers;
using System.Runtime.InteropServices;
using Hyperledger.Iroha.Http;
using Hyperledger.Iroha.Torii;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed partial class ToriiClientTests
{
    [Fact]
    public async Task HijiriQuoteUsesCanonicalAuthAndExactNativeNoritoTransport()
    {
        var encodedRequest = new byte[] { 1, 2, 3, 4 };
        var encodedResponse = new byte[] { 9, 8, 7 };
        var codec = new FakeHijiriQuoteCodec(
            _ => encodedRequest,
            (response, request) =>
            {
                Assert.Equal(encodedResponse, response);
                Assert.Equal(encodedRequest, request);
                return HijiriQuoteProjection(CanonicalAccountId, 2);
            });
        using var handler = new RecordingHandler(request =>
        {
            Assert.Equal(HttpMethod.Post, request.Method);
            Assert.Equal("/v1/validation-fee/hijiri/quote", request.RequestUri!.AbsolutePath);
            Assert.Equal("application/x-norito", request.Content!.Headers.ContentType!.MediaType);
            Assert.Equal(encodedRequest, request.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult());
            Assert.Equal(
                "application/x-norito",
                Assert.Single(request.Headers.Accept).MediaType);
            Assert.Equal("identity", Assert.Single(request.Headers.GetValues("Accept-Encoding")));
            Assert.True(request.Headers.CacheControl!.NoStore);
            Assert.True(request.Headers.Contains("X-Iroha-Account"));
            Assert.True(request.Headers.Contains("X-Iroha-Signature"));

            return HijiriQuoteResponse(encodedResponse);
        });
        using var client = HijiriQuoteClient(handler);

        var quote = await client.PostValidationFeeHijiriQuoteAsync(
            new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 2),
            codec,
            TestContext.Current.CancellationToken);

        Assert.Equal(CanonicalAccountId, quote.AccountId);
        Assert.Equal(2U, quote.QualifyingTransferCount);
        Assert.Equal("43", quote.QuotedExecutionHeight);
        Assert.Equal(1, codec.EncodeCalls);
        Assert.Equal(1, codec.VerifyCalls);
    }

    [Fact]
    public async Task HijiriQuoteRequiresCredentialsAndOneShotInjectedTransportBeforeEncoding()
    {
        var codec = new FakeHijiriQuoteCodec(
            _ => throw new InvalidOperationException("codec must not run"),
            (_, _) => throw new InvalidOperationException("codec must not run"));
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("HTTP must not run"));
        using var missingCredentials = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler));
        await Assert.ThrowsAsync<InvalidOperationException>(() =>
            missingCredentials.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                codec,
                TestContext.Current.CancellationToken));

        using var unassured = new ToriiClient(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            HijiriQuoteOptions());
        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            unassured.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                codec,
                TestContext.Current.CancellationToken));
        Assert.Contains("one-shot", error.Message);
        Assert.Equal(0, codec.EncodeCalls);
    }

    [Fact]
    public async Task HijiriQuoteRequiresHttpsBeforeEncodingOrDispatch()
    {
        var codec = new FakeHijiriQuoteCodec(
            _ => throw new InvalidOperationException("codec must not run"),
            (_, _) => throw new InvalidOperationException("codec must not run"));
        using var handler = new RecordingHandler(_ =>
            throw new InvalidOperationException("HTTP must not run"));
        using var client = new ToriiClient(
            new Uri("http://torii.example"),
            new HttpClient(handler),
            HijiriQuoteOptions(),
            ValidationFeeHijiriQuoteTransportAssurance
                .OneShotWithoutRedirectsRetriesOrDecompression);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            client.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                codec,
                TestContext.Current.CancellationToken));

        Assert.Contains("HTTPS", error.Message, StringComparison.Ordinal);
        Assert.Equal(0, codec.EncodeCalls);
        Assert.Null(handler.LastRequest);
    }

    [Fact]
    public async Task HijiriQuoteRejectsResponseHeaderAndSizeDriftBeforeVerification()
    {
        var cases = new Func<HttpResponseMessage>[]
        {
            () => HijiriQuoteResponse([1], mediaType: "application/json"),
            () => HijiriQuoteResponse([1], includePrivateNoStore: false),
            () => HijiriQuoteResponse([1], contentEncoding: "gzip"),
            () => HijiriQuoteResponse([1], includeRejectCode: true),
            () => HijiriQuoteResponse(
                new byte[ValidationFeeHijiriQuoteV1.MaximumResponseBytes + 1]),
        };

        foreach (var responseFactory in cases)
        {
            var codec = new FakeHijiriQuoteCodec(
                _ => [1],
                (_, _) => throw new InvalidOperationException("verifier must not run"));
            using var handler = new RecordingHandler(_ => responseFactory());
            using var client = HijiriQuoteClient(handler);

            await Assert.ThrowsAnyAsync<Exception>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Equal(0, codec.VerifyCalls);
        }
    }

    [Fact]
    public async Task HijiriQuoteBoundsAndRequiresPrivateNoStoreOnErrorResponses()
    {
        var codec = new FakeHijiriQuoteCodec(
            _ => [1],
            (_, _) => throw new InvalidOperationException("verifier must not run"));

        var invalidCacheValues = new CacheControlHeaderValue?[]
        {
            null,
            new() { Private = true },
            new() { NoStore = true },
            new() { NoStore = true, Private = true, Public = true },
        };
        foreach (var cacheControl in invalidCacheValues)
        {
            using var handler = new RecordingHandler(_ =>
            {
                var response = HijiriQuoteResponse(
                    new ByteArrayContent("denied"u8.ToArray()),
                    HttpStatusCode.BadRequest,
                    includePrivateNoStore: false);
                response.Headers.CacheControl = cacheControl;
                return response;
            });
            using var client = HijiriQuoteClient(handler);
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Contains("private and no-store", error.Message, StringComparison.Ordinal);
        }

        var invalidRepresentationResponses = new Func<HttpResponseMessage>[]
        {
            () => HijiriQuoteResponse(
                new ByteArrayContent("denied"u8.ToArray()),
                HttpStatusCode.BadRequest,
                mediaType: "application/json"),
            () => HijiriQuoteResponse(
                new ByteArrayContent("denied"u8.ToArray()),
                HttpStatusCode.BadRequest,
                contentEncoding: "gzip"),
        };
        foreach (var responseFactory in invalidRepresentationResponses)
        {
            using var handler = new RecordingHandler(_ => responseFactory());
            using var client = HijiriQuoteClient(handler);
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
        }

        var stream = new HijiriQuoteCountingReadStream(10 * 1024 * 1024, 4 * 1024);
        using (var oversizedHandler = new RecordingHandler(_ =>
                   HijiriQuoteResponse(
                       new StreamContent(stream),
                       HttpStatusCode.BadRequest)))
        using (var oversizedClient = HijiriQuoteClient(oversizedHandler))
        {
            var oversizedError = await Assert.ThrowsAsync<ToriiApiException>(() =>
                oversizedClient.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Contains("response limit", oversizedError.ResponseBody, StringComparison.Ordinal);
        }
        Assert.InRange(
            stream.BytesRead,
            ValidationFeeHijiriQuoteV1.MaximumResponseBytes + 1,
            ValidationFeeHijiriQuoteV1.MaximumResponseBytes + (4 * 1024));
        Assert.True(stream.WasDisposed);

        using var boundedHandler = new RecordingHandler(_ =>
            HijiriQuoteResponse(
                new ByteArrayContent("denied"u8.ToArray()),
                HttpStatusCode.BadRequest));
        using var boundedClient = HijiriQuoteClient(boundedHandler);
        var apiError = await Assert.ThrowsAsync<ToriiApiException>(() =>
            boundedClient.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                codec,
                TestContext.Current.CancellationToken));
        Assert.Equal(HttpStatusCode.BadRequest, apiError.StatusCode);
        Assert.Equal("denied", apiError.ResponseBody);
        Assert.Equal(0, codec.VerifyCalls);
    }

    [Theory]
    [InlineData("private=\"field\", no-store")]
    [InlineData("private, no-store=field")]
    [InlineData("private, no-store, public")]
    [InlineData("private, no-store, public=max-age")]
    public async Task HijiriQuoteRejectsQualifiedOrContradictoryCacheDirectives(
        string cacheControl)
    {
        var codec = new FakeHijiriQuoteCodec(
            _ => [1],
            (_, _) => throw new InvalidOperationException("verifier must not run"));
        using var handler = new RecordingHandler(_ =>
        {
            var response = HijiriQuoteResponse(
                [2],
                includePrivateNoStore: false);
            Assert.True(
                response.Headers.TryAddWithoutValidation("Cache-Control", cacheControl));
            return response;
        });
        using var client = HijiriQuoteClient(handler);

        var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
            client.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                codec,
                TestContext.Current.CancellationToken));

        Assert.Contains("private and no-store", error.Message, StringComparison.Ordinal);
        Assert.Equal(0, codec.VerifyCalls);
    }

    [Fact]
    public async Task HijiriQuoteRequiresCanonicalMatchingBoundedContentLength()
    {
        var codec = new FakeHijiriQuoteCodec(
            _ => [1],
            (_, _) => throw new InvalidOperationException("verifier must not run"));

        using (var mismatchContent = new ByteArrayContent([2]))
        {
            mismatchContent.Headers.ContentLength = 2;
            using var handler = new RecordingHandler(_ =>
                HijiriQuoteResponse(mismatchContent, HttpStatusCode.OK));
            using var client = HijiriQuoteClient(handler);
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Contains("does not match", error.Message, StringComparison.Ordinal);
        }

        using (var understatedContent = new ByteArrayContent([2, 3]))
        {
            understatedContent.Headers.ContentLength = 1;
            using var handler = new RecordingHandler(_ =>
                HijiriQuoteResponse(understatedContent, HttpStatusCode.OK));
            using var client = HijiriQuoteClient(handler);
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Contains("does not match", error.Message, StringComparison.Ordinal);
        }

        var declaredOversizeStream = new HijiriQuoteCountingReadStream(1, 1);
        using (var declaredOversizeContent = new StreamContent(declaredOversizeStream))
        {
            declaredOversizeContent.Headers.ContentLength =
                ValidationFeeHijiriQuoteV1.MaximumResponseBytes + 1L;
            using var handler = new RecordingHandler(_ =>
                HijiriQuoteResponse(declaredOversizeContent, HttpStatusCode.OK));
            using var client = HijiriQuoteClient(handler);
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Equal(0, declaredOversizeStream.BytesRead);
        }

        using (var noncanonicalContent = new StreamContent(new MemoryStream([2], writable: false)))
        {
            Assert.True(
                noncanonicalContent.Headers.TryAddWithoutValidation("Content-Length", "01"));
            using var handler = new RecordingHandler(_ =>
                HijiriQuoteResponse(noncanonicalContent, HttpStatusCode.OK));
            using var client = HijiriQuoteClient(handler);
            var error = await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
                    codec,
                    TestContext.Current.CancellationToken));
            Assert.Contains("noncanonical", error.Message, StringComparison.Ordinal);
        }

        Assert.Equal(0, codec.VerifyCalls);

        var exactCodec = new FakeHijiriQuoteCodec(
            _ => [1],
            (_, _) => HijiriQuoteProjection(CanonicalAccountId, 1));
        using var exactContent = new ByteArrayContent([2]);
        exactContent.Headers.ContentLength = 1;
        using var exactHandler = new RecordingHandler(_ =>
            HijiriQuoteResponse(exactContent, HttpStatusCode.OK));
        using var exactClient = HijiriQuoteClient(exactHandler);
        var quote = await exactClient.PostValidationFeeHijiriQuoteAsync(
            new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 1),
            exactCodec,
            TestContext.Current.CancellationToken);
        Assert.Equal(1U, quote.QualifyingTransferCount);
        Assert.Equal(1, exactCodec.VerifyCalls);
    }

    [Fact]
    public async Task HijiriQuoteRejectsCodecOutputAndVerifiedProjectionSubstitution()
    {
        var invalidBodies = new[]
        {
            Array.Empty<byte>(),
            new byte[ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes + 1],
        };
        foreach (var invalidBody in invalidBodies)
        {
            var codec = new FakeHijiriQuoteCodec(
                _ => invalidBody,
                (_, _) => throw new InvalidOperationException("verifier must not run"));
            using var handler = new RecordingHandler(_ =>
                throw new InvalidOperationException("HTTP must not run"));
            using var client = HijiriQuoteClient(handler);
            await Assert.ThrowsAsync<InvalidDataException>(() =>
                client.PostValidationFeeHijiriQuoteAsync(
                    new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 2),
                    codec,
                    TestContext.Current.CancellationToken));
        }

        var substituted = new FakeHijiriQuoteCodec(
            _ => [1],
            (_, _) => HijiriQuoteProjection(CanonicalAccountId, 3));
        using var substitutionHandler = new RecordingHandler(_ => HijiriQuoteResponse([2]));
        using var substitutionClient = HijiriQuoteClient(substitutionHandler);
        await Assert.ThrowsAsync<InvalidDataException>(() =>
            substitutionClient.PostValidationFeeHijiriQuoteAsync(
                new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 2),
                substituted,
                TestContext.Current.CancellationToken));
    }

    [Fact]
    public void HijiriQuoteNativeSurfacePinsAbiAndAdditiveSymbols()
    {
        Assert.Equal(23U, ValidationFeeHijiriQuoteNative.RequiredBridgeAbiVersion);
        var nativeMethods = typeof(ValidationFeeHijiriQuoteNative)
            .GetMethods(System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static)
            .Select(method => method.GetCustomAttributes(typeof(DllImportAttribute), false)
                .Cast<DllImportAttribute>()
                .SingleOrDefault())
            .Where(attribute => attribute is not null)
            .Select(attribute => attribute!.EntryPoint)
            .ToHashSet(StringComparer.Ordinal);

        Assert.Contains("connect_norito_bridge_abi_version", nativeMethods);
        Assert.Contains("connect_norito_validation_fee_hijiri_quote_request_v1", nativeMethods);
        Assert.Contains(
            "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
            nativeMethods);
        Assert.Contains("connect_norito_free", nativeMethods);

        var flags = System.Reflection.BindingFlags.NonPublic
            | System.Reflection.BindingFlags.Static;
        var encodeUnix = typeof(ValidationFeeHijiriQuoteNative)
            .GetMethod("NativeEncodeRequestV1Unix", flags)!;
        var encodeWindows = typeof(ValidationFeeHijiriQuoteNative)
            .GetMethod("NativeEncodeRequestV1Windows", flags)!;
        var verifyUnix = typeof(ValidationFeeHijiriQuoteNative)
            .GetMethod("NativeVerifyResponseV1Unix", flags)!;
        var verifyWindows = typeof(ValidationFeeHijiriQuoteNative)
            .GetMethod("NativeVerifyResponseV1Windows", flags)!;

        Assert.Equal(typeof(UIntPtr), encodeUnix.GetParameters()[1].ParameterType);
        Assert.Equal(typeof(UIntPtr).MakeByRefType(), encodeUnix.GetParameters()[4].ParameterType);
        Assert.Equal(typeof(uint), encodeWindows.GetParameters()[1].ParameterType);
        Assert.Equal(typeof(uint).MakeByRefType(), encodeWindows.GetParameters()[4].ParameterType);
        Assert.Equal(typeof(UIntPtr), verifyUnix.GetParameters()[1].ParameterType);
        Assert.Equal(typeof(UIntPtr), verifyUnix.GetParameters()[3].ParameterType);
        Assert.Equal(typeof(UIntPtr).MakeByRefType(), verifyUnix.GetParameters()[5].ParameterType);
        Assert.Equal(typeof(uint), verifyWindows.GetParameters()[1].ParameterType);
        Assert.Equal(typeof(uint), verifyWindows.GetParameters()[3].ParameterType);
        Assert.Equal(typeof(uint).MakeByRefType(), verifyWindows.GetParameters()[5].ParameterType);
    }

    [Fact]
    public void HijiriQuoteNativeBridgeEncodesFreesAndRejectsMalformedResponse()
    {
        var request = new ValidationFeeHijiriQuoteRequestV1(CanonicalAccountId, 2);

        var first = ValidationFeeHijiriQuoteNative.EncodeRequestV1(request);
        var second = ValidationFeeHijiriQuoteNative.EncodeRequestV1(request);

        Assert.NotEmpty(first);
        Assert.InRange(
            first.Length,
            1,
            ValidationFeeHijiriQuoteRequestV1.MaximumRequestBytes);
        Assert.Equal(first, second);
        var malformedResponse = first.Append((byte)0).ToArray();
        var error = Assert.Throws<InvalidDataException>(() =>
            ValidationFeeHijiriQuoteNative.VerifyResponseV1(malformedResponse, first));
        Assert.Contains("failed closed", error.Message, StringComparison.Ordinal);
    }

    private static ToriiClient HijiriQuoteClient(RecordingHandler handler) =>
        new(
            new Uri("https://torii.example"),
            new HttpClient(handler),
            HijiriQuoteOptions(),
            ValidationFeeHijiriQuoteTransportAssurance
                .OneShotWithoutRedirectsRetriesOrDecompression);

    private static ToriiClientOptions HijiriQuoteOptions() => new()
    {
        LocalSigningContext = new ToriiLocalSigningContext(NetworkId.Parse(CanonicalNetworkId)),
        CanonicalRequestCredentials = new CanonicalRequestCredentials(
            CanonicalAccountId,
            CanonicalPrivateKeySeed),
    };

    private static HttpResponseMessage HijiriQuoteResponse(
        byte[] body,
        string mediaType = "application/x-norito",
        bool includePrivateNoStore = true,
        string? contentEncoding = null,
        bool includeRejectCode = false)
        => HijiriQuoteResponse(
            new ByteArrayContent(body),
            HttpStatusCode.OK,
            mediaType,
            includePrivateNoStore,
            contentEncoding,
            includeRejectCode);

    private static HttpResponseMessage HijiriQuoteResponse(
        HttpContent content,
        HttpStatusCode statusCode,
        string mediaType = "application/x-norito",
        bool includePrivateNoStore = true,
        string? contentEncoding = null,
        bool includeRejectCode = false)
    {
        var response = new HttpResponseMessage(statusCode) { Content = content };
        response.Content.Headers.ContentType = new MediaTypeHeaderValue(mediaType);
        if (contentEncoding is not null)
        {
            response.Content.Headers.ContentEncoding.Add(contentEncoding);
        }
        if (includePrivateNoStore)
        {
            response.Headers.CacheControl = new CacheControlHeaderValue
            {
                NoStore = true,
                Private = true,
            };
        }
        if (includeRejectCode)
        {
            response.Headers.TryAddWithoutValidation(
                "X-Iroha-Reject-Code",
                "validation_fee_hijiri_quote_unavailable");
        }
        return response;
    }

    private static ValidationFeeHijiriQuoteV1 HijiriQuoteProjection(
        string accountId,
        uint qualifyingTransferCount) => new()
        {
            Schema = ValidationFeeHijiriQuoteV1.SchemaV1,
            Version = 1,
            Assurance = ValidationFeeHijiriQuoteV1.EvaluatedAssuranceV1,
            EvaluatedStateHeight = "42",
            QuotedExecutionHeight = "43",
            AccountId = accountId,
            ActivePolicyVersion = "1",
            ActivePolicyHash = new string('a', 64),
            FeeAssetDefinitionId = "asset",
            TreasuryAccountId = accountId,
            FeeScale = 2,
            HijiriParametersVersion = 1,
            HijiriParametersRevision = "1",
            HijiriParametersDigest = new string('b', 64),
            DefaultAccountRiskQ16 = 0,
            EffectiveAccountRiskQ16 = 0,
            AccountRiskRevision = null,
            AccountRiskDigest = null,
            FeeMultiplierQ16 = 65_536,
            HijiriFeeQuoteHash = new string('c', 64),
            BasePerTransferFeeMinorUnits = "10",
            AdjustedPerTransferFeeMinorUnits = "10",
            QualifyingTransferCount = qualifyingTransferCount,
            AggregateBaseFeeMinorUnits = (10U * qualifyingTransferCount).ToString(),
            AggregateAdjustedFeeMinorUnits = (10U * qualifyingTransferCount).ToString(),
        };

    private sealed class FakeHijiriQuoteCodec : IValidationFeeHijiriQuoteCodec
    {
        private readonly Func<ValidationFeeHijiriQuoteRequestV1, byte[]> encode;
        private readonly Func<byte[], byte[], ValidationFeeHijiriQuoteV1> verify;

        internal FakeHijiriQuoteCodec(
            Func<ValidationFeeHijiriQuoteRequestV1, byte[]> encode,
            Func<byte[], byte[], ValidationFeeHijiriQuoteV1> verify)
        {
            this.encode = encode;
            this.verify = verify;
        }

        internal int EncodeCalls { get; private set; }

        internal int VerifyCalls { get; private set; }

        public byte[] Encode(ValidationFeeHijiriQuoteRequestV1 request)
        {
            EncodeCalls++;
            return encode(request);
        }

        public ValidationFeeHijiriQuoteV1 Verify(
            byte[] responseNorito,
            byte[] requestNorito)
        {
            VerifyCalls++;
            return verify(responseNorito, requestNorito);
        }
    }

    private sealed class HijiriQuoteCountingReadStream(
        int totalBytes,
        int maximumChunkBytes) : Stream
    {
        internal int BytesRead { get; private set; }

        internal bool WasDisposed { get; private set; }

        public override bool CanRead => true;

        public override bool CanSeek => false;

        public override bool CanWrite => false;

        public override long Length => throw new NotSupportedException();

        public override long Position
        {
            get => BytesRead;
            set => throw new NotSupportedException();
        }

        public override int Read(byte[] buffer, int offset, int count)
        {
            var length = Math.Min(Math.Min(count, maximumChunkBytes), totalBytes - BytesRead);
            if (length <= 0)
            {
                return 0;
            }
            buffer.AsSpan(offset, length).Fill((byte)'x');
            BytesRead += length;
            return length;
        }

        public override int Read(Span<byte> buffer)
        {
            var length = Math.Min(Math.Min(buffer.Length, maximumChunkBytes), totalBytes - BytesRead);
            if (length <= 0)
            {
                return 0;
            }
            buffer[..length].Fill((byte)'x');
            BytesRead += length;
            return length;
        }

        public override ValueTask<int> ReadAsync(
            Memory<byte> buffer,
            CancellationToken cancellationToken = default)
        {
            cancellationToken.ThrowIfCancellationRequested();
            return ValueTask.FromResult(Read(buffer.Span));
        }

        protected override void Dispose(bool disposing)
        {
            WasDisposed = true;
            base.Dispose(disposing);
        }

        public override void Flush() { }

        public override long Seek(long offset, SeekOrigin origin) =>
            throw new NotSupportedException();

        public override void SetLength(long value) => throw new NotSupportedException();

        public override void Write(byte[] buffer, int offset, int count) =>
            throw new NotSupportedException();
    }
}
