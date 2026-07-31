import { Element, ParseResultElement } from '@swagger-api/apidom-core';
import type { ReferenceOptions } from '../options/index.ts';
/**
 * Dereferences ApiDOM with all its external references.
 */
export declare const dereferenceApiDOM: <T extends Element>(element: T, options: ReferenceOptions) => Promise<T>;
/**
 * Dereferences a file with all its external references.
 */
declare const dereference: (uri: string, options: ReferenceOptions) => Promise<ParseResultElement>;
export default dereference;
