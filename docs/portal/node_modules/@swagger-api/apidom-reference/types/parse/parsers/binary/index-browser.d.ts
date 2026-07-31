import { ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * Everything that is not recognized by other parsers will be considered by this parser
 * as a binary data and will be encoded to Base64 format.
 * @public
 */
export interface BinaryParserOptions extends Omit<ParserOptions, 'name'> {
}
/**
 * @public
 */
declare class BinaryParser extends Parser {
    constructor(options?: BinaryParserOptions);
    canParse(file: File): boolean;
    parse(file: File): ParseResultElement;
}
export default BinaryParser;
