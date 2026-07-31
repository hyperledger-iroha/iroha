"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
/* eslint-disable class-methods-use-this */

/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-01#section-6
 * @public
 */

class LinkDescription extends _apidomNsJsonSchemaDraft.LinkDescriptionElement {
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
    throw new _apidomError.UnsupportedOperationError('schema keyword from Hyper-Schema vocabulary has been renamed to submissionSchema.');
  }
  set schema(schema) {
    throw new _apidomError.UnsupportedOperationError('schema keyword from Hyper-Schema vocabulary has been renamed to submissionSchema.');
  }
  get submissionSchema() {
    return this.get('submissionSchema');
  }
  set submissionSchema(submissionSchema) {
    this.set('submissionSchema', submissionSchema);
  }
  get method() {
    throw new _apidomError.UnsupportedOperationError('method keyword from Hyper-Schema vocabulary has been removed.');
  }
  set method(method) {
    throw new _apidomError.UnsupportedOperationError('method keyword from Hyper-Schema vocabulary has been removed.');
  }
  get encType() {
    throw new _apidomError.UnsupportedOperationError('encType keyword from Hyper-Schema vocabulary has been renamed to submissionEncType.');
  }
  set encType(encType) {
    throw new _apidomError.UnsupportedOperationError('encType keyword from Hyper-Schema vocabulary has been renamed to submissionEncType.');
  }
  get submissionEncType() {
    return this.get('submissionEncType');
  }
  set submissionEncType(submissionEncType) {
    this.set('submissionEncType', submissionEncType);
  }
}
var _default = exports.default = LinkDescription;