//! Reproduces the domain-id truncation observed in `SignedBlock` roundtrip tests.

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use iroha_data_model::account::{
    AccountDomainSelector, AccountId, clear_account_domain_selector_resolver,
    set_account_domain_selector_resolver,
};
use iroha_data_model::domain::DomainId;
use norito::NoritoDeserialize;

struct DomainSelectorGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for DomainSelectorGuard {
    fn drop(&mut self) {
        clear_account_domain_selector_resolver();
    }
}

fn guard_domain_selector_resolver(domain: &DomainId) -> DomainSelectorGuard {
    static DOMAIN_SELECTOR_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    let lock = DOMAIN_SELECTOR_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let selector = AccountDomainSelector::from_domain(domain).expect("domain selector");
    let resolver_domain = domain.clone();
    set_account_domain_selector_resolver(Arc::new(move |candidate| {
        if candidate == &selector {
            Some(resolver_domain.clone())
        } else {
            None
        }
    }));
    DomainSelectorGuard { _lock: lock }
}

#[test]
fn account_id_roundtrip_via_codec() {
    let domain: DomainId = "wonderland".parse().expect("valid domain");
    let _guard = guard_domain_selector_resolver(&domain);
    let id: AccountId =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"
            .parse()
            .expect("valid account id");

    let framed = norito::to_bytes(&id).expect("encode account id");
    let archived = norito::core::from_bytes::<AccountId>(&framed).expect("decode via header");
    let header_decoded = AccountId::deserialize(archived);

    let decoded = norito::core::decode_from_bytes::<AccountId>(&framed).expect("decode payload");

    assert_eq!(header_decoded, id);
    assert_eq!(decoded, id);
}

#[cfg(feature = "gost")]
#[test]
fn account_id_roundtrip_supports_gost_public_key() {
    use iroha_crypto::{Algorithm, KeyPair};

    let domain: DomainId = "wonderland".parse().expect("valid domain");
    let _guard = guard_domain_selector_resolver(&domain);
    let seed = b"iroha-gost-account-id";
    let key_pair = KeyPair::from_seed(seed.to_vec(), Algorithm::Gost3410_2012_256ParamSetA);
    let expected_public = "80244058C5EBFD184A832A76C01D0EEAEF02C1D276BAA0372A3F345C71BCDE6E221791EFBBA233FD0D2F0F9B75B0BC3579D58632815ABE18E6747E6B180F2EDF1CCC55";
    assert_eq!(key_pair.public_key().to_string(), expected_public);
    let expected_private = "8c2620A63E89C1BEDC1AA2784193F595010BB9EC1FF8104665C871B1AF17738A7269BA";
    assert_eq!(
        iroha_crypto::ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        expected_private
    );
    let account_string = format!("{expected_public}@wonderland");

    let id: AccountId = account_string.parse().expect("parse GOST account id");
    let rendered = id.to_string();
    let (_address, format) = iroha_data_model::account::AccountAddress::parse_any(&rendered, None)
        .expect("IH58 encoding should parse");
    assert!(
        matches!(
            format,
            iroha_data_model::account::AccountAddressFormat::IH58 { .. }
        ),
        "expected IH58 rendering, got {format:?}"
    );

    let framed = norito::to_bytes(&id).expect("encode account id");
    let decoded = norito::core::decode_from_bytes::<AccountId>(&framed).expect("decode payload");
    assert_eq!(decoded, id);
}
