// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * `@iroha/connect-widget`: the IrohaConnect button and the SORA Nexus sakura
 * layer for any web app. Importing this module registers `<iroha-connect>`
 * and `<iroha-sakura>`; it is safe to import during server-side rendering,
 * where nothing is registered.
 */

import { defineIrohaConnect } from "./element.js";

export { IROHA_MARK_SVG, IROHA_RED, SAKURA_PINK, irohaMarkSvg } from "./brand.js";
export * from "./controller.js";
export {
  IROHA_CONNECT_EVENTS,
  IROHA_CONNECT_TAG,
  IrohaConnectElement,
  defineIrohaConnect,
} from "./element.js";
export * from "./qr.js";
export * from "./sakura.js";
export { STRINGS } from "./strings.js";
export { resolveTheme } from "./theme.js";

defineIrohaConnect();
