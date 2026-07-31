"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const jsonSchemaDraft6 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft6', _JSONSchema.default);
    base.register('jSONReference', _apidomNsJsonSchemaDraft.JSONReferenceElement);
    base.register('media', _apidomNsJsonSchemaDraft.MediaElement);
    base.register('linkDescription', _LinkDescription.default);
    return base;
  }
};
var _default = exports.default = jsonSchemaDraft6;