"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _JSONReference = _interopRequireDefault(require("./elements/JSONReference.cjs"));
var _Media = _interopRequireDefault(require("./elements/Media.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const jsonSchemaDraft4 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft4', _JSONSchema.default);
    base.register('jSONReference', _JSONReference.default);
    base.register('media', _Media.default);
    base.register('linkDescription', _LinkDescription.default);
    return base;
  }
};
var _default = exports.default = jsonSchemaDraft4;