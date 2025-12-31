/**
 * Go-to-Definition for Cadence
 * 
 * Cmd+Click or F12 on a symbol name to jump to its definition.
 */

import { EditorView, ViewPlugin, ViewUpdate } from "@codemirror/view";
import { get_definition_by_name } from "./cadence-wasm";

interface DefinitionResult {
    found: boolean;
    start: number;
    end: number;
}

/**
 * Get word at cursor position
 */
function getWordAt(doc: string, pos: number): string | null {
    // Find word boundaries (alphanumeric + underscore)
    let start = pos;
    let end = pos;

    // Scan backwards
    while (start > 0 && /[a-zA-Z0-9_]/.test(doc[start - 1])) {
        start--;
    }

    // Scan forwards
    while (end < doc.length && /[a-zA-Z0-9_]/.test(doc[end])) {
        end++;
    }

    if (start === end) return null;
    return doc.slice(start, end);
}

/**
 * Handle go-to-definition command
 * Returns true if handled, false otherwise
 */
export function gotoDefinition(view: EditorView): boolean {
    const code = view.state.doc.toString();
    const pos = view.state.selection.main.head;

    // Get the word at cursor
    const word = getWordAt(code, pos);
    if (!word) return false;

    // Look up definition
    const result = get_definition_by_name(code, word) as DefinitionResult;
    if (!result || !result.found) return false;

    // Jump to definition
    view.dispatch({
        selection: { anchor: result.start },
        scrollIntoView: true
    });

    return true;
}

/**
 * Cmd+Click handler for go-to-definition
 */
export const gotoDefinitionPlugin = ViewPlugin.fromClass(class {
    constructor(_view: EditorView) { }

    update(_update: ViewUpdate) { }
}, {
    eventHandlers: {
        mousedown(event: MouseEvent, view: EditorView) {
            // Check for Cmd (Mac) or Ctrl (Windows/Linux) + Click
            if (event.metaKey || event.ctrlKey) {
                // Get position from click coordinates
                const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
                if (pos === null) return false;

                const code = view.state.doc.toString();
                const word = getWordAt(code, pos);
                if (!word) return false;

                // Look up definition
                const result = get_definition_by_name(code, word) as DefinitionResult;
                if (!result || !result.found) return false;

                // Jump to definition
                view.dispatch({
                    selection: { anchor: result.start },
                    scrollIntoView: true
                });

                // Prevent default click behavior
                event.preventDefault();
                return true;
            }
            return false;
        }
    }
});

/**
 * CSS class for pointer cursor when Cmd/Ctrl is held
 */
const GOTO_CURSOR_CLASS = 'cm-goto-definition-active';

/**
 * Initialize Cmd/Ctrl key listeners for visual feedback
 * Call this once when the editor is created
 */
export function initGotoDefinitionCursor(editorElement: HTMLElement): void {
    const handleKeyDown = (e: KeyboardEvent) => {
        if (e.metaKey || e.ctrlKey) {
            editorElement.classList.add(GOTO_CURSOR_CLASS);
        }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
        if (!e.metaKey && !e.ctrlKey) {
            editorElement.classList.remove(GOTO_CURSOR_CLASS);
        }
    };

    // Also remove on blur (user switches windows while holding key)
    const handleBlur = () => {
        editorElement.classList.remove(GOTO_CURSOR_CLASS);
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    window.addEventListener('blur', handleBlur);
}
