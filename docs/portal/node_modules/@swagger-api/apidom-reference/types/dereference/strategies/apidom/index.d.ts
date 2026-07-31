import { Element } from '@swagger-api/apidom-core';
import DereferenceStrategy, { DereferenceStrategyOptions } from '../DereferenceStrategy.ts';
import File from '../../../File.ts';
import ApiDOMDereferenceVisitor from './visitor.ts';
import type { ReferenceOptions } from '../../../options/index.ts';
export type { default as DereferenceStrategy, DereferenceStrategyOptions, } from '../DereferenceStrategy.ts';
export type { default as File, FileOptions } from '../../../File.ts';
export type { default as Reference, ReferenceOptions } from '../../../Reference.ts';
export type { default as ReferenceSet, ReferenceSetOptions } from '../../../ReferenceSet.ts';
export type { ApiDOMDereferenceVisitorOptions } from './visitor.ts';
export type { ReferenceOptions as ApiDOMReferenceOptions, ReferenceBundleOptions as ApiDOMReferenceBundleOptions, ReferenceDereferenceOptions as ApiDOMReferenceDereferenceOptions, ReferenceParseOptions as ApiDOMReferenceParseOptions, ReferenceResolveOptions as ApiDOMReferenceResolveOptions, } from '../../../options/index.ts';
export type { default as Parser, ParserOptions } from '../../../parse/parsers/Parser.ts';
export type { default as Resolver, ResolverOptions } from '../../../resolve/resolvers/Resolver.ts';
export type { default as ResolveStrategy, ResolveStrategyOptions, } from '../../../resolve/strategies/ResolveStrategy.ts';
export type { default as BundleStrategy, BundleStrategyOptions, } from '../../../bundle/strategies/BundleStrategy.ts';
/**
 * @public
 */
export interface ApiDOMDereferenceStrategyOptions extends Omit<DereferenceStrategyOptions, 'name'> {
}
/**
 * @public
 */
declare class ApiDOMDereferenceStrategy extends DereferenceStrategy {
    constructor(options?: ApiDOMDereferenceStrategyOptions);
    canDereference(file: File): boolean;
    dereference(file: File, options: ReferenceOptions): Promise<Element>;
}
export { ApiDOMDereferenceVisitor };
export default ApiDOMDereferenceStrategy;
