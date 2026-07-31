"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Standard extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'standard';
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get iri() {
    return this.get('iri');
  }
  set iri(iri) {
    this.set('iri', iri);
  }
  get level() {
    return this.get('level');
  }
  set level(level) {
    this.set('level', level);
  }
}
var _default = exports.default = Standard;