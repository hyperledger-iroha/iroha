import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-7';

/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class JSONSchema extends JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONSchema201909';
  }

  /**
   * Core vocabulary
   *
   * URI: https://json-schema.org/draft/2019-09/vocab/core
   */

  get $vocabulary() {
    return this.get('$vocabulary');
  }
  set $vocabulary($vocabulary) {
    this.set('$vocabulary', $vocabulary);
  }
  get $anchor() {
    return this.get('$anchor');
  }
  set $anchor($anchor) {
    this.set('$anchor', $anchor);
  }
  get $recursiveAnchor() {
    return this.get('$recursiveAnchor');
  }
  set $recursiveAnchor($recursiveAnchor) {
    this.set('$recursiveAnchor', $recursiveAnchor);
  }
  get $recursiveRef() {
    return this.get('$recursiveRef');
  }
  set $recursiveRef($recursiveRef) {
    this.set('$recursiveRef', $recursiveRef);
  }
  get $ref() {
    return this.get('$ref');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
  }
  get $defs() {
    return this.get('$defs');
  }
  set $defs($defs) {
    this.set('$defs', $defs);
  }
  get definitions() {
    throw new UnsupportedOperationError('definitions keyword from Validation vocabulary has been renamed to $defs.');
  }
  set definitions(definitions) {
    throw new UnsupportedOperationError('definitions keyword from Validation vocabulary has been renamed to $defs.');
  }

  /**
   * Applicator vocabulary
   *
   * URI: https://json-schema.org/draft/2019-09/vocab/applicator
   */

  get not() {
    return this.get('not');
  }
  set not(not) {
    this.set('not', not);
  }
  get if() {
    return this.get('if');
  }
  set if(ifSchema) {
    this.set('if', ifSchema);
  }
  get then() {
    return this.get('then');
  }
  set then(thenSchema) {
    this.set('then', thenSchema);
  }
  get else() {
    return this.get('else');
  }
  set else(elseSchema) {
    this.set('else', elseSchema);
  }
  get dependentSchemas() {
    return this.get('dependentSchemas');
  }
  set dependentSchemas(dependentSchemas) {
    this.set('dependentSchemas', dependentSchemas);
  }
  get dependencies() {
    throw new UnsupportedOperationError('dependencies keyword from Validation vocabulary has been renamed to dependentSchemas.');
  }
  set dependencies(dependencies) {
    throw new UnsupportedOperationError('dependencies keyword from Validation vocabulary has been renamed to dependentSchemas.');
  }
  get items() {
    return this.get('items');
  }
  set items(items) {
    this.set('items', items);
  }
  get containsProp() {
    return this.get('contains');
  }
  set containsProp(containsProp) {
    this.set('contains', containsProp);
  }
  get additionalProperties() {
    return this.get('additionalProperties');
  }
  set additionalProperties(additionalProperties) {
    this.set('additionalProperties', additionalProperties);
  }
  get additionalItems() {
    return this.get('additionalItems');
  }
  set additionalItems(additionalItems) {
    this.set('additionalItems', additionalItems);
  }
  get propertyNames() {
    return this.get('propertyNames');
  }
  set propertyNames(propertyNames) {
    this.set('propertyNames', propertyNames);
  }
  get unevaluatedItems() {
    return this.get('unevaluatedItems');
  }
  set unevaluatedItems(unevaluatedItems) {
    this.set('unevaluatedItems', unevaluatedItems);
  }
  get unevaluatedProperties() {
    return this.get('unevaluatedProperties');
  }
  set unevaluatedProperties(unevaluatedProperties) {
    this.set('unevaluatedProperties', unevaluatedProperties);
  }

  /**
   * Validation vocabulary
   *
   * URI: https://json-schema.org/draft/2019-09/json-schema-validation#rfc.section.6
   */

  /**
   * Validation Keywords for Arrays
   *
   * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-validation-02#rfc.section.6.4
   */

  get maxContains() {
    return this.get('maxContains');
  }
  set maxContains(maxContains) {
    this.set('maxContains', maxContains);
  }
  get minContains() {
    return this.get('minContains');
  }
  set minContains(minContains) {
    this.set('minContains', minContains);
  }

  /**
   * Validation Keywords for Objects
   *
   * URI: https://json-schema.org/draft/2019-09/draft-handrews-json-schema-validation-02#rfc.section.6.5
   */

  get dependentRequired() {
    return this.get('dependentRequired');
  }
  set dependentRequired(dependentRequired) {
    this.set('dependentRequired', dependentRequired);
  }

  /**
   * Vocabulary for Basic Meta-Data Annotations
   *
   * URI: https://json-schema.org/draft/2019-09/vocab/meta-data
   */

  get deprecated() {
    return this.get('deprecated');
  }
  set deprecated(deprecated) {
    this.set('deprecated', deprecated);
  }

  /**
   * Vocabulary for the Contents of String-Encoded Data
   *
   * URI: https://json-schema.org/draft/2019-09/vocab/content
   */

  get contentSchema() {
    return this.get('contentSchema');
  }
  set contentSchema(contentSchema) {
    this.set('contentSchema', contentSchema);
  }
}
export default JSONSchema;