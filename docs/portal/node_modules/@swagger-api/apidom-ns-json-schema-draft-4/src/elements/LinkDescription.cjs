"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00#section-5
 * @public
 */

class LinkDescription extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'linkDescription';
  }
  get href() {
    return this.get('href');
  }
  set href(href) {
    this.set('href', href);
  }
  get rel() {
    return this.get('rel');
  }
  set rel(rel) {
    this.set('rel', rel);
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get targetSchema() {
    return this.get('targetSchema');
  }
  set targetSchema(targetSchema) {
    this.set('targetSchema', targetSchema);
  }
  get mediaType() {
    return this.get('mediaType');
  }
  set mediaType(mediaType) {
    this.set('mediaType', mediaType);
  }
  get method() {
    return this.get('method');
  }
  set method(method) {
    this.set('method', method);
  }
  get encType() {
    return this.get('encType');
  }
  set encType(encType) {
    this.set('encType', encType);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
var _default = exports.default = LinkDescription;