// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * `@iroha/connect-widget/sakura`: the SORA Nexus sakura layer on its own,
 * without the Connect SDK. Importing this module registers `<iroha-sakura>`.
 */

import { defineIrohaSakura } from "./sakura.js";

export * from "./sakura.js";
export { resolveTheme } from "./theme.js";

defineIrohaSakura();
