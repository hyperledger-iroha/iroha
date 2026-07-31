"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.parse = exports.namespace = exports.mediaTypes = exports.detectionRegExp = exports.detect = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomParserAdapterJson = require("@swagger-api/apidom-parser-adapter-json");
var _apidomNsOpenapi = _interopRequireWildcard(require("@swagger-api/apidom-ns-openapi-3-0"));
var _mediaTypes = _interopRequireDefault(require("./media-types.cjs"));
exports.mediaTypes = _mediaTypes.default;
/**
 * @public
 */
const detectionRegExp = exports.detectionRegExp = /"openapi"\s*:\s*"(?<version_json>3\.0\.(?:[1-9]\d*|0))"/;

/**
 * @public
 */
const detect = async source => detectionRegExp.test(source) && (await (0, _apidomParserAdapterJson.detect)(source));

/**
 * @public
 */
exports.detect = detect;
const parse = async (source, options = {}) => {
  const refractorOpts = (0, _ramda.propOr)({}, 'refractorOpts', options);
  const parserOpts = (0, _ramda.omit)(['refractorOpts'], options);
  const parseResultElement = await (0, _apidomParserAdapterJson.parse)(source, parserOpts);
  const {
    result
  } = parseResultElement;
  if ((0, _ramdaAdjunct.isNotUndefined)(result)) {
    const openApiElement = _apidomNsOpenapi.OpenApi3_0Element.refract(result, refractorOpts);
    openApiElement.classes.push('result');
    parseResultElement.replaceResult(openApiElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
exports.parse = parse;
const namespace = exports.namespace = (0, _apidomCore.createNamespace)(_apidomNsOpenapi.default);