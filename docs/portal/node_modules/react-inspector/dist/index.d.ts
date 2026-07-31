import { FC, ComponentProps } from 'react';

declare const theme$1: {
    BASE_FONT_FAMILY: string;
    BASE_FONT_SIZE: string;
    BASE_LINE_HEIGHT: number;
    BASE_BACKGROUND_COLOR: string;
    BASE_COLOR: string;
    OBJECT_PREVIEW_ARRAY_MAX_PROPERTIES: number;
    OBJECT_PREVIEW_OBJECT_MAX_PROPERTIES: number;
    OBJECT_NAME_COLOR: string;
    OBJECT_VALUE_NULL_COLOR: string;
    OBJECT_VALUE_UNDEFINED_COLOR: string;
    OBJECT_VALUE_REGEXP_COLOR: string;
    OBJECT_VALUE_STRING_COLOR: string;
    OBJECT_VALUE_SYMBOL_COLOR: string;
    OBJECT_VALUE_NUMBER_COLOR: string;
    OBJECT_VALUE_BOOLEAN_COLOR: string;
    OBJECT_VALUE_FUNCTION_PREFIX_COLOR: string;
    HTML_TAG_COLOR: string;
    HTML_TAGNAME_COLOR: string;
    HTML_TAGNAME_TEXT_TRANSFORM: string;
    HTML_ATTRIBUTE_NAME_COLOR: string;
    HTML_ATTRIBUTE_VALUE_COLOR: string;
    HTML_COMMENT_COLOR: string;
    HTML_DOCTYPE_COLOR: string;
    ARROW_COLOR: string;
    ARROW_MARGIN_RIGHT: number;
    ARROW_FONT_SIZE: number;
    ARROW_ANIMATION_DURATION: string;
    TREENODE_FONT_FAMILY: string;
    TREENODE_FONT_SIZE: string;
    TREENODE_LINE_HEIGHT: number;
    TREENODE_PADDING_LEFT: number;
    TABLE_BORDER_COLOR: string;
    TABLE_TH_BACKGROUND_COLOR: string;
    TABLE_TH_HOVER_COLOR: string;
    TABLE_SORT_ICON_COLOR: string;
    TABLE_DATA_BACKGROUND_IMAGE: string;
    TABLE_DATA_BACKGROUND_SIZE: string;
};

declare const theme: {
    BASE_FONT_FAMILY: string;
    BASE_FONT_SIZE: string;
    BASE_LINE_HEIGHT: number;
    BASE_BACKGROUND_COLOR: string;
    BASE_COLOR: string;
    OBJECT_PREVIEW_ARRAY_MAX_PROPERTIES: number;
    OBJECT_PREVIEW_OBJECT_MAX_PROPERTIES: number;
    OBJECT_NAME_COLOR: string;
    OBJECT_VALUE_NULL_COLOR: string;
    OBJECT_VALUE_UNDEFINED_COLOR: string;
    OBJECT_VALUE_REGEXP_COLOR: string;
    OBJECT_VALUE_STRING_COLOR: string;
    OBJECT_VALUE_SYMBOL_COLOR: string;
    OBJECT_VALUE_NUMBER_COLOR: string;
    OBJECT_VALUE_BOOLEAN_COLOR: string;
    OBJECT_VALUE_FUNCTION_PREFIX_COLOR: string;
    HTML_TAG_COLOR: string;
    HTML_TAGNAME_COLOR: string;
    HTML_TAGNAME_TEXT_TRANSFORM: string;
    HTML_ATTRIBUTE_NAME_COLOR: string;
    HTML_ATTRIBUTE_VALUE_COLOR: string;
    HTML_COMMENT_COLOR: string;
    HTML_DOCTYPE_COLOR: string;
    ARROW_COLOR: string;
    ARROW_MARGIN_RIGHT: number;
    ARROW_FONT_SIZE: number;
    ARROW_ANIMATION_DURATION: string;
    TREENODE_FONT_FAMILY: string;
    TREENODE_FONT_SIZE: string;
    TREENODE_LINE_HEIGHT: number;
    TREENODE_PADDING_LEFT: number;
    TABLE_BORDER_COLOR: string;
    TABLE_TH_BACKGROUND_COLOR: string;
    TABLE_TH_HOVER_COLOR: string;
    TABLE_SORT_ICON_COLOR: string;
    TABLE_DATA_BACKGROUND_IMAGE: string;
    TABLE_DATA_BACKGROUND_SIZE: string;
};

declare const themedObjectInspector: ({ theme, ...restProps }: {
    [x: string]: any;
    theme?: string | undefined;
}) => JSX.Element;

declare const themedTableInspector: ({ theme, ...restProps }: {
    [x: string]: any;
    theme?: string | undefined;
}) => JSX.Element;

declare const ObjectLabel: FC<any>;

declare const ObjectPreview: FC<any>;

declare const ObjectRootLabel: FC<any>;

declare const ObjectValue: FC<any>;

declare const ObjectName: FC<any>;

declare const Inspector: FC<TableInspectorProps | ObjectInspectorProps>;
interface TableInspectorProps extends ComponentProps<typeof themedTableInspector> {
    table: true;
}
interface ObjectInspectorProps extends ComponentProps<typeof themedObjectInspector> {
    table: false;
}

export { Inspector, themedObjectInspector as ObjectInspector, ObjectLabel, ObjectName, ObjectPreview, ObjectRootLabel, ObjectValue, themedTableInspector as TableInspector, theme$1 as chromeDark, theme as chromeLight };
