/**
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */
import React from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import {useAnchorTargetClassName} from '@docusaurus/theme-common';
export default function MDXA(props) {
  // MDX Footnotes have ids such as <a id="user-content-fn-1-953011" ...>
  const anchorTargetClassName = useAnchorTargetClassName(props.id);
  return (
    <Link {...props} className={clsx(anchorTargetClassName, props.className)} />
  );
}
