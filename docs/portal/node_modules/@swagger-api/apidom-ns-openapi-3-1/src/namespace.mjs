import CallbackElement from "./elements/Callback.mjs";
import ComponentsElement from "./elements/Components.mjs";
import ContactElement from "./elements/Contact.mjs";
import DiscriminatorElement from "./elements/Discriminator.mjs";
import EncodingElement from "./elements/Encoding.mjs";
import ExampleElement from "./elements/Example.mjs";
import ExternalDocumentationElement from "./elements/ExternalDocumentation.mjs";
import HeaderElement from "./elements/Header.mjs";
import InfoElement from "./elements/Info.mjs";
import JsonSchemaDialectElement from "./elements/JsonSchemaDialect.mjs";
import LicenseElement from "./elements/License.mjs";
import LinkElement from "./elements/Link.mjs";
import MediaTypeElement from "./elements/MediaType.mjs";
import OAuthFlowElement from "./elements/OAuthFlow.mjs";
import OAuthFlowsElement from "./elements/OAuthFlows.mjs";
import OpenapiElement from "./elements/Openapi.mjs";
import OpenApi3_1Element from "./elements/OpenApi3-1.mjs";
import OperationElement from "./elements/Operation.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import PathItemElement from "./elements/PathItem.mjs";
import PathsElement from "./elements/Paths.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import RequestBodyElement from "./elements/RequestBody.mjs";
import ResponseElement from "./elements/Response.mjs";
import ResponsesElement from "./elements/Responses.mjs";
import SchemaElement from "./elements/Schema.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
import SecuritySchemeElement from "./elements/SecurityScheme.mjs";
import ServerElement from "./elements/Server.mjs";
import ServerVariableElement from "./elements/ServerVariable.mjs";
import TagElement from "./elements/Tag.mjs";
import XmlElement from "./elements/Xml.mjs";
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
const openApi3_1 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('callback', CallbackElement);
    base.register('components', ComponentsElement);
    base.register('contact', ContactElement);
    base.register('discriminator', DiscriminatorElement);
    base.register('encoding', EncodingElement);
    base.register('example', ExampleElement);
    base.register('externalDocumentation', ExternalDocumentationElement);
    base.register('header', HeaderElement);
    base.register('info', InfoElement);
    base.register('jsonSchemaDialect', JsonSchemaDialectElement);
    base.register('license', LicenseElement);
    base.register('link', LinkElement);
    base.register('mediaType', MediaTypeElement);
    base.register('oAuthFlow', OAuthFlowElement);
    base.register('oAuthFlows', OAuthFlowsElement);
    base.register('openapi', OpenapiElement);
    base.register('openApi3_1', OpenApi3_1Element);
    base.register('operation', OperationElement);
    base.register('parameter', ParameterElement);
    base.register('pathItem', PathItemElement);
    base.register('paths', PathsElement);
    base.register('reference', ReferenceElement);
    base.register('requestBody', RequestBodyElement);
    base.register('response', ResponseElement);
    base.register('responses', ResponsesElement);
    base.register('schema', SchemaElement);
    base.register('securityRequirement', SecurityRequirementElement);
    base.register('securityScheme', SecuritySchemeElement);
    base.register('server', ServerElement);
    base.register('serverVariable', ServerVariableElement);
    base.register('tag', TagElement);
    base.register('xml', XmlElement);
    return base;
  }
};
export default openApi3_1;