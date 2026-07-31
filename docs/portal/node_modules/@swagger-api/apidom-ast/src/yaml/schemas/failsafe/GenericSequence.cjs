"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Tag = _interopRequireDefault(require("../Tag.cjs"));
var _YamlTag = require("../../nodes/YamlTag.cjs");
/* eslint-disable class-methods-use-this */
class GenericSequence extends _Tag.default {
  static uri = 'tag:yaml.org,2002:seq';
  test(node) {
    return node.tag.kind === _YamlTag.YamlNodeKind.Sequence;
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = GenericSequence;