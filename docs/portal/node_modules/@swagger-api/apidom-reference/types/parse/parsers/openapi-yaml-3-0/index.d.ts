import { ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface OpenAPIYAML3_0ParserOptions extends Omit<ParserOptions, 'name'> {
}
/**
 * @public
 */
declare class OpenAPIYAML3_0Parser extends Parser {
    refractorOpts: object;
    constructor(options?: OpenAPIYAML3_0ParserOptions);
    canParse(file: File): Promise<boolean>;
    parse(file: File): Promise<ParseResultElement>;
}
export default OpenAPIYAML3_0Parser;
