//! Verification gates (`specs/network_deployment.md` §9).
//!
//! Every gate runs in-process against public, untrusted routes. Transport lives
//! behind small traits so that the checks themselves stay pure and testable.
//!
//! [`finality`] is the light finality verifier behind gate G5 and the network
//! card check (§11.2 D-7).
// TODO: P2 adds the remaining gates (G0-G4, G6-G8, G11) and the HTTP
// `FinalitySource`; P6 adds the dataspace gate G12.

pub mod finality;
