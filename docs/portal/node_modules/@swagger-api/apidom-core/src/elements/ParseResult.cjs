"use strict";

exports.__esModule = true;
exports.default = void 0;
var _minim = require("minim");
var _ramdaAdjunct = require("ramda-adjunct");
/**
 * @public
 */
class ParseResult extends _minim.ArrayElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'parseResult';
  }
  get api() {
    return this.children.filter(item => item.classes.contains('api')).first;
  }
  get results() {
    return this.children.filter(item => item.classes.contains('result'));
  }
  get result() {
    return this.results.first;
  }
  get annotations() {
    return this.children.filter(item => item.element === 'annotation');
  }
  get warnings() {
    return this.children.filter(item => item.element === 'annotation' && item.classes.contains('warning'));
  }
  get errors() {
    return this.children.filter(item => item.element === 'annotation' && item.classes.contains('error'));
  }
  get isEmpty() {
    return this.children.reject(item => item.element === 'annotation').isEmpty;
  }
  replaceResult(replacement) {
    const {
      result
    } = this;
    if ((0, _ramdaAdjunct.isUndefined)(result)) {
      return false;
    }

    // @ts-ignore
    const searchIndex = this.content.findIndex(e => e === result);
    if (searchIndex === -1) {
      return false;
    }
    this.content[searchIndex] = replacement;
    return true;
  }
}
var _default = exports.default = ParseResult;