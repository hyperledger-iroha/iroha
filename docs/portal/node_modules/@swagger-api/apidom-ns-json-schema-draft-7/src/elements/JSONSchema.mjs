import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-draft-6';

/* eslint-disable class-methods-use-this */
/**
 * @public
 */
class JSONSchema extends JSONSchemaElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'JSONSchemaDraft7';
  }

  /**
   * Core vocabulary
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-01
   */

  get $comment() {
    return this.get('$comment');
  }
  set $comment($comment) {
    this.set('$comment', $comment);
  }

  /**
   * Validation vocabulary
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01
   */

  /**
   * Validation keywords for arrays
   */
  get items() {
    return this.get('items');
  }
  set items(items) {
    this.set('items', items);
  }

  /**
   * Keywords for Applying Subschemas Conditionally
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.6
   */

  get if() {
    return this.get('if');
  }
  set if(ifValue) {
    this.set('if', ifValue);
  }
  get then() {
    return this.get('then');
  }
  set then(then) {
    this.set('then', then);
  }
  get else() {
    return this.get('else');
  }
  set else(elseValue) {
    this.set('else', elseValue);
  }

  /**
   * Keywords for Applying Subschemas With Boolean Logic
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-6.7
   */

  get not() {
    return this.get('not');
  }
  set not(not) {
    this.set('not', not);
  }

  /**
   * String-Encoding Non-JSON Data
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-00#section-8
   */

  get contentEncoding() {
    return this.get('contentEncoding');
  }
  set contentEncoding(contentEncoding) {
    this.set('contentEncoding', contentEncoding);
  }
  get contentMediaType() {
    return this.get('contentMediaType');
  }
  set contentMediaType(contentMediaType) {
    this.set('contentMediaType', contentMediaType);
  }
  get media() {
    throw new UnsupportedOperationError('media keyword from Hyper-Schema vocabulary has been moved to validation vocabulary as "contentMediaType" / "contentEncoding"');
  }
  set media(media) {
    throw new UnsupportedOperationError('media keyword from Hyper-Schema vocabulary has been moved to validation vocabulary as "contentMediaType" / "contentEncoding"');
  }

  /**
   * Schema annotations
   *
   * URI: https://datatracker.ietf.org/doc/html/draft-handrews-json-schema-validation-01#section-10
   */

  get writeOnly() {
    return this.get('writeOnly');
  }
  set writeOnly(writeOnly) {
    this.set('writeOnly', writeOnly);
  }
}
export default JSONSchema;