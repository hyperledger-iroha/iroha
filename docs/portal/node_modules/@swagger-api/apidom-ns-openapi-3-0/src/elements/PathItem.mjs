import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class PathItem extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'pathItem';
  }
  get $ref() {
    return this.get('$ref');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
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
  get GET() {
    return this.get('get');
  }
  set GET(operation) {
    this.set('GET', operation);
  }
  get PUT() {
    return this.get('put');
  }
  set PUT(operation) {
    this.set('PUT', operation);
  }
  get POST() {
    return this.get('post');
  }
  set POST(operation) {
    this.set('POST', operation);
  }
  get DELETE() {
    return this.get('delete');
  }
  set DELETE(operation) {
    this.set('DELETE', operation);
  }
  get OPTIONS() {
    return this.get('options');
  }
  set OPTIONS(operation) {
    this.set('OPTIONS', operation);
  }
  get HEAD() {
    return this.get('head');
  }
  set HEAD(operation) {
    this.set('HEAD', operation);
  }
  get PATCH() {
    return this.get('patch');
  }
  set PATCH(operation) {
    this.set('PATCH', operation);
  }
  get TRACE() {
    return this.get('trace');
  }
  set TRACE(operation) {
    this.set('TRACE', operation);
  }
  get servers() {
    return this.get('servers');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
}
export default PathItem;