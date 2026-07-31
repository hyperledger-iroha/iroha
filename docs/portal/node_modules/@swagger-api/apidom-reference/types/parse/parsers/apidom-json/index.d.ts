import { Namespace, ParseResultElement } from '@swagger-api/apidom-core';
import Parser, { ParserOptions } from '../Parser.ts';
import File from '../../../File.ts';
export type { default as Parser, ParserOptions } from '../Parser.ts';
export type { default as File, FileOptions } from '../../../File.ts';
/**
 * @public
 */
export interface ApiDOMJSONParserOptions extends Omit<ParserOptions, 'name'> {
    readonly namespace?: Namespace;
}
/**
 * @public
 */
declare class ApiDOMJSONParser extends Parser {
    namespace: Namespace;
    ['apidom-json']: {
        namespace?: Namespace;
    };
    constructor(options?: ApiDOMJSONParserOptions);
    canParse(file: File): boolean;
    parse(file: File): ParseResultElement;
}
export default ApiDOMJSONParser;
