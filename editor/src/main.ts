/**
 * Cadence Editor - Main Entry Point
 * 
 * A web-based editor for the Cadence music programming language
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
import { initTheme, getTheme, toggleTheme, onThemeChange, buildCMTheme } from './theme';
import { debouncedRefreshSymbols } from './hover';
import { gotoDefinition, gotoDefinitionPlugin, initGotoDefinitionCursor } from './gotoDefinition';
import { initializeFileSystem, getFileSystemService, FileSystemService } from './filesystem-service';
import { FileTreePanel } from './file-tree';
import { TabBar } from './tab-bar';

// Global instances
let pianoRoll: PianoRoll | null = null;
let propertiesPanel: PropertiesPanel | null = null;
let fileTree: FileTreePanel | null = null;
let tabBar: TabBar | null = null;
let editorView: EditorView | null = null;

// CodeMirror theme compartment for dynamic theme switching
const themeCompartment = new Compartment();

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
});

/**
 * Save the current file to the virtual filesystem
 * Called by Cmd+S / Ctrl+S keyboard shortcut
 */
function saveCurrentFile(): boolean {
  const activeTab = tabBar?.getActiveTab();

  if (!activeTab) {
    // No file open - offer to create one
    const name = prompt('Save as:', 'untitled.cadence');
    if (name && editorView) {
      const path = `/${name}`;
      const content = editorView.state.doc.toString();
      const fs = getFileSystemService();

      fs.writeFile(path, content).then(() => {
        tabBar?.openTab(path, content);
        tabBar?.setDirty(path, false);
        fileTree?.refresh();
        log(`Saved as: ${path}`);
      }).catch(e => {
        log(`Save failed: ${e}`);
      });
    }
    return true;
  }

  // Save existing file
  if (editorView) {
    const content = editorView.state.doc.toString();
    const fs = getFileSystemService();

    fs.writeFile(activeTab, content).then(() => {
      tabBar?.setDirty(activeTab, false);
      tabBar?.updateContent(activeTab, content);
      log(`Saved: ${activeTab}`);
    }).catch(e => {
      log(`Save failed: ${e}`);
    });
  }

  return true;  // Prevent browser default
}

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
        { key: 'Mod-s', run: saveCurrentFile, preventDefault: true },
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

  // Get pattern events with cycle timing for the statement at cursor position
  const patternEvents = getEventsAtPosition(code, cursorPos);

  // Debug: uncomment to trace cursor positions
  // console.log(`🎹 Piano roll: cursor=${cursorPos}, events=${patternEvents?.events?.length ?? 'null'}, cycle=${patternEvents?.beats_per_cycle}`);

  if (patternEvents && patternEvents.events && patternEvents.events.length > 0) {
    pianoRoll.update(patternEvents);

    // Clear error status if valid events
    const statusEl = document.getElementById('status');
    if (statusEl && statusEl.textContent?.startsWith('Error:')) {
      statusEl.innerHTML = '<span class="status-dot"></span>Valid';
      const dot = statusEl.querySelector('.status-dot') as HTMLElement;
      if (dot) dot.style.backgroundColor = '#7fb069';
    }

  } else {
    // Check if there was an error in the pattern evaluation (e.g. arity error)
    if (patternEvents?.error) {
      const statusEl = document.getElementById('status');
      if (statusEl) {
        statusEl.innerHTML = `<span class="status-dot"></span>Error: ${patternEvents.error}`;
        const dot = statusEl.querySelector('.status-dot') as HTMLElement;
        if (dot) dot.style.backgroundColor = '#c9736f';
      }
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
    // Update status dot color based on result
    const statusDot = statusEl.querySelector('.status-dot') as HTMLElement | null;

    if (result.success) {
      statusEl.innerHTML = '<span class="status-dot"></span>Valid';
      if (statusDot || statusEl.querySelector('.status-dot')) {
        (statusEl.querySelector('.status-dot') as HTMLElement).style.backgroundColor = '#7fb069'; // --color-success
      }
    } else {
      const msg = result.error ? result.error.message : 'Parse error';
      statusEl.innerHTML = `<span class="status-dot"></span>${msg}`;
      if (statusEl.querySelector('.status-dot')) {
        (statusEl.querySelector('.status-dot') as HTMLElement).style.backgroundColor = '#c9736f'; // --color-error
      }
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

  // Initialize virtual filesystem (OPFS)
  if (FileSystemService.isSupported()) {
    try {
      await initializeFileSystem();
      log('File system initialized (OPFS)');
    } catch (e) {
      log(`File system failed: ${e}`);
    }
  } else {
    log('OPFS not supported - file browser disabled');
  }

  const editor = createEditor(editorContainer);
  editorView = editor;

  // Initialize tab bar
  const tabBarContainer = document.getElementById('tab-bar');
  if (tabBarContainer) {
    tabBar = new TabBar(tabBarContainer, {
      onTabSelect: async (path) => {
        // Load file content into editor
        const content = tabBar?.getContent(path);
        if (content !== null && content !== undefined && editorView) {
          editorView.dispatch({
            changes: { from: 0, to: editorView.state.doc.length, insert: content }
          });
        }
        log(`Opened: ${path}`);
      },
      onTabClose: (path) => {
        log(`Closed: ${path}`);
        // If no more tabs, show sample code
        if (tabBar && !tabBar.getActiveTab() && editorView) {
          editorView.dispatch({
            changes: { from: 0, to: editorView.state.doc.length, insert: SAMPLE_CODE }
          });
        }
      },
      onSaveRequest: async (path) => {
        // Save current content to filesystem
        const content = editorView?.state.doc.toString() || '';
        const fs = getFileSystemService();
        await fs.writeFile(path, content);
        tabBar?.setDirty(path, false);
        tabBar?.updateContent(path, content);
        log(`Saved: ${path}`);
      }
    });
  }

  // Initialize file tree panel
  const fileTreeContainer = document.getElementById('file-tree-panel');
  if (fileTreeContainer && FileSystemService.isSupported()) {
    fileTree = new FileTreePanel(fileTreeContainer, {
      onFileSelect: async (path) => {
        // Open file in tab
        const fs = getFileSystemService();
        try {
          const content = await fs.readFile(path);
          tabBar?.openTab(path, content);
          // Load into editor
          if (editorView) {
            editorView.dispatch({
              changes: { from: 0, to: editorView.state.doc.length, insert: content }
            });
          }
        } catch (e) {
          log(`Failed to open: ${e}`);
        }
      },
      onFileCreate: (path) => {
        log(`Created: ${path}`);
      },
      onFileDelete: (path) => {
        tabBar?.removeTab(path);
        log(`Deleted: ${path}`);
      },
      onFileRename: (oldPath, newPath) => {
        tabBar?.renameTab(oldPath, newPath);
        log(`Renamed: ${oldPath} → ${newPath}`);
      }
    });

    // Initial refresh
    await fileTree.refresh();
  }

  // Track dirty state on edits if a tab is open
  editor.dom.addEventListener('input', () => {
    const activeTab = tabBar?.getActiveTab();
    if (activeTab) {
      tabBar?.setDirty(activeTab, true);
      // Update content in memory
      tabBar?.updateContent(activeTab, editor.state.doc.toString());
    }
  });

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
          console.log(`✏️ Replaced waveform: ${match[0]} → .wave("${waveformValue}")`);
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
          console.log(`✏️ Added waveform at pos ${insertPos}: .wave("${waveformValue}")`);
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
          console.log(`✏️ Replaced envelope: ${match[0]} → .env(${a}, ${d}, ${s}, ${r})`);
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
          console.log(`✏️ Added envelope at pos ${insertPos}: .env(${a}, ${d}, ${s}, ${r})`);
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

  // Play button
  const playBtn = document.getElementById('play-btn');
  playBtn?.addEventListener('click', () => {
    const code = editor.state.doc.toString();

    // Play script reactively
    log('Playing...');
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
    log('Stopped');

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

  // Theme toggle - use centralized theme system
  const themeToggle = document.getElementById('theme-toggle');
  themeToggle?.addEventListener('click', () => {
    toggleTheme();
    log(`Theme: ${getTheme().name}`);
  });

  // Subscribe to theme changes to update CodeMirror
  onThemeChange((theme) => {
    editor.dispatch({
      effects: themeCompartment.reconfigure(buildCMTheme(theme))
    });
    // Piano roll and ADSR will subscribe separately
  });

  // Initial log
  log('Cadence Editor initialized');
  log('Ready to make music!');

  // Focus editor
  editor.focus();
}

// Start the app
document.addEventListener('DOMContentLoaded', init);
