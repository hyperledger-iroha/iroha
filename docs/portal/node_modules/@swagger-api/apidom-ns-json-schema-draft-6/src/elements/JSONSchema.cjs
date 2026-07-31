"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class JSONSchema extends _apidomNsJsonSchemaDraft.JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONSchemaDraft6';
  }

  /**
   * Core vocabulary
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-01
   */

  get idProp() {
    throw new _apidomError.UnsupportedOperationError('id keyword from Core vocabulary has been renamed to $id.');
  }
  set idProp(id) {
    throw new _apidomError.UnsupportedOperationError('id keyword from Core vocabulary has been renamed to $id.');
  }
  get $id() {
    return this.get('$id');
  }
  set $id($id) {
    this.set('$id', $id);
  }

  /**
   * Validation vocabulary
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-01
   */

  /**
   *  Validation keywords for numeric instances (number and integer)
   */

  get exclusiveMaximum() {
    return this.get('exclusiveMaximum');
  }
  set exclusiveMaximum(exclusiveMaximum) {
    this.set('exclusiveMaximum', exclusiveMaximum);
  }
  get exclusiveMinimum() {
    return this.get('exclusiveMinimum');
  }
  set exclusiveMinimum(exclusiveMinimum) {
    this.set('exclusiveMinimum', exclusiveMinimum);
  }

  /**
   * Validation keywords for arrays
   */

  get containsProp() {
    return this.get('contains');
  }
  set containsProp(contains) {
    this.set('contains', contains);
  }
  get items() {
    return this.get('items');
  }
  set items(items) {
    this.set('items', items);
  }

  /**
   * Validation keywords for objects
   */

  get propertyNames() {
    return this.get('propertyNames');
  }
  set propertyNames(propertyNames) {
    this.set('propertyNames', propertyNames);
  }

  /**
   *  Validation keywords for any instance type
   */

  get const() {
    return this.get('const');
  }
  set const(constValue) {
    this.set('const', constValue);
  }
  get not() {
    return this.get('not');
  }
  set not(not) {
    this.set('not', not);
  }

  /**
   * Metadata keywords
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-01#section-7
   */

  get examples() {
    return this.get('examples');
  }
  set examples(examples) {
    this.set('examples', examples);
  }
}
var _default = exports.default = JSONSchema;