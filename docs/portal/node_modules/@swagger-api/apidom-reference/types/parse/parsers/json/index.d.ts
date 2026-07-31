import { ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface JSONParserOptions extends Omit<ParserOptions, 'name'> {
}
/**
 * @public
 */
declare class JSONParser extends Parser {
    syntacticAnalysis?: 'direct' | 'indirect';
    constructor(options?: JSONParserOptions);
    canParse(file: File): Promise<boolean>;
    parse(file: File): Promise<ParseResultElement>;
}
export default JSONParser;
