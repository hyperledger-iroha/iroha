//! Native witness compiler for one complete P-256 ECDSA AIR instance.
//!
//! The group and ECDSA modules describe the fixed computation generically.
//! This module is their production witness compiler: it assigns symbolic SSA
//! values, records every exact base/scalar operation, emits all 128 fixed
//! window-selector traces, and records reduction/low-s bindings.  Native P-256
//! arithmetic is used only to construct witness values.  Verification is
//! performed by the arithmetic, value-bus, window, reduction, and scalar-bit
//! AIRs; no native verification result is accepted as a proof.
//!
//! Initial values and derived values occupy separate symbolic namespaces while
//! the computation is built.  Finalization then assigns the only canonical
//! value-id layout: all initial writers first, followed by one result writer
//! for each arithmetic operation.  This avoids forward IDs and interleaved
//! allocator state without introducing aliases or legacy layouts.

use p256::{
    FieldBytes, FieldElement, ProjectivePoint, Scalar,
    elliptic_curve::{PrimeField as _, sec1::ToEncodedPoint as _},
};
use thiserror::Error;

use super::{
    p256_air::{
        P256_BASE_MODULUS_BE_V1, P256_SCALAR_MODULUS_BE_V1, ZkX509P256AirErrorV1,
        ZkX509P256ArithmeticKindV1, ZkX509P256ArithmeticOperationV1, ZkX509P256ArithmeticTraceV1,
        ZkX509P256ModulusV1, build_zk_x509_p256_arithmetic_trace_v1,
    },
    p256_ecdsa_air::{
        P256EcdsaAssignedV1, P256EcdsaCircuitV1, P256EcdsaRoleV1, P256EcdsaWitnessV1,
        constrain_p256_ecdsa_v1,
    },
    p256_group_air::{P256BaseFieldCircuitV1, P256ProjectiveValueV1, P256WindowCircuitV1},
    p256_reduction_air::{
        P256LowSTraceV1, P256ReductionTraceV1, build_p256_low_s_trace_v1,
        build_p256_reduction_trace_v1,
    },
    p256_value_bus::{
        P256BooleanBridgeBindingV1, P256EqualityBindingV1, P256InitialValueBindingV1,
        P256InitialValueKindV1, P256LinkedOperationV1, P256ValueIdV1,
    },
    p256_window_air::{
        P256WindowPointV1, P256WindowScalarV1, P256WindowTraceV1, build_p256_window_trace_v1,
    },
};

/// Stable descriptor for the one-signature witness compiler.
pub(crate) const ZK_X509_P256_TRACE_COMPILER_DESCRIPTOR_V1: &[u8] = b"zk-x509-p256-trace-compiler-v1-incompatible:one-signature-per-instance:symbolic-initial-and-derived-values:canonical-initial-then-operation-result-ssa-ids:no-forward-operation-reads:exact-arithmetic-operations:128-verifier-positioned-window-traces:final-order-u1-window0-through63-then-u2-window0-through63:reduction-and-low-s-bindings:native-arithmetic-witness-generation-only:no-native-verifier-recheck:activation=false";

const ZERO_BE_V1: [u8; 32] = [0; 32];
const ONE_BE_V1: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];

/// Canonical value-bus binding for one selected point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct P256BoundWindowTraceV1 {
    /// Verifier-positioned U1/U2 selector trace.
    pub(crate) trace: P256WindowTraceV1,
    /// Candidate coordinate IDs, ordered candidate then x/y/z.
    pub(crate) candidates: [[P256ValueIdV1; 3]; 16],
    /// Selected output x/y/z IDs.
    pub(crate) output: [P256ValueIdV1; 3],
    /// Arithmetic operation whose scalar result supplies the 256 bits.
    pub(crate) scalar_source_operation: u32,
}

/// Origin of a word reduced modulo the scalar-field order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum P256ReductionSourceV1 {
    /// Exact 32-byte SHA-256 digest; bound by the SHA/byte-I/O lanes.
    Digest {
        /// Exact unreduced word.
        word_be: [u8; 32],
    },
    /// Canonical base-field coordinate produced by arithmetic.
    BaseCoordinate {
        /// Assigned coordinate ID.
        id: P256ValueIdV1,
        /// Exact unreduced coordinate word.
        word_be: [u8; 32],
    },
}

/// One exact reduction and its value-bus endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct P256BoundReductionTraceV1 {
    /// Digest word or assigned base coordinate.
    pub(crate) source: P256ReductionSourceV1,
    /// Canonical scalar result ID.
    pub(crate) output: P256ValueIdV1,
    /// Exact one-subtraction reduction trace.
    pub(crate) trace: P256ReductionTraceV1,
}

/// One wallet-only low-s comparison bound to a scalar ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct P256BoundLowSTraceV1 {
    /// Scalar constrained by the comparison.
    pub(crate) scalar: P256ValueIdV1,
    /// Exact comparison trace.
    pub(crate) trace: P256LowSTraceV1,
}

/// Canonical resolved IDs retained from the generic ECDSA composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct P256ResolvedEcdsaAssignedV1 {
    /// Constrained affine public key.
    pub(crate) public_key: P256ProjectiveValueV1<P256ValueIdV1>,
    /// Canonical signature scalar r.
    pub(crate) r: P256ValueIdV1,
    /// Canonical signature scalar s.
    pub(crate) s: P256ValueIdV1,
    /// Reduced digest.
    pub(crate) z: P256ValueIdV1,
    /// `z * s^-1`.
    pub(crate) u1: P256ValueIdV1,
    /// `r * s^-1`.
    pub(crate) u2: P256ValueIdV1,
    /// Complete projective result x/y/z.
    pub(crate) result: P256ProjectiveValueV1<P256ValueIdV1>,
    /// Normalized affine result x-coordinate.
    pub(crate) result_x: P256ValueIdV1,
    /// Scalar reduction of `result_x`, constrained equal to r.
    pub(crate) reduced_x: P256ValueIdV1,
}

/// Final one-signature witness material before aggregate column streaming.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct P256EcdsaTraceMaterialV1 {
    /// Signature role fixed by the surrounding relation.
    pub(crate) role: P256EcdsaRoleV1,
    /// Canonical initial writers.
    pub(crate) initial_values: Vec<P256InitialValueBindingV1>,
    /// Canonical SSA-linked arithmetic operations.
    pub(crate) linked_operations: Vec<P256LinkedOperationV1>,
    /// Explicit same-modulus equalities.
    pub(crate) equalities: Vec<P256EqualityBindingV1>,
    /// Cross-modulus Boolean bridges, empty for the direct scalar-bit bus.
    pub(crate) boolean_bridges: Vec<P256BooleanBridgeBindingV1>,
    /// Exactly 128 selectors: U1 windows 0..63, then U2 windows 0..63.
    pub(crate) windows: Vec<P256BoundWindowTraceV1>,
    /// Digest and result-x reductions.
    pub(crate) reductions: Vec<P256BoundReductionTraceV1>,
    /// One comparison for wallet ownership and none for certificate/CRL roles.
    pub(crate) low_s: Vec<P256BoundLowSTraceV1>,
    /// Resolved outputs of the complete equation.
    pub(crate) assigned: P256ResolvedEcdsaAssignedV1,
}

impl core::fmt::Debug for P256EcdsaTraceMaterialV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("P256EcdsaTraceMaterialV1")
            .field("role", &self.role)
            .field("private_material", &"<redacted>")
            .finish()
    }
}

impl P256EcdsaTraceMaterialV1 {
    /// Recursively overwrite every witness-derived scalar, coordinate, and
    /// committed field row while leaving only public topology metadata.
    pub(crate) fn zeroize_private_v1(&mut self) {
        for initial in &mut self.initial_values {
            initial.value.fill(0);
        }
        self.initial_values.clear();
        for linked in &mut self.linked_operations {
            linked.operation.a.fill(0);
            linked.operation.b.fill(0);
            linked.operation.c.fill(0);
        }
        self.linked_operations.clear();
        self.equalities.clear();
        self.boolean_bridges.clear();
        for window in &mut self.windows {
            window.trace.zeroize_private_v1();
        }
        self.windows.clear();
        for reduction in &mut self.reductions {
            match &mut reduction.source {
                P256ReductionSourceV1::Digest { word_be }
                | P256ReductionSourceV1::BaseCoordinate { word_be, .. } => word_be.fill(0),
            }
            reduction.trace.zeroize_private_v1();
        }
        self.reductions.clear();
        for low_s in &mut self.low_s {
            low_s.trace.zeroize_private_v1();
        }
        self.low_s.clear();
    }

    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        self.initial_values.is_empty()
            && self.linked_operations.is_empty()
            && self.equalities.is_empty()
            && self.boolean_bridges.is_empty()
            && self.windows.is_empty()
            && self.reductions.is_empty()
            && self.low_s.is_empty()
    }

    /// Materialize the exact wide arithmetic rows.
    ///
    /// Aggregate proving should stream/shard these columns.  This helper is
    /// retained for focused validation and differential tests.
    pub(crate) fn build_arithmetic_trace_v1(
        &self,
    ) -> Result<ZkX509P256ArithmeticTraceV1, ZkX509P256AirErrorV1> {
        let operations = self
            .linked_operations
            .iter()
            .map(|linked| linked.operation)
            .collect::<Vec<_>>();
        build_zk_x509_p256_arithmetic_trace_v1(&operations)
    }
}

/// Witness compilation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum P256TraceCompilerErrorV1 {
    /// A base/scalar input or constant is noncanonical.
    #[error("zk-X509 P-256 trace compiler received a noncanonical value")]
    NonCanonical,
    /// A requested inverse is undefined.
    #[error("zk-X509 P-256 trace compiler cannot invert zero")]
    ZeroInverse,
    /// An asserted relation is false.
    #[error("zk-X509 P-256 trace compiler equality is false")]
    Equality,
    /// Scalar bits were assigned to the wrong U1/U2 window position.
    #[error("zk-X509 P-256 trace compiler window topology is invalid")]
    WindowTopology,
    /// SSA origins, writers, or operation dependencies are inconsistent.
    #[error("zk-X509 P-256 trace compiler value topology is invalid")]
    ValueTopology,
    /// Cross-chip owners or role-specific bindings are inconsistent.
    #[error("zk-X509 P-256 trace compiler binding topology is invalid")]
    BindingTopology,
    /// A reduction or low-s witness is invalid.
    #[error("zk-X509 P-256 trace compiler reduction is invalid")]
    Reduction,
    /// Value IDs or operation indices exceed the first-release envelope.
    #[error("zk-X509 P-256 trace compiler resource bound is exceeded")]
    Resource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymbolicOriginV1 {
    Initial {
        index: usize,
        kind: P256InitialValueKindV1,
    },
    Derived {
        operation: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SymbolicValueRecordV1 {
    modulus: ZkX509P256ModulusV1,
    value_be: [u8; 32],
    origin: SymbolicOriginV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct P256BaseValueV1(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct P256ScalarValueV1(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct P256ScalarBitV1 {
    source: P256ScalarValueV1,
    role: P256WindowScalarV1,
    global_be: u16,
    value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecordedOperationV1 {
    a: usize,
    b: usize,
    c: usize,
    operation: ZkX509P256ArithmeticOperationV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SymbolicWindowV1 {
    trace: P256WindowTraceV1,
    candidates: [[usize; 3]; 16],
    output: [usize; 3],
    scalar_source: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SymbolicReductionSourceV1 {
    Digest { word_be: [u8; 32] },
    BaseCoordinate { handle: usize, word_be: [u8; 32] },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SymbolicReductionV1 {
    source: SymbolicReductionSourceV1,
    output: usize,
    trace: P256ReductionTraceV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SymbolicLowSV1 {
    scalar: usize,
    trace: P256LowSTraceV1,
}

#[derive(Clone, Default)]
struct P256TraceCompilerV1 {
    values: Vec<SymbolicValueRecordV1>,
    initial_handles: Vec<usize>,
    operations: Vec<RecordedOperationV1>,
    equalities: Vec<(usize, usize)>,
    windows: Vec<SymbolicWindowV1>,
    reductions: Vec<SymbolicReductionV1>,
    low_s: Vec<SymbolicLowSV1>,
}

impl P256TraceCompilerV1 {
    fn record(&self, handle: usize) -> Result<SymbolicValueRecordV1, P256TraceCompilerErrorV1> {
        self.values
            .get(handle)
            .copied()
            .ok_or(P256TraceCompilerErrorV1::Resource)
    }

    fn push_initial_v1(
        &mut self,
        modulus: ZkX509P256ModulusV1,
        value_be: [u8; 32],
        kind: P256InitialValueKindV1,
    ) -> Result<usize, P256TraceCompilerErrorV1> {
        let bound = match modulus {
            ZkX509P256ModulusV1::BaseField => P256_BASE_MODULUS_BE_V1,
            ZkX509P256ModulusV1::ScalarField => P256_SCALAR_MODULUS_BE_V1,
        };
        if value_be >= bound {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        let initial = self.initial_handles.len();
        let handle = self.values.len();
        self.values.push(SymbolicValueRecordV1 {
            modulus,
            value_be,
            origin: SymbolicOriginV1::Initial {
                index: initial,
                kind,
            },
        });
        self.initial_handles.push(handle);
        Ok(handle)
    }

    fn record_operation_v1(
        &mut self,
        kind: ZkX509P256ArithmeticKindV1,
        modulus: ZkX509P256ModulusV1,
        a: usize,
        b: usize,
        c_be: [u8; 32],
    ) -> Result<usize, P256TraceCompilerErrorV1> {
        let a_record = self.record(a)?;
        let b_record = self.record(b)?;
        if a_record.modulus != modulus || b_record.modulus != modulus {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        let bound = match modulus {
            ZkX509P256ModulusV1::BaseField => P256_BASE_MODULUS_BE_V1,
            ZkX509P256ModulusV1::ScalarField => P256_SCALAR_MODULUS_BE_V1,
        };
        if c_be >= bound {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        let operation = self.operations.len();
        let c = self.values.len();
        self.values.push(SymbolicValueRecordV1 {
            modulus,
            value_be: c_be,
            origin: SymbolicOriginV1::Derived { operation },
        });
        self.operations.push(RecordedOperationV1 {
            a,
            b,
            c,
            operation: ZkX509P256ArithmeticOperationV1 {
                kind,
                modulus,
                a: a_record.value_be,
                b: b_record.value_be,
                c: c_be,
            },
        });
        Ok(c)
    }

    fn base_field_v1(
        &self,
        value: P256BaseValueV1,
    ) -> Result<FieldElement, P256TraceCompilerErrorV1> {
        let record = self.record(value.0)?;
        if record.modulus != ZkX509P256ModulusV1::BaseField {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        Option::<FieldElement>::from(FieldElement::from_bytes(&FieldBytes::from(record.value_be)))
            .ok_or(P256TraceCompilerErrorV1::NonCanonical)
    }

    fn scalar_field_v1(
        &self,
        value: P256ScalarValueV1,
    ) -> Result<Scalar, P256TraceCompilerErrorV1> {
        let record = self.record(value.0)?;
        if record.modulus != ZkX509P256ModulusV1::ScalarField {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        Option::<Scalar>::from(Scalar::from_repr(record.value_be.into()))
            .ok_or(P256TraceCompilerErrorV1::NonCanonical)
    }

    fn assert_equal_handles_v1(
        &mut self,
        left: usize,
        right: usize,
    ) -> Result<(), P256TraceCompilerErrorV1> {
        let left_record = self.record(left)?;
        let right_record = self.record(right)?;
        if left_record.modulus != right_record.modulus
            || left_record.value_be != right_record.value_be
        {
            return Err(P256TraceCompilerErrorV1::Equality);
        }
        if left != right {
            self.equalities.push((left, right));
        }
        Ok(())
    }

    fn generator_table_v1(
        &mut self,
    ) -> Result<[P256ProjectiveValueV1<P256BaseValueV1>; 16], P256TraceCompilerErrorV1> {
        let zero = P256BaseValueV1(self.push_initial_v1(
            ZkX509P256ModulusV1::BaseField,
            ZERO_BE_V1,
            P256InitialValueKindV1::Constant,
        )?);
        let one = P256BaseValueV1(self.push_initial_v1(
            ZkX509P256ModulusV1::BaseField,
            ONE_BE_V1,
            P256InitialValueKindV1::Constant,
        )?);
        let identity = P256ProjectiveValueV1 {
            x: zero,
            y: one,
            z: zero,
        };
        let mut table = [identity; 16];
        for (multiple, assigned) in table.iter_mut().enumerate().skip(1) {
            let point = ProjectivePoint::GENERATOR * Scalar::from(multiple as u64);
            let encoded = point.to_affine().to_encoded_point(false);
            let mut x_be = [0_u8; 32];
            let mut y_be = [0_u8; 32];
            x_be.copy_from_slice(
                encoded
                    .x()
                    .ok_or(P256TraceCompilerErrorV1::WindowTopology)?,
            );
            y_be.copy_from_slice(
                encoded
                    .y()
                    .ok_or(P256TraceCompilerErrorV1::WindowTopology)?,
            );
            *assigned = P256ProjectiveValueV1 {
                x: self.constant_v1(x_be)?,
                y: self.constant_v1(y_be)?,
                z: self.constant_v1(ONE_BE_V1)?,
            };
        }
        Ok(table)
    }

    fn resolve_id_v1(&self, handle: usize) -> Result<P256ValueIdV1, P256TraceCompilerErrorV1> {
        let record = self.record(handle)?;
        let index = match record.origin {
            SymbolicOriginV1::Initial { index, .. } => index,
            SymbolicOriginV1::Derived { operation } => self
                .initial_handles
                .len()
                .checked_add(operation)
                .ok_or(P256TraceCompilerErrorV1::Resource)?,
        };
        Ok(P256ValueIdV1(
            u32::try_from(index).map_err(|_| P256TraceCompilerErrorV1::Resource)?,
        ))
    }

    fn validate_final_topology_v1(
        &self,
        role: P256EcdsaRoleV1,
        assigned: &P256EcdsaAssignedV1<P256ScalarValueV1, P256BaseValueV1>,
    ) -> Result<(), P256TraceCompilerErrorV1> {
        if self.windows.len() != 128 || self.reductions.len() != 2 {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        }
        if self.initial_handles.len() > self.operations.len()
            || self.values.len()
                != self
                    .initial_handles
                    .len()
                    .checked_add(self.operations.len())
                    .ok_or(P256TraceCompilerErrorV1::Resource)?
        {
            return Err(P256TraceCompilerErrorV1::ValueTopology);
        }

        for (index, handle) in self.initial_handles.iter().copied().enumerate() {
            if !matches!(
                self.record(handle)?.origin,
                SymbolicOriginV1::Initial {
                    index: origin_index,
                    ..
                } if origin_index == index
            ) {
                return Err(P256TraceCompilerErrorV1::ValueTopology);
            }
        }
        for (index, operation) in self.operations.iter().enumerate() {
            let a = self.record(operation.a)?;
            let b = self.record(operation.b)?;
            let c = self.record(operation.c)?;
            if !matches!(
                c.origin,
                SymbolicOriginV1::Derived {
                    operation: origin_operation
                } if origin_operation == index
            ) || [a.modulus, b.modulus, c.modulus]
                .iter()
                .any(|modulus| *modulus != operation.operation.modulus)
                || a.value_be != operation.operation.a
                || b.value_be != operation.operation.b
                || c.value_be != operation.operation.c
            {
                return Err(P256TraceCompilerErrorV1::ValueTopology);
            }
            for input in [a, b] {
                if matches!(
                    input.origin,
                    SymbolicOriginV1::Derived {
                        operation: dependency
                    } if dependency >= index
                ) {
                    return Err(P256TraceCompilerErrorV1::ValueTopology);
                }
            }
            let c_id = self.resolve_id_v1(operation.c)?;
            if self.resolve_id_v1(operation.a)?.0 >= c_id.0
                || self.resolve_id_v1(operation.b)?.0 >= c_id.0
            {
                return Err(P256TraceCompilerErrorV1::ValueTopology);
            }
        }

        for (ordinal, window) in self.windows.iter().enumerate() {
            let expected_role = if ordinal % 2 == 0 {
                P256WindowScalarV1::U1
            } else {
                P256WindowScalarV1::U2
            };
            let expected_window =
                u8::try_from(ordinal / 2).map_err(|_| P256TraceCompilerErrorV1::Resource)?;
            window
                .trace
                .validate_for_v1(expected_role, expected_window)
                .map_err(|_| P256TraceCompilerErrorV1::BindingTopology)?;
            let expected_source = if expected_role == P256WindowScalarV1::U1 {
                assigned.u1.0
            } else {
                assigned.u2.0
            };
            if window.scalar_source != expected_source {
                return Err(P256TraceCompilerErrorV1::BindingTopology);
            }
            for handle in window.candidates.iter().flatten().copied() {
                if self.record(handle)?.modulus != ZkX509P256ModulusV1::BaseField {
                    return Err(P256TraceCompilerErrorV1::BindingTopology);
                }
            }
            for handle in window.output {
                let record = self.record(handle)?;
                if record.modulus != ZkX509P256ModulusV1::BaseField
                    || !matches!(
                        record.origin,
                        SymbolicOriginV1::Initial {
                            kind: P256InitialValueKindV1::Input,
                            ..
                        }
                    )
                {
                    return Err(P256TraceCompilerErrorV1::BindingTopology);
                }
            }
            let selected = window
                .trace
                .selected_point_v1()
                .map_err(|_| P256TraceCompilerErrorV1::BindingTopology)?;
            if [
                self.record(window.output[0])?.value_be,
                self.record(window.output[1])?.value_be,
                self.record(window.output[2])?.value_be,
            ] != [selected.x_be, selected.y_be, selected.z_be]
            {
                return Err(P256TraceCompilerErrorV1::BindingTopology);
            }
        }

        let digest = self
            .reductions
            .first()
            .ok_or(P256TraceCompilerErrorV1::BindingTopology)?;
        let SymbolicReductionSourceV1::Digest { word_be } = digest.source else {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        };
        if digest.output != assigned.z.0
            || digest.trace
                != build_p256_reduction_trace_v1(word_be)
                    .map_err(|_| P256TraceCompilerErrorV1::Reduction)?
        {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        }
        let result_x = self
            .reductions
            .get(1)
            .ok_or(P256TraceCompilerErrorV1::BindingTopology)?;
        let SymbolicReductionSourceV1::BaseCoordinate { handle, word_be } = result_x.source else {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        };
        if handle != assigned.result_x.0
            || result_x.output != assigned.reduced_x.0
            || self.record(handle)?.value_be != word_be
            || result_x.trace
                != build_p256_reduction_trace_v1(word_be)
                    .map_err(|_| P256TraceCompilerErrorV1::Reduction)?
        {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        }

        match role {
            P256EcdsaRoleV1::WalletOwnership => {
                let [low_s] = self.low_s.as_slice() else {
                    return Err(P256TraceCompilerErrorV1::BindingTopology);
                };
                let scalar = self.record(assigned.s.0)?;
                if low_s.scalar != assigned.s.0
                    || low_s.trace
                        != build_p256_low_s_trace_v1(scalar.value_be)
                            .map_err(|_| P256TraceCompilerErrorV1::Reduction)?
                {
                    return Err(P256TraceCompilerErrorV1::BindingTopology);
                }
            }
            P256EcdsaRoleV1::CertificateOrCrl if !self.low_s.is_empty() => {
                return Err(P256TraceCompilerErrorV1::BindingTopology);
            }
            P256EcdsaRoleV1::CertificateOrCrl => {}
        }

        let required_initials = [
            (
                assigned.public_key.x.0,
                ZkX509P256ModulusV1::BaseField,
                P256InitialValueKindV1::Input,
            ),
            (
                assigned.public_key.y.0,
                ZkX509P256ModulusV1::BaseField,
                P256InitialValueKindV1::Input,
            ),
            (
                assigned.r.0,
                ZkX509P256ModulusV1::ScalarField,
                P256InitialValueKindV1::Input,
            ),
            (
                assigned.s.0,
                ZkX509P256ModulusV1::ScalarField,
                P256InitialValueKindV1::Input,
            ),
            (
                assigned.z.0,
                ZkX509P256ModulusV1::ScalarField,
                P256InitialValueKindV1::Input,
            ),
            (
                assigned.reduced_x.0,
                ZkX509P256ModulusV1::ScalarField,
                P256InitialValueKindV1::Input,
            ),
        ];
        for (handle, modulus, kind) in required_initials {
            let record = self.record(handle)?;
            if record.modulus != modulus
                || !matches!(
                    record.origin,
                    SymbolicOriginV1::Initial {
                        kind: actual_kind,
                        ..
                    } if actual_kind == kind
                )
            {
                return Err(P256TraceCompilerErrorV1::BindingTopology);
            }
        }
        let public_key_z = self.record(assigned.public_key.z.0)?;
        if public_key_z.modulus != ZkX509P256ModulusV1::BaseField
            || public_key_z.value_be != ONE_BE_V1
            || !matches!(
                public_key_z.origin,
                SymbolicOriginV1::Initial {
                    kind: P256InitialValueKindV1::Constant,
                    ..
                }
            )
        {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        }
        for handle in [
            assigned.u1.0,
            assigned.u2.0,
            assigned.result.x.0,
            assigned.result.y.0,
            assigned.result.z.0,
            assigned.result_x.0,
        ] {
            if !matches!(
                self.record(handle)?.origin,
                SymbolicOriginV1::Derived { .. }
            ) {
                return Err(P256TraceCompilerErrorV1::BindingTopology);
            }
        }
        if !self.equalities.iter().any(|(left, right)| {
            (*left == assigned.reduced_x.0 && *right == assigned.r.0)
                || (*left == assigned.r.0 && *right == assigned.reduced_x.0)
        }) {
            return Err(P256TraceCompilerErrorV1::BindingTopology);
        }
        Ok(())
    }

    fn finalize_v1(
        self,
        role: P256EcdsaRoleV1,
        assigned: P256EcdsaAssignedV1<P256ScalarValueV1, P256BaseValueV1>,
    ) -> Result<P256EcdsaTraceMaterialV1, P256TraceCompilerErrorV1> {
        self.validate_final_topology_v1(role, &assigned)?;
        let initial_values = self
            .initial_handles
            .iter()
            .copied()
            .enumerate()
            .map(|(index, handle)| {
                let record = self.record(handle)?;
                let SymbolicOriginV1::Initial {
                    index: origin_index,
                    kind,
                } = record.origin
                else {
                    return Err(P256TraceCompilerErrorV1::Resource);
                };
                if origin_index != index {
                    return Err(P256TraceCompilerErrorV1::Resource);
                }
                Ok(P256InitialValueBindingV1 {
                    id: P256ValueIdV1(
                        u32::try_from(index).map_err(|_| P256TraceCompilerErrorV1::Resource)?,
                    ),
                    modulus: record.modulus,
                    value: record.value_be,
                    kind,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let linked_operations = self
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                let c = self.resolve_id_v1(operation.c)?;
                let expected = self
                    .initial_handles
                    .len()
                    .checked_add(index)
                    .ok_or(P256TraceCompilerErrorV1::Resource)?;
                if c.0 as usize != expected {
                    return Err(P256TraceCompilerErrorV1::Resource);
                }
                Ok(P256LinkedOperationV1 {
                    a: self.resolve_id_v1(operation.a)?,
                    b: self.resolve_id_v1(operation.b)?,
                    c,
                    operation: operation.operation,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let equalities = self
            .equalities
            .iter()
            .map(|(left, right)| {
                Ok(P256EqualityBindingV1 {
                    left: self.resolve_id_v1(*left)?,
                    right: self.resolve_id_v1(*right)?,
                })
            })
            .collect::<Result<Vec<_>, P256TraceCompilerErrorV1>>()?;
        let windows = (0..128)
            .map(|ordinal| {
                let internal = if ordinal < 64 {
                    ordinal * 2
                } else {
                    (ordinal - 64) * 2 + 1
                };
                self.windows
                    .get(internal)
                    .ok_or(P256TraceCompilerErrorV1::WindowTopology)
            })
            .map(|window| {
                let window = window?;
                Ok(window)
            })
            .collect::<Result<Vec<_>, P256TraceCompilerErrorV1>>()?
            .into_iter()
            .map(|window| {
                let source = self.record(window.scalar_source)?;
                let SymbolicOriginV1::Derived { operation } = source.origin else {
                    return Err(P256TraceCompilerErrorV1::WindowTopology);
                };
                let mut candidates = [[P256ValueIdV1(0); 3]; 16];
                for (resolved, symbolic) in candidates.iter_mut().zip(window.candidates.iter()) {
                    for (resolved, symbolic) in resolved.iter_mut().zip(symbolic.iter()) {
                        *resolved = self.resolve_id_v1(*symbolic)?;
                    }
                }
                let mut output = [P256ValueIdV1(0); 3];
                for (resolved, symbolic) in output.iter_mut().zip(window.output.iter()) {
                    *resolved = self.resolve_id_v1(*symbolic)?;
                }
                Ok(P256BoundWindowTraceV1 {
                    trace: window.trace.clone(),
                    candidates,
                    output,
                    scalar_source_operation: u32::try_from(operation)
                        .map_err(|_| P256TraceCompilerErrorV1::Resource)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reductions = self
            .reductions
            .iter()
            .map(|reduction| {
                Ok(P256BoundReductionTraceV1 {
                    source: match reduction.source {
                        SymbolicReductionSourceV1::Digest { word_be } => {
                            P256ReductionSourceV1::Digest { word_be }
                        }
                        SymbolicReductionSourceV1::BaseCoordinate { handle, word_be } => {
                            P256ReductionSourceV1::BaseCoordinate {
                                id: self.resolve_id_v1(handle)?,
                                word_be,
                            }
                        }
                    },
                    output: self.resolve_id_v1(reduction.output)?,
                    trace: reduction.trace.clone(),
                })
            })
            .collect::<Result<Vec<_>, P256TraceCompilerErrorV1>>()?;
        let low_s = self
            .low_s
            .iter()
            .map(|binding| {
                Ok(P256BoundLowSTraceV1 {
                    scalar: self.resolve_id_v1(binding.scalar)?,
                    trace: binding.trace.clone(),
                })
            })
            .collect::<Result<Vec<_>, P256TraceCompilerErrorV1>>()?;
        Ok(P256EcdsaTraceMaterialV1 {
            role,
            initial_values,
            linked_operations,
            equalities,
            boolean_bridges: Vec::new(),
            windows,
            reductions,
            low_s,
            assigned: P256ResolvedEcdsaAssignedV1 {
                public_key: P256ProjectiveValueV1 {
                    x: self.resolve_id_v1(assigned.public_key.x.0)?,
                    y: self.resolve_id_v1(assigned.public_key.y.0)?,
                    z: self.resolve_id_v1(assigned.public_key.z.0)?,
                },
                r: self.resolve_id_v1(assigned.r.0)?,
                s: self.resolve_id_v1(assigned.s.0)?,
                z: self.resolve_id_v1(assigned.z.0)?,
                u1: self.resolve_id_v1(assigned.u1.0)?,
                u2: self.resolve_id_v1(assigned.u2.0)?,
                result: P256ProjectiveValueV1 {
                    x: self.resolve_id_v1(assigned.result.x.0)?,
                    y: self.resolve_id_v1(assigned.result.y.0)?,
                    z: self.resolve_id_v1(assigned.result.z.0)?,
                },
                result_x: self.resolve_id_v1(assigned.result_x.0)?,
                reduced_x: self.resolve_id_v1(assigned.reduced_x.0)?,
            },
        })
    }
}

impl P256BaseFieldCircuitV1 for P256TraceCompilerV1 {
    type Value = P256BaseValueV1;
    type Error = P256TraceCompilerErrorV1;

    fn constant_v1(&mut self, value: [u8; 32]) -> Result<Self::Value, Self::Error> {
        self.push_initial_v1(
            ZkX509P256ModulusV1::BaseField,
            value,
            P256InitialValueKindV1::Constant,
        )
        .map(P256BaseValueV1)
    }

    fn add_v1(
        &mut self,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let result = (self.base_field_v1(left)? + self.base_field_v1(right)?)
            .to_bytes()
            .into();
        self.record_operation_v1(
            ZkX509P256ArithmeticKindV1::Add,
            ZkX509P256ModulusV1::BaseField,
            left.0,
            right.0,
            result,
        )
        .map(P256BaseValueV1)
    }

    fn subtract_v1(
        &mut self,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let result = (self.base_field_v1(left)? - self.base_field_v1(right)?)
            .to_bytes()
            .into();
        self.record_operation_v1(
            ZkX509P256ArithmeticKindV1::Subtract,
            ZkX509P256ModulusV1::BaseField,
            left.0,
            right.0,
            result,
        )
        .map(P256BaseValueV1)
    }

    fn multiply_v1(
        &mut self,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let result = (self.base_field_v1(left)? * self.base_field_v1(right)?)
            .to_bytes()
            .into();
        self.record_operation_v1(
            ZkX509P256ArithmeticKindV1::Multiply,
            ZkX509P256ModulusV1::BaseField,
            left.0,
            right.0,
            result,
        )
        .map(P256BaseValueV1)
    }

    fn assert_equal_v1(
        &mut self,
        left: Self::Value,
        right: Self::Value,
    ) -> Result<(), Self::Error> {
        self.assert_equal_handles_v1(left.0, right.0)
    }

    fn inverse_nonzero_v1(&mut self, value: Self::Value) -> Result<Self::Value, Self::Error> {
        let inverse = Option::<FieldElement>::from(self.base_field_v1(value)?.invert())
            .ok_or(P256TraceCompilerErrorV1::ZeroInverse)?;
        let inverse = P256BaseValueV1(self.push_initial_v1(
            ZkX509P256ModulusV1::BaseField,
            inverse.to_bytes().into(),
            P256InitialValueKindV1::Input,
        )?);
        let product = self.multiply_v1(value, inverse)?;
        let one = self.constant_v1(ONE_BE_V1)?;
        self.assert_equal_v1(product, one)?;
        Ok(inverse)
    }
}

impl P256WindowCircuitV1 for P256TraceCompilerV1 {
    type Bit = P256ScalarBitV1;

    fn select_window_v1(
        &mut self,
        table: &[P256ProjectiveValueV1<Self::Value>; 16],
        bits_be: [Self::Bit; 4],
    ) -> Result<P256ProjectiveValueV1<Self::Value>, Self::Error> {
        let role = bits_be[0].role;
        let source = bits_be[0].source;
        let start = usize::from(bits_be[0].global_be);
        if start % 4 != 0
            || start >= 256
            || bits_be.iter().enumerate().any(|(offset, bit)| {
                bit.role != role
                    || bit.source != source
                    || usize::from(bit.global_be) != start + offset
                    || bit.value > 1
            })
        {
            return Err(P256TraceCompilerErrorV1::WindowTopology);
        }
        let window = start / 4;
        let expected_ordinal = self.windows.len();
        let expected_role = if expected_ordinal.is_multiple_of(2) {
            P256WindowScalarV1::U1
        } else {
            P256WindowScalarV1::U2
        };
        if role != expected_role || window != expected_ordinal / 2 {
            return Err(P256TraceCompilerErrorV1::WindowTopology);
        }

        let candidate_handles = core::array::from_fn(|candidate| {
            [
                table[candidate].x.0,
                table[candidate].y.0,
                table[candidate].z.0,
            ]
        });
        let mut candidate_points = [P256WindowPointV1 {
            x_be: ZERO_BE_V1,
            y_be: ZERO_BE_V1,
            z_be: ZERO_BE_V1,
        }; 16];
        for (point, handles) in candidate_points.iter_mut().zip(candidate_handles.iter()) {
            let x = self.record(handles[0])?;
            let y = self.record(handles[1])?;
            let z = self.record(handles[2])?;
            if [x.modulus, y.modulus, z.modulus]
                .iter()
                .any(|modulus| *modulus != ZkX509P256ModulusV1::BaseField)
            {
                return Err(P256TraceCompilerErrorV1::WindowTopology);
            }
            *point = P256WindowPointV1 {
                x_be: x.value_be,
                y_be: y.value_be,
                z_be: z.value_be,
            };
        }
        let bit_values = bits_be.map(|bit| bit.value);
        let selected = bit_values
            .iter()
            .fold(0_usize, |value, bit| (value << 1) | usize::from(*bit));
        let selected_point = candidate_points[selected];
        let output = [
            self.push_initial_v1(
                ZkX509P256ModulusV1::BaseField,
                selected_point.x_be,
                P256InitialValueKindV1::Input,
            )?,
            self.push_initial_v1(
                ZkX509P256ModulusV1::BaseField,
                selected_point.y_be,
                P256InitialValueKindV1::Input,
            )?,
            self.push_initial_v1(
                ZkX509P256ModulusV1::BaseField,
                selected_point.z_be,
                P256InitialValueKindV1::Input,
            )?,
        ];
        let trace = build_p256_window_trace_v1(role, window as u8, candidate_points, bit_values)
            .map_err(|_| P256TraceCompilerErrorV1::WindowTopology)?;
        self.windows.push(SymbolicWindowV1 {
            trace,
            candidates: candidate_handles,
            output,
            scalar_source: source.0,
        });
        Ok(P256ProjectiveValueV1 {
            x: P256BaseValueV1(output[0]),
            y: P256BaseValueV1(output[1]),
            z: P256BaseValueV1(output[2]),
        })
    }
}

impl P256EcdsaCircuitV1 for P256TraceCompilerV1 {
    type Scalar = P256ScalarValueV1;

    fn base_input_v1(&mut self, value_be: [u8; 32]) -> Result<Self::Value, Self::Error> {
        self.push_initial_v1(
            ZkX509P256ModulusV1::BaseField,
            value_be,
            P256InitialValueKindV1::Input,
        )
        .map(P256BaseValueV1)
    }

    fn scalar_input_v1(&mut self, value_be: [u8; 32]) -> Result<Self::Scalar, Self::Error> {
        self.push_initial_v1(
            ZkX509P256ModulusV1::ScalarField,
            value_be,
            P256InitialValueKindV1::Input,
        )
        .map(P256ScalarValueV1)
    }

    fn scalar_inverse_nonzero_v1(
        &mut self,
        value: Self::Scalar,
    ) -> Result<Self::Scalar, Self::Error> {
        let inverse = Option::<Scalar>::from(self.scalar_field_v1(value)?.invert())
            .ok_or(P256TraceCompilerErrorV1::ZeroInverse)?;
        let inverse = P256ScalarValueV1(self.push_initial_v1(
            ZkX509P256ModulusV1::ScalarField,
            inverse.to_bytes().into(),
            P256InitialValueKindV1::Input,
        )?);
        let product = self.scalar_multiply_v1(value, inverse)?;
        let one = P256ScalarValueV1(self.push_initial_v1(
            ZkX509P256ModulusV1::ScalarField,
            ONE_BE_V1,
            P256InitialValueKindV1::Constant,
        )?);
        self.scalar_assert_equal_v1(product, one)?;
        Ok(inverse)
    }

    fn scalar_multiply_v1(
        &mut self,
        left: Self::Scalar,
        right: Self::Scalar,
    ) -> Result<Self::Scalar, Self::Error> {
        let result = (self.scalar_field_v1(left)? * self.scalar_field_v1(right)?)
            .to_bytes()
            .into();
        self.record_operation_v1(
            ZkX509P256ArithmeticKindV1::Multiply,
            ZkX509P256ModulusV1::ScalarField,
            left.0,
            right.0,
            result,
        )
        .map(P256ScalarValueV1)
    }

    fn scalar_assert_equal_v1(
        &mut self,
        left: Self::Scalar,
        right: Self::Scalar,
    ) -> Result<(), Self::Error> {
        self.assert_equal_handles_v1(left.0, right.0)
    }

    fn reduce_digest_v1(&mut self, digest_be: [u8; 32]) -> Result<Self::Scalar, Self::Error> {
        let trace = build_p256_reduction_trace_v1(digest_be)
            .map_err(|_| P256TraceCompilerErrorV1::Reduction)?;
        let output = self.push_initial_v1(
            ZkX509P256ModulusV1::ScalarField,
            trace.reduced_be_v1(),
            P256InitialValueKindV1::Input,
        )?;
        self.reductions.push(SymbolicReductionV1 {
            source: SymbolicReductionSourceV1::Digest { word_be: digest_be },
            output,
            trace,
        });
        Ok(P256ScalarValueV1(output))
    }

    fn reduce_base_coordinate_v1(
        &mut self,
        coordinate: Self::Value,
    ) -> Result<Self::Scalar, Self::Error> {
        let coordinate_record = self.record(coordinate.0)?;
        if coordinate_record.modulus != ZkX509P256ModulusV1::BaseField {
            return Err(P256TraceCompilerErrorV1::NonCanonical);
        }
        let trace = build_p256_reduction_trace_v1(coordinate_record.value_be)
            .map_err(|_| P256TraceCompilerErrorV1::Reduction)?;
        let output = self.push_initial_v1(
            ZkX509P256ModulusV1::ScalarField,
            trace.reduced_be_v1(),
            P256InitialValueKindV1::Input,
        )?;
        self.reductions.push(SymbolicReductionV1 {
            source: SymbolicReductionSourceV1::BaseCoordinate {
                handle: coordinate.0,
                word_be: coordinate_record.value_be,
            },
            output,
            trace,
        });
        Ok(P256ScalarValueV1(output))
    }

    fn scalar_bits_be_v1(
        &mut self,
        scalar: Self::Scalar,
        role: P256WindowScalarV1,
    ) -> Result<[Self::Bit; 256], Self::Error> {
        let record = self.record(scalar.0)?;
        if record.modulus != ZkX509P256ModulusV1::ScalarField
            || !matches!(record.origin, SymbolicOriginV1::Derived { .. })
        {
            return Err(P256TraceCompilerErrorV1::WindowTopology);
        }
        Ok(core::array::from_fn(|global_be| P256ScalarBitV1 {
            source: scalar,
            role,
            global_be: global_be as u16,
            value: (record.value_be[global_be / 8] >> (7 - global_be % 8)) & 1,
        }))
    }

    fn constrain_low_s_v1(&mut self, scalar: Self::Scalar) -> Result<(), Self::Error> {
        let record = self.record(scalar.0)?;
        let trace = build_p256_low_s_trace_v1(record.value_be)
            .map_err(|_| P256TraceCompilerErrorV1::Reduction)?;
        self.low_s.push(SymbolicLowSV1 {
            scalar: scalar.0,
            trace,
        });
        Ok(())
    }
}

/// Compile one complete role-separated ECDSA witness into native AIR material.
pub(crate) fn compile_p256_ecdsa_trace_material_v1(
    role: P256EcdsaRoleV1,
    witness: P256EcdsaWitnessV1,
) -> Result<P256EcdsaTraceMaterialV1, P256TraceCompilerErrorV1> {
    let mut compiler = P256TraceCompilerV1::default();
    let generator_table = compiler.generator_table_v1()?;
    let assigned = constrain_p256_ecdsa_v1(&mut compiler, &generator_table, role, witness)?;
    compiler.finalize_v1(role, assigned)
}

#[cfg(test)]
mod tests {
    use p256::ecdsa::{Signature, SigningKey, signature::hazmat::PrehashSigner as _};

    use super::*;
    use crate::privacy_engines::zk_x509::p256_group_air::P256_TWO_SCALAR_ARITHMETIC_OPERATIONS_V1;

    fn signing_key_v1(seed: u8) -> SigningKey {
        let mut bytes = [0_u8; 32];
        bytes[31] = seed.max(1);
        SigningKey::from_slice(&bytes).expect("valid nonzero key")
    }

    fn witness_v1(
        key: &SigningKey,
        digest_be: [u8; 32],
        signature: Signature,
    ) -> P256EcdsaWitnessV1 {
        let encoded = key.verifying_key().to_encoded_point(false);
        let mut public_key_x_be = [0_u8; 32];
        let mut public_key_y_be = [0_u8; 32];
        public_key_x_be.copy_from_slice(encoded.x().expect("x"));
        public_key_y_be.copy_from_slice(encoded.y().expect("y"));
        P256EcdsaWitnessV1 {
            public_key_x_be,
            public_key_y_be,
            r_be: signature.r().to_bytes().into(),
            s_be: signature.s().to_bytes().into(),
            digest_be,
        }
    }

    fn valid_compiler_v1() -> (
        P256TraceCompilerV1,
        P256EcdsaAssignedV1<P256ScalarValueV1, P256BaseValueV1>,
    ) {
        let key = signing_key_v1(73);
        let digest = core::array::from_fn(|index| (index as u8).wrapping_mul(37).wrapping_add(11));
        let signature: Signature = key.sign_prehash(&digest).expect("sign");
        let signature = signature.normalize_s().unwrap_or(signature);
        let mut compiler = P256TraceCompilerV1::default();
        let generator = compiler.generator_table_v1().expect("generator table");
        let assigned = constrain_p256_ecdsa_v1(
            &mut compiler,
            &generator,
            P256EcdsaRoleV1::WalletOwnership,
            witness_v1(&key, digest, signature),
        )
        .expect("compile symbolic equation");
        (compiler, assigned)
    }

    fn valid_material_v1() -> P256EcdsaTraceMaterialV1 {
        let (compiler, assigned) = valid_compiler_v1();
        compiler
            .finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned)
            .expect("finalize exact witness")
    }

    #[test]
    fn complete_compiler_emits_canonical_ssa_windows_and_bindings() {
        let material = valid_material_v1();
        assert_eq!(
            material.linked_operations.len(),
            P256_TWO_SCALAR_ARITHMETIC_OPERATIONS_V1 + 18
        );
        assert!(material.initial_values.len() <= material.linked_operations.len());
        assert_eq!(material.windows.len(), 128);
        assert_eq!(material.reductions.len(), 2);
        assert_eq!(material.low_s.len(), 1);
        assert!(material.boolean_bridges.is_empty());

        for (index, initial) in material.initial_values.iter().enumerate() {
            assert_eq!(initial.id, P256ValueIdV1(index as u32));
        }
        for (index, operation) in material.linked_operations.iter().enumerate() {
            assert_eq!(
                operation.c,
                P256ValueIdV1((material.initial_values.len() + index) as u32)
            );
            assert!(operation.a.0 < operation.c.0);
            assert!(operation.b.0 < operation.c.0);
        }
        for (ordinal, window) in material.windows.iter().enumerate() {
            let role = if ordinal < 64 {
                P256WindowScalarV1::U1
            } else {
                P256WindowScalarV1::U2
            };
            let index = (ordinal % 64) as u8;
            window
                .trace
                .validate_for_v1(role, index)
                .expect("fixed window trace");
            assert_eq!(
                window.scalar_source_operation,
                if role == P256WindowScalarV1::U1 {
                    material.assigned.u1.0 - material.initial_values.len() as u32
                } else {
                    material.assigned.u2.0 - material.initial_values.len() as u32
                }
            );
        }
    }

    #[test]
    fn sampled_operations_are_exact_native_arithmetic_traces() {
        let material = valid_material_v1();
        for linked in material.linked_operations.iter().step_by(127) {
            let trace = build_zk_x509_p256_arithmetic_trace_v1(&[linked.operation])
                .expect("build sampled operation");
            trace.validate().expect("validate sampled operation");
        }
        for binding in &material.reductions {
            binding.trace.validate().expect("exact reduction");
        }
        for binding in &material.low_s {
            binding.trace.validate().expect("exact low-s");
        }
    }

    #[test]
    fn certificate_role_omits_low_s_but_invalid_equations_fail() {
        let key = signing_key_v1(79);
        let digest = [0xa7; 32];
        let signature: Signature = key.sign_prehash(&digest).expect("sign");
        let signature = signature.normalize_s().unwrap_or(signature);
        let high = Signature::from_scalars(signature.r().to_bytes(), (-*signature.s()).to_bytes())
            .expect("high-s representative");
        let material = compile_p256_ecdsa_trace_material_v1(
            P256EcdsaRoleV1::CertificateOrCrl,
            witness_v1(&key, digest, high),
        )
        .expect("RFC 5280 high-s");
        assert!(material.low_s.is_empty());

        let mut wrong = witness_v1(&key, digest, signature);
        wrong.digest_be[0] ^= 1;
        assert_eq!(
            compile_p256_ecdsa_trace_material_v1(P256EcdsaRoleV1::WalletOwnership, wrong,),
            Err(P256TraceCompilerErrorV1::Equality)
        );
    }

    #[test]
    fn zero_noncanonical_and_high_s_inputs_fail_closed() {
        let key = signing_key_v1(83);
        let digest = [0x5c; 32];
        let signature: Signature = key.sign_prehash(&digest).expect("sign");
        let signature = signature.normalize_s().unwrap_or(signature);
        let witness = witness_v1(&key, digest, signature);

        let mut zero = witness;
        zero.r_be = [0; 32];
        assert_eq!(
            compile_p256_ecdsa_trace_material_v1(P256EcdsaRoleV1::WalletOwnership, zero,),
            Err(P256TraceCompilerErrorV1::ZeroInverse)
        );

        let high = Signature::from_scalars(signature.r().to_bytes(), (-*signature.s()).to_bytes())
            .expect("high-s representative");
        assert_eq!(
            compile_p256_ecdsa_trace_material_v1(
                P256EcdsaRoleV1::WalletOwnership,
                witness_v1(&key, digest, high),
            ),
            Err(P256TraceCompilerErrorV1::Reduction)
        );

        let mut noncanonical = witness;
        noncanonical.s_be = P256_SCALAR_MODULUS_BE_V1;
        assert_eq!(
            compile_p256_ecdsa_trace_material_v1(P256EcdsaRoleV1::CertificateOrCrl, noncanonical,),
            Err(P256TraceCompilerErrorV1::NonCanonical)
        );
    }

    #[test]
    fn finalization_rejects_value_window_reduction_and_low_s_rebinding() {
        let (compiler, assigned) = valid_compiler_v1();

        let mut forward_read = compiler.clone();
        forward_read.operations[0].a = forward_read.operations[0].c;
        assert_eq!(
            forward_read.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::ValueTopology)
        );

        let mut wrong_source = compiler.clone();
        wrong_source.windows[0].scalar_source = assigned.u2.0;
        assert_eq!(
            wrong_source.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::BindingTopology)
        );

        let mut wrong_position = compiler.clone();
        wrong_position.windows[0].trace.fixed[0].scalar = P256WindowScalarV1::U2;
        assert_eq!(
            wrong_position.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::BindingTopology)
        );

        let mut swapped_reductions = compiler.clone();
        swapped_reductions.reductions.swap(0, 1);
        assert_eq!(
            swapped_reductions.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::BindingTopology)
        );

        let mut wrong_low_s = compiler.clone();
        wrong_low_s.low_s[0].scalar = assigned.r.0;
        assert_eq!(
            wrong_low_s.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::BindingTopology)
        );

        let mut wrong_output = compiler;
        wrong_output.windows[0].output[0] = assigned.public_key.x.0;
        assert_eq!(
            wrong_output.finalize_v1(P256EcdsaRoleV1::WalletOwnership, assigned),
            Err(P256TraceCompilerErrorV1::BindingTopology)
        );
    }
}
