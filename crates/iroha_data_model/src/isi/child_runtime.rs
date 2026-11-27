//! Selected, certificate-authenticated administration of one independently founded child root.

use super::*;

isi! {
    /// Consume one current administrator threshold certificate and its exact replay nonce.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::child_runtime::AuthorizeChildRuntimeActionV1")]
    pub struct AuthorizeChildRuntimeActionV1 {
        /// Exact root, policy, validity, nonce, action and distinct administrator signatures.
        pub certificate: crate::runtime::child_admin::ChildRuntimeAdminCertificateV1,
    }
}
isi! {
    /// Record independent validator release support for an already admin-scheduled child change.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::child_runtime::AttestChildRuntimeSupportV1")]
    pub struct AttestChildRuntimeSupportV1 {
        /// Exact existing validator-support contract; administrator signatures cannot replace it.
        pub attestation: crate::runtime::lifecycle::SignedSupportAttestationV1,
    }
}

impl crate::seal::Instruction for AuthorizeChildRuntimeActionV1 {}
impl crate::seal::Instruction for AttestChildRuntimeSupportV1 {}

impl AuthorizeChildRuntimeActionV1 {
    /// Immutable wire identity, executable only through the selected child-admin addition.
    pub const WIRE_ID: &'static str = "iroha.runtime.child_admin.authorize.v1";
}
impl AttestChildRuntimeSupportV1 {
    /// Immutable wire identity, executable only through the selected child-admin addition.
    pub const WIRE_ID: &'static str = "iroha.runtime.child_admin.attest.v1";
}
