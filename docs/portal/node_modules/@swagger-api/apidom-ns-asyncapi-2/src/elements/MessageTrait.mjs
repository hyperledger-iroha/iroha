import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class MessageTrait extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'messageTrait';
  }
  get messageId() {
    return this.get('messageId');
  }
  set messageId(messageId) {
    this.set('messageId', messageId);
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get correlationId() {
    return this.get('correlationId');
  }
  set correlationId(correlationId) {
    this.set('correlationId', correlationId);
  }
  get schemaFormat() {
    return this.get('schemaFormat');
  }
  set schemaFormat(schemaFormat) {
    this.set('schemaFormat', schemaFormat);
  }
  get contentType() {
    return this.get('contentType');
  }
  set contentType(contentType) {
    this.set('contentType', contentType);
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
  get bindings() {
    return this.get('bindings');
  }
  set bindings(bindings) {
    this.set('bindings', bindings);
  }
  get examples() {
    return this.get('examples');
  }
  set examples(examples) {
    this.set('examples', examples);
  }
}
export default MessageTrait;