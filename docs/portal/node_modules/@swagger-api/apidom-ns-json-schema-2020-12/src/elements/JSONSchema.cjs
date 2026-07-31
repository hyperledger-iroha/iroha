"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2019-09");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class JSONSchema extends _apidomNsJsonSchema.JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONSchema202012';
  }

  /**
   * Core vocabulary
   *
   * URI: https://json-schema.org/draft/2020-12/vocab/core
   */

  get $dynamicAnchor() {
    return this.get('$dynamicAnchor');
  }
  set $dynamicAnchor($dynamicAnchor) {
    this.set('$dynamicAnchor', $dynamicAnchor);
  }
  get $recursiveAnchor() {
    throw new _apidomError.UnsupportedOperationError('$recursiveAnchor keyword from Core vocabulary has been renamed to $dynamicAnchor.');
  }
  set $recursiveAnchor($recursiveAnchor) {
    throw new _apidomError.UnsupportedOperationError('$recursiveAnchor keyword from Core vocabulary has been renamed to $dynamicAnchor.');
  }
  get $dynamicRef() {
    return this.get('$dynamicRef');
  }
  set $dynamicRef($dynamicRef) {
    this.set('$dynamicRef', $dynamicRef);
  }
  get $recursiveRef() {
    throw new _apidomError.UnsupportedOperationError('$recursiveRef keyword from Core vocabulary has been renamed to $dynamicRef.');
  }
  set $recursiveRef($recursiveRef) {
    throw new _apidomError.UnsupportedOperationError('$recursiveRef keyword from Core vocabulary has been renamed to $dynamicRef.');
  }

  /**
   * Applicator vocabulary
   *
   * URI: https://json-schema.org/draft/2020-12/vocab/applicator
   */

  get prefixItems() {
    return this.get('prefixItems');
  }
  set prefixItems(prefixItems) {
    this.set('prefixItems', prefixItems);
  }
}
var _default = exports.default = JSONSchema;