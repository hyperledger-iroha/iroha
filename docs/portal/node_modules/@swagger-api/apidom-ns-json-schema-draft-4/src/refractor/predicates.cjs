"use strict";

exports.__esModule = true;
exports.isJSONReferenceLikeElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */

// eslint-disable-next-line import/prefer-default-export
/**
 * @public
 */
const isJSONReferenceLikeElement = element => {
  return (0, _apidomCore.isObjectElement)(element) && element.hasKey('$ref');
};
exports.isJSONReferenceLikeElement = isJSONReferenceLikeElement;