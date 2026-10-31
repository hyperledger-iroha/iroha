//! Apply the SDK's explicitly selected HTTP trust-root profile.
//!
//! Reqwest 0.13 uses the platform verifier for rustls. The SDK's WebPKI-only
//! profile supplies Mozilla certificates explicitly so it remains independent of
//! the machine's trust store. Native TLS takes precedence when both backends are
//! enabled, matching reqwest's backend selection.

/// Apply the SDK TLS features to a blocking HTTP builder.
pub fn configure_blocking_client(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    #[cfg(all(
        feature = "tls-rustls-webpki-roots",
        not(any(feature = "tls-native", feature = "tls-native-vendored"))
    ))]
    {
        #[cfg(feature = "tls-rustls-native-roots")]
        return builder
            .tls_backend_rustls()
            .tls_certs_merge(mozilla_roots());
        #[cfg(not(feature = "tls-rustls-native-roots"))]
        return builder.tls_backend_rustls().tls_certs_only(mozilla_roots());
    }
    #[cfg(any(
        not(feature = "tls-rustls-webpki-roots"),
        feature = "tls-native",
        feature = "tls-native-vendored"
    ))]
    builder
}

/// Apply the SDK TLS features to an asynchronous HTTP builder.
pub fn configure_async_client(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    #[cfg(all(
        feature = "tls-rustls-webpki-roots",
        not(any(feature = "tls-native", feature = "tls-native-vendored"))
    ))]
    {
        #[cfg(feature = "tls-rustls-native-roots")]
        return builder
            .tls_backend_rustls()
            .tls_certs_merge(mozilla_roots());
        #[cfg(not(feature = "tls-rustls-native-roots"))]
        return builder.tls_backend_rustls().tls_certs_only(mozilla_roots());
    }
    #[cfg(any(
        not(feature = "tls-rustls-webpki-roots"),
        feature = "tls-native",
        feature = "tls-native-vendored"
    ))]
    builder
}

#[cfg(all(
    feature = "tls-rustls-webpki-roots",
    not(any(feature = "tls-native", feature = "tls-native-vendored"))
))]
fn mozilla_roots() -> impl Iterator<Item = reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().map(|root| {
        reqwest::Certificate::from_der(root.as_ref())
            .expect("Mozilla root certificates contain valid DER")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_profile_builds_with_current_certificate_bundle() {
        configure_blocking_client(reqwest::blocking::Client::builder())
            .build()
            .expect("supported blocking TLS profile");
    }

    #[test]
    fn asynchronous_profile_builds_with_current_certificate_bundle() {
        configure_async_client(reqwest::Client::builder())
            .build()
            .expect("supported asynchronous TLS profile");
    }

    #[cfg(all(
        feature = "tls-rustls-webpki-roots",
        not(any(feature = "tls-native", feature = "tls-native-vendored"))
    ))]
    #[test]
    fn bundled_roots_are_nonempty_and_all_parse() {
        assert!(mozilla_roots().count() > 0);
    }
}
