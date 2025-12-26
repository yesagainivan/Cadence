/**
 * Cadence Language Mode for CodeMirror 6
 * 
 * Provides syntax highlighting for the Cadence music programming language.
 * Uses a simple tokenizer that will later be replaced with WASM-based tokenization.
 */

import { HighlightStyle, syntaxHighlighting, StreamLanguage } from '@codemirror/language';
import { tags } from '@lezer/highlight';

// Cadence keywords
const KEYWORDS = new Set([
    'let', 'fn', 'loop', 'repeat', 'if', 'else', 'break', 'continue', 'return',
    'play', 'stop', 'track', 'on', 'load'
]);

const CONTROL_KEYWORDS = new Set([
    'tempo', 'volume', 'waveform', 'queue'
]);

// Note pattern: A-G optionally followed by # or b, optionally followed by octave
const NOTE_PATTERN = /^[A-G][#b]?-?\d*/;

/**
 * Simple stream-based tokenizer for Cadence
 * This will be replaced with WASM tokenization for full accuracy
 */
const cadenceStreamParser = {
    name: 'cadence',

    token(stream: any): string | null {
        // Skip whitespace
        if (stream.eatSpace()) return null;

        // Comments
        if (stream.match('//')) {
            stream.skipToEnd();
            return 'comment';
        }

        // Multi-line comments
        if (stream.match('/*')) {
            while (!stream.match('*/') && !stream.eol()) {
                stream.next();
            }
            return 'comment';
        }

        // Strings
        if (stream.match('"')) {
            while (!stream.eol()) {
                const ch = stream.next();
                if (ch === '"') break;
                if (ch === '\\') stream.next(); // escape
            }
            return 'string';
        }

        // Numbers (including floats)
        if (stream.match(/^-?\d+\.?\d*/)) {
            return 'number';
        }

        // Identifiers, keywords, and notes
        if (stream.match(/^[A-Za-z_][A-Za-z0-9_#b-]*/)) {
            const word = stream.current();

            // Check if it's a note (A-G followed by optional accidental and octave)
            if (NOTE_PATTERN.test(word) && word.length <= 4) {
                return 'atom'; // Notes
            }

            if (KEYWORDS.has(word)) {
                return 'keyword';
            }

            if (CONTROL_KEYWORDS.has(word)) {
                return 'keyword'; // Will be styled as control keyword via class
            }

            if (word === 'true' || word === 'false') {
                return 'bool';
            }

            return 'variableName';
        }

        // Operators
        if (stream.match(/^[+\-&|^]/)) {
            return 'operator';
        }

        // Comparison operators
        if (stream.match('==') || stream.match('!=')) {
            return 'operator';
        }

        // Assignment
        if (stream.match('=')) {
            return 'operator';
        }

        // Brackets
        if (stream.match('[[') || stream.match(']]')) {
            return 'bracket';
        }

        if (stream.match(/^[\[\](){},;.]/)) {
            return 'punctuation';
        }

        // Skip unknown characters
        stream.next();
        return null;
    }
};

/**
 * CodeMirror StreamLanguage for Cadence
 */
export const cadenceLanguage = StreamLanguage.define(cadenceStreamParser);

/**
 * Cadence-specific highlight style
 */
export const cadenceHighlightStyle = HighlightStyle.define([
    { tag: tags.keyword, class: 'cm-cadence-keyword' },
    { tag: tags.atom, class: 'cm-cadence-note' },
    { tag: tags.number, class: 'cm-cadence-number' },
    { tag: tags.string, class: 'cm-cadence-string' },
    { tag: tags.variableName, class: 'cm-cadence-variable' },
    { tag: tags.operator, class: 'cm-cadence-operator' },
    { tag: tags.punctuation, class: 'cm-cadence-punctuation' },
    { tag: tags.bracket, class: 'cm-cadence-punctuation' },
    { tag: tags.comment, class: 'cm-cadence-comment' },
    { tag: tags.bool, class: 'cm-cadence-number' },
]);

/**
 * Complete Cadence language support extension
 */
export function cadence() {
    return [
        cadenceLanguage,
        syntaxHighlighting(cadenceHighlightStyle),
    ];
}
