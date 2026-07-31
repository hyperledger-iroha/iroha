import { ParseResultElement } from '@swagger-api/apidom-core';
import type { ReferenceOptions } from '../options/index.ts';
/**
 * Parses a file into ApiDOM.
 */
declare const parse: (uri: string, options: ReferenceOptions) => Promise<ParseResultElement>;
export default parse;
