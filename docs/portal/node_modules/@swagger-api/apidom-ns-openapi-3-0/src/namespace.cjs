"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _Callback = _interopRequireDefault(require("./elements/Callback.cjs"));
var _Components = _interopRequireDefault(require("./elements/Components.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _Discriminator = _interopRequireDefault(require("./elements/Discriminator.cjs"));
var _Encoding = _interopRequireDefault(require("./elements/Encoding.cjs"));
var _Example = _interopRequireDefault(require("./elements/Example.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("./elements/ExternalDocumentation.cjs"));
var _Header = _interopRequireDefault(require("./elements/Header.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Link = _interopRequireDefault(require("./elements/Link.cjs"));
var _MediaType = _interopRequireDefault(require("./elements/MediaType.cjs"));
var _OAuthFlow = _interopRequireDefault(require("./elements/OAuthFlow.cjs"));
var _OAuthFlows = _interopRequireDefault(require("./elements/OAuthFlows.cjs"));
var _Openapi = _interopRequireDefault(require("./elements/Openapi.cjs"));
var _OpenApi = _interopRequireDefault(require("./elements/OpenApi3-0.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _PathItem = _interopRequireDefault(require("./elements/PathItem.cjs"));
var _Paths = _interopRequireDefault(require("./elements/Paths.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _RequestBody = _interopRequireDefault(require("./elements/RequestBody.cjs"));
var _Response = _interopRequireDefault(require("./elements/Response.cjs"));
var _Responses = _interopRequireDefault(require("./elements/Responses.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("./elements/SecurityRequirement.cjs"));
var _SecurityScheme = _interopRequireDefault(require("./elements/SecurityScheme.cjs"));
var _Server = _interopRequireDefault(require("./elements/Server.cjs"));
var _ServerVariable = _interopRequireDefault(require("./elements/ServerVariable.cjs"));
var _Tag = _interopRequireDefault(require("./elements/Tag.cjs"));
var _Xml = _interopRequireDefault(require("./elements/Xml.cjs"));
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
const openApi3_0 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('callback', _Callback.default);
    base.register('components', _Components.default);
    base.register('contact', _Contact.default);
    base.register('discriminator', _Discriminator.default);
    base.register('encoding', _Encoding.default);
    base.register('example', _Example.default);
    base.register('externalDocumentation', _ExternalDocumentation.default);
    base.register('header', _Header.default);
    base.register('info', _Info.default);
    base.register('license', _License.default);
    base.register('link', _Link.default);
    base.register('mediaType', _MediaType.default);
    base.register('oAuthFlow', _OAuthFlow.default);
    base.register('oAuthFlows', _OAuthFlows.default);
    base.register('openapi', _Openapi.default);
    base.register('openApi3_0', _OpenApi.default);
    base.register('operation', _Operation.default);
    base.register('parameter', _Parameter.default);
    base.register('pathItem', _PathItem.default);
    base.register('paths', _Paths.default);
    base.register('reference', _Reference.default);
    base.register('requestBody', _RequestBody.default);
    base.register('response', _Response.default);
    base.register('responses', _Responses.default);
    base.register('schema', _Schema.default);
    base.register('securityRequirement', _SecurityRequirement.default);
    base.register('securityScheme', _SecurityScheme.default);
    base.register('server', _Server.default);
    base.register('serverVariable', _ServerVariable.default);
    base.register('tag', _Tag.default);
    base.register('xml', _Xml.default);
    return base;
  }
};
var _default = exports.default = openApi3_0;