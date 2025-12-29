mod fixes;
#[path = "../generated/CoreData/mod.rs"]
mod generated;

#[allow(unreachable_pub)]
pub use self::generated::*;

#[cfg_attr(feature = "apple", link(name = "CoreData", kind = "framework"))]
extern "C" {}
