import SwaggerElement from "./elements/Swagger.mjs";
import SwaggerVersionElement from "./elements/SwaggerVersion.mjs";
import InfoElement from "./elements/Info.mjs";
import ContactElement from "./elements/Contact.mjs";
import LicenseElement from "./elements/License.mjs";
import PathsElement from "./elements/Paths.mjs";
import PathItemElement from "./elements/PathItem.mjs";
import OperationElement from "./elements/Operation.mjs";
import ExternalDocumentation from "./elements/ExternalDocumentation.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import ItemsElement from "./elements/Items.mjs";
import ExampleElement from "./elements/Example.mjs";
import ResponsesElement from "./elements/Responses.mjs";
import ResponseElement from "./elements/Response.mjs";
import HeadersElement from "./elements/Headers.mjs";
import HeaderElement from "./elements/Header.mjs";
import TagElement from "./elements/Tag.mjs";
import SchemaElement from "./elements/Schema.mjs";
import XmlElement from "./elements/Xml.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import DefinitionsElement from "./elements/Definitions.mjs";
import ParametersDefinitionsElement from "./elements/ParametersDefinitions.mjs";
import ResponsesDefinitionsElement from "./elements/ResponsesDefinitions.mjs";
import SecurityDefinitionsElement from "./elements/SecurityDefinitions.mjs";
import SecuritySchemeElement from "./elements/SecurityScheme.mjs";
import ScopesElement from "./elements/Scopes.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
/**
 * @public
 */
const openApi2 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('swagger', SwaggerElement);
    base.register('swaggerVersion', SwaggerVersionElement);
    base.register('info', InfoElement);
    base.register('contact', ContactElement);
    base.register('license', LicenseElement);
    base.register('paths', PathsElement);
    base.register('pathItem', PathItemElement);
    base.register('operation', OperationElement);
    base.register('externalDocumentation', ExternalDocumentation);
    base.register('parameter', ParameterElement);
    base.register('items', ItemsElement);
    base.register('responses', ResponsesElement);
    base.register('response', ResponseElement);
    base.register('headers', HeadersElement);
    base.register('example', ExampleElement);
    base.register('header', HeaderElement);
    base.register('tag', TagElement);
    base.register('reference', ReferenceElement);
    base.register('schema', SchemaElement);
    base.register('xml', XmlElement);
    base.register('definitions', DefinitionsElement);
    base.register('parametersDefinitions', ParametersDefinitionsElement);
    base.register('responsesDefinitions', ResponsesDefinitionsElement);
    base.register('securityDefinitions', SecurityDefinitionsElement);
    base.register('securityScheme', SecuritySchemeElement);
    base.register('scopes', ScopesElement);
    base.register('securityRequirement', SecurityRequirementElement);
    return base;
  }
};
export default openApi2;