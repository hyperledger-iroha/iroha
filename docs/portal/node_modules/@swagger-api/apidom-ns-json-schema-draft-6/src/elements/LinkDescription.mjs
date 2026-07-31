import { UnsupportedOperationError } from '@swagger-api/apidom-error';
import { LinkDescriptionElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
/* eslint-disable class-methods-use-this */

/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-01#section-6
 * @public
 */

class LinkDescription extends LinkDescriptionElement {
  get hrefSchema() {
    return this.get('hrefSchema');
  }
  set hrefSchema(hrefSchema) {
    this.set('hrefSchema', hrefSchema);
  }
  get targetSchema() {
    return this.get('targetSchema');
  }
  set targetSchema(targetSchema) {
    this.set('targetSchema', targetSchema);
  }
  get schema() {
    throw new UnsupportedOperationError('schema keyword from Hyper-Schema vocabulary has been renamed to submissionSchema.');
  }
  set schema(schema) {
    throw new UnsupportedOperationError('schema keyword from Hyper-Schema vocabulary has been renamed to submissionSchema.');
  }
  get submissionSchema() {
    return this.get('submissionSchema');
  }
  set submissionSchema(submissionSchema) {
    this.set('submissionSchema', submissionSchema);
  }
  get method() {
    throw new UnsupportedOperationError('method keyword from Hyper-Schema vocabulary has been removed.');
  }
  set method(method) {
    throw new UnsupportedOperationError('method keyword from Hyper-Schema vocabulary has been removed.');
  }
  get encType() {
    throw new UnsupportedOperationError('encType keyword from Hyper-Schema vocabulary has been renamed to submissionEncType.');
  }
  set encType(encType) {
    throw new UnsupportedOperationError('encType keyword from Hyper-Schema vocabulary has been renamed to submissionEncType.');
  }
  get submissionEncType() {
    return this.get('submissionEncType');
  }
  set submissionEncType(submissionEncType) {
    this.set('submissionEncType', submissionEncType);
  }
}
export default LinkDescription;