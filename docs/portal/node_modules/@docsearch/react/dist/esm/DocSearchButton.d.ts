import { DocSearchModalShortcuts } from '@docsearch/core';
import React from 'react';

type DocSearchTheme = 'dark' | 'light';

type ButtonTranslations = Partial<{
    buttonText: string;
    buttonAriaLabel: string;
}>;
type DocSearchButtonProps = React.ComponentProps<'button'> & {
    theme?: DocSearchTheme;
    translations?: ButtonTranslations;
    keyboardShortcuts?: DocSearchModalShortcuts;
};
declare const DocSearchButton: React.ForwardRefExoticComponent<Omit<DocSearchButtonProps, "ref"> & React.RefAttributes<HTMLButtonElement>>;

export { DocSearchButton };
export type { ButtonTranslations, DocSearchButtonProps };
