//! Immutable application trust shared by runtime admission and Android packaging.

include!(concat!(
    env!("OUT_DIR"),
    "/kagemusha_wallet_runtime_trust.rs"
));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "The immutable native build selects one configured deployment authority; unselected variants remain for other builds."
)]
#[repr(u8)]
pub(super) enum RuntimeAuthority {
    BpngTairaV7 = 1,
    BoiTairaV1 = 2,
    CbsiReleaseV1 = 3,
    ApplicationReleaseV1 = 4,
}

/// Android packagers read the authority tag and Ed25519 key from the same immutable
/// value that authenticates the application manifest. Tag zero denotes no installed trust.
#[repr(C)]
struct EncodedRuntimeTrust {
    authority: u8,
    public_key: [u8; 32],
}

const _: () = assert!(size_of::<EncodedRuntimeTrust>() == 33);

#[used]
#[cfg_attr(target_os = "android", unsafe(link_section = ".iroha.wallet_trust.v1"))]
#[cfg_attr(
    target_os = "android",
    unsafe(export_name = "iroha_wallet_runtime_trust_v1")
)]
static ENCODED_RUNTIME_TRUST: EncodedRuntimeTrust = match INSTALLED_RUNTIME_TRUST {
    Some((authority, public_key)) => EncodedRuntimeTrust {
        authority: authority as u8,
        public_key,
    },
    None => EncodedRuntimeTrust {
        authority: 0,
        public_key: [0; 32],
    },
};

pub(super) fn installed() -> Option<(RuntimeAuthority, &'static [u8; 32])> {
    let authority = match ENCODED_RUNTIME_TRUST.authority {
        1 => RuntimeAuthority::BpngTairaV7,
        2 => RuntimeAuthority::BoiTairaV1,
        3 => RuntimeAuthority::CbsiReleaseV1,
        4 => RuntimeAuthority::ApplicationReleaseV1,
        _ => return None,
    };
    Some((authority, &ENCODED_RUNTIME_TRUST.public_key))
}
