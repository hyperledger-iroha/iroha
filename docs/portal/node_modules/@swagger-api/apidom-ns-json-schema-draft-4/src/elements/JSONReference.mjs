import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * In Draft 4, $ref behaves a little differently. When an object contains a $ref property,
 * the object is considered a reference, not a schema. Therefore, any other properties you put in that object will
 * not be treated as JSON Schema keywords and will be ignored by the validator.
 * $ref can only be used where a schema is expected.
 *
 * URI: https://json-schema.org/understanding-json-schema/structuring.html?highlight=ref#id18
 * @public
 */

class JSONReference extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONReference';
    this.classes.push('json-reference');
  }
  get $ref() {
    return this.get('$ref');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
  }
}
export default JSONReference;