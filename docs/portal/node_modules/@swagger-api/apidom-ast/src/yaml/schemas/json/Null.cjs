"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Tag = _interopRequireDefault(require("../Tag.cjs"));
/* eslint-disable class-methods-use-this */
class Null extends _Tag.default {
  static uri = 'tag:yaml.org,2002:null';
  test(node) {
    return /^null$/.test(node.content);
  }
  resolve(node) {
    const nodeClone = node.clone();
    nodeClone.content = null;
    return nodeClone;
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = Null;