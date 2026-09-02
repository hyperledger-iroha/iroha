using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.OfflineCash;

/// <summary>
/// Fail-closed bridge to the audited Rust Offline Cash V1 canonical-shape boundary.
/// These checks do not authenticate signatures, credentials, proof releases, recursive proofs,
/// or monetary authority. This type deliberately has no managed fallback.
/// </summary>
public static class OfflineCashNativeShapeV1
{
    public const uint RequiredBridgeAbiVersion = 23;

    private const string LibraryName = "connect_norito_bridge";

    private static readonly string[] RequiredExports =
    [
        "connect_norito_offline_cash_v1_payment_request_validate",
        "connect_norito_offline_cash_v1_acceptance_intent_authorization_validate",
        "connect_norito_offline_cash_v1_acceptance_ticket_validate",
        "connect_norito_offline_cash_v1_no_commit_closure_validate",
        "connect_norito_offline_cash_v1_payment_validate",
        "connect_norito_offline_cash_v1_acknowledgement_validate",
        "connect_norito_offline_cash_v1_complete_exchange_validate",
        "connect_norito_offline_cash_v1_mint_authorization_validate",
        "connect_norito_offline_cash_v1_mint_credit_validate",
        "connect_norito_offline_cash_v1_mint_credit_against_authorization_validate",
        "connect_norito_offline_cash_v1_redemption_voucher_validate",
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
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        ValidateOne("payment request", first,
            NativeValidatePaymentRequestUnix, NativeValidatePaymentRequestWindows);
    }

    public static void ValidateAcceptanceIntentAuthorizationShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> authorization)
    {
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        var second = Bounded(authorization,
            OfflineCashV1.MaximumAcceptanceIntentAuthorizationBytes, nameof(authorization));
        ValidateTwo("acceptance intent authorization", first, second,
            NativeValidateIntentAuthorizationUnix, NativeValidateIntentAuthorizationWindows);
    }

    public static void ValidateAcceptanceTicketShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> authorization,
        ReadOnlySpan<byte> ticket)
    {
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        var second = Bounded(authorization,
            OfflineCashV1.MaximumAcceptanceIntentAuthorizationBytes, nameof(authorization));
        var third = Bounded(ticket, OfflineCashV1.MaximumAcceptanceTicketBytes, nameof(ticket));
        ValidateThree("acceptance ticket", first, second, third,
            NativeValidateTicketUnix, NativeValidateTicketWindows);
    }

    public static void ValidatePaymentShape(ReadOnlySpan<byte> request, ReadOnlySpan<byte> payment)
    {
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        var second = Bounded(payment, OfflineCashV1.MaximumPaymentBytes, nameof(payment));
        ValidateTwo("payment", first, second,
            NativeValidatePaymentUnix, NativeValidatePaymentWindows);
    }

    public static void ValidateNoCommitClosureShape(ReadOnlySpan<byte> closure)
    {
        var first = Bounded(closure, OfflineCashV1.MaximumNoCommitClosureBytes, nameof(closure));
        ValidateOne("no-commit closure", first,
            NativeValidateNoCommitClosureUnix, NativeValidateNoCommitClosureWindows);
    }

    public static void ValidateAcknowledgementShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> payment,
        ReadOnlySpan<byte> acknowledgement)
    {
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        var second = Bounded(payment, OfflineCashV1.MaximumPaymentBytes, nameof(payment));
        var third = Bounded(acknowledgement,
            OfflineCashV1.MaximumAcknowledgementBytes, nameof(acknowledgement));
        ValidateThree("acknowledgement", first, second, third,
            NativeValidateAcknowledgementUnix, NativeValidateAcknowledgementWindows);
    }

    public static void ValidateCompleteExchangeShape(
        ReadOnlySpan<byte> request,
        ReadOnlySpan<byte> authorization,
        ReadOnlySpan<byte> ticket,
        ReadOnlySpan<byte> payment,
        ReadOnlySpan<byte> acknowledgement)
    {
        var first = Bounded(request, OfflineCashV1.MaximumRequestBytes, nameof(request));
        var second = Bounded(authorization,
            OfflineCashV1.MaximumAcceptanceIntentAuthorizationBytes, nameof(authorization));
        var third = Bounded(ticket, OfflineCashV1.MaximumAcceptanceTicketBytes, nameof(ticket));
        var fourth = Bounded(payment, OfflineCashV1.MaximumPaymentBytes, nameof(payment));
        var fifth = Bounded(acknowledgement,
            OfflineCashV1.MaximumAcknowledgementBytes, nameof(acknowledgement));
        ValidateFive("complete exchange", first, second, third, fourth, fifth,
            NativeValidateCompleteExchangeUnix, NativeValidateCompleteExchangeWindows);
    }

    public static void ValidateMintAuthorizationShape(ReadOnlySpan<byte> authorization)
    {
        var first = Bounded(authorization,
            OfflineCashV1.MaximumMintAuthorizationBytes, nameof(authorization));
        ValidateOne("mint authorization", first,
            NativeValidateMintAuthorizationUnix, NativeValidateMintAuthorizationWindows);
    }

    public static void ValidateMintCreditShape(ReadOnlySpan<byte> credit)
    {
        var first = Bounded(credit, OfflineCashV1.MaximumMintCreditBytes, nameof(credit));
        ValidateOne("mint credit", first,
            NativeValidateMintCreditUnix, NativeValidateMintCreditWindows);
    }

    public static void ValidateMintCreditShape(
        ReadOnlySpan<byte> authorization,
        ReadOnlySpan<byte> credit)
    {
        var first = Bounded(authorization,
            OfflineCashV1.MaximumMintAuthorizationBytes, nameof(authorization));
        var second = Bounded(credit, OfflineCashV1.MaximumMintCreditBytes, nameof(credit));
        ValidateTwo("mint credit against authorization", first, second,
            NativeValidateMintCreditAgainstAuthorizationUnix,
            NativeValidateMintCreditAgainstAuthorizationWindows);
    }

    public static void ValidateRedemptionVoucherShape(ReadOnlySpan<byte> voucher)
    {
        var first = Bounded(voucher, OfflineCashV1.MaximumRedemptionVoucherBytes, nameof(voucher));
        ValidateOne("redemption voucher", first,
            NativeValidateRedemptionVoucherUnix, NativeValidateRedemptionVoucherWindows);
    }

    public static void ValidatePaymentRequestTextShape(string request) =>
        ValidatePaymentRequestShape(OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request));

    public static void ValidateAcceptanceIntentAuthorizationTextShape(string request, string authorization) =>
        ValidateAcceptanceIntentAuthorizationShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.AcceptanceIntentAuthorization, authorization));

    public static void ValidateAcceptanceTicketTextShape(string request, string authorization, string ticket) =>
        ValidateAcceptanceTicketShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.AcceptanceIntentAuthorization, authorization),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.AcceptanceTicket, ticket));

    public static void ValidateNoCommitClosureTextShape(string closure) =>
        ValidateNoCommitClosureShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.NoCommitClosure, closure));

    public static void ValidatePaymentTextShape(string request, string payment) =>
        ValidatePaymentShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.Payment, payment));

    public static void ValidateAcknowledgementTextShape(
        string request,
        string payment,
        string acknowledgement) => ValidateAcknowledgementShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.Payment, payment),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.Acknowledgement, acknowledgement));

    public static void ValidateCompleteExchangeTextShape(
        string request,
        string authorization,
        string ticket,
        string payment,
        string acknowledgement)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(authorization);
        ArgumentNullException.ThrowIfNull(ticket);
        ArgumentNullException.ThrowIfNull(payment);
        ArgumentNullException.ThrowIfNull(acknowledgement);
        if ((long)request.Length + authorization.Length + ticket.Length + payment.Length
            + acknowledgement.Length > OfflineCashV1.MaximumCompleteExchangeTextBytes)
            throw new ArgumentOutOfRangeException(nameof(request),
                "Complete Offline Cash V1 text exchange exceeds its aggregate byte cap.");
        ValidateCompleteExchangeShape(
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.PaymentRequest, request),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.AcceptanceIntentAuthorization, authorization),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.AcceptanceTicket, ticket),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.Payment, payment),
            OfflineCashV1.DecodeText(OfflineCashV1.PayloadKind.Acknowledgement, acknowledgement));
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

    private static void ValidateFive(
        string context,
        byte[] first,
        byte[] second,
        byte[] third,
        byte[] fourth,
        byte[] fifth,
        NativeFiveUnix unix,
        NativeFiveWindows windows)
    {
        EnsureAvailable();
        var status = OperatingSystem.IsWindows()
            ? windows(first, checked((uint)first.Length), second, checked((uint)second.Length),
                third, checked((uint)third.Length), fourth, checked((uint)fourth.Length),
                fifth, checked((uint)fifth.Length))
            : unix(first, new UIntPtr((uint)first.Length), second, new UIntPtr((uint)second.Length),
                third, new UIntPtr((uint)third.Length), fourth, new UIntPtr((uint)fourth.Length),
                fifth, new UIntPtr((uint)fifth.Length));
        RequireSuccess(status, context);
    }

    private static void RequireSuccess(int status, string context)
    {
        if (status != 0)
            throw new InvalidDataException(
                $"Native Offline Cash V1 {context} canonical-shape check failed closed (status {status}).");
    }

    private static void EnsureAvailable()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(OfflineCashNativeShapeV1).Assembly,
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
        "ABI-23 connect_norito_bridge with Offline Cash V1 shape symbols is required; no managed fallback exists.",
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
    private delegate int NativeFiveUnix(
        byte[] first, UIntPtr firstLength, byte[] second, UIntPtr secondLength,
        byte[] third, UIntPtr thirdLength, byte[] fourth, UIntPtr fourthLength,
        byte[] fifth, UIntPtr fifthLength);
    private delegate int NativeFiveWindows(
        byte[] first, uint firstLength, byte[] second, uint secondLength,
        byte[] third, uint thirdLength, byte[] fourth, uint fourthLength,
        byte[] fifth, uint fifthLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_payment_request_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentRequestUnix([In] byte[] request, UIntPtr requestLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_payment_request_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentRequestWindows([In] byte[] request, uint requestLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acceptance_intent_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateIntentAuthorizationUnix(
        [In] byte[] request, UIntPtr requestLength, [In] byte[] authorization, UIntPtr authorizationLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acceptance_intent_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateIntentAuthorizationWindows(
        [In] byte[] request, uint requestLength, [In] byte[] authorization, uint authorizationLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acceptance_ticket_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateTicketUnix(
        [In] byte[] request, UIntPtr requestLength, [In] byte[] authorization, UIntPtr authorizationLength,
        [In] byte[] ticket, UIntPtr ticketLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acceptance_ticket_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateTicketWindows(
        [In] byte[] request, uint requestLength, [In] byte[] authorization, uint authorizationLength,
        [In] byte[] ticket, uint ticketLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_no_commit_closure_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateNoCommitClosureUnix([In] byte[] closure, UIntPtr closureLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_no_commit_closure_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateNoCommitClosureWindows([In] byte[] closure, uint closureLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_payment_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentUnix(
        [In] byte[] request, UIntPtr requestLength, [In] byte[] payment, UIntPtr paymentLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_payment_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidatePaymentWindows(
        [In] byte[] request, uint requestLength, [In] byte[] payment, uint paymentLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acknowledgement_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateAcknowledgementUnix(
        [In] byte[] request, UIntPtr requestLength, [In] byte[] payment, UIntPtr paymentLength,
        [In] byte[] acknowledgement, UIntPtr acknowledgementLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_acknowledgement_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateAcknowledgementWindows(
        [In] byte[] request, uint requestLength, [In] byte[] payment, uint paymentLength,
        [In] byte[] acknowledgement, uint acknowledgementLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_complete_exchange_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCompleteExchangeUnix(
        [In] byte[] request, UIntPtr requestLength, [In] byte[] authorization, UIntPtr authorizationLength,
        [In] byte[] ticket, UIntPtr ticketLength, [In] byte[] payment, UIntPtr paymentLength,
        [In] byte[] acknowledgement, UIntPtr acknowledgementLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_complete_exchange_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateCompleteExchangeWindows(
        [In] byte[] request, uint requestLength, [In] byte[] authorization, uint authorizationLength,
        [In] byte[] ticket, uint ticketLength, [In] byte[] payment, uint paymentLength,
        [In] byte[] acknowledgement, uint acknowledgementLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintAuthorizationUnix([In] byte[] authorization, UIntPtr authorizationLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintAuthorizationWindows([In] byte[] authorization, uint authorizationLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_credit_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditUnix([In] byte[] credit, UIntPtr creditLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_credit_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditWindows([In] byte[] credit, uint creditLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_credit_against_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditAgainstAuthorizationUnix(
        [In] byte[] authorization, UIntPtr authorizationLength, [In] byte[] credit, UIntPtr creditLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_mint_credit_against_authorization_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateMintCreditAgainstAuthorizationWindows(
        [In] byte[] authorization, uint authorizationLength, [In] byte[] credit, uint creditLength);

    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_redemption_voucher_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateRedemptionVoucherUnix([In] byte[] voucher, UIntPtr voucherLength);
    [DllImport(LibraryName, EntryPoint = "connect_norito_offline_cash_v1_redemption_voucher_validate", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateRedemptionVoucherWindows([In] byte[] voucher, uint voucherLength);
}
