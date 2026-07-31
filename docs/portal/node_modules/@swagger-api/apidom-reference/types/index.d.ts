import { ParseResultElement, Element } from '@swagger-api/apidom-core';
import File from './File.ts';
import ReferenceSet from './ReferenceSet.ts';
import * as url from './util/url.ts';
export { url };
export { default as Parser } from './parse/parsers/Parser.ts';
export type { ParserOptions } from './parse/parsers/Parser.ts';
export { default as Resolver } from './resolve/resolvers/Resolver.ts';
export { default as HTTPResolver } from './resolve/resolvers/HTTPResolver.ts';
export type { HTTPResolverOptions } from './resolve/resolvers/HTTPResolver.ts';
export { default as ResolveStrategy } from './resolve/strategies/ResolveStrategy.ts';
export type { ResolverOptions } from './resolve/resolvers/Resolver.ts';
export type { ResolveStrategyOptions } from './resolve/strategies/ResolveStrategy.ts';
export { default as DereferenceStrategy } from './dereference/strategies/DereferenceStrategy.ts';
export type { DereferenceStrategyOptions } from './dereference/strategies/DereferenceStrategy.ts';
export { AncestorLineage as DereferenceAncestorLineage } from './dereference/util.ts';
export { default as BundleStrategy } from './bundle/strategies/BundleStrategy.ts';
export type { BundleStrategyOptions } from './bundle/strategies/BundleStrategy.ts';
export { default as options } from './options/index.ts';
export { merge as mergeOptions } from './options/util.ts';
export type { ReferenceOptions as ApiDOMReferenceOptions, ReferenceBundleOptions as ApiDOMReferenceBundleOptions, ReferenceDereferenceOptions as ApiDOMReferenceDereferenceOptions, ReferenceParseOptions as ApiDOMReferenceParseOptions, ReferenceResolveOptions as ApiDOMReferenceResolveOptions, } from './options/index.ts';
export { File };
export { default as Reference } from './Reference.ts';
export { ReferenceSet };
export type { FileOptions } from './File.ts';
export type { ReferenceOptions } from './Reference.ts';
export type { ReferenceSetOptions } from './ReferenceSet.ts';
export { default as BundleError } from './errors/BundleError.ts';
export { default as MaximumBundleDepthError } from './errors/MaximumBundleDepthError.ts';
export { default as UnmatchedBundleStrategyError } from './errors/UnmatchedBundleStrategyError.ts';
export { default as DereferenceError } from './errors/DereferenceError.ts';
export { default as EvaluationElementIdError } from './errors/EvaluationElementIdError.ts';
export { default as EvaluationJsonSchema$anchorError } from './errors/EvaluationJsonSchema$anchorError.ts';
export { default as EvaluationJsonSchemaUriError } from './errors/EvaluationJsonSchemaUriError.ts';
export { default as InvalidJsonSchema$anchorError } from './errors/InvalidJsonSchema$anchorError.ts';
export { default as JsonSchema$anchorError } from './errors/JsonSchema$anchorError.ts';
export { default as JsonSchemaURIError } from './errors/JsonSchemaUriError.ts';
export { default as MaximumDereferenceDepthError } from './errors/MaximumDereferenceDepthError.ts';
export { default as MaximumResolveDepthError } from './errors/MaximumResolveDepthError.ts';
export { default as ParseError } from './errors/ParseError.ts';
export { default as ParserError } from './errors/ParserError.ts';
export { default as PluginError } from './errors/PluginError.ts';
export { default as ResolveError } from './errors/ResolveError.ts';
export { default as ResolverError } from './errors/ResolverError.ts';
export { default as UnmatchedDereferenceStrategyError } from './errors/UnmatchedDereferenceStrategyError.ts';
export { default as UnmatchedResolveStrategyError } from './errors/UnmatchedResolveStrategyError.ts';
export { default as UnmatchedResolverError } from './errors/UnmatchedResolverError.ts';
/**
 * @public
 */
export declare const readFile: (uri: string, options?: {}) => Promise<Buffer | string>;
/**
 * @public
 */
export declare const parse: (uri: string, options?: {}) => Promise<ParseResultElement>;
/**
 * @public
 */
export declare const resolve: (uri: string, options?: {}) => Promise<ReferenceSet>;
/**
 * @public
 */
export declare const resolveApiDOM: <T extends Element>(element: T, options?: {}) => Promise<ReferenceSet>;
/**
 * @public
 */
export declare const dereference: (uri: string, options?: {}) => Promise<ParseResultElement>;
/**
 * @public
 */
export declare const dereferenceApiDOM: <T extends Element>(element: T, options?: {}) => Promise<T>;
/**
 * @public
 */
export declare const bundle: (uri: string, options?: {}) => Promise<ParseResultElement>;
