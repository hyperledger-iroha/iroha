import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class Parameter extends JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'parameter';
    this.classes.push('json-schema-draft-4');
  }

  /**
   * Core vocabulary
   *
   * URI: https://tools.ietf.org/html/draft-wright-json-schema-00
   */

  get idProp() {
    throw new UnsupportedOperationError('idProp getter in Parameter class is not not supported.');
  }
  set idProp(idProps) {
    throw new UnsupportedOperationError('idProp setter in Parameter class is not not supported.');
  }
  get $schema() {
    throw new UnsupportedOperationError('$schema getter in Parameter class is not not supported.');
  }
  set $schema($schema) {
    throw new UnsupportedOperationError('$schema setter in Parameter class is not not supported.');
  }

  /**
   * Validation keywords for arrays
   */

  get additionalItems() {
    throw new UnsupportedOperationError('additionalItems getter in Parameter class is not not supported.');
  }
  set additionalItems(additionalItems) {
    throw new UnsupportedOperationError('additionalItems setter in Parameter class is not not supported.');
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
    throw new UnsupportedOperationError('maxProperties getter in Parameter class is not not supported.');
  }
  set maxProperties(maxProperties) {
    throw new UnsupportedOperationError('maxProperties setter in Parameter class is not not supported.');
  }
  get minProperties() {
    throw new UnsupportedOperationError('minProperties getter in Parameter class is not not supported.');
  }
  set minProperties(minProperties) {
    throw new UnsupportedOperationError('minProperties setter in Parameter class is not not supported.');
  }

  // eslint-disable-next-line  @typescript-eslint/no-explicit-any
  get required() {
    return this.get('required');
  }

  // eslint-disable-next-line  @typescript-eslint/no-explicit-any
  set required(required) {
    this.set('required', required);
  }
  get properties() {
    throw new UnsupportedOperationError('properties getter in Parameter class is not not supported.');
  }
  set properties(properties) {
    throw new UnsupportedOperationError('properties setter in Parameter class is not not supported.');
  }
  get additionalProperties() {
    throw new UnsupportedOperationError('additionalProperties getter in Parameter class is not not supported.');
  }
  set additionalProperties(additionalProperties) {
    throw new UnsupportedOperationError('additionalProperties setter in Parameter class is not not supported.');
  }
  get patternProperties() {
    throw new UnsupportedOperationError('patternProperties getter in Parameter class is not not supported.');
  }
  set patternProperties(patternProperties) {
    throw new UnsupportedOperationError('patternProperties setter in Parameter class is not not supported.');
  }
  get dependencies() {
    throw new UnsupportedOperationError('dependencies getter in Parameter class is not not supported.');
  }
  set dependencies(dependencies) {
    throw new UnsupportedOperationError('dependencies setter in Parameter class is not not supported.');
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
    throw new UnsupportedOperationError('allOf getter in Parameter class is not not supported.');
  }
  set allOf(allOf) {
    throw new UnsupportedOperationError('allOf setter in Parameter class is not not supported.');
  }
  get anyOf() {
    throw new UnsupportedOperationError('anyOf getter in Parameter class is not not supported.');
  }
  set anyOf(anyOf) {
    throw new UnsupportedOperationError('anyOf setter in Parameter class is not not supported.');
  }
  get oneOf() {
    throw new UnsupportedOperationError('oneOf getter in Parameter class is not not supported.');
  }
  set oneOf(oneOf) {
    throw new UnsupportedOperationError('oneOf setter in Parameter class is not not supported.');
  }
  get not() {
    throw new UnsupportedOperationError('not getter in Parameter class is not not supported.');
  }
  set not(not) {
    throw new UnsupportedOperationError('not setter in Parameter class is not not supported.');
  }
  get definitions() {
    throw new UnsupportedOperationError('definitions getter in Parameter class is not not supported.');
  }
  set definitions(definitions) {
    throw new UnsupportedOperationError('definitions setter in Parameter class is not not supported.');
  }

  /**
   * Metadata keywords
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-validation-00#section-6
   */

  get title() {
    throw new UnsupportedOperationError('title getter in Parameter class is not not supported.');
  }
  set title(title) {
    throw new UnsupportedOperationError('title setter in Parameter class is not not supported.');
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description,', description);
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
    throw new UnsupportedOperationError('base getter in Parameter class is not not supported.');
  }
  set base(base) {
    throw new UnsupportedOperationError('base setter in Parameter class is not not supported.');
  }
  get links() {
    throw new UnsupportedOperationError('links getter in Parameter class is not not supported.');
  }
  set links(links) {
    throw new UnsupportedOperationError('links setter in Parameter class is not not supported.');
  }
  get media() {
    throw new UnsupportedOperationError('media getter in Parameter class is not not supported.');
  }
  set media(media) {
    throw new UnsupportedOperationError('media setter in Parameter class is not not supported.');
  }
  get readOnly() {
    throw new UnsupportedOperationError('readOnly getter in Parameter class is not not supported.');
  }
  set readOnly(readOnly) {
    throw new UnsupportedOperationError('readOnly setter in Parameter class is not not supported.');
  }

  /**
   * OpenAPI vocabulary
   */
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get in() {
    return this.get('in');
  }
  set in(val) {
    this.set('in', val);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
/* eslint-enable class-methods-use-this */

export default Parameter;