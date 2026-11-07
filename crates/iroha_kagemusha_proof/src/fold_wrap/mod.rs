//! The KAGEMUSHA-Fold compression wraps (output `IVC.md` sections 7 and 11).
//!
//! After the last business instance, two k16 PIPA-R proofs turn the folded
//! state into an Omega in today's transport:
//!
//! - **P**, over `Fp` (a Vesta proof): the commitment side of the Q-side
//!   decider (Pallas group operations) and its bound opening, the field side
//!   of the A-side decider (native `Fp`), the restructure of the Pallas claim
//!   accumulator, the PIPA-AS fold into `acc_P`, `D_A`, and the recomputed
//!   `h_out` of the last A instance;
//! - **Omega**, over `Fq` (a Pallas proof, today's 19 public instances):
//!   verifies P, the field side of the Q-side decider, the commitment side of
//!   the A-side decider and its opening, the restructure of the Vesta claim
//!   accumulator, the fold of the last A instance and the PIPA-AS fold into
//!   `acc_V`.
//!
//! Each decider is split (DESIGN.md section 9.0): its field messages are
//! native in the circuit over its scalar field, its commitments in the circuit
//! over its commitment curve's base field. The values that cross (digests,
//! challenges, the batched value and the folded eq) are canonical S6 limbs in
//! P's instance; Omega verifies P, so every crossing value is bound on both
//! sides. The prover computes everything natively first, proves P, then Omega.

pub mod p;
