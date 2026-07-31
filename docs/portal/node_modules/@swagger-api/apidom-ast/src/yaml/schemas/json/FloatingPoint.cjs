"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Tag = _interopRequireDefault(require("../Tag.cjs"));
/* eslint-disable class-methods-use-this */
class FloatingPoint extends _Tag.default {
  static uri = 'tag:yaml.org,2002:float';
  test(node) {
    return /^-?(0|[1-9][0-9]*)(\.[0-9]*)?([eE][-+]?[0-9]+)?$/.test(node.content);
  }
  resolve(node) {
    const content = parseFloat(node.content);
    const nodeClone = node.clone();
    nodeClone.content = content;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = FloatingPoint;