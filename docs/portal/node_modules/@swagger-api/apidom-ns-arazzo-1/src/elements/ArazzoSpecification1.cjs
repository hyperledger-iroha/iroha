"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ArazzoSpecification1 extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'arazzoSpecification1';
    this.classes.push('api');
    this.classes.push('arazzo');
  }
  get arazzoSpec() {
    return this.get('arazzoSpec');
  }
  set arazzoSpec(arazzoSpec) {
    this.set('arazzoSpec', arazzoSpec);
  }
  get info() {
    return this.get('info');
  }
  set info(info) {
    this.set('info', info);
  }
  get sourceDescriptions() {
    return this.get('sourceDescriptions');
  }
  set sourceDescriptions(sourceDescriptions) {
    this.set('sourceDescriptions', sourceDescriptions);
  }
  get workflows() {
    return this.get('workflows');
  }
  set workflows(workflows) {
    this.set('workflows', workflows);
  }
  get components() {
    return this.get('components');
  }
  set components(components) {
    this.set('components', components);
  }
}
var _default = exports.default = ArazzoSpecification1;