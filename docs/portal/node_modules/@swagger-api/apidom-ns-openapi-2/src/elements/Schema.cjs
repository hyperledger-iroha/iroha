"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class Schema extends _apidomNsJsonSchemaDraft.JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'schema';
    this.classes.push('json-schema-draft-4');
  }

  /**
   * Core vocabulary
   *
   * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
   */
  get idProp() {
    throw new _apidomError.UnsupportedOperationError('idProp getter in Schema class is not not supported.');
  }
  set idProp(idProps) {
    throw new _apidomError.UnsupportedOperationError('idProp setter in Schema class is not not supported.');
  }
  get $schema() {
    throw new _apidomError.UnsupportedOperationError('$schema getter in Schema class is not not supported.');
  }
  set $schema($schema) {
    throw new _apidomError.UnsupportedOperationError('$schema setter in Schema class is not not supported.');
  }

  /**
   * Validation keywords for arrays
   */

  get additionalItems() {
    throw new _apidomError.UnsupportedOperationError('additionalItems getter in Schema class is not not supported.');
  }
  set additionalItems(additionalItems) {
    throw new _apidomError.UnsupportedOperationError('additionalItems setter in Schema class is not not supported.');
  }

  /**
   * Validation keywords for objects
   */

  get patternProperties() {
    throw new _apidomError.UnsupportedOperationError('patternProperties getter in Schema class is not not supported.');
  }
  set patternProperties(patternProperties) {
    throw new _apidomError.UnsupportedOperationError('patternProperties setter in Schema class is not not supported.');
  }
  get dependencies() {
    throw new _apidomError.UnsupportedOperationError('dependencies getter in Schema class is not not supported.');
  }
  set dependencies(dependencies) {
    throw new _apidomError.UnsupportedOperationError('dependencies setter in Schema class is not not supported.');
  }

  /**
   *  Validation keywords for any instance type
   */
  get anyOf() {
    throw new _apidomError.UnsupportedOperationError('anyOf getter in Schema class is not not supported.');
  }
  set anyOf(anyOf) {
    throw new _apidomError.UnsupportedOperationError('anyOf setter in Schema class is not not supported.');
  }
  get oneOf() {
    throw new _apidomError.UnsupportedOperationError('oneOf getter in Schema class is not not supported.');
  }
  set oneOf(oneOf) {
    throw new _apidomError.UnsupportedOperationError('oneOf setter in Schema class is not not supported.');
  }
  get not() {
    throw new _apidomError.UnsupportedOperationError('not getter in Schema class is not not supported.');
  }
  set not(not) {
    throw new _apidomError.UnsupportedOperationError('not setter in Schema class is not not supported.');
  }
  get definitions() {
    throw new _apidomError.UnsupportedOperationError('definitions getter in Schema class is not not supported.');
  }
  set definitions(definitions) {
    throw new _apidomError.UnsupportedOperationError('definitions setter in Schema class is not not supported.');
  }

  /**
   * JSON Hyper-Schema
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00
   */

  get base() {
    throw new _apidomError.UnsupportedOperationError('base getter in Schema class is not not supported.');
  }
  set base(base) {
    throw new _apidomError.UnsupportedOperationError('base setter in Schema class is not not supported.');
  }
  get links() {
    throw new _apidomError.UnsupportedOperationError('links getter in Schema class is not not supported.');
  }
  set links(links) {
    throw new _apidomError.UnsupportedOperationError('links setter in Schema class is not not supported.');
  }
  get media() {
    throw new _apidomError.UnsupportedOperationError('media getter in Schema class is not not supported.');
  }
  set media(media) {
    throw new _apidomError.UnsupportedOperationError('media setter in Schema class is not not supported.');
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = Schema;