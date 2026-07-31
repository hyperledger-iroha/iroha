"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _File = _interopRequireDefault(require("../File.cjs"));
var plugins = _interopRequireWildcard(require("../util/plugins.cjs"));
var _UnmatchedBundleStrategyError = _interopRequireDefault(require("../errors/UnmatchedBundleStrategyError.cjs"));
var _BundleError = _interopRequireDefault(require("../errors/BundleError.cjs"));
var _index = _interopRequireDefault(require("../parse/index.cjs"));
var _util = require("../options/util.cjs");
var url = _interopRequireWildcard(require("../util/url.cjs"));
/**
 * Bundle a file with all its external references to a compound document.
 */
const bundle = async (uri, options) => {
  const {
    refSet
  } = options.bundle;
  const sanitizedURI = url.sanitize(uri);
  const mergedOptions = (0, _util.merge)(options, {
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
    } = refSet.find((0, _ramda.propEq)(sanitizedURI, 'uri')));
  } else {
    parseResult = await (0, _index.default)(uri, mergedOptions);
  }
  const file = new _File.default({
    uri: mergedOptions.resolve.baseURI,
    parseResult,
    mediaType: mergedOptions.parse.mediaType
  });
  const bundleStrategies = await plugins.filter('canBundle', [file, mergedOptions], mergedOptions.bundle.strategies);

  // we couldn't find any bundle strategy for this File
  if ((0, _ramda.isEmpty)(bundleStrategies)) {
    throw new _UnmatchedBundleStrategyError.default(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('bundle', [file, mergedOptions], bundleStrategies);
    return result;
  } catch (error) {
    throw new _BundleError.default(`Error while bundling file "${file.uri}"`, {
      cause: error
    });
  }
};
var _default = exports.default = bundle;