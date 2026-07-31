"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _JSONSchema = _interopRequireDefault(require("./elements/JSONSchema.cjs"));
var _LinkDescription = _interopRequireDefault(require("./elements/LinkDescription.cjs"));
/**
 * @public
 */
const jsonSchema201909 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchema201909', _JSONSchema.default);
    base.register('linkDescription', _LinkDescription.default);
    return base;
  }
};
var _default = exports.default = jsonSchema201909;