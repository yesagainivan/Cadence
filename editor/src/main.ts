/**
 * Cadence Editor - Main Entry Point
 * 
 * A web-based editor for the Cadence music programming language
 * with CodeMirror 6 integration and WASM-powered features.
 */

import './style.css';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightActiveLine } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching, foldGutter, foldKeymap } from '@codemirror/language';
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
import { autocompletion, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { cadence } from './lang-cadence';

// Sample Cadence code
const SAMPLE_CODE = `// Welcome to Cadence! 🎵
// A music programming language for live coding

// Set the tempo
tempo 120

// Define a simple chord progression
let cmaj = [C, E, G]
let fmaj = [F, A, C]
let gmaj = [G, B, D]
let amin = [A, C, E]

// Create a pattern
let pattern = "C E G _ E G C _"

// Play a chord
play cmaj

// Play with a pattern (uncomment to try)
// play pattern loop

// Try some transformations:
// play cmaj + 2     // transpose up 2 semitones
// play invert(cmaj) // first inversion
`;

// Dark theme for CodeMirror
const darkTheme = EditorView.theme({
  '&': {
    backgroundColor: '#1a1a2e',
    color: '#e8e8e8',
  },
  '.cm-content': {
    caretColor: '#e94560',
  },
  '.cm-cursor': {
    borderLeftColor: '#e94560',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'rgba(233, 69, 96, 0.3)',
  },
  '.cm-gutters': {
    backgroundColor: '#16213e',
    color: '#5c6370',
    border: 'none',
  },
  '.cm-activeLineGutter': {
    backgroundColor: '#0f3460',
  },
  '.cm-activeLine': {
    backgroundColor: 'rgba(255, 255, 255, 0.03)',
  },
}, { dark: true });

/**
 * Create the CodeMirror editor
 */
function createEditor(container: HTMLElement): EditorView {
  const state = EditorState.create({
    doc: SAMPLE_CODE,
    extensions: [
      // Core
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightActiveLine(),
      history(),
      foldGutter(),
      bracketMatching(),
      closeBrackets(),
      autocompletion(),
      highlightSelectionMatches(),

      // Keymaps
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        ...foldKeymap,
        ...closeBracketsKeymap,
        ...searchKeymap,
      ]),

      // Cadence language support
      cadence(),

      // Theme
      darkTheme,

      // Update listener for cursor position
      EditorView.updateListener.of((update) => {
        if (update.selectionSet) {
          updateCursorPosition(update.view);
        }
        if (update.docChanged) {
          validateCode(update.view);
        }
      }),
    ],
  });

  return new EditorView({
    state,
    parent: container,
  });
}

/**
 * Update cursor position display
 */
function updateCursorPosition(view: EditorView): void {
  const pos = view.state.selection.main.head;
  const line = view.state.doc.lineAt(pos);
  const col = pos - line.from + 1;

  const cursorPosEl = document.getElementById('cursor-pos');
  if (cursorPosEl) {
    cursorPosEl.textContent = `Ln ${line.number}, Col ${col}`;
  }
}

/**
 * Validate code and update status
 * TODO: Use WASM parse_and_check() for real validation
 */
function validateCode(_view: EditorView): void {
  const statusEl = document.getElementById('status');
  if (statusEl) {
    statusEl.textContent = 'Ready';
  }
}

/**
 * Log to output panel
 */
function log(message: string): void {
  const outputEl = document.getElementById('output');
  if (outputEl) {
    const timestamp = new Date().toLocaleTimeString();
    outputEl.textContent += `[${timestamp}] ${message}\n`;
    outputEl.scrollTop = outputEl.scrollHeight;
  }
}

/**
 * Initialize the editor and UI
 */
function init(): void {
  const editorContainer = document.getElementById('editor');
  if (!editorContainer) {
    console.error('Editor container not found');
    return;
  }

  const editor = createEditor(editorContainer);

  // Play button
  const playBtn = document.getElementById('play-btn');
  playBtn?.addEventListener('click', () => {
    const code = editor.state.doc.toString();
    log(`▶ Playing...`);
    log(`Code: ${code.split('\n')[0]}...`);
    // TODO: Send to WASM interpreter
  });

  // Stop button
  const stopBtn = document.getElementById('stop-btn');
  stopBtn?.addEventListener('click', () => {
    log('■ Stopped');
    // TODO: Stop playback
  });

  // Tempo slider
  const tempoSlider = document.getElementById('tempo') as HTMLInputElement;
  const tempoValue = document.getElementById('tempo-value');
  tempoSlider?.addEventListener('input', () => {
    if (tempoValue) {
      tempoValue.textContent = tempoSlider.value;
    }
    log(`Tempo: ${tempoSlider.value} BPM`);
  });

  // Initial log
  log('Cadence Editor initialized');
  log('Ready to make music! 🎵');

  // Focus editor
  editor.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
