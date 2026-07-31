import { RefElement } from 'minim';
import type Parser from '../parse/parsers/Parser.ts';
import type Resolver from '../resolve/resolvers/Resolver.ts';
import type ResolveStrategy from '../resolve/strategies/ResolveStrategy.ts';
import type DereferenceStrategy from '../dereference/strategies/DereferenceStrategy.ts';
import type ReferenceSet from '../ReferenceSet.ts';
import type BundleStrategy from '../bundle/strategies/BundleStrategy.ts';
/**
 * @public
 */
export interface ReferenceParseOptions {
    mediaType: string;
    parsers: Array<Parser>;
    parserOpts: Record<string, any>;
}
/**
 * @public
 */
export interface ReferenceResolveOptions {
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
export interface ReferenceDereferenceOptions {
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
export interface ReferenceBundleOptions {
    strategies: Array<BundleStrategy>;
    refSet: null | ReferenceSet;
    maxDepth: number;
}
/**
 * @public
 */
export interface ReferenceOptions {
    readonly parse: ReferenceParseOptions;
    readonly resolve: ReferenceResolveOptions;
    readonly dereference: ReferenceDereferenceOptions;
    readonly bundle: ReferenceBundleOptions;
}
/**
 * @public
 */
declare const defaultOptions: ReferenceOptions;
export default defaultOptions;
