import { isEmpty } from 'ramda';
import { isParseResultElement, ParseResultElement, cloneShallow } from '@swagger-api/apidom-core';
import { merge as mergeOptions } from "../options/util.mjs";
import parse from "../parse/index.mjs";
import * as plugins from "../util/plugins.mjs";
import File from "../File.mjs";
import ResolveError from "../errors/ResolverError.mjs";
import UnmatchedResolveStrategyError from "../errors/UnmatchedResolveStrategyError.mjs";
import * as url from "../util/url.mjs";
/**
 * Resolves ApiDOM with all its external references.
 */
export const resolveApiDOM = async (element, options) => {
  // @ts-ignore
  let parseResult = element;

  // wrap element into parse result
  if (!isParseResultElement(element)) {
    // shallow clone of the element
    const elementClone = cloneShallow(element);
    elementClone.classes.push('result');
    parseResult = new ParseResultElement([elementClone]);
  }
  const sanitizedURI = url.sanitize(url.stripHash(options.resolve.baseURI));
  const file = new File({
    uri: sanitizedURI,
    parseResult,
    mediaType: options.parse.mediaType
  });
  const resolveStrategies = await plugins.filter('canResolve', [file, options], options.resolve.strategies);

  // we couldn't find any resolver for this File
  if (isEmpty(resolveStrategies)) {
    throw new UnmatchedResolveStrategyError(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('resolve', [file, options], resolveStrategies);
    return result;
  } catch (error) {
    throw new ResolveError(`Error while resolving file "${file.uri}"`, {
      cause: error
    });
  }
};

/**
 * Resolves a file with all its external references.
 */
const resolve = async (uri, options) => {
  const parseResult = await parse(uri, options);
  const mergedOptions = mergeOptions(options, {
    resolve: {
      baseURI: url.sanitize(uri)
    }
  });
  return resolveApiDOM(parseResult, mergedOptions);
};
export default resolve;