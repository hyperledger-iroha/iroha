"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Swagger = _interopRequireDefault(require("./elements/Swagger.cjs"));
var _SwaggerVersion = _interopRequireDefault(require("./elements/SwaggerVersion.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Paths = _interopRequireDefault(require("./elements/Paths.cjs"));
var _PathItem = _interopRequireDefault(require("./elements/PathItem.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("./elements/ExternalDocumentation.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _Items = _interopRequireDefault(require("./elements/Items.cjs"));
var _Example = _interopRequireDefault(require("./elements/Example.cjs"));
var _Responses = _interopRequireDefault(require("./elements/Responses.cjs"));
var _Response = _interopRequireDefault(require("./elements/Response.cjs"));
var _Headers = _interopRequireDefault(require("./elements/Headers.cjs"));
var _Header = _interopRequireDefault(require("./elements/Header.cjs"));
var _Tag = _interopRequireDefault(require("./elements/Tag.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _Xml = _interopRequireDefault(require("./elements/Xml.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _Definitions = _interopRequireDefault(require("./elements/Definitions.cjs"));
var _ParametersDefinitions = _interopRequireDefault(require("./elements/ParametersDefinitions.cjs"));
var _ResponsesDefinitions = _interopRequireDefault(require("./elements/ResponsesDefinitions.cjs"));
var _SecurityDefinitions = _interopRequireDefault(require("./elements/SecurityDefinitions.cjs"));
var _SecurityScheme = _interopRequireDefault(require("./elements/SecurityScheme.cjs"));
var _Scopes = _interopRequireDefault(require("./elements/Scopes.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("./elements/SecurityRequirement.cjs"));
/**
 * @public
 */
const openApi2 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('swagger', _Swagger.default);
    base.register('swaggerVersion', _SwaggerVersion.default);
    base.register('info', _Info.default);
    base.register('contact', _Contact.default);
    base.register('license', _License.default);
    base.register('paths', _Paths.default);
    base.register('pathItem', _PathItem.default);
    base.register('operation', _Operation.default);
    base.register('externalDocumentation', _ExternalDocumentation.default);
    base.register('parameter', _Parameter.default);
    base.register('items', _Items.default);
    base.register('responses', _Responses.default);
    base.register('response', _Response.default);
    base.register('headers', _Headers.default);
    base.register('example', _Example.default);
    base.register('header', _Header.default);
    base.register('tag', _Tag.default);
    base.register('reference', _Reference.default);
    base.register('schema', _Schema.default);
    base.register('xml', _Xml.default);
    base.register('definitions', _Definitions.default);
    base.register('parametersDefinitions', _ParametersDefinitions.default);
    base.register('responsesDefinitions', _ResponsesDefinitions.default);
    base.register('securityDefinitions', _SecurityDefinitions.default);
    base.register('securityScheme', _SecurityScheme.default);
    base.register('scopes', _Scopes.default);
    base.register('securityRequirement', _SecurityRequirement.default);
    return base;
  }
};
var _default = exports.default = openApi2;