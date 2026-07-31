import File from "./File.mjs";
import ReferenceSet from "./ReferenceSet.mjs";
import * as url from "./util/url.mjs";
import defaultOptions from "./options/index.mjs";
import { merge as mergeOptions } from "./options/util.mjs";
import parseFn from "./parse/index.mjs";
import resolveFn, { resolveApiDOM as resolveApiDOMFn } from "./resolve/index.mjs";
import { readFile as readFileFn } from "./resolve/util.mjs";
import dereferenceFn, { dereferenceApiDOM as dereferenceApiDOMFn } from "./dereference/index.mjs";
import bundleFn from "./bundle/index.mjs";
export { url };
export { default as Parser } from "./parse/parsers/Parser.mjs";
export { default as Resolver } from "./resolve/resolvers/Resolver.mjs";
export { default as HTTPResolver } from "./resolve/resolvers/HTTPResolver.mjs";
export { default as ResolveStrategy } from "./resolve/strategies/ResolveStrategy.mjs";
export { default as DereferenceStrategy } from "./dereference/strategies/DereferenceStrategy.mjs";
export { AncestorLineage as DereferenceAncestorLineage } from "./dereference/util.mjs";
export { default as BundleStrategy } from "./bundle/strategies/BundleStrategy.mjs";
export { default as options } from "./options/index.mjs";
export { merge as mergeOptions } from "./options/util.mjs";
export { File };
export { default as Reference } from "./Reference.mjs";
export { ReferenceSet };
export { default as BundleError } from "./errors/BundleError.mjs";
export { default as MaximumBundleDepthError } from "./errors/MaximumBundleDepthError.mjs";
export { default as UnmatchedBundleStrategyError } from "./errors/UnmatchedBundleStrategyError.mjs";
export { default as DereferenceError } from "./errors/DereferenceError.mjs";
export { default as EvaluationElementIdError } from "./errors/EvaluationElementIdError.mjs";
export { default as EvaluationJsonSchema$anchorError } from "./errors/EvaluationJsonSchema$anchorError.mjs";
export { default as EvaluationJsonSchemaUriError } from "./errors/EvaluationJsonSchemaUriError.mjs";
export { default as InvalidJsonSchema$anchorError } from "./errors/InvalidJsonSchema$anchorError.mjs";
export { default as JsonSchema$anchorError } from "./errors/JsonSchema$anchorError.mjs";
export { default as JsonSchemaURIError } from "./errors/JsonSchemaUriError.mjs";
export { default as MaximumDereferenceDepthError } from "./errors/MaximumDereferenceDepthError.mjs";
export { default as MaximumResolveDepthError } from "./errors/MaximumResolveDepthError.mjs";
export { default as ParseError } from "./errors/ParseError.mjs";
export { default as ParserError } from "./errors/ParserError.mjs";
export { default as PluginError } from "./errors/PluginError.mjs";
export { default as ResolveError } from "./errors/ResolveError.mjs";
export { default as ResolverError } from "./errors/ResolverError.mjs";
export { default as UnmatchedDereferenceStrategyError } from "./errors/UnmatchedDereferenceStrategyError.mjs";
export { default as UnmatchedResolveStrategyError } from "./errors/UnmatchedResolveStrategyError.mjs";
export { default as UnmatchedResolverError } from "./errors/UnmatchedResolverError.mjs";
/**
 * @public
 */
export const readFile = async (uri, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  const file = new File({
    uri: url.sanitize(uri)
  });
  return readFileFn(file, mergedOptions);
};

/**
 * @public
 */
export const parse = async (uri, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return parseFn(uri, mergedOptions);
};

/**
 * @public
 */
export const resolve = async (uri, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return resolveFn(uri, mergedOptions);
};

/**
 * @public
 */
export const resolveApiDOM = async (element, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return resolveApiDOMFn(element, mergedOptions);
};

/**
 * @public
 */
export const dereference = async (uri, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return dereferenceFn(uri, mergedOptions);
};

/**
 * @public
 */
export const dereferenceApiDOM = async (element, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return dereferenceApiDOMFn(element, mergedOptions);
};

/**
 * @public
 */
export const bundle = async (uri, options = {}) => {
  const mergedOptions = mergeOptions(defaultOptions, options);
  return bundleFn(uri, mergedOptions);
};