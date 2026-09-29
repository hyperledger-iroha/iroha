//! Shared finite ownership for hardware acceleration.
//!
//! Algorithm policy and arithmetic stay in consumers. This lower boundary owns
//! physical device/context lineage, explicit allocation custody, checked cleanup
//! and complete-work publication. The process configuration supplies a finite resource envelope explicitly;
//! performance qualification is independent of resource-policy defaults.

mod artifact;
#[cfg(any(feature = "cuda", test))]
mod context_binding;
#[cfg(any(feature = "cuda", test))]
mod custody;
mod identity;
mod limits;
mod output;
mod process;
mod registry_limits;
mod resources;
mod slots;
#[allow(unsafe_code)] // Exact aligned backing allocation/deallocation only.
mod unified;
pub use process::NativeCommandPermit;
pub use unified::{UnifiedBuffer, UnifiedBufferError};

#[cfg(feature = "cuda")]
#[allow(unsafe_code)] // Exact driver FFI and checked native custody live only here.
pub mod cuda;

pub use artifact::PtxArtifact;
pub use identity::DeviceIdentity;
pub use limits::GpuResourceLimits;
pub use output::{HostOutput, HostOutputError};
pub use process::{ProcessResources, ProcessUsage};
pub use registry_limits::RegistryLimits;
