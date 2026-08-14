//! Versioned pre/post-`z` cross-field binding contract.
//!
//! The parent V2 prerequisite hashed challenge-independent and inverse points
//! together.  This additive child separates those roots so the sole global
//! lookup `z` may be derived without a dependency cycle.  Production entry
//! remains uninhabited and this module constructs no point, witness, proof, or
//! authority.

use super::*;

const JOINT_Z_BINDING_VERSION_V3: u8 = 3;
const PRE_Z_BINDING_DOMAIN_V3: &[u8] = b"iroha.zk-ams.v3.phase23.cross-field.pre-z-binding\0";
const POST_Z_BINDING_DOMAIN_V3: &[u8] = b"iroha.zk-ams.v3.phase23.cross-field.post-z-binding\0";
const PRE_Z_ORDER_V3: &[u8] = b"fixed-axes,source-manifest,source-receipt,source-formula,source-mapping,terminal-binding,radix-pre-z,qpcs-parameter,existing-d-root,added-pre-z-root";
const POST_Z_ORDER_V3: &[u8] = b"pre-z-binding,shared-existing-inverse-root,added-inverse-root,alias-map-root,global-inverse-root";

const EXISTING_D_POINTS_V3: u32 = 6_192;
const EXISTING_S_POINTS_V3: u32 = 6_192;
const SOURCE_POINTS_V3: u32 = 344;
const PRE_Z_SCALAR_COMMITMENTS_V3: u32 = 2;
const COMPARATOR_PRE_Z_POINTS_V3: u32 = 12_384;
const SMALL_SOURCE_PRE_Z_POINTS_V3: u32 = 2_064;
const Q_MASK_PRE_Z_POINTS_V3: u32 = 12_160;
const ADDED_PRE_Z_POINTS_V3: u32 = 26_608;
const CHALLENGE_INDEPENDENT_POINTS_V3: u32 = 39_338;
const SHARED_EXISTING_INVERSE_POINTS_V3: u32 = 11_696;
const COMPARATOR_POST_Z_POINTS_V3: u32 = 5_848;
const SMALL_SOURCE_POST_Z_POINTS_V3: u32 = 2_064;
const Q_MASK_POST_Z_POINTS_V3: u32 = 12_160;
const ADDED_POST_Z_POINTS_V3: u32 = 20_072;
const GLOBAL_INVERSE_POINTS_V3: u32 = 31_768;
const POST_Z_PHYSICAL_POINTS_V3: u32 = 31_768;
const POST_DELTA_POINTS_V3: u32 = 3;
const DENSE_PHYSICAL_INVENTORY_V3: u32 = 71_109;

const PRE_Z_BINDING_INHABITED_V3: bool = false;
const POST_Z_BINDING_INHABITED_V3: bool = false;
const CROSS_FIELD_PROOF_VERIFIED_V3: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V3: bool = false;
const COMPLETE_ACCOUNTING_QUALIFIED_V3: bool = false;
const AUTHORITY_MINTED_V3: bool = false;
const RSS_QUALIFIED_V3: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V3: bool = false;
const RELEASE_READY_V3: bool = false;

const _: () = {
    assert!(EXISTING_D_POINTS_V3 == 344 * 18);
    assert!(COMPARATOR_PRE_Z_POINTS_V3 == 344 * (17 + 1 + 18));
    assert!(SMALL_SOURCE_PRE_Z_POINTS_V3 == 1_032 * 2);
    assert!(Q_MASK_PRE_Z_POINTS_V3 == 1_520 * 8);
    assert!(ADDED_PRE_Z_POINTS_V3 == 26_608);
    assert!(
        CHALLENGE_INDEPENDENT_POINTS_V3
            == SOURCE_POINTS_V3
                + EXISTING_D_POINTS_V3
                + EXISTING_S_POINTS_V3
                + ADDED_PRE_Z_POINTS_V3
                + PRE_Z_SCALAR_COMMITMENTS_V3
    );
    assert!(SHARED_EXISTING_INVERSE_POINTS_V3 == 2 * 5_848);
    assert!(ADDED_POST_Z_POINTS_V3 == 5_848 + 2_064 + 12_160);
    assert!(GLOBAL_INVERSE_POINTS_V3 == 11_696 + 20_072);
    assert!(
        DENSE_PHYSICAL_INVENTORY_V3
            == CHALLENGE_INDEPENDENT_POINTS_V3 + POST_Z_PHYSICAL_POINTS_V3 + POST_DELTA_POINTS_V3
    );
    assert!(!PRE_Z_BINDING_INHABITED_V3);
    assert!(!POST_Z_BINDING_INHABITED_V3);
    assert!(!CROSS_FIELD_PROOF_VERIFIED_V3);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V3);
    assert!(!COMPLETE_ACCOUNTING_QUALIFIED_V3);
    assert!(!AUTHORITY_MINTED_V3);
    assert!(!RSS_QUALIFIED_V3);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V3);
    assert!(!RELEASE_READY_V3);
};

enum CrossFieldPreZOwnerSealV3 {
    Production {
        authenticated_source: Infallible,
        canonical_radix: Infallible,
        canonical_q_mask: Infallible,
        authenticated_qpcs: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum CrossFieldPostZOwnerSealV3 {
    Production {
        shared_radix_inverses: Infallible,
        added_lookup_inverses: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct CrossFieldPreZAxesV3 {
    fixed_axes_digest: [u8; 32],
    source_manifest_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    source_formula_digest: [u8; 32],
    source_mapping_digest: [u8; 32],
    terminal_binding_digest: [u8; 32],
    radix_pre_z_binding_digest: [u8; 32],
    qpcs_parameter_digest: [u8; 32],
}

impl CrossFieldPreZAxesV3 {
    fn validate_v3(&self) -> Result<(), CrossFieldErrorV2> {
        if [
            self.fixed_axes_digest,
            self.source_manifest_digest,
            self.source_receipt_digest,
            self.source_formula_digest,
            self.source_mapping_digest,
            self.terminal_binding_digest,
            self.radix_pre_z_binding_digest,
            self.qpcs_parameter_digest,
        ]
        .contains(&[0; 32])
        {
            return Err(CrossFieldErrorV2::Context);
        }
        Ok(())
    }
}

struct CrossFieldPreZBindingV3 {
    _seal: CrossFieldPreZOwnerSealV3,
    axes: CrossFieldPreZAxesV3,
    existing_d_root: [u8; 32],
    added_pre_z_root: [u8; 32],
    binding_digest: [u8; 32],
}

impl CrossFieldPreZBindingV3 {
    fn bind_v3(
        seal: CrossFieldPreZOwnerSealV3,
        axes: CrossFieldPreZAxesV3,
        existing_d_root: [u8; 32],
        added_pre_z_root: [u8; 32],
    ) -> Result<Self, CrossFieldErrorV2> {
        axes.validate_v3()?;
        if [existing_d_root, added_pre_z_root].contains(&[0; 32]) {
            return Err(CrossFieldErrorV2::Context);
        }
        let binding_digest = pre_z_digest_v3(&axes, existing_d_root, added_pre_z_root);
        Ok(Self {
            _seal: seal,
            axes,
            existing_d_root,
            added_pre_z_root,
            binding_digest,
        })
    }

    fn validate_v3(&self) -> Result<(), CrossFieldErrorV2> {
        self.axes.validate_v3()?;
        if self.binding_digest
            != pre_z_digest_v3(&self.axes, self.existing_d_root, self.added_pre_z_root)
        {
            return Err(CrossFieldErrorV2::Context);
        }
        Ok(())
    }
}

struct CrossFieldPostZBindingV3 {
    _seal: CrossFieldPostZOwnerSealV3,
    pre_z: CrossFieldPreZBindingV3,
    shared_existing_inverse_root: [u8; 32],
    added_inverse_root: [u8; 32],
    alias_map_root: [u8; 32],
    global_inverse_root: [u8; 32],
    binding_digest: [u8; 32],
}

impl CrossFieldPostZBindingV3 {
    fn bind_v3(
        seal: CrossFieldPostZOwnerSealV3,
        pre_z: CrossFieldPreZBindingV3,
        roots: [[u8; 32]; 4],
    ) -> Result<Self, CrossFieldErrorV2> {
        pre_z.validate_v3()?;
        if roots.contains(&[0; 32]) {
            return Err(CrossFieldErrorV2::Context);
        }
        let binding_digest = post_z_digest_v3(pre_z.binding_digest, roots);
        Ok(Self {
            _seal: seal,
            pre_z,
            shared_existing_inverse_root: roots[0],
            added_inverse_root: roots[1],
            alias_map_root: roots[2],
            global_inverse_root: roots[3],
            binding_digest,
        })
    }

    fn validate_v3(&self) -> Result<(), CrossFieldErrorV2> {
        self.pre_z.validate_v3()?;
        let roots = [
            self.shared_existing_inverse_root,
            self.added_inverse_root,
            self.alias_map_root,
            self.global_inverse_root,
        ];
        if roots.contains(&[0; 32])
            || self.binding_digest != post_z_digest_v3(self.pre_z.binding_digest, roots)
        {
            return Err(CrossFieldErrorV2::Context);
        }
        Ok(())
    }
}

fn pre_z_digest_v3(
    axes: &CrossFieldPreZAxesV3,
    existing_d_root: [u8; 32],
    added_pre_z_root: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(PRE_Z_BINDING_DOMAIN_V3);
    hash.update(&[JOINT_Z_BINDING_VERSION_V3]);
    hash.update(PRE_Z_ORDER_V3);
    for digest in [
        axes.fixed_axes_digest,
        axes.source_manifest_digest,
        axes.source_receipt_digest,
        axes.source_formula_digest,
        axes.source_mapping_digest,
        axes.terminal_binding_digest,
        axes.radix_pre_z_binding_digest,
        axes.qpcs_parameter_digest,
        existing_d_root,
        added_pre_z_root,
    ] {
        hash.update(&digest);
    }
    for count in [
        EXISTING_D_POINTS_V3,
        COMPARATOR_PRE_Z_POINTS_V3,
        SMALL_SOURCE_PRE_Z_POINTS_V3,
        Q_MASK_PRE_Z_POINTS_V3,
        ADDED_PRE_Z_POINTS_V3,
        CHALLENGE_INDEPENDENT_POINTS_V3,
    ] {
        hash.update(&count.to_be_bytes());
    }
    hash.finalize()
}

fn post_z_digest_v3(pre_z_digest: [u8; 32], roots: [[u8; 32]; 4]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(POST_Z_BINDING_DOMAIN_V3);
    hash.update(&[JOINT_Z_BINDING_VERSION_V3]);
    hash.update(POST_Z_ORDER_V3);
    hash.update(&pre_z_digest);
    for root in roots {
        hash.update(&root);
    }
    for count in [
        SHARED_EXISTING_INVERSE_POINTS_V3,
        COMPARATOR_POST_Z_POINTS_V3,
        SMALL_SOURCE_POST_Z_POINTS_V3,
        Q_MASK_POST_Z_POINTS_V3,
        ADDED_POST_Z_POINTS_V3,
        GLOBAL_INVERSE_POINTS_V3,
        DENSE_PHYSICAL_INVENTORY_V3,
    ] {
        hash.update(&count.to_be_bytes());
    }
    hash.finalize()
}

#[cfg(test)]
#[path = "joint_z_binding_v3_tests.rs"]
mod tests;
