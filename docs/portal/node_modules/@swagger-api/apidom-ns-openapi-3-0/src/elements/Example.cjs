"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Example extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'example';
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get value() {
    return this.get('value');
  }
  set value(value) {
    this.set('value', value);
  }
  get externalValue() {
    return this.get('externalValue');
  }
  set externalValue(externalValue) {
    this.set('externalValue', externalValue);
  }
}
var _default = exports.default = Example;