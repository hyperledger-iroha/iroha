"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.dereferenceApiDOM = exports.default = void 0;
var _ramda = require("ramda");
var _apidomCore = require("@swagger-api/apidom-core");
var _File = _interopRequireDefault(require("../File.cjs"));
var plugins = _interopRequireWildcard(require("../util/plugins.cjs"));
var _UnmatchedDereferenceStrategyError = _interopRequireDefault(require("../errors/UnmatchedDereferenceStrategyError.cjs"));
var _DereferenceError = _interopRequireDefault(require("../errors/DereferenceError.cjs"));
var _index = _interopRequireDefault(require("../parse/index.cjs"));
var _util = require("../options/util.cjs");
var url = _interopRequireWildcard(require("../util/url.cjs"));
/**
 * Dereferences ApiDOM with all its external references.
 */
const dereferenceApiDOM = async (element, options) => {
  // @ts-ignore
  let parseResult = element;
  let surrogateWrapping = false;

  // wrap element into parse result
  if (!(0, _apidomCore.isParseResultElement)(element)) {
    const elementClone = (0, _apidomCore.cloneShallow)(element);
    elementClone.classes.push('result');
    parseResult = new _apidomCore.ParseResultElement([elementClone]);
    surrogateWrapping = true;
  }
  const file = new _File.default({
    uri: options.resolve.baseURI,
    parseResult,
    mediaType: options.parse.mediaType
  });
  const dereferenceStrategies = await plugins.filter('canDereference', [file, options], options.dereference.strategies);

  // we couldn't find any dereference strategy for this File
  if ((0, _ramda.isEmpty)(dereferenceStrategies)) {
    throw new _UnmatchedDereferenceStrategyError.default(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('dereference', [file, options], dereferenceStrategies);
    // unwrap the element from ParseResult assuming first element is the actual result
    return surrogateWrapping ? result.get(0) : result;
  } catch (error) {
    throw new _DereferenceError.default(`Error while dereferencing file "${file.uri}"`, {
      cause: error
    });
  }
};

/**
 * Dereferences a file with all its external references.
 */
exports.dereferenceApiDOM = dereferenceApiDOM;
const dereference = async (uri, options) => {
  const {
    refSet
  } = options.dereference;
  const sanitizedURI = url.sanitize(uri);
  let parseResult;

  // if refSet was provided, use it to avoid unnecessary parsing
  if (refSet !== null && refSet.has(sanitizedURI)) {
    // @ts-ignore
    ({
      value: parseResult
    } = refSet.find((0, _ramda.propEq)(sanitizedURI, 'uri')));
  } else {
    parseResult = await (0, _index.default)(uri, options);
  }
  const mergedOptions = (0, _util.merge)(options, {
    resolve: {
      baseURI: sanitizedURI
    },
    dereference: {
      // if refSet was not provided, then we can work in mutable mode
      immutable: options.dereference.immutable && refSet !== null
    }
  });
  return dereferenceApiDOM(parseResult, mergedOptions);
};
var _default = exports.default = dereference;