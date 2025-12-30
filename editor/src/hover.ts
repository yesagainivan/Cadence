import { hoverTooltip } from "@codemirror/view";
import { getDocumentation, getSymbols, type DocItem, type Symbol } from "./cadence-wasm";

// ============================================================================
// Symbol Cache - Reactively updated on code changes
// ============================================================================

let builtinDocs: Map<string, DocItem> = new Map();
let userSymbols: Map<string, Symbol> = new Map();

// Initialize built-in docs once
function ensureBuiltins() {
    if (builtinDocs.size > 0) return;
    const docs = getDocumentation();
    if (docs && docs.length > 0) {
        for (const doc of docs) {
            builtinDocs.set(doc.name, doc);
        }
    }
}

/**
 * Refresh symbols from source code (call on text change, debounced)
 */
export function refreshSymbols(code: string): void {
    const result = getSymbols(code);
    if (result.success) {
        userSymbols = new Map();
        for (const sym of result.symbols) {
            userSymbols.set(sym.name, sym);
        }
    }
}

// Debounce helper
let debounceTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Debounced symbol refresh - call this from editor update listener
 */
export function debouncedRefreshSymbols(code: string, delayMs: number = 150): void {
    if (debounceTimer) {
        clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(() => {
        refreshSymbols(code);
        debounceTimer = null;
    }, delayMs);
}

// ============================================================================
// Hover Tooltip Extension
// ============================================================================

export const cadenceHover = hoverTooltip((view, pos, _side) => {
    ensureBuiltins();

    const { from, to, text } = view.state.doc.lineAt(pos);

    // Find word boundaries
    let start = pos;
    let end = pos;

    // Scan left
    while (start > from) {
        const char = text[start - from - 1];
        if (!/[\w\d_]/.test(char)) break;
        start--;
    }

    // Scan right
    while (end < to) {
        const char = text[end - from];
        if (!/[\w\d_]/.test(char)) break;
        end++;
    }

    if (start === end) return null;

    const word = text.slice(start - from, end - from);

    // Check built-in docs first
    const builtinDoc = builtinDocs.get(word);
    if (builtinDoc) {
        return {
            pos: start,
            end,
            above: true,
            create() {
                return { dom: createBuiltinTooltip(builtinDoc) };
            }
        };
    }

    // Then check user symbols
    const userSymbol = userSymbols.get(word);
    if (userSymbol) {
        return {
            pos: start,
            end,
            above: true,
            create() {
                return { dom: createSymbolTooltip(userSymbol) };
            }
        };
    }

    return null;
});

// ============================================================================
// Tooltip DOM Creation
// ============================================================================

function createBuiltinTooltip(doc: DocItem): HTMLElement {
    const dom = document.createElement("div");
    dom.className = "cm-tooltip-cursor";
    dom.style.padding = "4px 8px";
    dom.style.fontFamily = "monospace";
    dom.style.maxWidth = "400px";

    dom.innerHTML = `
        <div style="font-weight: bold; border-bottom: 1px solid #444; margin-bottom: 4px; padding-bottom: 2px;">
            <span style="color: #61afef">${doc.name}</span>
            <span style="float: right; color: #abb2bf; font-size: 0.8em; font-weight: normal">${doc.category}</span>
        </div>
        <div style="color: #98c379; margin-bottom: 4px;">${doc.signature}</div>
        <div style="color: #abb2bf; white-space: pre-wrap;">${doc.description}</div>
    `;
    return dom;
}

function createSymbolTooltip(symbol: Symbol): HTMLElement {
    const dom = document.createElement("div");
    dom.className = "cm-tooltip-cursor";
    dom.style.cssText = `
        padding: 4px 8px;
        font-family: monospace;
        max-width: 400px;
        background: #21252b;
        border: 1px solid #3a3f4b;
        border-radius: 4px;
        color: #abb2bf;
    `;

    if (symbol.kind === 'Function') {
        dom.innerHTML = `
            <div style="font-weight: bold; border-bottom: 1px solid #3a3f4b; margin-bottom: 4px; padding-bottom: 2px;">
                <span style="color: #61afef">${symbol.name}</span>
                <span style="float: right; color: #7f848e; font-size: 0.85em; font-weight: normal">User</span>
            </div>
            <div style="color: #98c379;">${symbol.signature}</div>
        `;
    } else {
        // Variable - show name and type
        const typeInfo = symbol.value_type
            ? `<span style="color: #7f848e">:</span> <span style="color: #e5c07b">${symbol.value_type}</span>`
            : '';
        dom.innerHTML = `
            <div style="font-weight: bold; border-bottom: 1px solid #3a3f4b; margin-bottom: 4px; padding-bottom: 2px;">
                <span style="color: #e06c75">${symbol.name}</span>
                <span style="float: right; color: #7f848e; font-size: 0.85em; font-weight: normal">Variable</span>
            </div>
            <div style="color: #c678dd;">let ${symbol.name}${typeInfo}</div>
        `;
    }
    return dom;
}

// Legacy export for backward compatibility (will be removed)
export function updateUserFunctions(_funcs: DocItem[]) {
    // Deprecated - now using refreshSymbols
}
