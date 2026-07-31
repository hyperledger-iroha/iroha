import highlight from "./highlight.js";
import defaultStyle from "./styles/hljs/default-style.js";
import lowlight from 'lowlight';
import supportedLanguages from "./languages/hljs/supported-languages.js";
var highlighter = highlight(lowlight, defaultStyle);
highlighter.supportedLanguages = supportedLanguages;
export default highlighter;