"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.url = exports.resolveApiDOM = exports.resolve = exports.readFile = exports.parse = exports.options = exports.mergeOptions = exports.dereferenceApiDOM = exports.dereference = exports.bundle = exports.UnmatchedResolverError = exports.UnmatchedResolveStrategyError = exports.UnmatchedDereferenceStrategyError = exports.UnmatchedBundleStrategyError = exports.ResolverError = exports.Resolver = exports.ResolveStrategy = exports.ResolveError = exports.Reference = exports.PluginError = exports.ParserError = exports.Parser = exports.ParseError = exports.MaximumResolveDepthError = exports.MaximumDereferenceDepthError = exports.MaximumBundleDepthError = exports.JsonSchemaURIError = exports.JsonSchema$anchorError = exports.InvalidJsonSchema$anchorError = exports.HTTPResolver = exports.EvaluationJsonSchemaUriError = exports.EvaluationJsonSchema$anchorError = exports.EvaluationElementIdError = exports.DereferenceStrategy = exports.DereferenceError = exports.DereferenceAncestorLineage = exports.BundleStrategy = exports.BundleError = void 0;
var _File = _interopRequireDefault(require("./File.cjs"));
exports.File = _File.default;
var _ReferenceSet = _interopRequireDefault(require("./ReferenceSet.cjs"));
exports.ReferenceSet = _ReferenceSet.default;
var url = _interopRequireWildcard(require("./util/url.cjs"));
exports.url = url;
var _index = _interopRequireDefault(require("./options/index.cjs"));
exports.options = _index.default;
var _util = require("./options/util.cjs");
exports.mergeOptions = _util.merge;
var _index2 = _interopRequireDefault(require("./parse/index.cjs"));
var _index3 = _interopRequireWildcard(require("./resolve/index.cjs"));
var _util2 = require("./resolve/util.cjs");
var _index4 = _interopRequireWildcard(require("./dereference/index.cjs"));
var _index5 = _interopRequireDefault(require("./bundle/index.cjs"));
var _Parser = _interopRequireDefault(require("./parse/parsers/Parser.cjs"));
exports.Parser = _Parser.default;
var _Resolver = _interopRequireDefault(require("./resolve/resolvers/Resolver.cjs"));
exports.Resolver = _Resolver.default;
var _HTTPResolver = _interopRequireDefault(require("./resolve/resolvers/HTTPResolver.cjs"));
exports.HTTPResolver = _HTTPResolver.default;
var _ResolveStrategy = _interopRequireDefault(require("./resolve/strategies/ResolveStrategy.cjs"));
exports.ResolveStrategy = _ResolveStrategy.default;
var _DereferenceStrategy = _interopRequireDefault(require("./dereference/strategies/DereferenceStrategy.cjs"));
exports.DereferenceStrategy = _DereferenceStrategy.default;
var _util3 = require("./dereference/util.cjs");
exports.DereferenceAncestorLineage = _util3.AncestorLineage;
var _BundleStrategy = _interopRequireDefault(require("./bundle/strategies/BundleStrategy.cjs"));
exports.BundleStrategy = _BundleStrategy.default;
var _Reference = _interopRequireDefault(require("./Reference.cjs"));
exports.Reference = _Reference.default;
var _BundleError = _interopRequireDefault(require("./errors/BundleError.cjs"));
exports.BundleError = _BundleError.default;
var _MaximumBundleDepthError = _interopRequireDefault(require("./errors/MaximumBundleDepthError.cjs"));
exports.MaximumBundleDepthError = _MaximumBundleDepthError.default;
var _UnmatchedBundleStrategyError = _interopRequireDefault(require("./errors/UnmatchedBundleStrategyError.cjs"));
exports.UnmatchedBundleStrategyError = _UnmatchedBundleStrategyError.default;
var _DereferenceError = _interopRequireDefault(require("./errors/DereferenceError.cjs"));
exports.DereferenceError = _DereferenceError.default;
var _EvaluationElementIdError = _interopRequireDefault(require("./errors/EvaluationElementIdError.cjs"));
exports.EvaluationElementIdError = _EvaluationElementIdError.default;
var _EvaluationJsonSchema$anchorError = _interopRequireDefault(require("./errors/EvaluationJsonSchema$anchorError.cjs"));
exports.EvaluationJsonSchema$anchorError = _EvaluationJsonSchema$anchorError.default;
var _EvaluationJsonSchemaUriError = _interopRequireDefault(require("./errors/EvaluationJsonSchemaUriError.cjs"));
exports.EvaluationJsonSchemaUriError = _EvaluationJsonSchemaUriError.default;
var _InvalidJsonSchema$anchorError = _interopRequireDefault(require("./errors/InvalidJsonSchema$anchorError.cjs"));
exports.InvalidJsonSchema$anchorError = _InvalidJsonSchema$anchorError.default;
var _JsonSchema$anchorError = _interopRequireDefault(require("./errors/JsonSchema$anchorError.cjs"));
exports.JsonSchema$anchorError = _JsonSchema$anchorError.default;
var _JsonSchemaUriError = _interopRequireDefault(require("./errors/JsonSchemaUriError.cjs"));
exports.JsonSchemaURIError = _JsonSchemaUriError.default;
var _MaximumDereferenceDepthError = _interopRequireDefault(require("./errors/MaximumDereferenceDepthError.cjs"));
exports.MaximumDereferenceDepthError = _MaximumDereferenceDepthError.default;
var _MaximumResolveDepthError = _interopRequireDefault(require("./errors/MaximumResolveDepthError.cjs"));
exports.MaximumResolveDepthError = _MaximumResolveDepthError.default;
var _ParseError = _interopRequireDefault(require("./errors/ParseError.cjs"));
exports.ParseError = _ParseError.default;
var _ParserError = _interopRequireDefault(require("./errors/ParserError.cjs"));
exports.ParserError = _ParserError.default;
var _PluginError = _interopRequireDefault(require("./errors/PluginError.cjs"));
exports.PluginError = _PluginError.default;
var _ResolveError = _interopRequireDefault(require("./errors/ResolveError.cjs"));
exports.ResolveError = _ResolveError.default;
var _ResolverError = _interopRequireDefault(require("./errors/ResolverError.cjs"));
exports.ResolverError = _ResolverError.default;
var _UnmatchedDereferenceStrategyError = _interopRequireDefault(require("./errors/UnmatchedDereferenceStrategyError.cjs"));
exports.UnmatchedDereferenceStrategyError = _UnmatchedDereferenceStrategyError.default;
var _UnmatchedResolveStrategyError = _interopRequireDefault(require("./errors/UnmatchedResolveStrategyError.cjs"));
exports.UnmatchedResolveStrategyError = _UnmatchedResolveStrategyError.default;
var _UnmatchedResolverError = _interopRequireDefault(require("./errors/UnmatchedResolverError.cjs"));
exports.UnmatchedResolverError = _UnmatchedResolverError.default;
/**
 * @public
 */
const readFile = async (uri, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  const file = new _File.default({
    uri: url.sanitize(uri)
  });
  return (0, _util2.readFile)(file, mergedOptions);
};

/**
 * @public
 */
exports.readFile = readFile;
const parse = async (uri, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index2.default)(uri, mergedOptions);
};

/**
 * @public
 */
exports.parse = parse;
const resolve = async (uri, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index3.default)(uri, mergedOptions);
};

/**
 * @public
 */
exports.resolve = resolve;
const resolveApiDOM = async (element, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index3.resolveApiDOM)(element, mergedOptions);
};

/**
 * @public
 */
exports.resolveApiDOM = resolveApiDOM;
const dereference = async (uri, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index4.default)(uri, mergedOptions);
};

/**
 * @public
 */
exports.dereference = dereference;
const dereferenceApiDOM = async (element, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index4.dereferenceApiDOM)(element, mergedOptions);
};

/**
 * @public
 */
exports.dereferenceApiDOM = dereferenceApiDOM;
const bundle = async (uri, options = {}) => {
  const mergedOptions = (0, _util.merge)(_index.default, options);
  return (0, _index5.default)(uri, mergedOptions);
};
exports.bundle = bundle;