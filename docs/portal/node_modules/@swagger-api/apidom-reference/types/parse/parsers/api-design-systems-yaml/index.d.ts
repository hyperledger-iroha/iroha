import { ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface APIDesignSystemsYAMLParserOptions extends Omit<ParserOptions, 'name'> {
}
/**
 * @public
 */
declare class APIDesignSystemsYAMLParser extends Parser {
    refractorOpts: object;
    constructor(options?: APIDesignSystemsYAMLParserOptions);
    canParse(file: File): Promise<boolean>;
    parse(file: File): Promise<ParseResultElement>;
}
export default APIDesignSystemsYAMLParser;
