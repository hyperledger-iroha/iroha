import { Element } from '@swagger-api/apidom-core';
import DereferenceStrategy, { DereferenceStrategyOptions } from '../DereferenceStrategy.ts';
import File from '../../../File.ts';
import OpenAPI3_0DereferenceVisitor from './visitor.ts';
import type { ReferenceOptions } from '../../../options/index.ts';
export type { default as DereferenceStrategy, DereferenceStrategyOptions, } from '../DereferenceStrategy.ts';
export type { default as File, FileOptions } from '../../../File.ts';
export type { default as Reference, ReferenceOptions } from '../../../Reference.ts';
export type { default as ReferenceSet, ReferenceSetOptions } from '../../../ReferenceSet.ts';
export type { OpenAPI3_0DereferenceVisitorOptions, mutationReplacer } from './visitor.ts';
export type { ReferenceOptions as ApiDOMReferenceOptions, ReferenceBundleOptions as ApiDOMReferenceBundleOptions, ReferenceDereferenceOptions as ApiDOMReferenceDereferenceOptions, ReferenceParseOptions as ApiDOMReferenceParseOptions, ReferenceResolveOptions as ApiDOMReferenceResolveOptions, } from '../../../options/index.ts';
export type { default as Parser, ParserOptions } from '../../../parse/parsers/Parser.ts';
export type { default as Resolver, ResolverOptions } from '../../../resolve/resolvers/Resolver.ts';
export type { default as ResolveStrategy, ResolveStrategyOptions, } from '../../../resolve/strategies/ResolveStrategy.ts';
export type { default as BundleStrategy, BundleStrategyOptions, } from '../../../bundle/strategies/BundleStrategy.ts';
export type { AncestorLineage } from '../../util.ts';
/**
 * @public
 */
export interface OpenAPI3_0DereferenceStrategyOptions extends Omit<DereferenceStrategyOptions, 'name'> {
}
/**
 * @public
 */
declare class OpenAPI3_0DereferenceStrategy extends DereferenceStrategy {
    constructor(options?: OpenAPI3_0DereferenceStrategyOptions);
    canDereference(file: File): boolean;
    dereference(file: File, options: ReferenceOptions): Promise<Element>;
}
export { OpenAPI3_0DereferenceVisitor };
export default OpenAPI3_0DereferenceStrategy;
