using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.Json.Nodes;
using Hyperledger.Iroha.Norito;
using Hyperledger.Iroha.Privacy;
using Hyperledger.Iroha.Transactions;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class PrivacyX509PreparedTransactionV1Tests
{
    private static PrivacyExact12TypedFixtureRowV1 Row => PrivacyExact12FixtureCodecV1.DecodeCanonicalBase64(
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64")).Trim()).Rows[5];
    [Fact]
    public void PublicOwnerHasNoConstructorSetterHandleOrReplacementPayload()
    {
        var type = typeof(PrivacyX509PreparedTransactionV1);
        Assert.Empty(type.GetConstructors());
        Assert.All(type.GetProperties(), property => Assert.Null(property.SetMethod));
        Assert.DoesNotContain(type.GetProperties(), property => property.PropertyType == typeof(ulong));
        var method = type.GetMethod(nameof(PrivacyX509PreparedTransactionV1.Sign))!;
        Assert.Equal(new[] { "privateKeySeed", "credentialProof" }, method.GetParameters().Select(parameter => parameter.Name));
        Assert.DoesNotContain(typeof(TransactionBuilder).GetMethods(), method => method.Name.Contains("SignX509"));
    }
    [Fact]
    public void NullOrUninitializedAdmissionCannotConstructAnOwner()
    {
        var builder = (TransactionBuilder)RuntimeHelpers.GetUninitializedObject(typeof(TransactionBuilder));
        Assert.Throws<ArgumentNullException>(() => builder.PrepareX509Transaction(null!, Row.SubmitProofInstructionNorito));
        var forged = (PrivacyExact12CapabilityTupleAdmissionV1)RuntimeHelpers.GetUninitializedObject(typeof(PrivacyExact12CapabilityTupleAdmissionV1));
        Assert.Throws<InvalidOperationException>(() => forged.RequireAuthentic(PrivacyProtocolIdV1.IrohaZkX509StarkP256V1, NetworkId.FromBytes(Enumerable.Repeat((byte)0xd1, 32).ToArray())));
    }
    [Fact]
    public void MutableBuilderAndGenericSignedEnvelopeCannotReplaceAPreparedSource()
    {
        Assert.DoesNotContain(typeof(PrivacyX509PreparedTransactionV1).GetMethods(), method =>
            method.GetParameters().Any(parameter => parameter.ParameterType == typeof(TransactionBuilder)
                || parameter.ParameterType == typeof(SignedTransactionEnvelope)));
        var factory = typeof(TransactionBuilder).GetMethod(nameof(TransactionBuilder.PrepareX509Transaction))!;
        Assert.Equal(new[] { typeof(PrivacyExact12CapabilityTupleAdmissionV1), typeof(ReadOnlySpan<byte>) },
            factory.GetParameters().Select(parameter => parameter.ParameterType));
    }
    [Fact]
    public void NativeWireEnvelopeUsesTheCurrentDedicatedX509Fixture()
    {
        var row = Row;
        var envelope = SignedTransactionEnvelope.FromNativeSignedBytes(row.SignedTransactionVersionedNorito);
        Assert.Equal(row.SignedTransactionHash, envelope.TransactionHash);
        Assert.Equal(row.UnsignedTransactionPayloadNorito, envelope.PayloadBytes);
        var copied = envelope.VersionedNoritoBytes; copied[0] = 2;
        Assert.Equal(1, envelope.VersionedNoritoBytes[0]);
        Assert.Throws<ArgumentException>(() => SignedTransactionEnvelope.FromNativeSignedBytes(copied));
        Assert.Throws<ArgumentException>(() => SignedTransactionEnvelope.FromNativeSignedBytes(row.SignedTransactionVersionedNorito.Concat(new byte[] { 0 }).ToArray()));
    }
    [Fact]
    public void ResourceOwnerCopiesProjectionsAndClosesExactlyOnce()
    {
        var driver = new Driver(); var owner = new PrivacyX509NativeOwner(driver, 7);
        var projection = owner.Projection(3); projection[0] ^= 1;
        Assert.Equal(driver.ProjectionBytes, owner.Projection(3));
        owner.Dispose(); owner.Dispose();
        Assert.Equal(1, driver.Closes);
        Assert.Throws<ObjectDisposedException>(() => owner.Projection(3));
        Assert.Throws<ObjectDisposedException>(() => owner.Sign(new byte[32], new byte[] { 1 }));
        Assert.Throws<PrivacyX509NativeException>(() => new PrivacyX509NativeOwner(driver, 0));
    }
    [Fact]
    public void SigningCopiesProofAndErasesItsPrivateSeedCopyOnSuccessAndFailure()
    {
        foreach (var fail in new[] { false, true })
        {
            var driver = new Driver { Fail = fail }; using var owner = new PrivacyX509NativeOwner(driver, 7);
            var seed = Enumerable.Repeat((byte)0x77, 32).ToArray(); var proof = new byte[] { 4, 5 };
            if (fail) Assert.Throws<PrivacyX509NativeException>(() => owner.Sign(seed, proof));
            else Assert.Equal(driver.Wire, owner.Sign(seed, proof));
            Assert.All(driver.Seed!, value => Assert.Equal(0, value));
            Assert.All(seed, value => Assert.Equal(0x77, value));
            Assert.Equal(new byte[] { 4, 5 }, proof);
        }
    }
    [Fact]
    public void SigningBoundsFailBeforeTheNativeResourceDriver()
    {
        var driver = new Driver(); using var owner = new PrivacyX509NativeOwner(driver, 7);
        Assert.Throws<ArgumentException>(() => owner.Sign(new byte[31], new byte[] { 1 }));
        Assert.Throws<ArgumentException>(() => owner.Sign(new byte[32], Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => owner.Sign(new byte[32], new byte[9 * 1024 * 1024 + 1]));
        Assert.Null(driver.Seed);
    }
    [Fact]
    public async Task DisposeCannotCloseAResourceDuringAnAcceptedManagedCall()
    {
        var driver = new Driver { Block = true }; var owner = new PrivacyX509NativeOwner(driver, 7);
        var signing = Task.Run(() => owner.Sign(new byte[32], new byte[] { 1 }), TestContext.Current.CancellationToken);
        Assert.True(driver.Started.Wait(TimeSpan.FromSeconds(5), TestContext.Current.CancellationToken));
        var closing = Task.Run(owner.Dispose, TestContext.Current.CancellationToken);
        Assert.Equal(0, driver.Closes);
        driver.Release.Set(); await signing; await closing;
        Assert.Equal(1, driver.Closes); Assert.False(driver.ClosedDuringSign);
    }
    [Fact]
    public void NativeRefusalHasExactJointSecurityIdentity()
    {
        var error = Assert.Throws<PrivacyX509NativeException>(() => PrivacyX509NativeException.Check(-24));
        Assert.Equal(-24, error.Status); Assert.Contains("JointSecurityReductionIncomplete", error.Message);
    }
    // A test-owned resource driver, not native cryptography, admission, or an X509 producer.
    private sealed class Driver : IPrivacyX509Driver
    {
        internal byte[] ProjectionBytes = [1, 2, 3]; internal byte[] Wire = [1, 3, 5];
        internal byte[]? Seed; internal int Closes; internal bool Fail, Block, ClosedDuringSign;
        internal ManualResetEventSlim Started = new(), Release = new();
        public ulong Prepare(byte[] network, byte[] manifest, byte[] payload, byte[] request) => throw new NotSupportedException();
        public byte[] Projection(ulong handle, uint kind) => ProjectionBytes;
        public byte[] Sign(ulong handle, byte[] seed, byte[] proof)
        {
            Seed = seed; proof[0] ^= 1; Started.Set();
            if (Block && !Release.Wait(TimeSpan.FromSeconds(10))) throw new TimeoutException();
            ClosedDuringSign = Closes != 0;
            if (Fail) throw new PrivacyX509NativeException(-24);
            return Wire;
        }
        public void Close(ulong handle) => Interlocked.Increment(ref Closes);
    }
}
