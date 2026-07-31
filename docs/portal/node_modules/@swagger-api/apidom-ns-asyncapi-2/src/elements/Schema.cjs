"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
/**
 * @public
 */
class Schema extends _apidomNsJsonSchemaDraft.JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'schema';
    this.classes.push('json-schema-draft-7');
  }

  /**
   * Validation vocabulary
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01
   */

  /**
   *  Validation Keywords for Applying Subschemas With Boolean Logic
   *
   *  URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.7
   */

  get not() {
    return this.get('not');
  }

  /**
   *  Validation Keywords for Applying Subschemas Conditionally
   *
   *  URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.6
   */

  get if() {
    return this.get('if');
  }
  set if(ifValue) {
    this.set('if', ifValue);
  }
  get then() {
    return this.get('then');
  }
  set then(then) {
    this.set('then', then);
  }
  get else() {
    return this.get('else');
  }
  set else(elseValue) {
    this.set('else', elseValue);
  }

  /**
   * Validation Keywords for Arrays
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.4
   */

  get items() {
    return this.get('items');
  }
  set items(items) {
    this.set('items', items);
  }
  get additionalItems() {
    return this.get('additionalItems');
  }
  set additionalItems(additionalItems) {
    this.set('additionalItems', additionalItems);
  }
  get containsProp() {
    return this.get('contains');
  }
  set containsProp(contains) {
    this.set('contains', contains);
  }

  /**
   * Validation Keywords for Objects
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.5
   */

  get propertyNames() {
    return this.get('propertyNames');
  }
  set propertyNames(propertyNames) {
    this.set('propertyNames', propertyNames);
  }

  /**
   * AsyncAPI vocabulary
   *
   * URI: https://github.com/asyncapi/spec/blob/master/spec/asyncapi.md#fixed-fields-21
   */

  get discriminator() {
    return this.get('discriminator');
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  get deprecated() {
    return this.get('deprecated');
  }
}
var _default = exports.default = Schema;