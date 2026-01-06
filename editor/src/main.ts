/**
 * Cadence Editor - Main Entry Point
 * 
 * A web-based single-file playground for the Cadence music programming language
 * with CodeMirror 6 integration and WASM-powered features.
 */

import './style.css';
import { Compartment, EditorState } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLineGutter, highlightActiveLine } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { bracketMatching, foldGutter, foldKeymap } from '@codemirror/language';
import { searchKeymap, highlightSelectionMatches } from '@codemirror/search';
import { autocompletion, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { linter, lintGutter } from '@codemirror/lint';
import type { Diagnostic } from '@codemirror/lint';
import { cadenceWasm } from './lang-cadence-wasm';
import { initWasm, parseCode, isWasmReady, getEventsAtPosition, getContextAtCursor } from './cadence-wasm';
import { audioEngine } from './audio-engine';
import { PianoRoll } from './piano-roll';
import { PropertiesPanel } from './properties-panel';
import { initTheme, getTheme, onThemeChange, buildCMTheme } from './theme';
import { Toolbar, StatusBar, setupResizablePanels } from './components';
import { debouncedRefreshSymbols } from './hover';
import { gotoDefinition, gotoDefinitionPlugin, initGotoDefinitionCursor } from './gotoDefinition';
import { icon } from './icons';

// Global instances
let pianoRoll: PianoRoll | null = null;
let propertiesPanel: PropertiesPanel | null = null;
let toolbar: Toolbar | null = null;
let statusBar: StatusBar | null = null;

// CodeMirror theme compartment for dynamic theme switching
const themeCompartment = new Compartment();

// localStorage key for persisting code
const STORAGE_KEY = 'cadence_code';

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



/**
 * Cadence Linter
 * Uses the WASM parser to find errors and display them in the editor.
 */
const cadenceLinter = linter((view) => {
  if (!isWasmReady()) return [];

  const code = view.state.doc.toString();
  const result = parseCode(code);

  if (!result.success) {
    // Support both single error (legacy/fallback) and detailed errors array
    const errors = (result.errors && result.errors.length > 0) ? result.errors : (result.error ? [result.error] : []);

    return errors.map(err => {
      let from = err.start;
      let to = err.end;

      // If range is empty (0 length), highlight at least one char or end of line
      if (from === to) {
        to = Math.min(from + 1, code.length);
      }

      const diagnostic: Diagnostic = {
        from,
        to,
        severity: 'error',
        message: err.message,
      };
      return diagnostic;
    });
  }

  return [];
}, { delay: 100 });  // Fast 100ms delay for responsive error feedback

/**
 * Create the CodeMirror editor
 */
function createEditor(container: HTMLElement): EditorView {
  // Load saved code or use sample
  const savedCode = localStorage.getItem(STORAGE_KEY) || SAMPLE_CODE;

  const state = EditorState.create({
    doc: savedCode,
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
      lintGutter(),
      cadenceLinter,

      // Keymaps
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        ...foldKeymap,
        ...closeBracketsKeymap,
        ...searchKeymap,
        { key: 'F12', run: gotoDefinition },
      ]),

      // Cadence language support (WASM-powered highlighting)
      cadenceWasm(),

      // Go-to-definition (Cmd+Click)
      gotoDefinitionPlugin,

      // Theme (via Compartment for dynamic switching)
      themeCompartment.of(buildCMTheme(getTheme())),

      // Update listener for cursor position and validation
      EditorView.updateListener.of((update) => {
        if (update.selectionSet) {
          updateCursorPosition(update.view);
          // Also update piano roll when cursor moves
          updatePianoRollAtCursor(update.view);
        }
        if (update.docChanged) {
          const code = update.view.state.doc.toString();
          // Refresh symbols for hover (updates user function docs)
          debouncedRefreshSymbols(code);
          // Debounced validation
          scheduleValidation(update.view);
          // Auto-save to localStorage
          localStorage.setItem(STORAGE_KEY, code);
        }
      }),
    ],
  });

  const view = new EditorView({
    state,
    parent: container,
  });

  // Enable pointer cursor on Cmd/Ctrl hold
  initGotoDefinitionCursor(container);

  return view;
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
let currentCursorContext: { variable_name: string | null; span: { utf16_start: number } } | null = null;

function updatePianoRollAtCursor(view: EditorView): void {
  if (!pianoRoll || !isWasmReady()) return;

  const code = view.state.doc.toString();
  const cursorPos = view.state.selection.main.head;

  // Get context for lock target info
  const context = getContextAtCursor(code, cursorPos);
  currentCursorContext = context ? {
    variable_name: context.variable_name,
    span: { utf16_start: context.span.utf16_start }
  } : null;

  // Get pattern events via direct WASM call
  const patternEvents = getEventsAtPosition(code, cursorPos);

  if (patternEvents && patternEvents.events && patternEvents.events.length > 0) {
    pianoRoll.update(patternEvents);
    // Status is managed by validateCode, no need to update here

  } else {
    // Check if there was an error in the pattern evaluation (e.g. arity error)
    if (patternEvents?.error) {
      statusBar?.setError(`Error: ${patternEvents.error}`);
    }

    // Clear piano roll if no events
    pianoRoll.update({
      events: [],
      beats_per_cycle: { n: 4, d: 1 },
      error: null
    });
  }
}

/**
 * Refresh piano roll if locked (called on code changes)
 */
function refreshPianoRollIfLocked(code: string): void {
  if (!pianoRoll || !pianoRoll.isLocked()) return;

  // Helper to find a variable's position in the code
  const findVariablePosition = (code: string, varName: string): number | null => {
    // Look for `let varName = ` or `varName = `
    const letMatch = code.match(new RegExp(`\\blet\\s+${varName}\\s*=`));
    if (letMatch && letMatch.index !== undefined) {
      return letMatch.index;
    }
    const assignMatch = code.match(new RegExp(`\\b${varName}\\s*=`));
    if (assignMatch && assignMatch.index !== undefined) {
      return assignMatch.index;
    }
    return null;
  };

  pianoRoll.refreshLocked(code, findVariablePosition);
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

    // Refresh locked piano roll with new code (live updates)
    refreshPianoRollIfLocked(code);

    // Update piano roll with statement at cursor position (if not locked)
    updatePianoRollAtCursor(view);

    // Live update audio if valid and playing
    if (result && result.success && audioEngine.playing) {
      audioEngine.updateScript(code);
    }
  }, 100);  // 100ms debounce for responsive validation
}

/**
 * Validate code using WASM parser
 */
function validateCode(view: EditorView): ReturnType<typeof parseCode> | null {
  if (!isWasmReady()) {
    statusBar?.setStatus('WASM loading...', 'info');
    return null;
  }

  const code = view.state.doc.toString();
  const result = parseCode(code);

  if (result.success) {
    statusBar?.setValid();
  } else {
    const msg = result.error ? result.error.message : 'Parse error';
    statusBar?.setError(msg);
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
 * Initialize the editor and UI
 */
async function init(): Promise<void> {
  // Initialize theme from localStorage FIRST (before any rendering)
  initTheme();

  const editorContainer = document.getElementById('editor');
  if (!editorContainer) {
    console.error('Editor container not found');
    return;
  }

  // Initialize WASM
  log('Loading WASM...');
  try {
    await initWasm();
    log('WASM loaded');
  } catch (e) {
    log(`WASM failed: ${e}`);
    console.error('WASM init error:', e);
  }

  const editor = createEditor(editorContainer);

  // Initialize piano roll
  try {
    pianoRoll = new PianoRoll('piano-roll');

    // Set up events callback for locked refresh
    pianoRoll.setEventsCallback((code, position) => {
      return getEventsAtPosition(code, position);
    });

    // Wire up lock button to pass current context
    const lockBtn = document.getElementById('piano-roll-lock');
    if (lockBtn) {
      // Remove the piano roll's own listener and add ours with context
      lockBtn.replaceWith(lockBtn.cloneNode(true));
      const newLockBtn = document.getElementById('piano-roll-lock');
      newLockBtn?.addEventListener('click', () => {
        pianoRoll?.toggleLock(currentCursorContext);
      });
    }

    log('Piano roll initialized');
  } catch (e) {
    log(`Piano roll failed: ${e}`);
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

      // Find the correct insertion point for new method calls
      // Should be BEFORE 'loop' or 'queue' keywords, not at the statement end
      const findInsertionPoint = (): number => {
        // Look for standalone 'loop' or 'queue' keywords (not inside strings)
        // Match word boundary to avoid matching inside identifiers
        const loopMatch = fullStatementText.match(/\s+(loop|queue)(\s|$)/);
        if (loopMatch && loopMatch.index !== undefined) {
          // Insert just before the space+keyword
          return spanStart + loopMatch.index;
        }
        // No loop/queue keyword, use spanEnd
        return spanEnd;
      };

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
          // console.log(`✏️ Replaced waveform: ${match[0]} → .wave("${waveformValue}")`);
        } else {
          // Insert .wave(...) at correct position (before loop/queue if present)
          const insertPos = findInsertionPoint();
          transaction = editor.state.update({
            changes: {
              from: insertPos,
              to: insertPos,
              insert: `.wave("${waveformValue}")`,
            },
          });
          // console.log(`✏️ Added waveform at pos ${insertPos}: .wave("${waveformValue}")`);
        }

        editor.dispatch(transaction);
      } else if (change.type === 'envelope') {
        const [a, d, s, r] = change.value as number[];

        // Check if .env(...) already exists in the statement
        const envRegex = /\.env\s*\([^)]*\)/g;
        const match = envRegex.exec(fullStatementText);

        let transaction;

        if (match && match.index !== undefined) {
          // Replace existing .env(...) at exact position
          const matchStart = spanStart + match.index;
          const matchEnd = matchStart + match[0].length;
          transaction = editor.state.update({
            changes: {
              from: matchStart,
              to: matchEnd,
              insert: `.env(${a}, ${d}, ${s}, ${r})`,
            },
          });
          // console.log(`✏️ Replaced envelope: ${match[0]} → .env(${a}, ${d}, ${s}, ${r})`);
        } else {
          // Insert .env(...) at correct position (before loop/queue if present)
          const insertPos = findInsertionPoint();
          transaction = editor.state.update({
            changes: {
              from: insertPos,
              to: insertPos,
              insert: `.env(${a}, ${d}, ${s}, ${r})`,
            },
          });
          // console.log(`✏️ Added envelope at pos ${insertPos}: .env(${a}, ${d}, ${s}, ${r})`);
        }

        editor.dispatch(transaction);
      }
    });



    log('Properties panel initialized');
  } catch (e) {
    log(`Properties panel failed: ${e}`);
  }

  // Validate initial code and update piano roll
  validateCode(editor);
  scheduleValidation(editor);

  // Initialize resizable panels
  const editorEl = document.getElementById('editor');
  const pianoRollContainerEl = document.getElementById('piano-roll-container');
  const editorContainerEl = document.querySelector('.editor-container') as HTMLElement;
  const sidebarEl = document.getElementById('sidebar');
  const mainEl = document.querySelector('.editor-main') as HTMLElement;

  // Editor / Piano Roll vertical resize
  // We want to resize piano-roll (the second panel, below editor)
  if (editorEl && pianoRollContainerEl && editorContainerEl) {
    setupResizablePanels({
      container: editorContainerEl,
      firstPanel: editorEl,
      secondPanel: pianoRollContainerEl,
      direction: 'horizontal',
      storageKey: 'cadence_piano_roll_height',
      minFirstSize: 150,
      minSecondSize: 100,
      defaultSize: 160,
      resizeSecond: true,  // Resize the piano roll, not the editor
    });
  }

  // Main / Sidebar horizontal resize  
  // We want to resize sidebar (the second panel, on right)
  if (editorContainerEl && sidebarEl && mainEl) {
    setupResizablePanels({
      container: mainEl,
      firstPanel: editorContainerEl,
      secondPanel: sidebarEl,
      direction: 'vertical',
      storageKey: 'cadence_sidebar_width',
      minFirstSize: 300,
      minSecondSize: 200,
      defaultSize: 280,
      resizeSecond: true,  // Resize the sidebar, not the main editor
    });
  }

  // Initialize toolbar with callbacks
  toolbar = new Toolbar();
  toolbar.setCallbacks({
    onPlay: async () => {
      const code = editor.state.doc.toString();
      log('Playing...');
      await audioEngine.playScript(code);

      // Start playhead animation
      if (pianoRoll) {
        pianoRoll.startAnimation(() => {
          const pos = audioEngine.getPlaybackPosition();
          return pos.isPlaying ? pos.beat : null;
        });
      }
    },
    onStop: () => {
      audioEngine.stop();
      log('Stopped');

      // Stop playhead animation
      if (pianoRoll) {
        pianoRoll.stopAnimation();
      }
    },
    onTempoChange: (bpm) => {
      audioEngine.setTempo(bpm);
    },
  });

  // Initialize status bar
  statusBar = new StatusBar();

  // Initialize output mode toggle and MIDI device selector
  const midiSelect = document.getElementById('midi-output') as HTMLSelectElement;
  const modeButtons = document.querySelectorAll('.mode-btn');
  const midiOutputService = audioEngine.getMidiOutput();

  // Set icons for mode buttons
  const btnAudio = document.getElementById('mode-audio');
  const btnMidi = document.getElementById('mode-midi');
  const btnBoth = document.getElementById('mode-both');

  if (btnAudio) btnAudio.innerHTML = icon('speaker');
  if (btnMidi) btnMidi.innerHTML = icon('piano');
  if (btnBoth) btnBoth.innerHTML = `<div style="display: flex; gap: 2px;">${icon('speaker', 14)}${icon('piano', 14)}</div>`;

  // Populate MIDI devices
  midiOutputService.init().then((success) => {
    if (success) {
      const outputs = midiOutputService.getOutputs();
      for (const output of outputs) {
        const option = document.createElement('option');
        option.value = output.id;
        option.textContent = output.name ?? output.id;
        midiSelect.appendChild(option);
      }
      if (outputs.length > 0) {
        log(`🎹 Found ${outputs.length} MIDI output(s)`);
      }
    } else {
      // Disable MIDI modes if not supported
      document.getElementById('mode-midi')?.classList.add('disabled');
      document.getElementById('mode-both')?.classList.add('disabled');
    }
  });

  // Handle MIDI device selection
  midiSelect.addEventListener('change', () => {
    midiOutputService.selectOutput(midiSelect.value);
    if (midiSelect.value) {
      log(`🎹 MIDI: ${midiSelect.options[midiSelect.selectedIndex].text}`);
    }
  });

  // Handle output mode toggle
  modeButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = (btn as HTMLElement).dataset.mode as 'audio' | 'midi' | 'both';

      // Update engine mode
      audioEngine.setOutputMode(mode);

      // Update button states
      modeButtons.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');

      // Show/hide MIDI device selector
      if (mode === 'midi' || mode === 'both') {
        midiSelect.classList.remove('hidden');
        // Auto-select first device if none selected
        if (!midiSelect.value && midiSelect.options.length > 1) {
          midiSelect.value = midiSelect.options[1].value;
          midiOutputService.selectOutput(midiSelect.value);
          log(`🎹 MIDI: ${midiSelect.options[midiSelect.selectedIndex].text}`);
        }
      } else {
        midiSelect.classList.add('hidden');
      }

      log(`🔊 Output: ${mode}`);
    });
  });

  // Subscribe to theme changes to update CodeMirror
  onThemeChange((theme) => {
    editor.dispatch({
      effects: themeCompartment.reconfigure(buildCMTheme(theme))
    });
    log(`Theme: ${theme.name}`);
  });

  // Initial log
  log('Cadence Editor initialized');
  log('Ready to make music!');

  // Focus editor
  editor.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
