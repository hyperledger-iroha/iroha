import ResolveStrategy, { ResolveStrategyOptions } from '../ResolveStrategy.ts';
import ReferenceSet from '../../../ReferenceSet.ts';
import File from '../../../File.ts';
import type { ReferenceOptions } from '../../../options/index.ts';
export type { default as ResolveStrategy, ResolveStrategyOptions } from '../ResolveStrategy.ts';
export type { default as ReferenceSet, ReferenceSetOptions } from '../../../ReferenceSet.ts';
export type { default as File, FileOptions } from '../../../File.ts';
export type { ReferenceOptions as ApiDOMReferenceOptions, ReferenceBundleOptions as ApiDOMReferenceBundleOptions, ReferenceDereferenceOptions as ApiDOMReferenceDereferenceOptions, ReferenceParseOptions as ApiDOMReferenceParseOptions, ReferenceResolveOptions as ApiDOMReferenceResolveOptions, } from '../../../options/index.ts';
export type { default as Parser, ParserOptions } from '../../../parse/parsers/Parser.ts';
export type { default as Resolver, ResolverOptions } from '../../resolvers/Resolver.ts';
export type { default as DereferenceStrategy, DereferenceStrategyOptions, } from '../../../dereference/strategies/DereferenceStrategy.ts';
export type { default as BundleStrategy, BundleStrategyOptions, } from '../../../bundle/strategies/BundleStrategy.ts';
export type { default as Reference, ReferenceOptions } from '../../../Reference.ts';
/**
 * @public
 */
export interface AsyncAPI2ResolveStrategyOptions extends Omit<ResolveStrategyOptions, 'name'> {
}
/**
 * @public
 */
declare class AsyncAPI2ResolveStrategy extends ResolveStrategy {
    constructor(options?: AsyncAPI2ResolveStrategyOptions);
    canResolve(file: File, options: ReferenceOptions): boolean;
    resolve(file: File, options: ReferenceOptions): Promise<ReferenceSet>;
}
export default AsyncAPI2ResolveStrategy;
