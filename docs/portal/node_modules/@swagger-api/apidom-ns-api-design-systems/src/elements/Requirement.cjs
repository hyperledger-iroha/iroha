"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Requirement extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'requirement';
  }
  get subject() {
    return this.get('subject');
  }
  set subject(subject) {
    this.set('subject', subject);
  }
  get level() {
    return this.get('level');
  }
  set level(level) {
    this.set('level', level);
  }
  get values() {
    return this.get('values');
  }
  set values(values) {
    this.set('values', values);
  }
  get follows() {
    return this.get('follows');
  }
  set follows(follows) {
    this.set('follows', follows);
  }
}
var _default = exports.default = Requirement;