"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-7");
var _SchemaOrReferenceVisitor = _interopRequireDefault(require("./SchemaOrReferenceVisitor.cjs"));
// @ts-ignore
const inheritedFixedFields = (0, _ramda.map)(visitor => {
  if (visitor === _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor) {
    return _SchemaOrReferenceVisitor.default;
  }
  return visitor;
}, _apidomNsJsonSchemaDraft.specificationObj.visitors.document.objects.JSONSchema.fixedFields);
var _default = exports.default = inheritedFixedFields;