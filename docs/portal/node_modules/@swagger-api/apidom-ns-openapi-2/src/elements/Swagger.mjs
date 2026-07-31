import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class Swagger extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'swagger';
    this.classes.push('api');
  }
  get swagger() {
    return this.get('swagger');
  }
  set swagger(swagger) {
    this.set('swagger', swagger);
  }
  get info() {
    return this.get('info');
  }
  set info(info) {
    this.set('info', info);
  }
  get host() {
    return this.get('host');
  }
  set host(host) {
    this.set('host', host);
  }
  get basePath() {
    return this.get('basePath');
  }
  set basePath(basePath) {
    this.set('basePath', basePath);
  }
  get schemes() {
    return this.get('schemes');
  }
  set schemes(schemes) {
    this.set('schemes', schemes);
  }
  get consumes() {
    return this.get('consumes');
  }
  set consumes(consumes) {
    this.set('consumes', consumes);
  }
  get produces() {
    return this.get('produces');
  }
  set produces(produces) {
    this.set('produces', produces);
  }
  get paths() {
    return this.get('paths');
  }
  set paths(paths) {
    this.set('paths', paths);
  }
  get definitions() {
    return this.get('definitions');
  }
  set definitions(definitions) {
    this.set('definitions', definitions);
  }
  get parameters() {
    return this.get('parameters');
  }
  set parameters(parameters) {
    this.set('parameters', parameters);
  }
  get responses() {
    return this.get('responses');
  }
  set responses(responses) {
    this.set('responses', responses);
  }
  get securityDefinitions() {
    return this.get('securityDefinitions');
  }
  set securityDefinitions(securityDefinitions) {
    this.set('securityDefinitions', securityDefinitions);
  }
  get security() {
    return this.get('security');
  }
  set security(security) {
    this.set('security', security);
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
}
export default Swagger;