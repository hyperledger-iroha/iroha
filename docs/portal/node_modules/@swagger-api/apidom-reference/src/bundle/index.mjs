import { isEmpty, propEq } from 'ramda';
import File from "../File.mjs";
import * as plugins from "../util/plugins.mjs";
import UnmatchedBundleStrategyError from "../errors/UnmatchedBundleStrategyError.mjs";
import BundleError from "../errors/BundleError.mjs";
import parse from "../parse/index.mjs";
import { merge as mergeOptions } from "../options/util.mjs";
import * as url from "../util/url.mjs";
/**
 * Bundle a file with all its external references to a compound document.
 */
const bundle = async (uri, options) => {
  const {
    refSet
  } = options.bundle;
  const sanitizedURI = url.sanitize(uri);
  const mergedOptions = mergeOptions(options, {
    resolve: {
      baseURI: sanitizedURI
    }
  });
  let parseResult;

  // if refSet was provided, use it to avoid unnecessary parsing
  if (refSet !== null && refSet.has(sanitizedURI)) {
    // @ts-ignore
    ({
      value: parseResult
    } = refSet.find(propEq(sanitizedURI, 'uri')));
  } else {
    parseResult = await parse(uri, mergedOptions);
  }
  const file = new File({
    uri: mergedOptions.resolve.baseURI,
    parseResult,
    mediaType: mergedOptions.parse.mediaType
  });
  const bundleStrategies = await plugins.filter('canBundle', [file, mergedOptions], mergedOptions.bundle.strategies);

  // we couldn't find any bundle strategy for this File
  if (isEmpty(bundleStrategies)) {
    throw new UnmatchedBundleStrategyError(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('bundle', [file, mergedOptions], bundleStrategies);
    return result;
  } catch (error) {
    throw new BundleError(`Error while bundling file "${file.uri}"`, {
      cause: error
    });
  }
};
export default bundle;