pub mod privacy {
    //! Authenticated privacy-state query definitions.

    use derive_more::Display;

    queries! {
        /// Find finalized provenance for one consumed ZK-ACE replay nullifier.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized ZK-ACE replay nullifier `{replay_nullifier:?}` for policy `{policy_id:?}`"
        )]
        pub struct FindPrivacyZkAceReplayNullifierV1 {
            /// Stable governed ZK-ACE policy lineage.
            pub policy_id: crate::privacy::PrivacyPolicyIdV1,
            /// Exact replay nullifier whose consumption must be proven.
            pub replay_nullifier: crate::privacy::PrivacyNullifierV1,
        }

        /// Find finalized typed state for one FCMP++, private-IVM, or PQ-MASP pool.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized `{protocol_id:?}` proof-managed pool state for `{pool_id:?}`"
        )]
        pub struct FindPrivacyProofManagedPoolStateV1 {
            /// Exact proof-managed protocol namespace.
            pub protocol_id: crate::privacy::PrivacyProtocolIdV1,
            /// Stable pool identity within that protocol.
            pub pool_id: crate::privacy::PrivacyPoolIdV1,
        }

        /// Find finalized typed state for one governed Orchard pool.
        #[derive(Copy, Display)]
        #[display("Find finalized Orchard pool state for `{pool_id:?}`")]
        pub struct FindPrivacyOrchardPoolStateV1 {
            /// Stable pool identity inside the Orchard namespace.
            pub pool_id: crate::privacy::PrivacyPoolIdV1,
        }

        /// Find finalized provenance for one consumed pool-scoped Orchard nullifier.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized Orchard nullifier `{nullifier:?}` for pool `{pool_id:?}`"
        )]
        pub struct FindPrivacyOrchardNullifierV1 {
            /// Stable pool identity inside the Orchard namespace.
            pub pool_id: crate::privacy::PrivacyPoolIdV1,
            /// Exact canonical Orchard nullifier bytes.
            pub nullifier: [u8; 32],
        }

        /// Find finalized bounded state for one Anonymous PGC pool.
        #[derive(Copy, Display)]
        #[display("Find finalized Anonymous PGC pool state for `{pool_id:?}`")]
        pub struct FindPrivacyAnonymousPgcPoolStateV1 {
            /// Stable pool identity inside the Anonymous PGC namespace.
            pub pool_id: crate::privacy::PrivacyPoolIdV1,
        }

        /// Find finalized provenance for one admitted ZK-AMS PHC anchor.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized ZK-AMS admission `{phc_hash:?}` for issuer `{issuer_id:?}`, registry `{registry_id:?}`, policy `{policy_id:?}`"
        )]
        pub struct FindPrivacyZkAmsAdmissionV1 {
            /// Credential issuer governing the admission relation.
            pub issuer_id: crate::privacy::PrivacyIssuerIdV1,
            /// Admitted-identity registry containing the PHC anchor.
            pub registry_id: crate::privacy::PrivacyZkAmsRegistryIdV1,
            /// Governed admission policy.
            pub policy_id: crate::privacy::PrivacyPolicyIdV1,
            /// Exact canonical PHC hash to resolve.
            pub phc_hash: crate::privacy::PrivacyZkAmsPhcHashV1,
        }

        /// Find finalized provenance for one ZK-AMS account provisioning.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized ZK-AMS provision `{key_image:?}` for issuer `{issuer_id:?}`, registry `{registry_id:?}`, policy `{policy_id:?}`"
        )]
        pub struct FindPrivacyZkAmsProvisionV1 {
            /// Credential issuer governing the provisioning relation.
            pub issuer_id: crate::privacy::PrivacyIssuerIdV1,
            /// Admitted-identity registry used by the LSAG ring.
            pub registry_id: crate::privacy::PrivacyZkAmsRegistryIdV1,
            /// Governed admission policy.
            pub policy_id: crate::privacy::PrivacyPolicyIdV1,
            /// Exact consumed LSAG key image to resolve.
            pub key_image: crate::privacy::PrivacyZkAmsKeyImageV1,
        }

        /// Find finalized provenance for one consumed ZK-X509 certificate nullifier.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized ZK-X509 certificate nullifier `{nullifier:?}` for trust anchor `{trust_anchor_id:?}`, policy `{policy_id:?}`"
        )]
        pub struct FindPrivacyZkX509CertificateNullifierV1 {
            /// Stable trust-anchor lineage selected by the certificate proof.
            pub trust_anchor_id: crate::privacy::PrivacyIssuerIdV1,
            /// Stable certificate-policy lineage selected by the certificate proof.
            pub policy_id: crate::privacy::PrivacyPolicyIdV1,
            /// Exact consumed certificate-and-policy-derived nullifier.
            pub nullifier: crate::privacy::PrivacyNullifierV1,
        }

        /// Find the finalized native execution receipt for one Exact12 action.
        #[derive(Copy, Display)]
        #[display(
            "Find finalized `{protocol_id:?}` execution receipt for transaction `{transaction_hash:?}` action `{action_index}`"
        )]
        pub struct FindPrivacyActionExecutionReceiptV1 {
            /// Closed protocol selected by the verified proof envelope.
            pub protocol_id: crate::privacy::PrivacyProtocolIdV1,
            /// Hash of the exact signed transaction containing the action.
            pub transaction_hash: [u8; 32],
            /// Zero-based privacy-action position in the signed transaction.
            pub action_index: u32,
        }
    }

    impl FindPrivacyZkAceReplayNullifierV1 {
        /// Return the exact policy lineage that scopes the replay key.
        #[must_use]
        pub const fn policy_id(&self) -> crate::privacy::PrivacyPolicyIdV1 {
            self.policy_id
        }

        /// Return the exact replay nullifier to resolve.
        #[must_use]
        pub const fn replay_nullifier(&self) -> crate::privacy::PrivacyNullifierV1 {
            self.replay_nullifier
        }
    }

    impl FindPrivacyProofManagedPoolStateV1 {
        /// Return the exact proof-managed protocol namespace.
        #[must_use]
        pub const fn protocol_id(&self) -> crate::privacy::PrivacyProtocolIdV1 {
            self.protocol_id
        }

        /// Return the exact pool identity to resolve.
        #[must_use]
        pub const fn pool_id(&self) -> crate::privacy::PrivacyPoolIdV1 {
            self.pool_id
        }
    }

    impl FindPrivacyOrchardPoolStateV1 {
        /// Return the exact governed Orchard pool identity.
        #[must_use]
        pub const fn pool_id(&self) -> crate::privacy::PrivacyPoolIdV1 {
            self.pool_id
        }
    }

    impl FindPrivacyOrchardNullifierV1 {
        /// Return the exact governed Orchard pool identity.
        #[must_use]
        pub const fn pool_id(&self) -> crate::privacy::PrivacyPoolIdV1 {
            self.pool_id
        }

        /// Return the exact canonical Orchard nullifier bytes.
        #[must_use]
        pub const fn nullifier(&self) -> [u8; 32] {
            self.nullifier
        }
    }

    impl FindPrivacyAnonymousPgcPoolStateV1 {
        /// Return the exact governed Anonymous PGC pool identity.
        #[must_use]
        pub const fn pool_id(&self) -> crate::privacy::PrivacyPoolIdV1 {
            self.pool_id
        }
    }

    impl FindPrivacyZkAmsAdmissionV1 {
        /// Return the exact issuer/registry/policy namespace.
        #[must_use]
        pub const fn namespace_components(
            &self,
        ) -> (
            crate::privacy::PrivacyIssuerIdV1,
            crate::privacy::PrivacyZkAmsRegistryIdV1,
            crate::privacy::PrivacyPolicyIdV1,
        ) {
            (self.issuer_id, self.registry_id, self.policy_id)
        }

        /// Return the exact PHC hash to resolve.
        #[must_use]
        pub const fn phc_hash(&self) -> crate::privacy::PrivacyZkAmsPhcHashV1 {
            self.phc_hash
        }
    }

    impl FindPrivacyZkAmsProvisionV1 {
        /// Return the exact issuer/registry/policy namespace.
        #[must_use]
        pub const fn namespace_components(
            &self,
        ) -> (
            crate::privacy::PrivacyIssuerIdV1,
            crate::privacy::PrivacyZkAmsRegistryIdV1,
            crate::privacy::PrivacyPolicyIdV1,
        ) {
            (self.issuer_id, self.registry_id, self.policy_id)
        }

        /// Return the exact consumed key image to resolve.
        #[must_use]
        pub const fn key_image(&self) -> crate::privacy::PrivacyZkAmsKeyImageV1 {
            self.key_image
        }
    }

    impl FindPrivacyZkX509CertificateNullifierV1 {
        /// Return the exact trust-anchor and policy lineage.
        #[must_use]
        pub const fn namespace_components(
            &self,
        ) -> (
            crate::privacy::PrivacyIssuerIdV1,
            crate::privacy::PrivacyPolicyIdV1,
        ) {
            (self.trust_anchor_id, self.policy_id)
        }

        /// Return the exact consumed certificate nullifier.
        #[must_use]
        pub const fn nullifier(&self) -> crate::privacy::PrivacyNullifierV1 {
            self.nullifier
        }
    }

    impl FindPrivacyActionExecutionReceiptV1 {
        /// Return the closed protocol identity.
        #[must_use]
        pub const fn protocol_id(&self) -> crate::privacy::PrivacyProtocolIdV1 {
            self.protocol_id
        }

        /// Return the signed transaction hash.
        #[must_use]
        pub const fn transaction_hash(&self) -> [u8; 32] {
            self.transaction_hash
        }

        /// Return the transaction-local action index.
        #[must_use]
        pub const fn action_index(&self) -> u32 {
            self.action_index
        }
    }

    /// Prelude re-exports for authenticated privacy-state queries.
    pub mod prelude {
        pub use super::{
            FindPrivacyActionExecutionReceiptV1, FindPrivacyAnonymousPgcPoolStateV1,
            FindPrivacyOrchardNullifierV1, FindPrivacyOrchardPoolStateV1,
            FindPrivacyProofManagedPoolStateV1, FindPrivacyZkAceReplayNullifierV1,
            FindPrivacyZkAmsAdmissionV1, FindPrivacyZkAmsProvisionV1,
            FindPrivacyZkX509CertificateNullifierV1,
        };
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::query::{SingularQuery, SingularQueryBox, SingularQueryOutputBox};
        use norito::codec::{Decode, Encode};

        fn assert_output_variant<T: Into<SingularQueryOutputBox>>() {}

        #[test]
        fn zk_ace_replay_query_has_exact_typed_output_and_roundtrips() {
            fn assert_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyZkAceReplayNullifierProvenanceV1>,
            >() {
            }
            assert_output::<FindPrivacyZkAceReplayNullifierV1>();

            let query = FindPrivacyZkAceReplayNullifierV1::new(
                crate::privacy::PrivacyPolicyIdV1::new([0xA1; 32]),
                crate::privacy::PrivacyNullifierV1::new([0xB1; 32]),
            );
            let boxed = SingularQueryBox::from(query);
            let encoded_bytes = boxed.encode();
            let mut encoded = encoded_bytes.as_slice();
            let decoded = SingularQueryBox::decode(&mut encoded).expect("decode privacy query");
            assert_eq!(decoded, boxed);
            assert!(encoded.is_empty());

            assert_output_variant::<crate::privacy::PrivacyZkAceReplayNullifierProvenanceV1>();
        }

        #[test]
        fn proof_managed_pool_query_has_exact_typed_output_and_roundtrips() {
            fn assert_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyProofManagedPoolStateViewV1>,
            >() {
            }
            assert_output::<FindPrivacyProofManagedPoolStateV1>();

            let query = FindPrivacyProofManagedPoolStateV1::new(
                crate::privacy::PrivacyProtocolIdV1::PqMaspStarkV0,
                crate::privacy::PrivacyPoolIdV1::new([0xC1; 32]),
            );
            let boxed = SingularQueryBox::from(query);
            let encoded_bytes = boxed.encode();
            let mut encoded = encoded_bytes.as_slice();
            let decoded =
                SingularQueryBox::decode(&mut encoded).expect("decode proof-managed privacy query");
            assert_eq!(decoded, boxed);
            assert!(encoded.is_empty());

            assert_output_variant::<crate::privacy::PrivacyProofManagedPoolStateViewV1>();
        }

        #[test]
        fn orchard_queries_have_exact_typed_outputs_and_roundtrip() {
            fn assert_pool_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyOrchardPoolStateViewV1>,
            >() {
            }
            fn assert_nullifier_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyOrchardNullifierProvenanceV1>,
            >() {
            }
            assert_pool_output::<FindPrivacyOrchardPoolStateV1>();
            assert_nullifier_output::<FindPrivacyOrchardNullifierV1>();

            let pool_id = crate::privacy::PrivacyPoolIdV1::new([0xC2; 32]);
            let queries = [
                SingularQueryBox::from(FindPrivacyOrchardPoolStateV1::new(pool_id)),
                SingularQueryBox::from(FindPrivacyOrchardNullifierV1::new(pool_id, [0xD2; 32])),
            ];
            for boxed in queries {
                let encoded_bytes = boxed.encode();
                let mut encoded = encoded_bytes.as_slice();
                let decoded = SingularQueryBox::decode(&mut encoded)
                    .expect("decode typed Orchard privacy query");
                assert_eq!(decoded, boxed);
                assert!(encoded.is_empty());
            }

            assert_output_variant::<crate::privacy::PrivacyOrchardPoolStateViewV1>();
            assert_output_variant::<crate::privacy::PrivacyOrchardNullifierProvenanceV1>();
        }

        #[test]
        fn exact12_remaining_effect_queries_have_typed_outputs_and_roundtrip() {
            fn assert_pgc_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyAnonymousPgcPoolStateViewV1>,
            >() {
            }
            fn assert_admission_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyZkAmsAdmissionViewV1>,
            >() {
            }
            fn assert_provision_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyZkAmsProvisionViewV1>,
            >() {
            }
            fn assert_x509_output<
                Q: SingularQuery<
                    Output = crate::privacy::PrivacyZkX509CertificateNullifierProvenanceV1,
                >,
            >() {
            }
            fn assert_receipt_output<
                Q: SingularQuery<Output = crate::privacy::PrivacyActionExecutionReceiptViewV1>,
            >() {
            }
            assert_pgc_output::<FindPrivacyAnonymousPgcPoolStateV1>();
            assert_admission_output::<FindPrivacyZkAmsAdmissionV1>();
            assert_provision_output::<FindPrivacyZkAmsProvisionV1>();
            assert_x509_output::<FindPrivacyZkX509CertificateNullifierV1>();
            assert_receipt_output::<FindPrivacyActionExecutionReceiptV1>();

            let issuer_id = crate::privacy::PrivacyIssuerIdV1::new([0x41; 32]);
            let registry_id = crate::privacy::PrivacyZkAmsRegistryIdV1::new([0x42; 32]);
            let policy_id = crate::privacy::PrivacyPolicyIdV1::new([0x43; 32]);
            let queries = [
                SingularQueryBox::from(FindPrivacyAnonymousPgcPoolStateV1::new(
                    crate::privacy::PrivacyPoolIdV1::new([0x31; 32]),
                )),
                SingularQueryBox::from(FindPrivacyZkAmsAdmissionV1::new(
                    issuer_id,
                    registry_id,
                    policy_id,
                    crate::privacy::PrivacyZkAmsPhcHashV1::new([0x44; 32]),
                )),
                SingularQueryBox::from(FindPrivacyZkAmsProvisionV1::new(
                    issuer_id,
                    registry_id,
                    policy_id,
                    crate::privacy::PrivacyZkAmsKeyImageV1::new([0x45; 32]),
                )),
                SingularQueryBox::from(FindPrivacyZkX509CertificateNullifierV1::new(
                    issuer_id,
                    policy_id,
                    crate::privacy::PrivacyNullifierV1::new([0x46; 32]),
                )),
                SingularQueryBox::from(FindPrivacyActionExecutionReceiptV1::new(
                    crate::privacy::PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                    [0x47; 32],
                    0,
                )),
            ];
            for boxed in queries {
                let encoded_bytes = boxed.encode();
                let mut encoded = encoded_bytes.as_slice();
                let decoded = SingularQueryBox::decode(&mut encoded)
                    .expect("decode remaining Exact12 typed query");
                assert_eq!(decoded, boxed);
                assert!(encoded.is_empty());
            }

            assert_output_variant::<crate::privacy::PrivacyAnonymousPgcPoolStateViewV1>();
            assert_output_variant::<crate::privacy::PrivacyZkAmsAdmissionViewV1>();
            assert_output_variant::<crate::privacy::PrivacyZkAmsProvisionViewV1>();
            assert_output_variant::<crate::privacy::PrivacyZkX509CertificateNullifierProvenanceV1>(
            );
            assert_output_variant::<crate::privacy::PrivacyActionExecutionReceiptViewV1>();
        }

        #[test]
        fn finalized_replay_provenance_rejects_stale_or_zero_anchors() {
            let genesis_hash =
                iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::prehashed([0x11; 32]),
                );
            let finalized_block_hash =
                iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::prehashed([0x22; 32]),
                );
            let mut provenance = crate::privacy::PrivacyZkAceReplayNullifierProvenanceV1 {
                network_id: crate::NetworkId::from_genesis_hash(genesis_hash),
                policy_id: crate::privacy::PrivacyPolicyIdV1::new([0xA1; 32]),
                replay_nullifier: crate::privacy::PrivacyNullifierV1::new([0xB1; 32]),
                policy_record_digest: crate::privacy::PrivacyZkAcePolicyRecordDigestV1::new(
                    [0xC1; 32],
                ),
                statement_digest: crate::privacy::PrivacyStatementDigestV1::new([0xD1; 32]),
                admitted_at_height: 7,
                action_index: 0,
                finalized_height: 9,
                finalized_block_hash,
            };
            provenance
                .validate()
                .expect("canonical finalized provenance");

            provenance.finalized_height = 6;
            assert_eq!(
                provenance
                    .validate()
                    .expect_err("finality before admission must reject"),
                "ZK-ACE replay provenance finality predates marker admission"
            );
            provenance.finalized_height = 9;
            provenance.network_id = crate::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                crate::block::BlockHeader,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0; 32]),
            ));
            assert_eq!(
                provenance
                    .validate()
                    .expect_err("zero NetworkId must reject"),
                "ZK-ACE replay provenance NetworkId must be non-zero"
            );
        }
    }
}
