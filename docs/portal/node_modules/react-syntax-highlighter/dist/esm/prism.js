import highlight from "./highlight.js";
import defaultStyle from "./styles/prism/prism.js";
import { refractor } from 'refractor/all';
import supportedLanguages from "./languages/prism/supported-languages.js";
var highlighter = highlight(refractor, defaultStyle);
highlighter.supportedLanguages = supportedLanguages;
export default highlighter;