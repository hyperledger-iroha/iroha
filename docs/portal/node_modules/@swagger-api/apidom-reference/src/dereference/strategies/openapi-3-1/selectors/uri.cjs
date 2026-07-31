"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.evaluate = exports.JsonSchemaUriError = void 0;
var _ramdaAdjunct = require("ramda-adjunct");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
var _modern = require("@swagger-api/apidom-json-pointer/modern");
var url = _interopRequireWildcard(require("../../../../util/url.cjs"));
var _EvaluationJsonSchemaUriError = _interopRequireDefault(require("../../../../errors/EvaluationJsonSchemaUriError.cjs"));
exports.EvaluationJsonSchemaUriError = _EvaluationJsonSchemaUriError.default;
var _$anchor = require("./$anchor.cjs");
var _util = require("../util.cjs");
var _JsonSchemaUriError = _interopRequireDefault(require("../../../../errors/JsonSchemaUriError.cjs"));
exports.JsonSchemaUriError = _JsonSchemaUriError.default;
/**
 * Evaluates JSON Schema $ref containing unknown URI against ApiDOM fragment.
 * @public
 */
const evaluate = (uri, element) => {
  const {
    cache
  } = evaluate;
  const uriStrippedHash = url.stripHash(uri);
  const isSchemaElementWith$id = e => (0, _apidomNsOpenapi.isSchemaElement)(e) && typeof e.$id !== 'undefined';

  // warm the cache
  if (!cache.has(element)) {
    const schemaObjectElements = (0, _apidomCore.filter)(isSchemaElementWith$id, element);
    cache.set(element, Array.from(schemaObjectElements));
  }

  // search for the matching schema
  const result = cache.get(element).find(e => {
    const $idBaseURI = (0, _util.resolveSchema$idField)(uriStrippedHash, e);
    return $idBaseURI === uriStrippedHash;
  });
  if ((0, _ramdaAdjunct.isUndefined)(result)) {
    throw new _EvaluationJsonSchemaUriError.default(`Evaluation failed on URI: "${uri}"`);
  }
  if ((0, _$anchor.isAnchor)((0, _$anchor.uriToAnchor)(uri))) {
    // we're dealing with JSON Schema $anchor here
    return (0, _$anchor.evaluate)((0, _$anchor.uriToAnchor)(uri), result);
  }
  return (0, _modern.evaluate)(result, _modern.URIFragmentIdentifier.fromURIReference(uri));
};
exports.evaluate = evaluate;
evaluate.cache = new WeakMap();