import { ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface AsyncAPIYAML2ParserOptions extends Omit<ParserOptions, 'name'> {
}
/**
 * @public
 */
declare class AsyncAPIYAML2Parser extends Parser {
    refractorOpts: object;
    constructor(options?: AsyncAPIYAML2ParserOptions);
    canParse(file: File): Promise<boolean>;
    parse(file: File): Promise<ParseResultElement>;
}
export default AsyncAPIYAML2Parser;
