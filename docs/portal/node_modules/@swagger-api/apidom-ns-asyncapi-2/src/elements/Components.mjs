import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class Components extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'components';
  }
  get schemas() {
    return this.get('schemas');
  }
  set schemas(schemas) {
    this.set('schemas', schemas);
  }
  get servers() {
    return this.get('servers');
  }
  set servers(servers) {
    this.set('servers', servers);
  }
  get serverVariables() {
    return this.get('serverVariables');
  }
  set serverVariables(serverVariables) {
    this.set('serverVariables', serverVariables);
  }
  get messages() {
    return this.get('messages');
  }
  set messages(messages) {
    this.set('messages', messages);
  }
  get securitySchemes() {
    return this.get('securitySchemes');
  }
  set securitySchemes(securitySchemes) {
    this.set('securitySchemes', securitySchemes);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
  get correlationIds() {
    return this.get('correlationIds');
  }
  set correlationIds(correlationIds) {
    this.set('correlationIds', correlationIds);
  }
  get operationTraits() {
    return this.get('operationTraits');
  }
  set operationTraits(operationTraits) {
    this.set('operationTraits', operationTraits);
  }
  get messageTraits() {
    return this.get('messageTraits');
  }
  set messageTraits(messageTraits) {
    this.set('messageTraits', messageTraits);
  }
  get serverBindings() {
    return this.get('serverBindings');
  }
  set serverBindings(serverBindings) {
    this.set('serverBindings', serverBindings);
  }
  get channelBindings() {
    return this.get('channelBindings');
  }
  set channelBindings(channelBindings) {
    this.set('channelBindings', channelBindings);
  }
  get operationBindings() {
    return this.get('operationBindings');
  }
  set operationBindings(operationBindings) {
    this.set('operationBindings', operationBindings);
  }
  get messageBindings() {
    return this.get('messageBindings');
  }
  set messageBindings(messageBindings) {
    this.set('messageBindings', messageBindings);
  }
}
export default Components;