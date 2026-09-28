//! Pipeline utilities: access-set derivation and future scheduler glue.
pub mod access;
#[cfg(test)]
pub mod gpu;
pub mod overlay;
#[cfg(test)]
pub mod smallset;
/// Background ZK verification lane (non-forking).
pub mod zk_lane;
