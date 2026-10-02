//! Exact first-release signed HTTPS directory bases for inventory and public clock transports.
//! This parser admits URL shape only; installed signed authority still selects every endpoint.
use eyre::{Result, ensure};
use url::Url;

/// Parse only the exact canonical original; never normalize a signed endpoint or add a slash.
/// Root `/` and nonempty ASCII-unreserved directory segments are accepted. Route joins keep
/// this complete prefix, while credentials, query/fragment, percent escapes and dot segments fail.
pub(super) fn require_https_directory_base(original: &str) -> Result<Url> {
    let url = Url::parse(original)?;
    ensure!(
        url.scheme() == "https"
            && url.host().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.as_str() == original,
        "Native HTTPS directory base changed canonical original"
    );
    let path = url.path();
    let directory = if path == "/" {
        true
    } else {
        path.strip_prefix('/')
            .and_then(|path| path.strip_suffix('/'))
            .is_some_and(|path| {
                path.split('/').all(|segment| {
                    !segment.is_empty()
                        && segment != "."
                        && segment != ".."
                        && segment.bytes().all(|byte| {
                            byte.is_ascii_alphanumeric()
                                || matches!(byte, b'-' | b'.' | b'_' | b'~')
                        })
                })
            })
    };
    ensure!(directory, "Native HTTPS directory base path rejected");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::join_torii_url;

    // These are public URL data vectors only, never installed network or financial authority.
    #[test]
    fn native_directory_base_accepts_exact_root_and_unreserved_directories() {
        for original in [
            "https://node.example/",
            "https://node.example:8443/api/",
            "https://node.example/.well-known/role_1/v1.0/~peer-1/",
            "https://[::1]:8443/api/",
        ] {
            assert_eq!(
                require_https_directory_base(original).unwrap().as_str(),
                original
            );
        }
        for role in 1..=4 {
            let original = format!("https://taira.sora.org/bpng-validators/{role}/");
            assert_eq!(
                require_https_directory_base(&original).unwrap().as_str(),
                original
            );
        }
    }

    #[test]
    fn native_directory_base_rejects_originals_that_url_parse_would_normalize() {
        for original in [
            "HTTPS://node.example/",
            "https://NODE.example/",
            "https://node.example:443/",
            "https://node.example",
            "https://node.example:/",
            "https://node.example/api/./",
            "https://node.example/api/../",
            "https://node.example/api/%2e/",
            "https://node.example/api/%2E%2E/",
            "https://node.example/api\\nested/",
            " https://node.example/api/",
            "https://node.example/api/\n",
        ] {
            assert!(
                require_https_directory_base(original).is_err(),
                "{original:?}"
            );
        }
    }

    #[test]
    fn native_directory_base_rejects_ambiguous_or_non_directory_paths() {
        for original in [
            "http://node.example/",
            "https://user@node.example/",
            "https://user:password@node.example/",
            "https://@node.example/",
            "https://node.example/api/?x=1",
            "https://node.example/api/?",
            "https://node.example/api/#part",
            "https://node.example/api/#",
            "https://node.example/api",
            "https://node.example//",
            "https://node.example/api//nested/",
            "https://node.example/%7Epeer/",
            "https://node.example/api%2fnested/",
            "https://node.example/api%5cnested/",
            "https://node.example/a;b/",
            "https://node.example/a:b/",
            "https://node.example/é/",
        ] {
            assert!(
                require_https_directory_base(original).is_err(),
                "{original:?}"
            );
        }
    }

    #[test]
    fn native_directory_base_keeps_each_approved_role_prefix_for_actual_torii_join() {
        for role in 1..=4 {
            let original = format!("https://taira.sora.org/bpng-validators/{role}/");
            let base = require_https_directory_base(&original).unwrap();
            for route in [
                "v1/bridge/finality/attestation/41",
                "/v1/bridge/finality/attestation/41",
                "v1/bridge/finality/41",
            ] {
                let joined = join_torii_url(&base, route);
                assert_eq!(
                    joined.as_str(),
                    format!("{original}{}", route.trim_start_matches('/'))
                );
                // The same relative join used by public clock retains the exact mount as well.
                assert_eq!(base.join(route.trim_start_matches('/')).unwrap(), joined);
            }
        }
    }

    #[test]
    fn native_directory_base_keeps_root_and_current_wallet_control_prefixes() {
        let root = require_https_directory_base("https://node.example/").unwrap();
        assert_eq!(
            join_torii_url(&root, "/v1/bridge/finality/41").as_str(),
            "https://node.example/v1/bridge/finality/41"
        );
        let base = require_https_directory_base("https://fi.example/bpng-fi/").unwrap();
        let route = "/v1/kagemusha/enrollment/ordinary/current-control";
        assert_eq!(
            join_torii_url(&base, route).as_str(),
            "https://fi.example/bpng-fi/v1/kagemusha/enrollment/ordinary/current-control"
        );
        // BPNG's maintained Core HTTP adapter concatenates its fixed leading-slash route.
        let product_url =
            Url::parse(&format!("{}{}", base.as_str().trim_end_matches('/'), route)).unwrap();
        assert_eq!(product_url, join_torii_url(&base, route));
    }
}
