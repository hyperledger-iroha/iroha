"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class JSONSchema extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONSchemaDraft4';
  }

  /**
   * Core vocabulary
   *
   * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
   */

  get idProp() {
    return this.get('id');
  }
  set idProp(idProp) {
    this.set('id', idProp);
  }
  get $schema() {
    return this.get('$schema');
  }
  set $schema($schema) {
    this.set('$schema', $schema);
  }

  /**
   * Validation vocabulary
   *
   * URI: https://tools.ietf.org/html/draft-wright-json-schema-validation-00
   */

  /**
   *  Validation keywords for numeric instances (number and integer)
   */

  get multipleOf() {
    return this.get('multipleOf');
  }
  set multipleOf(multipleOf) {
    this.set('multipleOf', multipleOf);
  }
  get maximum() {
    return this.get('maximum');
  }
  set maximum(maximum) {
    this.set('maximum', maximum);
  }
  get exclusiveMaximum() {
    return this.get('exclusiveMaximum');
  }
  set exclusiveMaximum(exclusiveMaximum) {
    this.set('exclusiveMaximum', exclusiveMaximum);
  }
  get minimum() {
    return this.get('minimum');
  }
  set minimum(minimum) {
    this.set('minimum', minimum);
  }
  get exclusiveMinimum() {
    return this.get('exclusiveMinimum');
  }
  set exclusiveMinimum(exclusiveMinimum) {
    this.set('exclusiveMinimum', exclusiveMinimum);
  }

  /**
   * Validation keywords for strings
   */

  get maxLength() {
    return this.get('maxLength');
  }
  set maxLength(maxLength) {
    this.set('maxLength', maxLength);
  }
  get minLength() {
    return this.get('minLength');
  }
  set minLength(minLength) {
    this.set('minLength', minLength);
  }
  get pattern() {
    return this.get('pattern');
  }
  set pattern(pattern) {
    this.set('pattern', pattern);
  }

  /**
   * Validation keywords for arrays
   */

  get additionalItems() {
    return this.get('additionalItems');
  }
  set additionalItems(additionalItems) {
    this.set('additionalItems', additionalItems);
  }
  get items() {
    return this.get('items');
  }
  set items(items) {
    this.set('items', items);
  }
  get maxItems() {
    return this.get('maxItems');
  }
  set maxItems(maxItems) {
    this.set('maxItems', maxItems);
  }
  get minItems() {
    return this.get('minItems');
  }
  set minItems(minItems) {
    this.set('minItems', minItems);
  }
  get uniqueItems() {
    return this.get('uniqueItems');
  }
  set uniqueItems(uniqueItems) {
    this.set('uniqueItems', uniqueItems);
  }

  /**
   * Validation keywords for objects
   */

  get maxProperties() {
    return this.get('maxProperties');
  }
  set maxProperties(maxProperties) {
    this.set('maxProperties', maxProperties);
  }
  get minProperties() {
    return this.get('minProperties');
  }
  set minProperties(minProperties) {
    this.set('minProperties', minProperties);
  }
  get required() {
    return this.get('required');
  }
  set required(required) {
    this.set('required', required);
  }
  get properties() {
    return this.get('properties');
  }
  set properties(properties) {
    this.set('properties', properties);
  }
  get additionalProperties() {
    return this.get('additionalProperties');
  }
  set additionalProperties(additionalProperties) {
    this.set('additionalProperties', additionalProperties);
  }
  get patternProperties() {
    return this.get('patternProperties');
  }
  set patternProperties(patternProperties) {
    this.set('patternProperties', patternProperties);
  }
  get dependencies() {
    return this.get('dependencies');
  }
  set dependencies(dependencies) {
    this.set('dependencies', dependencies);
  }

  /**
   *  Validation keywords for any instance type
   */

  get enum() {
    return this.get('enum');
  }
  set enum(enumValue) {
    this.set('enum', enumValue);
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
  get allOf() {
    return this.get('allOf');
  }
  set allOf(allOf) {
    this.set('allOf', allOf);
  }
  get anyOf() {
    return this.get('anyOf');
  }
  set anyOf(anyOf) {
    this.set('anyOf', anyOf);
  }
  get oneOf() {
    return this.get('oneOf');
  }
  set oneOf(oneOf) {
    this.set('oneOf', oneOf);
  }
  get not() {
    return this.get('not');
  }
  set not(not) {
    this.set('not', not);
  }
  get definitions() {
    return this.get('definitions');
  }
  set definitions(definitions) {
    this.set('definitions', definitions);
  }

  /**
   * Metadata keywords
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
   */

  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get default() {
    return this.get('default');
  }
  set default(defaultValue) {
    this.set('default', defaultValue);
  }

  /**
   * Semantic validation with "format"
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-7
   */

  get format() {
    return this.get('format');
  }
  set format(format) {
    this.set('format', format);
  }

  /**
   * JSON Hyper-Schema
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00
   */

  get base() {
    return this.get('base');
  }
  set base(base) {
    this.set('base', base);
  }
  get links() {
    return this.get('links');
  }
  set links(links) {
    this.set('links', links);
  }
  get media() {
    return this.get('media');
  }
  set media(media) {
    this.set('media', media);
  }
  get readOnly() {
    return this.get('readOnly');
  }
  set readOnly(readOnly) {
    this.set('readOnly', readOnly);
  }
}
var _default = exports.default = JSONSchema;