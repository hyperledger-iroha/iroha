"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _Node = _interopRequireDefault(require("../../Node.cjs"));
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
class YamlDirective extends _Node.default {
  static type = 'directive';
  name;
  parameters;
  constructor({
    name,
    parameters,
    ...rest
  }) {
    super({
      ...rest
    });
    this.name = name;
    this.parameters = (0, _ramda.mergeRight)({
      version: undefined,
      handle: undefined,
      prefix: undefined
    }, parameters);
  }
}
var _default = exports.default = YamlDirective;