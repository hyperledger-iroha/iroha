import YamlSchemaError from "./YamlSchemaError.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YamlTagError extends YamlSchemaError {
  specificTagName;
  explicitTagName;
  tagKind;
  tagStartPositionRow;
  tagStartPositionColumn;
  tagStartPositionIndex;
  tagEndPositionRow;
  tagEndPositionColumn;
  tagEndPositionIndex;
  nodeCanonicalContent;
  node;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.specificTagName = structuredOptions.specificTagName;
      this.explicitTagName = structuredOptions.explicitTagName;
      this.tagKind = structuredOptions.tagKind;
      this.tagStartPositionRow = structuredOptions.tagStartPositionRow;
      this.tagStartPositionColumn = structuredOptions.tagStartPositionColumn;
      this.tagStartPositionIndex = structuredOptions.tagStartPositionIndex;
      this.tagEndPositionRow = structuredOptions.tagEndPositionRow;
      this.tagEndPositionColumn = structuredOptions.tagEndPositionColumn;
      this.tagEndPositionIndex = structuredOptions.tagEndPositionIndex;
      this.nodeCanonicalContent = structuredOptions.nodeCanonicalContent;
      this.node = structuredOptions.node;
    }
  }
}
export default YamlTagError;