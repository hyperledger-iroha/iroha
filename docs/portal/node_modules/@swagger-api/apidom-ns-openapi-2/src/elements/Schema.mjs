import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';

/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class Schema extends JSONSchemaElement {
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
    throw new UnsupportedOperationError('idProp getter in Schema class is not not supported.');
  }
  set idProp(idProps) {
    throw new UnsupportedOperationError('idProp setter in Schema class is not not supported.');
  }
  get $schema() {
    throw new UnsupportedOperationError('$schema getter in Schema class is not not supported.');
  }
  set $schema($schema) {
    throw new UnsupportedOperationError('$schema setter in Schema class is not not supported.');
  }

  /**
   * Validation keywords for arrays
   */

  get additionalItems() {
    throw new UnsupportedOperationError('additionalItems getter in Schema class is not not supported.');
  }
  set additionalItems(additionalItems) {
    throw new UnsupportedOperationError('additionalItems setter in Schema class is not not supported.');
  }

  /**
   * Validation keywords for objects
   */

  get patternProperties() {
    throw new UnsupportedOperationError('patternProperties getter in Schema class is not not supported.');
  }
  set patternProperties(patternProperties) {
    throw new UnsupportedOperationError('patternProperties setter in Schema class is not not supported.');
  }
  get dependencies() {
    throw new UnsupportedOperationError('dependencies getter in Schema class is not not supported.');
  }
  set dependencies(dependencies) {
    throw new UnsupportedOperationError('dependencies setter in Schema class is not not supported.');
  }

  /**
   *  Validation keywords for any instance type
   */
  get anyOf() {
    throw new UnsupportedOperationError('anyOf getter in Schema class is not not supported.');
  }
  set anyOf(anyOf) {
    throw new UnsupportedOperationError('anyOf setter in Schema class is not not supported.');
  }
  get oneOf() {
    throw new UnsupportedOperationError('oneOf getter in Schema class is not not supported.');
  }
  set oneOf(oneOf) {
    throw new UnsupportedOperationError('oneOf setter in Schema class is not not supported.');
  }
  get not() {
    throw new UnsupportedOperationError('not getter in Schema class is not not supported.');
  }
  set not(not) {
    throw new UnsupportedOperationError('not setter in Schema class is not not supported.');
  }
  get definitions() {
    throw new UnsupportedOperationError('definitions getter in Schema class is not not supported.');
  }
  set definitions(definitions) {
    throw new UnsupportedOperationError('definitions setter in Schema class is not not supported.');
  }

  /**
   * JSON Hyper-Schema
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00
   */

  get base() {
    throw new UnsupportedOperationError('base getter in Schema class is not not supported.');
  }
  set base(base) {
    throw new UnsupportedOperationError('base setter in Schema class is not not supported.');
  }
  get links() {
    throw new UnsupportedOperationError('links getter in Schema class is not not supported.');
  }
  set links(links) {
    throw new UnsupportedOperationError('links setter in Schema class is not not supported.');
  }
  get media() {
    throw new UnsupportedOperationError('media getter in Schema class is not not supported.');
  }
  set media(media) {
    throw new UnsupportedOperationError('media setter in Schema class is not not supported.');
  }
}
/* eslint-enable class-methods-use-this */

export default Schema;