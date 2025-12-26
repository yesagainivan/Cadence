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
import { cadenceWasm } from './lang-cadence-wasm';
import { initWasm, parseCode, isWasmReady, runScript, type Action } from './cadence-wasm';
import { audioEngine } from './audio-engine';

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

      // Cadence language support (WASM-powered highlighting)
      cadenceWasm(),

      // Theme
      darkTheme,

      // Update listener for cursor position and validation
      EditorView.updateListener.of((update) => {
        if (update.selectionSet) {
          updateCursorPosition(update.view);
        }
        if (update.docChanged) {
          // Debounced validation
          scheduleValidation(update.view);
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

// Validation debounce timer
let validationTimer: number | null = null;

/**
 * Schedule validation with debouncing
 */
function scheduleValidation(view: EditorView): void {
  if (validationTimer) {
    clearTimeout(validationTimer);
  }

  validationTimer = window.setTimeout(() => {
    validateCode(view);
  }, 200);
}

/**
 * Validate code using WASM parser
 */
function validateCode(view: EditorView): void {
  const statusEl = document.getElementById('status');

  if (!isWasmReady()) {
    if (statusEl) statusEl.textContent = 'WASM loading...';
    return;
  }

  const code = view.state.doc.toString();
  const result = parseCode(code);

  if (statusEl) {
    if (result.success) {
      statusEl.textContent = '✓ Valid';
      statusEl.style.color = '#4ecca3';
    } else {
      statusEl.textContent = `✗ ${result.error || 'Parse error'}`;
      statusEl.style.color = '#e94560';
    }
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
 * Log an action to the output panel
 */
function logAction(action: Action): void {
  switch (action.type) {
    case 'Play':
      log(`  ▶ Play: ${action.events.length} event(s), loop=${action.looping}`);
      break;
    case 'SetTempo':
      log(`  🎵 Tempo: ${action.bpm} BPM`);
      break;
    case 'SetVolume':
      log(`  🔊 Volume: ${(action.volume * 100).toFixed(0)}%`);
      break;
    case 'SetWaveform':
      log(`  📈 Waveform: ${action.waveform}`);
      break;
    case 'Stop':
      log(`  ⏹ Stop`);
      break;
  }
}

/**
 * Initialize the editor and UI
 */
async function init(): Promise<void> {
  const editorContainer = document.getElementById('editor');
  if (!editorContainer) {
    console.error('Editor container not found');
    return;
  }

  // Initialize WASM
  log('Loading WASM...');
  try {
    await initWasm();
    log('✓ WASM loaded successfully');
  } catch (e) {
    log(`✗ WASM failed to load: ${e}`);
    console.error('WASM init error:', e);
  }

  const editor = createEditor(editorContainer);

  // Validate initial code
  validateCode(editor);

  // Play button
  const playBtn = document.getElementById('play-btn');
  playBtn?.addEventListener('click', () => {
    const code = editor.state.doc.toString();

    // Run the script via WASM interpreter
    const result = runScript(code);
    if (!result.success) {
      log(`✗ Cannot play: ${result.error}`);
      return;
    }

    log('▶ Playing...');
    log(`📋 ${result.actions.length} action(s) from script`);

    // Route each action to the audio engine
    for (const action of result.actions) {
      logAction(action);
      audioEngine.handleAction(action);
    }
  });

  // Stop button
  const stopBtn = document.getElementById('stop-btn');
  stopBtn?.addEventListener('click', () => {
    audioEngine.stop();
    log('■ Stopped');
  });

  // Tempo slider
  const tempoSlider = document.getElementById('tempo') as HTMLInputElement;
  const tempoValue = document.getElementById('tempo-value');
  tempoSlider?.addEventListener('input', () => {
    const bpm = parseInt(tempoSlider.value, 10);
    if (tempoValue) {
      tempoValue.textContent = tempoSlider.value;
    }
    audioEngine.setTempo(bpm);
  });

  // Initial log
  log('🎵 Cadence Editor initialized');
  log('Ready to make music!');

  // Focus editor
  editor.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
