"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.resolveApiDOM = exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _util = require("../options/util.cjs");
var _index = _interopRequireDefault(require("../parse/index.cjs"));
var plugins = _interopRequireWildcard(require("../util/plugins.cjs"));
var _File = _interopRequireDefault(require("../File.cjs"));
var _ResolverError = _interopRequireDefault(require("../errors/ResolverError.cjs"));
var _UnmatchedResolveStrategyError = _interopRequireDefault(require("../errors/UnmatchedResolveStrategyError.cjs"));
var url = _interopRequireWildcard(require("../util/url.cjs"));
/**
 * Resolves ApiDOM with all its external references.
 */
const resolveApiDOM = async (element, options) => {
  // @ts-ignore
  let parseResult = element;

  // wrap element into parse result
  if (!(0, _apidomCore.isParseResultElement)(element)) {
    // shallow clone of the element
    const elementClone = (0, _apidomCore.cloneShallow)(element);
    elementClone.classes.push('result');
    parseResult = new _apidomCore.ParseResultElement([elementClone]);
  }
  const sanitizedURI = url.sanitize(url.stripHash(options.resolve.baseURI));
  const file = new _File.default({
    uri: sanitizedURI,
    parseResult,
    mediaType: options.parse.mediaType
  });
  const resolveStrategies = await plugins.filter('canResolve', [file, options], options.resolve.strategies);

  // we couldn't find any resolver for this File
  if ((0, _ramda.isEmpty)(resolveStrategies)) {
    throw new _UnmatchedResolveStrategyError.default(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('resolve', [file, options], resolveStrategies);
    return result;
  } catch (error) {
    throw new _ResolverError.default(`Error while resolving file "${file.uri}"`, {
      cause: error
    });
  }
};

/**
 * Resolves a file with all its external references.
 */
exports.resolveApiDOM = resolveApiDOM;
const resolve = async (uri, options) => {
  const parseResult = await (0, _index.default)(uri, options);
  const mergedOptions = (0, _util.merge)(options, {
    resolve: {
      baseURI: url.sanitize(uri)
    }
  });
  return resolveApiDOM(parseResult, mergedOptions);
};
var _default = exports.default = resolve;