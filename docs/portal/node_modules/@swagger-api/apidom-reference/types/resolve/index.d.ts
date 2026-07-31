import { Element } from '@swagger-api/apidom-core';
import ReferenceSet from '../ReferenceSet.ts';
import type { ReferenceOptions } from '../options/index.ts';
/**
 * Resolves ApiDOM with all its external references.
 */
export declare const resolveApiDOM: <T extends Element>(element: T, options: ReferenceOptions) => Promise<ReferenceSet>;
/**
 * Resolves a file with all its external references.
 */
declare const resolve: (uri: string, options: ReferenceOptions) => Promise<ReferenceSet>;
export default resolve;
