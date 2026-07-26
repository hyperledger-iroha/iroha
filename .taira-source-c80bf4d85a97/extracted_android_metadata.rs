        struct AndroidFixtureMetadata {
            package_name: &'static str,
            signing_digest: [u8; 32],
        }

        impl Default for AndroidFixtureMetadata {
            fn default() -> Self {
                Self {
                    package_name: ANDROID_PACKAGE,
                    signing_digest: ANDROID_SIGNING_DIGEST,
                }
            }
        }

        impl AndroidFixtureMetadata {
            fn package_names(&self) -> Vec<String> {
                vec![self.package_name.to_string()]
            }

            fn signing_digests_hex(&self) -> Vec<String> {
                vec![hex::encode(self.signing_digest)]
            }
        }