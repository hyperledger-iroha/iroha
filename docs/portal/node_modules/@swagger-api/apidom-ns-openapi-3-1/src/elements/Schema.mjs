import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class Schema extends JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'schema';
  }

  /**
   * OAS base vocabulary
   *
   * URI: https://github.com/OAI/OpenAPI-Specification/blob/master/versions/3.1.0.md#baseVocabulary
   */

  get discriminator() {
    return this.get('discriminator');
  }
  set discriminator(discriminator) {
    this.set('discriminator', discriminator);
  }
  get xml() {
    return this.get('xml');
  }
  set xml(xml) {
    this.set('xml', xml);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }

  /**
   * @deprecated The example property has been deprecated in favor of the JSON Schema examples keyword. Use of example is discouraged, and later versions of this specification may remove it.
   */
  get example() {
    return this.get('example');
  }
  set example(example) {
    this.set('example', example);
  }
}
export default Schema;