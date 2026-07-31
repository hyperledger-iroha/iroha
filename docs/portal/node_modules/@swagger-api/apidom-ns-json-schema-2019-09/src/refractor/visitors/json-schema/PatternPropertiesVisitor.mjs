import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { ObjectElement } from '@swagger-api/apidom-core';
import { FallbackVisitor, MapVisitor, ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';

/**
 * @public
 */

/**
 * @public
 */
class PatternPropertiesVisitor extends Mixin(MapVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ObjectElement();
    this.element.classes.push('json-schema-patternProperties');
    this.specPath = always(['document', 'objects', 'JSONSchema']);
  }
}
export default PatternPropertiesVisitor;