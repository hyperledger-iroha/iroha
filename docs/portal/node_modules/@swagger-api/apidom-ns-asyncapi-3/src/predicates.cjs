"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isServersElement = exports.isServerVariableElement = exports.isServerElement = exports.isServerBindingsElement = exports.isSchemaElement = exports.isReferenceElement = exports.isParametersElement = exports.isParameterElement = exports.isOperationElement = exports.isMultiFormatSchemaElement = exports.isLicenseElement = exports.isInfoElement = exports.isIdentifierElement = exports.isContactElement = exports.isComponentsElement = exports.isChannelsElement = exports.isChannelElement = exports.isChannelBindingsElement = exports.isAsyncApiVersionElement = exports.isAsyncApi3Element = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _AsyncApi = _interopRequireDefault(require("./elements/AsyncApi3.cjs"));
var _AsyncApiVersion = _interopRequireDefault(require("./elements/AsyncApiVersion.cjs"));
var _MultiFormatSchema = _interopRequireDefault(require("./elements/MultiFormatSchema.cjs"));
var _ChannelBindings = _interopRequireDefault(require("./elements/ChannelBindings.cjs"));
var _Channel = _interopRequireDefault(require("./elements/Channel.cjs"));
var _Channels = _interopRequireDefault(require("./elements/Channels.cjs"));
var _Components = _interopRequireDefault(require("./elements/Components.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _Identifier = _interopRequireDefault(require("./elements/Identifier.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _Parameters = _interopRequireDefault(require("./elements/Parameters.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _Server = _interopRequireDefault(require("./elements/Server.cjs"));
var _Servers = _interopRequireDefault(require("./elements/Servers.cjs"));
var _ServerBindings = _interopRequireDefault(require("./elements/ServerBindings.cjs"));
var _ServerVariable = _interopRequireDefault(require("./elements/ServerVariable.cjs"));
/**
 * @public
 */
const isAsyncApi3Element = exports.isAsyncApi3Element = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _AsyncApi.default || hasBasicElementProps(element) && isElementType('asyncApi3', element) && primitiveEq('object', element) && hasClass('api', element);
});

/**
 * @public
 */
const isAsyncApiVersionElement = exports.isAsyncApiVersionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _AsyncApiVersion.default || hasBasicElementProps(element) && isElementType('asyncApiVersion', element) && primitiveEq('string', element);
});
/**
 * @public
 */
const isChannelBindingsElement = exports.isChannelBindingsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ChannelBindings.default || hasBasicElementProps(element) && isElementType('channelBindings', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isChannelElement = exports.isChannelElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Channel.default || hasBasicElementProps(element) && isElementType('channel', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isChannelsElement = exports.isChannelsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Channels.default || hasBasicElementProps(element) && isElementType('channels', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isComponentsElement = exports.isComponentsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Components.default || hasBasicElementProps(element) && isElementType('components', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isContactElement = exports.isContactElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Contact.default || hasBasicElementProps(element) && isElementType('contact', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isIdentifierElement = exports.isIdentifierElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Identifier.default || hasBasicElementProps(element) && isElementType('identifier', element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isInfoElement = exports.isInfoElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Info.default || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isLicenseElement = exports.isLicenseElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _License.default || hasBasicElementProps(element) && isElementType('license', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isOperationElement = exports.isOperationElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Operation.default || hasBasicElementProps(element) && isElementType('operation', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isParameterElement = exports.isParameterElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Parameter.default || hasBasicElementProps(element) && isElementType('parameter', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isParametersElement = exports.isParametersElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Parameters.default || hasBasicElementProps(element) && isElementType('parameters', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isReferenceElement = exports.isReferenceElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Reference.default || hasBasicElementProps(element) && isElementType('reference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSchemaElement = exports.isSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Schema.default || hasBasicElementProps(element) && isElementType('schema', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isServerElement = exports.isServerElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Server.default || hasBasicElementProps(element) && isElementType('server', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isServerBindingsElement = exports.isServerBindingsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ServerBindings.default || hasBasicElementProps(element) && isElementType('serverBindings', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isServersElement = exports.isServersElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Servers.default || hasBasicElementProps(element) && isElementType('servers', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isServerVariableElement = exports.isServerVariableElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ServerVariable.default || hasBasicElementProps(element) && isElementType('serverVariable', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isMultiFormatSchemaElement = exports.isMultiFormatSchemaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _MultiFormatSchema.default || hasBasicElementProps(element) && isElementType('multiFormatSchema', element) && primitiveEq('object', element);
});