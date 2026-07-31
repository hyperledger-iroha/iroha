"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomNsJsonSchemaDraft = require("@swagger-api/apidom-ns-json-schema-draft-4");
var _index = _interopRequireDefault(require("./visitors/json-schema/index.cjs"));
var _ItemsVisitor = _interopRequireDefault(require("./visitors/json-schema/ItemsVisitor.cjs"));
var _ExamplesVisitor = _interopRequireDefault(require("./visitors/json-schema/ExamplesVisitor.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/json-schema/link-description/index.cjs"));
const specification = (0, _ramda.pipe)(
// JSON Schema object modifications
(0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', '$visitor'], _index.default), (0, _ramda.dissocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'id']), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', '$id'], _apidomNsJsonSchemaDraft.specificationObj.visitors.value), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'contains'], _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'items'], _ItemsVisitor.default), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'propertyNames'], _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'const'], _apidomNsJsonSchemaDraft.specificationObj.visitors.value), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'JSONSchema', 'fixedFields', 'examples'], _ExamplesVisitor.default),
// Link Description object modifications
(0, _ramda.assocPath)(['visitors', 'document', 'objects', 'LinkDescription', '$visitor'], _index2.default), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'hrefSchema'], _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor), (0, _ramda.dissocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'schema']), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'submissionSchema'], _apidomNsJsonSchemaDraft.specificationObj.visitors.JSONSchemaOrJSONReferenceVisitor), (0, _ramda.dissocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'method']), (0, _ramda.dissocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'encType']), (0, _ramda.assocPath)(['visitors', 'document', 'objects', 'LinkDescription', 'fixedFields', 'submissionEncType'], _apidomNsJsonSchemaDraft.specificationObj.visitors.value))(_apidomNsJsonSchemaDraft.specificationObj);
var _default = exports.default = specification;