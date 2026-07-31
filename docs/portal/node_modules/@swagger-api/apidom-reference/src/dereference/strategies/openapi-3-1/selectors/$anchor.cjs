"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.uriToAnchor = exports.parse = exports.isAnchor = exports.evaluate = exports.JsonSchema$anchorError = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
var _url = require("../../../../util/url.cjs");
var _EvaluationJsonSchema$anchorError = _interopRequireDefault(require("../../../../errors/EvaluationJsonSchema$anchorError.cjs"));
exports.EvaluationJsonSchema$anchorError = _EvaluationJsonSchema$anchorError.default;
var _InvalidJsonSchema$anchorError = _interopRequireDefault(require("../../../../errors/InvalidJsonSchema$anchorError.cjs"));
exports.InvalidJsonSchema$anchorError = _InvalidJsonSchema$anchorError.default;
var _JsonSchema$anchorError = _interopRequireDefault(require("../../../../errors/JsonSchema$anchorError.cjs"));
exports.JsonSchema$anchorError = _JsonSchema$anchorError.default;
/**
 * @public
 */
const isAnchor = uri => {
  /**
   *  MUST start with a letter ([A-Za-z]) or underscore ("_"), followed by any number of letters,
   *  digits ([0-9]), hyphens ("-"), underscores ("_"), and periods (".").
   *
   *  https://json-schema.org/draft/2020-12/json-schema-core.html#rfc.section.8.2.2
   */
  return /^[A-Za-z_][A-Za-z_0-9.-]*$/.test(uri);
};

/**
 * @public
 */
exports.isAnchor = isAnchor;
const uriToAnchor = uri => {
  const hash = (0, _url.getHash)(uri);
  return (0, _ramdaAdjunct.trimCharsStart)('#', hash);
};

/**
 * @public
 */
exports.uriToAnchor = uriToAnchor;
const parse = anchor => {
  if (!isAnchor(anchor)) {
    throw new _InvalidJsonSchema$anchorError.default(anchor);
  }
  return anchor;
};

/**
 * Evaluates JSON Schema $anchor against ApiDOM fragment.
 * @public
 */
exports.parse = parse;
const evaluate = (anchor, element) => {
  const token = parse(anchor);

  // @ts-ignore
  const result = (0, _apidomCore.find)(e => (0, _apidomNsOpenapi.isSchemaElement)(e) && (0, _apidomCore.toValue)(e.$anchor) === token, element);
  if ((0, _ramdaAdjunct.isUndefined)(result)) {
    throw new _EvaluationJsonSchema$anchorError.default(`Evaluation failed on token: "${token}"`);
  }

  // @ts-ignore
  return result;
};
exports.evaluate = evaluate;