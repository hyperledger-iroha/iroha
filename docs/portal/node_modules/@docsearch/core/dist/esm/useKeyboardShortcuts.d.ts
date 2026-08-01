interface DocSearchModalShortcuts {
    /**
     * Enable/disable the Ctrl/Cmd+K shortcut to toggle the DocSearch modal.
     *
     * @default true
     */
    'Ctrl/Cmd+K'?: boolean;
    /**
     * Enable/disable the / shortcut to open the DocSearch modal.
     *
     * @default true
     */
    '/'?: boolean;
}
interface SidepanelShortcuts {
    /**
     * Enable/disable the Ctrl/Cmd+I shortcut to toggle the DocSearch sidepanel.
     *
     * @default true
     */
    'Ctrl/Cmd+I'?: boolean;
}
type KeyboardShortcuts = DocSearchModalShortcuts & SidepanelShortcuts;
/**
 * Default keyboard shortcuts configuration for DocSearch.
 * These values are used when no keyboardShortcuts prop is provided
 * or when specific shortcuts are not configured.
 */
declare const DEFAULT_KEYBOARD_SHORTCUTS: Required<KeyboardShortcuts>;
/**
 * Merges user-provided keyboard shortcuts with defaults.
 *
 * @param userShortcuts - Optional user configuration.
 * @returns Complete keyboard shortcuts configuration with defaults applied.
 */
declare function useKeyboardShortcuts(userShortcuts?: KeyboardShortcuts): Required<KeyboardShortcuts>;

export { DEFAULT_KEYBOARD_SHORTCUTS, useKeyboardShortcuts };
export type { DocSearchModalShortcuts, KeyboardShortcuts, SidepanelShortcuts };
