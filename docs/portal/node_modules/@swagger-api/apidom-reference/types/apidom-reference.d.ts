import { ApiDOMError } from '@swagger-api/apidom-error';
import { Element as Element_2 } from 'minim';
import { Element as Element_3 } from '@swagger-api/apidom-core';
import { ParseResultElement } from '@swagger-api/apidom-core';
import { RefElement } from 'minim';

/**
 * @public
 */
export declare interface ApiDOMReferenceBundleOptions {
    strategies: Array<BundleStrategy>;
    refSet: null | ReferenceSet;
    maxDepth: number;
}

/**
 * @public
 */
export declare interface ApiDOMReferenceDereferenceOptions {
    strategies: Array<DereferenceStrategy>;
    strategyOpts: Record<string, any>;
    refSet: null | ReferenceSet;
    maxDepth: number;
    circular: 'ignore' | 'replace' | 'error';
    circularReplacer: (ref: RefElement) => unknown;
    immutable: boolean;
    dereferenceOpts: Record<string, any>;
}

/**
 * @public
 */
export declare interface ApiDOMReferenceOptions {
    readonly parse: ApiDOMReferenceParseOptions;
    readonly resolve: ApiDOMReferenceResolveOptions;
    readonly dereference: ApiDOMReferenceDereferenceOptions;
    readonly bundle: ApiDOMReferenceBundleOptions;
}

/**
 * @public
 */
export declare interface ApiDOMReferenceParseOptions {
    mediaType: string;
    parsers: Array<Parser>;
    parserOpts: Record<string, any>;
}

/**
 * @public
 */
export declare interface ApiDOMReferenceResolveOptions {
    baseURI: string;
    resolvers: Array<Resolver>;
    resolverOpts: Record<string, any>;
    strategies: Array<ResolveStrategy>;
    strategyOpts: Record<string, any>;
    internal: boolean;
    external: boolean;
    maxDepth: number;
}

/**
 * @public
 */
export declare const bundle: (uri: string, options?: {}) => Promise<ParseResultElement>;

/**
 * @public
 */
export declare class BundleError extends ApiDOMError {
}

/**
 * @public
 */
export declare abstract class BundleStrategy {
    readonly name: string;
    constructor({ name }: BundleStrategyOptions);
    abstract canBundle(file: File_2, options: ApiDOMReferenceOptions): boolean;
    abstract bundle(file: File_2, options: ApiDOMReferenceOptions): Promise<ParseResultElement>;
}

/**
 * @public
 */
export declare interface BundleStrategyOptions {
    readonly name: string;
}

/**
 * Returns the current working directory (in Node) or the current page URL (in browsers).
 * @public
 */
declare const cwd: () => string;

/**
 * @public
 */
export declare const dereference: (uri: string, options?: {}) => Promise<ParseResultElement>;

/**
 * @public
 */
export declare class DereferenceAncestorLineage<T extends Element_3> extends Array<Set<T>> {
    includesCycle(element: T): boolean;
    includes(searchElement: Set<T> | T, fromIndex?: number): boolean;
    findItem(predicate: (item: T) => boolean): T | undefined;
}

/**
 * @public
 */
export declare const dereferenceApiDOM: <T extends Element_3>(element: T, options?: {}) => Promise<T>;

/**
 * @public
 */
export declare class DereferenceError extends ApiDOMError {
}

/**
 * @public
 */
export declare abstract class DereferenceStrategy {
    readonly name: string;
    constructor({ name }: DereferenceStrategyOptions);
    abstract canDereference(file: File_2, options: ApiDOMReferenceOptions): boolean;
    abstract dereference(file: File_2, options: ApiDOMReferenceOptions): Promise<Element_2>;
}

/**
 * @public
 */
export declare interface DereferenceStrategyOptions {
    readonly name: string;
}

/**
 * @public
 */
export declare class EvaluationElementIdError extends ApiDOMError {
}

/**
 * @public
 */
export declare class EvaluationJsonSchema$anchorError extends JsonSchema$anchorError {
}

/**
 * @public
 */
export declare class EvaluationJsonSchemaUriError extends JsonSchemaURIError {
}

/**
 * @public
 */
declare class File_2 {
    uri: string;
    mediaType: string;
    data?: Buffer | DataView | ArrayBuffer | string;
    parseResult?: ParseResultElement;
    constructor({ uri, mediaType, data, parseResult }: FileOptions);
    get extension(): string;
    toString(): string;
}
export { File_2 as File }

/**
 * This class represents a File object with url and data.
 * @public
 */
export declare interface FileOptions {
    readonly uri: string;
    readonly mediaType?: string;
    readonly data?: Buffer | DataView | ArrayBuffer | string;
    readonly parseResult?: ParseResultElement;
}

/**
 * Converts a filesystem path to a properly-encoded URL.
 *
 * This is intended to handle situations where resolver is called
 * with a filesystem path that contains characters which are not allowed in URLs.
 *
 * @example
 * The following filesystem paths would be converted to the following URLs:
 *```
 *    <"!@#$%^&*+=?'>.json              ==>   %3C%22!@%23$%25%5E&*+=%3F\'%3E.json
 *    C:\\My Documents\\File (1).json   ==>   C:/My%20Documents/File%20(1).json
 *    file://Project #42/file.json      ==>   file://Project%20%2342/file.json
 * ```
 * @public
 */
declare const fromFileSystemPath: (uri: string) => string;

/**
 * Returns the lower-cased file extension of the given URL,
 * or an empty string if it has no extension.
 * @public
 */
declare const getExtension: (url: string) => string;

/**
 * Returns the hash (URL fragment), of the given path.
 * If there is no hash, then the root hash ("#") is returned.
 * @public
 */
declare const getHash: (uri: string) => string;

/**
 * Returns the protocol of the given URL, or `undefined` if it has no protocol.
 * @public
 */
declare const getProtocol: (url: string) => string | undefined;

/**
 * Returns true if given URL has protocol.
 * @public
 */
declare const hasProtocol: (url: string) => boolean;

/**
 * @public
 */
export declare abstract class HTTPResolver extends Resolver {
    protected readonly timeout: number;
    protected readonly redirects: number;
    protected readonly withCredentials: boolean;
    constructor(options?: HTTPResolverOptions);
    canRead(file: File_2): boolean;
    abstract getHttpClient(): unknown;
}

/**
 * @public
 */
export declare interface HTTPResolverOptions extends ResolverOptions {
    readonly timeout?: number;
    readonly redirects?: number;
    readonly withCredentials?: boolean;
}

/**
 * @public
 */
export declare class InvalidJsonSchema$anchorError extends JsonSchema$anchorError {
    constructor(anchor: string);
}

/**
 * Determines whether the given path is a filesystem path.
 * This includes "file://" URLs.
 * @public
 */
declare const isFileSystemPath: (uri: string) => boolean;

/**
 * Determines whether the given URI is an HTTP(S) URL.
 * @public
 */
declare const isHttpUrl: (url: string) => boolean;

/**
 * Determines whether the given URI
 * @public
 */
declare const isURI: (uri: string) => boolean;

/**
 * @public
 */
export declare class JsonSchema$anchorError extends ApiDOMError {
}

/**
 * @public
 */
export declare class JsonSchemaURIError extends ApiDOMError {
}

/**
 * @public
 */
export declare class MaximumBundleDepthError extends BundleError {
}

/**
 * @public
 */
export declare class MaximumDereferenceDepthError extends DereferenceError {
}

/**
 * @public
 */
export declare class MaximumResolveDepthError extends ResolveError {
}

/**
 * @public
 */
export declare const mergeOptions: (lObj: ApiDOMReferenceOptions, rObj: Record<string, any>) => ApiDOMReferenceOptions;

/**
 * @public
 */
export declare const options: ApiDOMReferenceOptions;

/**
 * @public
 */
export declare const parse: (uri: string, options?: {}) => Promise<ParseResultElement>;

/**
 * @public
 */
export declare class ParseError extends ApiDOMError {
}

/**
 * @public
 */
export declare abstract class Parser {
    readonly name: string;
    /**
     * Whether to allow "empty" files. This includes zero-byte files.
     */
    allowEmpty: boolean;
    /**
     * Whether to generate source map during parsing.
     */
    sourceMap: boolean;
    /**
     * List of supported file extensions.
     */
    fileExtensions: string[];
    /**
     * List of supported media types.
     */
    mediaTypes: string[];
    constructor({ name, allowEmpty, sourceMap, fileExtensions, mediaTypes, }: ParserOptions);
    abstract canParse(file: File_2): boolean | Promise<boolean>;
    abstract parse(file: File_2): ParseResultElement | Promise<ParseResultElement>;
}

/**
 * @public
 */
export declare class ParserError extends ParseError {
}

/**
 * @public
 */
export declare interface ParserOptions {
    readonly name: string;
    readonly allowEmpty?: boolean;
    readonly sourceMap?: boolean;
    readonly fileExtensions?: string[];
    readonly mediaTypes?: string[];
}

/**
 * @public
 */
export declare class PluginError extends ApiDOMError {
    plugin: any;
    constructor(message: string, options: {
        cause?: Error;
        plugin: any;
    });
}

/**
 * @public
 */
export declare const readFile: (uri: string, options?: {}) => Promise<Buffer | string>;

/**
 * @public
 */
export declare class Reference<T = Element_3> {
    readonly uri: string;
    readonly depth: number;
    readonly value: T;
    refSet?: ReferenceSet;
    readonly errors: Array<Error>;
    constructor({ uri, depth, refSet, value }: ReferenceOptions<T>);
}

/**
 * @public
 */
export declare interface ReferenceOptions<T = Element_3> {
    readonly uri: string;
    readonly depth?: number;
    readonly refSet?: ReferenceSet;
    readonly value: T;
}

/**
 * @public
 */
export declare class ReferenceSet {
    rootRef?: Reference;
    readonly refs: Reference[];
    circular: boolean;
    constructor({ refs, circular }?: ReferenceSetOptions);
    get size(): number;
    add(reference: Reference): this;
    merge(anotherRefSet: this): this;
    has(thing: string | Reference): boolean;
    find(predicate: (value: Reference, index: number, obj: Reference[]) => boolean): Reference | undefined;
    values(): Generator<Reference<Element_2>, void, undefined>;
    clean(): void;
}

/**
 * @public
 */
export declare interface ReferenceSetOptions {
    readonly refs?: Reference[];
    readonly circular?: boolean;
}

/**
 * @public
 */
export declare const resolve: (uri: string, options?: {}) => Promise<ReferenceSet>;

/**
 * Resolves a target URI relative to a base URI in a manner similar to that of a Web browser resolving an anchor tag HREF.
 * @public
 */
declare const resolve_2: (from: string, to: string) => string;

/**
 * @public
 */
export declare const resolveApiDOM: <T extends Element_3>(element: T, options?: {}) => Promise<ReferenceSet>;

/**
 * @public
 */
export declare class ResolveError extends ApiDOMError {
}

/**
 * @public
 */
export declare abstract class Resolver {
    readonly name: string;
    constructor({ name }: ResolverOptions);
    abstract canRead(file: File_2): boolean;
    abstract read(file: File_2): Promise<Buffer>;
}

/**
 * @public
 */
export declare class ResolverError extends ResolveError {
}

/**
 * @public
 */
export declare interface ResolverOptions {
    readonly name: string;
}

/**
 * @public
 */
export declare abstract class ResolveStrategy {
    readonly name: string;
    constructor({ name }: ResolveStrategyOptions);
    abstract canResolve(file: File_2, options: ApiDOMReferenceOptions): boolean;
    abstract resolve(file: File_2, options: ApiDOMReferenceOptions): Promise<ReferenceSet>;
}

/**
 * @public
 */
export declare interface ResolveStrategyOptions {
    readonly name: string;
}

/**
 * Sanitizes/Encodes URI to it's url encoded form.
 *
 * The functional will compensate with the usecase when
 * already sanitized URI is passed to it,
 * by first unsatizing it and then performing sanitization again.
 * @public
 */
declare const sanitize: (uri: string) => string;

/**
 * Removes the hash (URL fragment), if any, from the given path.
 * @public
 */
declare const stripHash: (uri: string) => string;

/**
 * Converts a URL to a local filesystem path.
 * @public
 */
declare const toFileSystemPath: (uri: string, options?: ToFileSystemPathOptions) => string;

/**
 * @public
 */
declare interface ToFileSystemPathOptions {
    keepFileProtocol?: boolean;
    isWindows?: WindowsPredicate;
}

/**
 * @public
 */
export declare class UnmatchedBundleStrategyError extends BundleError {
}

/**
 * @public
 */
export declare class UnmatchedDereferenceStrategyError extends DereferenceError {
}

/**
 * @public
 */
export declare class UnmatchedResolverError extends ResolverError {
}

/**
 * @public
 */
export declare class UnmatchedResolveStrategyError extends ResolveError {
}

/**
 * Unsanitizes/Decodes URI to it's url encoded form.
 * This function already assumes that hash part of the URI
 * has been removed prior to transforming it to it's sanitized form.
 * @public
 */
declare const unsanitize: (uri: string) => string;

declare namespace url {
    export {
        WindowsPredicate,
        getProtocol,
        hasProtocol,
        getExtension,
        isFileSystemPath,
        isHttpUrl,
        isURI,
        ToFileSystemPathOptions,
        toFileSystemPath,
        fromFileSystemPath,
        getHash,
        stripHash,
        cwd,
        resolve_2 as resolve,
        sanitize,
        unsanitize
    }
}
export { url }

/**
 * SPDX-FileCopyrightText: Copyright (c) 2015 James Messinger
 *
 * SPDX-License-Identifier: MIT
 */
/**
 * @public
 */
declare type WindowsPredicate = () => boolean;

export { }
