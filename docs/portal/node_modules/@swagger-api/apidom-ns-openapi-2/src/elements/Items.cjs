"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class Items extends _apidomNsJsonSchemaDraft.JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'items';
    this.classes.push('json-schema-draft-4');
  }

  /**
   * Core vocabulary
   *
   * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
   */
  get idProp() {
    throw new _apidomError.UnsupportedOperationError('idProp getter in Items class is not not supported.');
  }
  set idProp(idProps) {
    throw new _apidomError.UnsupportedOperationError('idProp setter in Items class is not not supported.');
  }
  get $schema() {
    throw new _apidomError.UnsupportedOperationError('$schema getter in Items class is not not supported.');
  }
  set $schema($schema) {
    throw new _apidomError.UnsupportedOperationError('$schema setter in Items class is not not supported.');
  }

  /**
   * Validation keywords for arrays
   */

  get additionalItems() {
    throw new _apidomError.UnsupportedOperationError('additionalItems getter in Items class is not not supported.');
  }
  set additionalItems(additionalItems) {
    throw new _apidomError.UnsupportedOperationError('additionalItems setter in Items class is not not supported.');
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

  get maxProperties() {
    throw new _apidomError.UnsupportedOperationError('maxProperties getter in Items class is not not supported.');
  }
  set maxProperties(maxProperties) {
    throw new _apidomError.UnsupportedOperationError('maxProperties setter in Items class is not not supported.');
  }
  get minProperties() {
    throw new _apidomError.UnsupportedOperationError('minProperties getter in Items class is not not supported.');
  }
  set minProperties(minProperties) {
    throw new _apidomError.UnsupportedOperationError('minProperties setter in Items class is not not supported.');
  }
  get required() {
    throw new _apidomError.UnsupportedOperationError('required getter in Items class is not not supported.');
  }
  set required(required) {
    throw new _apidomError.UnsupportedOperationError('required setter in Items class is not not supported.');
  }
  get properties() {
    throw new _apidomError.UnsupportedOperationError('properties getter in Items class is not not supported.');
  }
  set properties(properties) {
    throw new _apidomError.UnsupportedOperationError('properties setter in Items class is not not supported.');
  }
  get additionalProperties() {
    throw new _apidomError.UnsupportedOperationError('additionalProperties getter in Items class is not not supported.');
  }
  set additionalProperties(additionalProperties) {
    throw new _apidomError.UnsupportedOperationError('additionalProperties setter in Items class is not not supported.');
  }
  get patternProperties() {
    throw new _apidomError.UnsupportedOperationError('patternProperties getter in Items class is not not supported.');
  }
  set patternProperties(patternProperties) {
    throw new _apidomError.UnsupportedOperationError('patternProperties setter in Items class is not not supported.');
  }
  get dependencies() {
    throw new _apidomError.UnsupportedOperationError('dependencies getter in Items class is not not supported.');
  }
  set dependencies(dependencies) {
    throw new _apidomError.UnsupportedOperationError('dependencies setter in Items class is not not supported.');
  }

  /**
   *  Validation keywords for any instance type
   */

  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get allOf() {
    throw new _apidomError.UnsupportedOperationError('allOf getter in Items class is not not supported.');
  }
  set allOf(allOf) {
    throw new _apidomError.UnsupportedOperationError('allOf setter in Items class is not not supported.');
  }
  get anyOf() {
    throw new _apidomError.UnsupportedOperationError('anyOf getter in Items class is not not supported.');
  }
  set anyOf(anyOf) {
    throw new _apidomError.UnsupportedOperationError('anyOf setter in Items class is not not supported.');
  }
  get oneOf() {
    throw new _apidomError.UnsupportedOperationError('oneOf getter in Items class is not not supported.');
  }
  set oneOf(oneOf) {
    throw new _apidomError.UnsupportedOperationError('oneOf setter in Items class is not not supported.');
  }
  get not() {
    throw new _apidomError.UnsupportedOperationError('not getter in Items class is not not supported.');
  }
  set not(not) {
    throw new _apidomError.UnsupportedOperationError('not setter in Items class is not not supported.');
  }
  get definitions() {
    throw new _apidomError.UnsupportedOperationError('definitions getter in Items class is not not supported.');
  }
  set definitions(definitions) {
    throw new _apidomError.UnsupportedOperationError('definitions setter in Items class is not not supported.');
  }

  /**
   * Metadata keywords
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
   */

  get title() {
    throw new _apidomError.UnsupportedOperationError('title getter in Items class is not not supported.');
  }
  set title(title) {
    throw new _apidomError.UnsupportedOperationError('title setter in Items class is not not supported.');
  }
  get description() {
    throw new _apidomError.UnsupportedOperationError('description getter in Items class is not not supported.');
  }
  set description(description) {
    throw new _apidomError.UnsupportedOperationError('description setter in Items class is not not supported.');
  }

  /**
   * JSON Hyper-Schema
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00
   */

  get base() {
    throw new _apidomError.UnsupportedOperationError('base getter in Items class is not not supported.');
  }
  set base(base) {
    throw new _apidomError.UnsupportedOperationError('base setter in Items class is not not supported.');
  }
  get links() {
    throw new _apidomError.UnsupportedOperationError('links getter in Items class is not not supported.');
  }
  set links(links) {
    throw new _apidomError.UnsupportedOperationError('links setter in Items class is not not supported.');
  }
  get media() {
    throw new _apidomError.UnsupportedOperationError('media getter in Items class is not not supported.');
  }
  set media(media) {
    throw new _apidomError.UnsupportedOperationError('media setter in Items class is not not supported.');
  }
  get readOnly() {
    throw new _apidomError.UnsupportedOperationError('readOnly getter in Items class is not not supported.');
  }
  set readOnly(readOnly) {
    throw new _apidomError.UnsupportedOperationError('readOnly setter in Items class is not not supported.');
  }
}
/* eslint-enable class-methods-use-this */
var _default = exports.default = Items;