pub mod musubi {
    //! First-release Musubi registry query definitions.
    use std::fmt;
    use crate::musubi::{
        MusubiAliasQueryV1, MusubiArchiveLocationQueryV1, MusubiArchiveRetentionQueryV1,
        MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1, MusubiOrderedPrefixQueryV1,
        MusubiPackagePageQueryV1, MusubiProviderBundleAttestationKeyV1, MusubiResolverIndexQueryV1,
    };
    pub use self::model::*;
    #[iroha_data_model_derive::model]
    mod model {
        use super::*;
        use norito::codec::{Decode, Encode};
        /// Fetch one exact authoritative Musubi V1 package record.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiExactPackageV1 {
            /// Exact structural package request.
            pub request: MusubiExactPackageQueryV1,
        }
        /// Fetch one paired finalized Musubi V1 home/universal release view.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiExactReleaseV1 {
            /// Exact structural release request.
            pub request: MusubiExactReleaseQueryV1,
        }
        /// Fetch one exact immutable Musubi V1 provider bundle-attestation audit record.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiProviderBundleAttestationV1 {
            /// Exact archive/order/provider attestation key.
            pub key: MusubiProviderBundleAttestationKeyV1,
        }
        /// Fetch a finalized page from the universal Musubi V1 resolver index.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiResolverIndexV1 {
            /// Package, optional requirement, and finalized page controls.
            pub request: MusubiResolverIndexQueryV1,
        }
        /// Fetch a finalized page of structured Musubi V1 package versions.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiVersionsV1 {
            /// Package-scoped finalized page request.
            pub request: MusubiPackagePageQueryV1,
        }
        /// Fetch a finalized page of accepted Musubi V1 package members.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiMaintainersV1 {
            /// Package-scoped finalized page request.
            pub request: MusubiPackagePageQueryV1,
        }
        /// Fetch a finalized page of renewable Musubi V1 archive locations.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiArchiveLocationsV1 {
            /// Archive-scoped finalized page request.
            pub request: MusubiArchiveLocationQueryV1,
        }
        /// Fetch exact finalized cache-retention decisions for a bounded archive batch.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiArchiveRetentionV1 {
            /// Exact archive identities and optional finalized-snapshot binding.
            pub request: MusubiArchiveRetentionQueryV1,
        }
        /// Fetch one exact permanent Musubi V1 global alias record.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiAliasV1 {
            /// Exact alias request; page controls are ignored for this lookup.
            pub request: MusubiAliasQueryV1,
        }
        /// Fetch a finalized page of permanent Musubi V1 alias history.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiAliasHistoryV1 {
            /// Alias-scoped finalized page request.
            pub request: MusubiAliasQueryV1,
        }
        /// Fetch a finalized byte-ordered prefix page from the Musubi V1 directory.
        #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
        #[derive(derive_more::Constructor, iroha_schema::IntoSchema)]
        #[repr(transparent)]
        pub struct FindMusubiOrderedPrefixV1 {
            /// Ordered structural prefix and finalized page controls.
            pub request: MusubiOrderedPrefixQueryV1,
        }
    }
    impl fmt::Display for FindMusubiExactPackageV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find exact Musubi V1 package `{}`",
                self.request.package
            )
        }
    }
    impl fmt::Display for FindMusubiExactReleaseV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find exact Musubi V1 release `{}`",
                self.request.release
            )
        }
    }
    impl fmt::Display for FindMusubiProviderBundleAttestationV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find exact Musubi V1 provider bundle attestation for archive `{:?}`, order `{:?}`, provider `{:?}`",
                self.key.archive_id, self.key.replication_order, self.key.provider_id
            )
        }
    }
    impl fmt::Display for FindMusubiResolverIndexV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 resolver rows for `{}`",
                self.request.package
            )
        }
    }
    impl fmt::Display for FindMusubiVersionsV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 versions for `{}`",
                self.request.package
            )
        }
    }
    impl fmt::Display for FindMusubiMaintainersV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 maintainers for `{}`",
                self.request.package
            )
        }
    }
    impl fmt::Display for FindMusubiArchiveLocationsV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 archive locations for `{:?}`",
                self.request.archive_id
            )
        }
    }
    impl fmt::Display for FindMusubiArchiveRetentionV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 retention for {} exact archive(s)",
                self.request.archive_ids.len()
            )
        }
    }
    impl fmt::Display for FindMusubiAliasV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find exact Musubi V1 alias `{}`",
                self.request.alias
            )
        }
    }
    impl fmt::Display for FindMusubiAliasHistoryV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 alias history for `{}`",
                self.request.alias
            )
        }
    }
    impl fmt::Display for FindMusubiOrderedPrefixV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "Find Musubi V1 packages with prefix `{}`",
                self.request.prefix.as_str()
            )
        }
    }
    /// Prelude re-exports for the complete Musubi V1 query surface.
    pub mod prelude {
        pub use super::{
            FindMusubiAliasHistoryV1, FindMusubiAliasV1, FindMusubiArchiveLocationsV1,
            FindMusubiArchiveRetentionV1, FindMusubiExactPackageV1, FindMusubiExactReleaseV1,
            FindMusubiMaintainersV1, FindMusubiOrderedPrefixV1,
            FindMusubiProviderBundleAttestationV1, FindMusubiResolverIndexV1, FindMusubiVersionsV1,
        };
    }
    #[cfg(test)]
    mod tests {
        use norito::codec::{Decode, Encode};
        use super::prelude::*;
        use crate::{
            musubi::{
                ArchiveId, MusubiAliasNameV1, MusubiAliasQueryV1, MusubiArchiveLocationPageV1,
                MusubiArchiveLocationQueryV1, MusubiArchiveRetentionPageV1,
                MusubiArchiveRetentionQueryV1, MusubiExactPackageQueryV1,
                MusubiExactReleaseQueryV1, MusubiExactReleaseSnapshotV1, MusubiMaintainerPageV1,
                MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1,
                MusubiPackageIdV1, MusubiPackageNameV1, MusubiPackagePageQueryV1,
                MusubiPackageRecordV1, MusubiPackageScopeV1, MusubiPageRequestV1,
                MusubiProviderBundleAttestationKeyV1, MusubiProviderBundleAttestationRecordV1,
                MusubiReleaseIdV1, MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1,
                MusubiVersionPageV1, MusubiVersionV1,
            },
            nexus::DataSpaceId,
            query::{SingularQuery, SingularQueryBox, SingularQueryOutputBox},
            sorafs::{capacity::ProviderId, pin_registry::ReplicationOrderId},
        };
        fn package() -> MusubiPackageIdV1 {
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                MusubiPackageNameV1::new("ledger-tools").expect("package name"),
            )
        }
        fn page() -> MusubiPageRequestV1 {
            MusubiPageRequestV1 {
                limit: 50,
                cursor: None,
            }
        }
        #[test]
        fn v1_singular_query_payloads_roundtrip_in_registry_order() {
            let package = package();
            let release = MusubiReleaseIdV1::new(
                package.clone(),
                "1.2.3".parse::<MusubiVersionV1>().expect("version"),
            );
            let alias = "ledger".parse::<MusubiAliasNameV1>().expect("alias");
            let queries: Vec<SingularQueryBox> = vec![
                FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
                    package: package.clone(),
                })
                .into(),
                FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 { release }).into(),
                FindMusubiProviderBundleAttestationV1::new(MusubiProviderBundleAttestationKeyV1 {
                    archive_id: ArchiveId::new([0xA4; 32]),
                    replication_order: ReplicationOrderId::new([0xA5; 32]),
                    provider_id: ProviderId::new([0xA6; 32]),
                })
                .into(),
                FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
                    package: package.clone(),
                    requirement: None,
                    page: page(),
                })
                .into(),
                FindMusubiVersionsV1::new(MusubiPackagePageQueryV1 {
                    package: package.clone(),
                    page: page(),
                })
                .into(),
                FindMusubiMaintainersV1::new(MusubiPackagePageQueryV1 {
                    package,
                    page: page(),
                })
                .into(),
                FindMusubiArchiveLocationsV1::new(MusubiArchiveLocationQueryV1 {
                    archive_id: ArchiveId::new([0xA5; 32]),
                    page: page(),
                })
                .into(),
                FindMusubiArchiveRetentionV1::new(MusubiArchiveRetentionQueryV1 {
                    archive_ids: vec![ArchiveId::new([0xA5; 32])],
                    expected_snapshot: None,
                })
                .into(),
                FindMusubiAliasV1::new(MusubiAliasQueryV1 {
                    alias: alias.clone(),
                    page: page(),
                })
                .into(),
                FindMusubiAliasHistoryV1::new(MusubiAliasQueryV1 {
                    alias,
                    page: page(),
                })
                .into(),
                FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
                    prefix: MusubiOrderedPrefixV1::new("sora/").expect("ordered prefix"),
                    page: page(),
                })
                .into(),
            ];
            for query in queries {
                let encoded = query.encode();
                let mut bytes = encoded.as_slice();
                let decoded = SingularQueryBox::decode(&mut bytes).expect("decode Musubi query");
                assert!(bytes.is_empty(), "decoder must consume the whole query");
                assert_eq!(decoded, query);
            }
        }
        #[test]
        fn v1_queries_bind_their_exact_response_records_and_output_variants() {
            fn assert_query_output<Q, O>()
            where
                Q: SingularQuery<Output = O>,
            {
            }
            fn assert_output_variant<O: Into<SingularQueryOutputBox>>() {}
            assert_query_output::<FindMusubiExactPackageV1, MusubiPackageRecordV1>();
            assert_query_output::<FindMusubiExactReleaseV1, MusubiExactReleaseSnapshotV1>();
            assert_query_output::<
                FindMusubiProviderBundleAttestationV1,
                MusubiProviderBundleAttestationRecordV1,
            >();
            assert_query_output::<FindMusubiResolverIndexV1, MusubiResolverIndexPageV1>();
            assert_query_output::<FindMusubiVersionsV1, MusubiVersionPageV1>();
            assert_query_output::<FindMusubiMaintainersV1, MusubiMaintainerPageV1>();
            assert_query_output::<FindMusubiArchiveLocationsV1, MusubiArchiveLocationPageV1>();
            assert_query_output::<FindMusubiArchiveRetentionV1, MusubiArchiveRetentionPageV1>();
            assert_query_output::<FindMusubiAliasV1, crate::musubi::MusubiAliasRecordV1>();
            assert_query_output::<FindMusubiAliasHistoryV1, crate::musubi::MusubiAliasHistoryPageV1>(
            );
            assert_query_output::<FindMusubiOrderedPrefixV1, MusubiOrderedPackagePageV1>();
            assert_output_variant::<MusubiPackageRecordV1>();
            assert_output_variant::<MusubiExactReleaseSnapshotV1>();
            assert_output_variant::<MusubiProviderBundleAttestationRecordV1>();
            assert_output_variant::<MusubiResolverIndexPageV1>();
            assert_output_variant::<MusubiVersionPageV1>();
            assert_output_variant::<MusubiMaintainerPageV1>();
            assert_output_variant::<MusubiArchiveLocationPageV1>();
            assert_output_variant::<MusubiArchiveRetentionPageV1>();
            assert_output_variant::<crate::musubi::MusubiAliasRecordV1>();
            assert_output_variant::<crate::musubi::MusubiAliasHistoryPageV1>();
            assert_output_variant::<MusubiOrderedPackagePageV1>();
        }
        #[cfg(feature = "json")]
        #[test]
        fn v1_query_wrapper_json_rejects_unknown_fields() {
            let query =
                FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 { package: package() });
            let canonical = norito::json::to_json(&query)
                .expect("canonical Musubi V1 query wrapper JSON encodes");
            assert_eq!(
                norito::json::from_json::<FindMusubiExactPackageV1>(&canonical)
                    .expect("canonical Musubi V1 query wrapper JSON decodes"),
                query
            );
            let hostile = canonical.replacen('{', "{\"private_key\":\"must-not-be-accepted\",", 1);
            assert!(
                norito::json::from_json::<FindMusubiExactPackageV1>(&hostile).is_err(),
                "Musubi V1 query wrapper JSON must reject unknown secret-bearing fields"
            );
        }
    }
}
