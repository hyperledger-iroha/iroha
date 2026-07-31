"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-6");
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const jsonSchemaDraft7 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft7', _JSONSchema.default);
    base.register('jSONReference', _apidomNsJsonSchemaDraft.JSONReferenceElement);
    base.register('linkDescription', _LinkDescription.default);
    return base;
  }
};
var _default = exports.default = jsonSchemaDraft7;