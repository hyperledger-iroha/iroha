// Native export retention for Swift Package Manager consumers.
#ifndef IROHA_NORITO_BRIDGE_RETENTION_H
#define IROHA_NORITO_BRIDGE_RETENTION_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Retain every native export dynamically resolved by the Swift SDK.
/// Reads the function-address table and returns its number of non-null entries.
size_t iroha_norito_bridge_retain_exports(void);

#ifdef __cplusplus
}
#endif

#endif
