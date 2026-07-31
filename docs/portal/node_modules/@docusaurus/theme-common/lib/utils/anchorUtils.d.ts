/**
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
/**
 * When the navbar is sticky, this ensures that when clicking a hash link,
 * we do not navigate to an anchor that will appear below the navbar.
 * This happens in particular for MDX headings and footnotes.
 *
 * See https://github.com/facebook/docusaurus/issues/11232
 * See also headings case https://x.com/JoshWComeau/status/1332015868725891076
 */
export declare function useAnchorTargetClassName(id: string | undefined): string | undefined;
//# sourceMappingURL=anchorUtils.d.ts.map