import { ErrorType, errorMessages } from "./errors";
export { ErrorType, errorMessages };
/**
 * Replace raw escape character strings with their escape characters.
 * @param raw A string where escape characters are represented as raw string
 * values like `\'` rather than `'`.
 * @param allowOctals If `true`, will process the now-deprecated octal escape
 * sequences (ie, `\111`).
 * @returns The processed string, with escape characters replaced by their
 * respective actual Unicode characters.
 */
export declare function unraw(raw: string, allowOctals?: boolean): string;
export default unraw;
