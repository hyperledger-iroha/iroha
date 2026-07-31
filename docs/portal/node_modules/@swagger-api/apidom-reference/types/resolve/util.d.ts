import File from '../File.ts';
import type { ReferenceOptions } from '../options/index.ts';
/**
 * Reads the given file, using the configured resolver plugins.
 */
export declare const readFile: (file: File, options: ReferenceOptions) => Promise<Buffer | string>;
