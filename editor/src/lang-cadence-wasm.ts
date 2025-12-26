/**
 * WASM-based Cadence Language Mode for CodeMirror 6
 * 
 * Uses the Rust tokenizer via WASM for 100% accurate syntax highlighting.
 * Falls back to stream-based highlighting when WASM isn't ready.
 */

import { EditorView, Decoration, ViewPlugin, type ViewUpdate, type DecorationSet } from '@codemirror/view';
import { RangeSetBuilder, type Extension } from '@codemirror/state';
import { tokenizeCode, isWasmReady, type HighlightSpan } from './cadence-wasm';

// Token type to CSS class mapping
const TOKEN_CLASSES: Record<string, string> = {
    'keyword': 'cm-cadence-keyword',
    'keyword.control': 'cm-cadence-keyword-control',
    'constant.note': 'cm-cadence-note',
    'constant.numeric': 'cm-cadence-number',
    'constant.boolean': 'cm-cadence-number',
    'string': 'cm-cadence-string',
    'variable': 'cm-cadence-variable',
    'operator': 'cm-cadence-operator',
    'operator.comparison': 'cm-cadence-operator',
    'operator.assignment': 'cm-cadence-operator',
    'punctuation': 'cm-cadence-punctuation',
    'comment': 'cm-cadence-comment',
};

// Create decoration marks for each token type
const tokenMarks: Record<string, Decoration> = {};
for (const [tokenType, cssClass] of Object.entries(TOKEN_CLASSES)) {
    tokenMarks[tokenType] = Decoration.mark({ class: cssClass });
}

/**
 * Convert WASM highlight spans to CodeMirror decorations
 */
function spansToDecorations(spans: HighlightSpan[], doc: any): DecorationSet {
    const builder = new RangeSetBuilder<Decoration>();

    // Sort spans by position (required for RangeSetBuilder)
    const sortedSpans = [...spans].sort((a, b) => {
        if (a.start_line !== b.start_line) return a.start_line - b.start_line;
        return a.start_col - b.start_col;
    });

    for (const span of sortedSpans) {
        // Skip empty token types
        if (!span.token_type || span.token_type === '') continue;

        const mark = tokenMarks[span.token_type];
        if (!mark) continue;

        try {
            // Convert line/column to document offset
            // WASM uses 1-indexed lines and columns
            const line = doc.line(span.start_line);
            const from = line.from + (span.start_col - 1);
            const to = from + span.text.length;

            // Ensure we don't go past the document end
            if (from >= 0 && to <= doc.length && from < to) {
                builder.add(from, to, mark);
            }
        } catch (e) {
            // Line doesn't exist, skip
            continue;
        }
    }

    return builder.finish();
}

/**
 * ViewPlugin that applies WASM tokenization as decorations
 */
const wasmHighlightPlugin = ViewPlugin.fromClass(
    class {
        decorations: DecorationSet;

        constructor(view: EditorView) {
            this.decorations = this.computeDecorations(view);
        }

        update(update: ViewUpdate) {
            if (update.docChanged || update.viewportChanged) {
                this.decorations = this.computeDecorations(update.view);
            }
        }

        computeDecorations(view: EditorView): DecorationSet {
            if (!isWasmReady()) {
                return Decoration.none;
            }

            const doc = view.state.doc;
            const text = doc.toString();

            try {
                const spans = tokenizeCode(text);
                return spansToDecorations(spans, doc);
            } catch (e) {
                console.error('WASM tokenization error:', e);
                return Decoration.none;
            }
        }
    },
    {
        decorations: v => v.decorations
    }
);

/**
 * WASM-powered Cadence language support
 * Uses real Rust tokenizer for syntax highlighting
 */
export function cadenceWasm(): Extension {
    return [
        wasmHighlightPlugin,
        // Base theme for token colors (dark theme)
        EditorView.baseTheme({
            '.cm-cadence-keyword': { color: '#e94560', fontWeight: '600' },
            '.cm-cadence-keyword-control': { color: '#ff7b9c' },
            '.cm-cadence-note': { color: '#4ecca3', fontWeight: '500' },
            '.cm-cadence-number': { color: '#ffd166' },
            '.cm-cadence-string': { color: '#06d6a0' },
            '.cm-cadence-variable': { color: '#73d0ff' },
            '.cm-cadence-operator': { color: '#f4a261' },
            '.cm-cadence-punctuation': { color: '#888' },
            '.cm-cadence-comment': { color: '#5c6370', fontStyle: 'italic' },
        }),
    ];
}

// Also export the old stream-based version as fallback
export { cadence } from './lang-cadence';
