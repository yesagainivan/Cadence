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
import { initWasm, parseCode, isWasmReady, getEventsAtPosition, getContextAtCursor } from './cadence-wasm';
import { audioEngine } from './audio-engine';
import { PianoRoll } from './piano-roll';
import { PropertiesPanel } from './properties-panel';

// Global instances
let pianoRoll: PianoRoll | null = null;
let propertiesPanel: PropertiesPanel | null = null;

// Sample Cadence code
const SAMPLE_CODE = `// Welcome to Cadence!
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
          // Also update piano roll when cursor moves
          updatePianoRollAtCursor(update.view);
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

  // Update properties panel with cursor context
  if (propertiesPanel && isWasmReady()) {
    const code = view.state.doc.toString();
    const context = getContextAtCursor(code, pos);
    propertiesPanel.update(context);
  }
}

/**
 * Update piano roll based on statement at cursor position
 */
function updatePianoRollAtCursor(view: EditorView): void {
  if (!pianoRoll || !isWasmReady()) return;

  const code = view.state.doc.toString();
  const cursorPos = view.state.selection.main.head;

  // Get pattern events with cycle timing for the statement at cursor position
  const patternEvents = getEventsAtPosition(code, cursorPos);

  // Debug: uncomment to trace cursor positions
  // console.log(`🎹 Piano roll: cursor=${cursorPos}, events=${patternEvents?.events?.length ?? 'null'}, cycle=${patternEvents?.beats_per_cycle}`);

  if (patternEvents && patternEvents.events && patternEvents.events.length > 0) {
    pianoRoll.update(patternEvents);
  } else {
    // Clear piano roll if no events
    pianoRoll.update({ events: [], beats_per_cycle: 4 });
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
    const result = validateCode(view);
    const code = view.state.doc.toString();

    // Update piano roll with statement at cursor position
    updatePianoRollAtCursor(view);

    // Live update audio if valid and playing
    if (result && result.success && audioEngine.playing) {
      audioEngine.updateScript(code);
    }
  }, 200);
}

/**
 * Validate code using WASM parser
 */
function validateCode(view: EditorView): ReturnType<typeof parseCode> | null {
  const statusEl = document.getElementById('status');

  if (!isWasmReady()) {
    if (statusEl) statusEl.textContent = 'WASM loading...';
    return null;
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

  return result;
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

  // Initialize piano roll
  try {
    pianoRoll = new PianoRoll('piano-roll');
    log('✓ Piano roll initialized');
  } catch (e) {
    log(`⚠ Piano roll failed: ${e}`);
  }

  // Initialize properties panel
  try {
    propertiesPanel = new PropertiesPanel('properties-panel');

    // Set up property change handler
    propertiesPanel.setOnPropertyChange((change) => {
      const code = editor.state.doc.toString();

      // Use UTF-16 offsets for accurate positioning with emoji/multi-byte chars
      const spanStart = change.spanStart;  // Note: PropertyChange now uses UTF-16 positions
      const spanEnd = change.spanEnd;

      // Find the end of the full statement (including any chained methods)
      // by searching for newline or semicolon after spanEnd
      let fullEnd = spanEnd;
      while (fullEnd < code.length && code[fullEnd] !== '\n' && code[fullEnd] !== ';') {
        fullEnd++;
      }

      const fullStatementText = code.slice(spanStart, fullEnd);

      if (change.type === 'waveform') {
        const waveformValue = change.value as string;

        // Check if .wave(...) already exists in the FULL statement (including chained methods)
        const waveRegex = /\.wave\s*\(\s*["'][^"']*["']\s*\)/g;
        const match = waveRegex.exec(fullStatementText);

        let transaction;

        if (match && match.index !== undefined) {
          // Replace existing .wave(...) at exact position
          const matchStart = spanStart + match.index;
          const matchEnd = matchStart + match[0].length;
          transaction = editor.state.update({
            changes: {
              from: matchStart,
              to: matchEnd,
              insert: `.wave("${waveformValue}")`,
            },
          });
          console.log(`✏️ Replaced waveform: ${match[0]} → .wave("${waveformValue}")`);
        } else {
          // Insert .wave(...) at end of base statement (spanEnd, not fullEnd)
          transaction = editor.state.update({
            changes: {
              from: spanEnd,
              to: spanEnd,
              insert: `.wave("${waveformValue}")`,
            },
          });
          console.log(`✏️ Added waveform at pos ${spanEnd}: .wave("${waveformValue}")`);
        }

        editor.dispatch(transaction);
      }
    });


    log('✓ Properties panel initialized');
  } catch (e) {
    log(`⚠ Properties panel failed: ${e}`);
  }

  // Validate initial code and update piano roll
  validateCode(editor);
  scheduleValidation(editor);

  // Play button
  const playBtn = document.getElementById('play-btn');
  playBtn?.addEventListener('click', () => {
    const code = editor.state.doc.toString();

    // Play script reactively
    log('▶ Playing...');
    audioEngine.playScript(code);

    // Start playhead animation
    if (pianoRoll) {
      pianoRoll.startAnimation(() => {
        const pos = audioEngine.getPlaybackPosition();
        return pos.isPlaying ? pos.beat : null;
      });
    }
  });

  // Stop button
  const stopBtn = document.getElementById('stop-btn');
  stopBtn?.addEventListener('click', () => {
    audioEngine.stop();
    log('■ Stopped');

    // Stop playhead animation
    if (pianoRoll) {
      pianoRoll.stopAnimation();
    }
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
