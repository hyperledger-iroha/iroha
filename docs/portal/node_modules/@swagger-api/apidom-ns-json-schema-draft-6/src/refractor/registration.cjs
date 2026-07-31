"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
var _JSONSchema = _interopRequireDefault(require("../elements/JSONSchema.cjs"));
exports.JSONSchemaElement = _JSONSchema.default;
var _LinkDescription = _interopRequireDefault(require("../elements/LinkDescription.cjs"));
exports.LinkDescriptionElement = _LinkDescription.default;
var _index = require("./index.cjs");
// register refractors specific to element types

_JSONSchema.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'JSONSchema', '$visitor']);
_LinkDescription.default.refract = (0, _index.createRefractor)(['visitors', 'document', 'objects', 'LinkDescription', '$visitor']);