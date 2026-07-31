import createAsyncLoadingHighlighter from "./async-syntax-highlighter.js";
import languageLoaders from "./async-languages/prism.js";
export default createAsyncLoadingHighlighter({
  loader: function loader() {
    return import(/* webpackChunkName:"react-syntax-highlighter/refractor-core-import" */
    'refractor/core').then(function (module) {
      return module.refractor;
    });
  },
  isLanguageRegistered: function isLanguageRegistered(instance, language) {
    return instance.registered(language);
  },
  languageLoaders: languageLoaders,
  registerLanguage: function registerLanguage(instance, name, language) {
    return instance.register(language);
  }
});