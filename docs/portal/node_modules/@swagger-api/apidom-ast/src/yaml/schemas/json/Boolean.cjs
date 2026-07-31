"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Tag = _interopRequireDefault(require("../Tag.cjs"));
/* eslint-disable class-methods-use-this */
class Boolean extends _Tag.default {
  static uri = 'tag:yaml.org,2002:bool';
  test(node) {
    return /^(true|false)$/.test(node.content);
  }
  resolve(node) {
    const content = node.content === 'true';
    const nodeClone = node.clone();
    nodeClone.content = content;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = Boolean;