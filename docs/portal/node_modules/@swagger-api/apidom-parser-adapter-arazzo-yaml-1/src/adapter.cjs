"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.parse = exports.namespace = exports.mediaTypes = exports.detectionRegExp = exports.detect = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomParserAdapterYaml = require("@swagger-api/apidom-parser-adapter-yaml-1-2");
var _apidomNsArazzo = _interopRequireWildcard(require("@swagger-api/apidom-ns-arazzo-1"));
var _mediaTypes = _interopRequireDefault(require("./media-types.cjs"));
exports.mediaTypes = _mediaTypes.default;
/**
 * @public
 */
const detectionRegExp = exports.detectionRegExp = /(?<YAML>^(["']?)arazzo\2\s*:\s*(["']?)(?<version_yaml>1\.(?:[1-9]\d*|0)\.(?:[1-9]\d*|0))\3(?:\s+|$))|(?<JSON>"arazzo"\s*:\s*"(?<version_json>1\.(?:[1-9]\d*|0)\.(?:[1-9]\d*|0))")/m;

/**
 * @public
 */
const detect = async source => detectionRegExp.test(source) && (await (0, _apidomParserAdapterYaml.detect)(source));

/**
 * @public
 */
exports.detect = detect;
const parse = async (source, options = {}) => {
  const refractorOpts = (0, _ramda.propOr)({}, 'refractorOpts', options);
  const parserOpts = (0, _ramda.omit)(['refractorOpts'], options);
  const parseResultElement = await (0, _apidomParserAdapterYaml.parse)(source, parserOpts);
  const {
    result
  } = parseResultElement;
  if ((0, _ramdaAdjunct.isNotUndefined)(result)) {
    const arazzoSpecificationElement = _apidomNsArazzo.ArazzoSpecification1Element.refract(result, refractorOpts);
    arazzoSpecificationElement.classes.push('result');
    parseResultElement.replaceResult(arazzoSpecificationElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
exports.parse = parse;
const namespace = exports.namespace = (0, _apidomCore.createNamespace)(_apidomNsArazzo.default);