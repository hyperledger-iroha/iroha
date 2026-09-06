using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Kagemusha;

/// <summary>
/// Fail-closed bridge to the audited Rust KAGEMUSHA V1 canonical-shape boundary.
/// These checks do not authenticate signatures, credentials, proof releases, recursive proofs,
/// or monetary authority. This type deliberately has no managed fallback.
/// </summary>
public static class KagemushaNativeShapeV1
{
    public const uint RequiredBridgeAbiVersion = 23;

    private const string LibraryName = "connect_norito_bridge";

    private static readonly string[] RequiredExports =
    [
        "connect_norito_kagemusha_v1_payment_request_validate",
        "connect_norito_kagemusha_v1_payment_validate",
        "connect_norito_kagemusha_v1_acknowledgement_validate",
        "connect_norito_kagemusha_v1_complete_exchange_validate",
        "connect_norito_kagemusha_v1_complete_exchange_text_validate",
        "connect_norito_kagemusha_v1_mint_authorization_validate",
        "connect_norito_kagemusha_v1_mint_credit_validate",
        "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate",
        "connect_norito_kagemusha_v1_redemption_voucher_validate",
    ];

    /// <summary>Whether the exact ABI-23 native validation boundary is loadable.</summary>
    public static bool IsAvailable
    {
        get
        {
            try
            {
                EnsureAvailable();
                return true;
            }
            catch (InvalidOperationException)
            {
                return false;
            }
        }
    }

    public static void ValidatePaymentRequestShape(ReadOnlySpan<byte> request)
    {
        var first = Bounded(request, Kagemusha.MaximumRequestBytes, nameof(request));
        ValidateOne("payment request", first,
            NativeValidatePaymentRequestUnix, NativeValidatePaymentRequestWindows);
    }

    public static void ValidatePaymentShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> payment)
    {
        var first = Bounded(request, Kagemusha.MaximumRequestBytes, nameof(request));
        var second = Bounded(payment, Kagemusha.MaximumPaymentBytes, nameof(payment));
        ValidateTwo("payment", first, second,
            NativeValidatePaymentUnix, NativeValidatePaymentWindows);
    }

    public static void ValidateAcknowledgementShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> payment,
        ReadOnlySpan<byte> acknowledgement)
    {
        var buffers = BoundedCompleteExchange(request, payment, acknowledgement);
        ValidateThree("acknowledgement", buffers[0], buffers[1], buffers[2],
            NativeValidateAcknowledgementUnix, NativeValidateAcknowledgementWindows);
    }

    /// <summary>Validate the sole exact three-message handoff and its aggregate size contract.</summary>
    public static void ValidateCompleteExchangeShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> payment,
        ReadOnlySpan<byte> acknowledgement)
    {
        var buffers = BoundedCompleteExchange(request, payment, acknowledgement);
        ValidateThree("complete exchange", buffers[0], buffers[1], buffers[2],
            NativeValidateCompleteExchangeUnix, NativeValidateCompleteExchangeWindows);
    }

    public static void ValidateMintAuthorizationShape(ReadOnlySpan<byte> authorization)
    {
        var first = Bounded(authorization,
            Kagemusha.MaximumMintAuthorizationBytes, nameof(authorization));
        ValidateOne("mint authorization", first,
            NativeValidateMintAuthorizationUnix, NativeValidateMintAuthorizationWindows);
    }

    public static void ValidateMintCreditShape(ReadOnlySpan<byte> credit)
    {
        var first = Bounded(credit, Kagemusha.MaximumMintCreditBytes, nameof(credit));
        ValidateOne("mint credit", first,
            NativeValidateMintCreditUnix, NativeValidateMintCreditWindows);
    }

    public static void ValidateMintCreditShape(
        ReadOnlySpan<byte> authorization,
        ReadOnlySpan<byte> credit)
    {
        var first = Bounded(authorization,
            Kagemusha.MaximumMintAuthorizationBytes, nameof(authorization));
        var second = Bounded(credit, Kagemusha.MaximumMintCreditBytes, nameof(credit));
        ValidateTwo("mint credit against authorization", first, second,
            NativeValidateMintCreditAgainstAuthorizationUnix,
            NativeValidateMintCreditAgainstAuthorizationWindows);
    }

    public static void ValidateRedemptionVoucherShape(ReadOnlySpan<byte> voucher)
    {
        var first = Bounded(voucher, Kagemusha.MaximumRedemptionVoucherBytes, nameof(voucher));
        ValidateOne("redemption voucher", first,
            NativeValidateRedemptionVoucherUnix, NativeValidateRedemptionVoucherWindows);
    }

    public static void ValidatePaymentRequestTextShape(string request) =>
        ValidatePaymentRequestShape(Kagemusha.DecodeText(Kagemusha.PayloadKind.PaymentRequest, request));

    public static void ValidatePaymentTextShape(
        string request,
        string payment) =>
        ValidatePaymentShape(
            Kagemusha.DecodeText(Kagemusha.PayloadKind.PaymentRequest, request),
            Kagemusha.DecodeText(Kagemusha.PayloadKind.Payment, payment));

    public static void ValidateAcknowledgementTextShape(
        string request,
        string payment,
        string acknowledgement) => ValidateAcknowledgementShape(
            Kagemusha.DecodeText(Kagemusha.PayloadKind.PaymentRequest, request),
            Kagemusha.DecodeText(Kagemusha.PayloadKind.Payment, payment),
            Kagemusha.DecodeText(Kagemusha.PayloadKind.Acknowledgement, acknowledgement));

    public static void ValidateCompleteExchangeTextShape(
        string request,
        string payment,
        string acknowledgement)
    {
        RequireCompleteExchangeTextSize(request, payment, acknowledgement);
        ValidateCompleteExchangeShape(
            Kagemusha.DecodeText(Kagemusha.PayloadKind.PaymentRequest, request),
            Kagemusha.DecodeText(Kagemusha.PayloadKind.Payment, payment),
            Kagemusha.DecodeText(Kagemusha.PayloadKind.Acknowledgement, acknowledgement));
    }

    private static byte[][] BoundedCompleteExchange(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> payment,
        ReadOnlySpan<byte> acknowledgement)
    {
        byte[][] buffers =
        [
            Bounded(request, Kagemusha.MaximumRequestBytes, nameof(request)),
            Bounded(payment, Kagemusha.MaximumPaymentBytes, nameof(payment)),
            Bounded(
                acknowledgement,
                Kagemusha.MaximumAcknowledgementBytes,
                nameof(acknowledgement)),
        ];
        if (buffers.Sum(static value => value.Length) > Kagemusha.MaximumCompleteExchangeRawBytes)
            throw new ArgumentException("KAGEMUSHA V1 complete three-message raw exchange is oversized.");
        return buffers;
    }

    private static void RequireCompleteExchangeTextSize(
        string request,
        string payment,
        string acknowledgement)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(payment);
        ArgumentNullException.ThrowIfNull(acknowledgement);
        if (checked(request.Length + payment.Length + acknowledgement.Length)
            > Kagemusha.MaximumCompleteExchangeTextBytes)
            throw new ArgumentException("KAGEMUSHA V1 complete three-message text exchange is oversized.");
    }

    private static byte[] Bounded(ReadOnlySpan<byte> value, int maximum, string name)
    {
        if (value.IsEmpty || value.Length > maximum)
            throw new ArgumentOutOfRangeException(name, $"{name} must contain 1..{maximum} bytes.");
        return value.ToArray();
    }

    private static void ValidateOne(
        string context,
        byte[] first,
        NativeOneUnix unix,
        NativeOneWindows windows)
    {
        EnsureAvailable();
        var status = OperatingSystem.IsWindows()
            ? windows(first, checked((uint)first.Length))
            : unix(first, new UIntPtr((uint)first.Length));
        RequireSuccess(status, context);
    }

    private static void ValidateTwo(
        string context,
        byte[] first,
        byte[] second,
        NativeTwoUnix unix,
        NativeTwoWindows windows)
    {
        EnsureAvailable();
        var status = OperatingSystem.IsWindows()
            ? windows(first, checked((uint)first.Length), second, checked((uint)second.Length))
            : unix(first, new UIntPtr((uint)first.Length), second, new UIntPtr((uint)second.Length));
        RequireSuccess(status, context);
    }

    private static void ValidateThree(
        string context,
        byte[] first,
        byte[] second,
        byte[] third,
        NativeThreeUnix unix,
        NativeThreeWindows windows)
    {
        EnsureAvailable();
        var status = OperatingSystem.IsWindows()
            ? windows(first, checked((uint)first.Length), second, checked((uint)second.Length),
                third, checked((uint)third.Length))
            : unix(first, new UIntPtr((uint)first.Length), second, new UIntPtr((uint)second.Length),
                third, new UIntPtr((uint)third.Length));
        RequireSuccess(status, context);
    }

    private static void RequireSuccess(int status, string context)
    {
        if (status != 0)
            throw new InvalidDataException(
                $"Native KAGEMUSHA V1 {context} canonical-shape check failed closed (status {status}).");
    }

    private static void EnsureAvailable()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(KagemushaNativeShapeV1).Assembly,
                    null,
                    out handle)
                || RequiredExports.Any(symbol => !NativeLibrary.TryGetExport(handle, symbol, out _))
                || NativeBridgeAbiVersion() != RequiredBridgeAbiVersion)
            {
                throw Unavailable();
            }
        }
        catch (Exception error) when (
            error is DllNotFoundException
            or EntryPointNotFoundException
            or BadImageFormatException)
        {
            throw Unavailable(error);
        }
        finally
        {
            if (handle != IntPtr.Zero) NativeLibrary.Free(handle);
        }
    }

    private static InvalidOperationException Unavailable(Exception? inner = null) => new(
        "ABI-23 connect_norito_bridge with KAGEMUSHA V1 shape symbols is required; no managed fallback exists.",
        inner);

    private delegate int NativeOneUnix(byte[] first, UIntPtr firstLength);
    private delegate int NativeOneWindows(byte[] first, uint firstLength);
    private delegate int NativeTwoUnix(byte[] first, UIntPtr firstLength, byte[] second, UIntPtr secondLength);
    private delegate int NativeTwoWindows(byte[] first, uint firstLength, byte[] second, uint secondLength);
    private delegate int NativeThreeUnix(
        byte[] first, UIntPtr firstLength, byte[] second, UIntPtr secondLength,
        byte[] third, UIntPtr thirdLength);
    private delegate int NativeThreeWindows(
        byte[] first, uint firstLength, byte[] second, uint secondLength,
        byte[] third, uint thirdLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_payment_request_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentRequestUnix([In] byte[] request, UIntPtr requestLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_payment_request_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentRequestWindows([In] byte[] request, uint requestLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_payment_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentUnix(
        [In] byte[] request, UIntPtr requestLength,
        [In] byte[] payment, UIntPtr paymentLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_payment_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentWindows(
        [In] byte[] request, uint requestLength,
        [In] byte[] payment, uint paymentLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_acknowledgement_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateAcknowledgementUnix(
        [In] byte[] request, UIntPtr requestLength,
        [In] byte[] payment, UIntPtr paymentLength,
        [In] byte[] acknowledgement, UIntPtr acknowledgementLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_acknowledgement_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateAcknowledgementWindows(
        [In] byte[] request, uint requestLength,
        [In] byte[] payment, uint paymentLength,
        [In] byte[] acknowledgement, uint acknowledgementLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_complete_exchange_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCompleteExchangeUnix(
        [In] byte[] request, UIntPtr requestLength,
        [In] byte[] payment, UIntPtr paymentLength,
        [In] byte[] acknowledgement, UIntPtr acknowledgementLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_complete_exchange_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCompleteExchangeWindows(
        [In] byte[] request, uint requestLength,
        [In] byte[] payment, uint paymentLength,
        [In] byte[] acknowledgement, uint acknowledgementLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintAuthorizationUnix([In] byte[] authorization, UIntPtr authorizationLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintAuthorizationWindows([In] byte[] authorization, uint authorizationLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_credit_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditUnix([In] byte[] credit, UIntPtr creditLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_credit_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditWindows([In] byte[] credit, uint creditLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditAgainstAuthorizationUnix(
        [In] byte[] authorization, UIntPtr authorizationLength, [In] byte[] credit, UIntPtr creditLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditAgainstAuthorizationWindows(
        [In] byte[] authorization, uint authorizationLength, [In] byte[] credit, uint creditLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_redemption_voucher_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateRedemptionVoucherUnix([In] byte[] voucher, UIntPtr voucherLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_v1_redemption_voucher_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateRedemptionVoucherWindows([In] byte[] voucher, uint voucherLength);
}
