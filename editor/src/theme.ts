/**
 * Centralized Theme System for Cadence Editor
 * 
 * Single source of truth for all colors across CSS, CodeMirror, and canvas components.
 */

import { EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';

// ============================================================================
// Theme Interface & Definitions
// ============================================================================

export interface ThemeColors {
    // Backgrounds
    bg: string;
    bgElevated: string;
    bgInset: string;
    bgHover: string;

    // Foregrounds
    fg: string;
    fgMuted: string;
    fgSubtle: string;

    // Borders
    border: string;
    borderSubtle: string;

    // Accent
    accent: string;
    accentHover: string;
    accentActive: string;

    // Syntax highlighting
    synKeyword: string;
    synKeywordControl: string;
    synNote: string;
    synNumber: string;
    synString: string;
    synVariable: string;
    synOperator: string;
    synPunctuation: string;
    synComment: string;

    // Status
    success: string;
    error: string;
    warning: string;
    info: string;

    // Piano roll note colors (by pitch class)
    noteColors: string[];
}

export interface Theme {
    name: 'dark' | 'light';
    colors: ThemeColors;
}

// ============================================================================
// Theme Definitions
// ============================================================================

export const DARK_THEME: Theme = {
    name: 'dark',
    colors: {
        // Backgrounds
        bg: '#282c34',
        bgElevated: '#32363e',
        bgInset: '#21242b',
        bgHover: 'rgba(255, 255, 255, 0.04)',

        // Foregrounds
        fg: '#e0dcd4',
        fgMuted: '#9a958e',
        fgSubtle: '#6b6560',

        // Borders
        border: '#3e4249',
        borderSubtle: '#32363e',

        // Accent
        accent: '#7099aa',
        accentHover: '#5a7d8d',
        accentActive: '#4a6d7d',

        // Syntax
        synKeyword: '#c9736f',
        synKeywordControl: '#d4908c',
        synNote: '#7fb069',
        synNumber: '#d4a656',
        synString: '#7fb069',
        synVariable: '#7099aa',
        synOperator: '#c9a869',
        synPunctuation: '#7a746d',
        synComment: '#6b6560',

        // Status
        success: '#7fb069',
        error: '#c9736f',
        warning: '#d4a656',
        info: '#7099aa',

        // Piano roll note colors (muted, earthy palette)
        noteColors: [
            '#c9736f', // C  - muted red
            '#b86662', // C#
            '#d4a656', // D  - warm gold
            '#c49a4e', // D#
            '#d9bf6a', // E  - light gold
            '#7fb069', // F  - earthy green
            '#6e9d5c', // F#
            '#7099aa', // G  - blue-grey
            '#5d8495', // G#
            '#9a8fbd', // A  - muted purple
            '#877baa', // A#
            '#bf8fa3', // B  - dusty rose
        ],
    },
};

export const LIGHT_THEME: Theme = {
    name: 'light',
    colors: {
        // Backgrounds
        bg: '#f5f2ec',
        bgElevated: '#ebe7df',
        bgInset: '#e2ddd5',
        bgHover: 'rgba(0, 0, 0, 0.03)',

        // Foregrounds
        fg: '#3c4245',
        fgMuted: '#6b6863',
        fgSubtle: '#9a958e',

        // Borders
        border: '#d5d0c8',
        borderSubtle: '#e2ddd5',

        // Accent
        accent: '#4d7686',
        accentHover: '#3a5c69',
        accentActive: '#2d4a54',

        // Syntax (slightly darker for light bg)
        synKeyword: '#a85450',
        synKeywordControl: '#b86662',
        synNote: '#5a8a48',
        synNumber: '#b08530',
        synString: '#5a8a48',
        synVariable: '#4d7686',
        synOperator: '#a08540',
        synPunctuation: '#6b6560',
        synComment: '#9a958e',

        // Status
        success: '#5a8a48',
        error: '#a85450',
        warning: '#b08530',
        info: '#4d7686',

        // Piano roll note colors (deeper for light bg)
        noteColors: [
            '#a85450', // C
            '#9a4a47', // C#
            '#b08530', // D
            '#9a7528', // D#
            '#b89840', // E
            '#5a8a48', // F
            '#4d7a3d', // F#
            '#4d7686', // G
            '#3d6070', // G#
            '#7a6f9a', // A
            '#6a5f8a', // A#
            '#9a707a', // B
        ],
    },
};

// ============================================================================
// Theme State & Event System
// ============================================================================

type ThemeChangeCallback = (theme: Theme) => void;

let currentTheme: Theme = DARK_THEME;
const listeners: ThemeChangeCallback[] = [];

/**
 * Get the current theme
 */
export function getTheme(): Theme {
    return currentTheme;
}

/**
 * Set the current theme and notify all listeners
 */
export function setTheme(name: 'dark' | 'light'): void {
    currentTheme = name === 'light' ? LIGHT_THEME : DARK_THEME;

    // Update CSS custom properties
    applyCSSVariables(currentTheme);

    // Update data-theme attribute for any CSS that still uses it
    if (name === 'light') {
        document.documentElement.setAttribute('data-theme', 'light');
    } else {
        document.documentElement.removeAttribute('data-theme');
    }

    // Persist preference
    localStorage.setItem('cadence-theme', name);

    // Notify listeners
    listeners.forEach(cb => cb(currentTheme));
}

/**
 * Initialize theme from localStorage (call on app startup)
 */
export function initTheme(): void {
    const saved = localStorage.getItem('cadence-theme');
    if (saved === 'light' || saved === 'dark') {
        currentTheme = saved === 'light' ? LIGHT_THEME : DARK_THEME;
    }
    applyCSSVariables(currentTheme);
    if (currentTheme.name === 'light') {
        document.documentElement.setAttribute('data-theme', 'light');
    }
}

/**
 * Subscribe to theme changes
 */
export function onThemeChange(callback: ThemeChangeCallback): void {
    listeners.push(callback);
}

/**
 * Toggle between dark and light themes
 */
export function toggleTheme(): void {
    setTheme(currentTheme.name === 'dark' ? 'light' : 'dark');
}

// ============================================================================
// CSS Variable Sync
// ============================================================================

function applyCSSVariables(theme: Theme): void {
    const root = document.documentElement;
    const c = theme.colors;

    root.style.setProperty('--color-bg', c.bg);
    root.style.setProperty('--color-bg-elevated', c.bgElevated);
    root.style.setProperty('--color-bg-inset', c.bgInset);
    root.style.setProperty('--color-bg-hover', c.bgHover);

    root.style.setProperty('--color-fg', c.fg);
    root.style.setProperty('--color-fg-muted', c.fgMuted);
    root.style.setProperty('--color-fg-subtle', c.fgSubtle);

    root.style.setProperty('--color-border', c.border);
    root.style.setProperty('--color-border-subtle', c.borderSubtle);

    root.style.setProperty('--color-accent', c.accent);
    root.style.setProperty('--color-accent-hover', c.accentHover);
    root.style.setProperty('--color-accent-active', c.accentActive);

    root.style.setProperty('--color-syn-keyword', c.synKeyword);
    root.style.setProperty('--color-syn-keyword-control', c.synKeywordControl);
    root.style.setProperty('--color-syn-note', c.synNote);
    root.style.setProperty('--color-syn-number', c.synNumber);
    root.style.setProperty('--color-syn-string', c.synString);
    root.style.setProperty('--color-syn-variable', c.synVariable);
    root.style.setProperty('--color-syn-operator', c.synOperator);
    root.style.setProperty('--color-syn-punctuation', c.synPunctuation);
    root.style.setProperty('--color-syn-comment', c.synComment);

    root.style.setProperty('--color-success', c.success);
    root.style.setProperty('--color-error', c.error);
    root.style.setProperty('--color-warning', c.warning);
    root.style.setProperty('--color-info', c.info);
}

// ============================================================================
// CodeMirror Theme Builder
// ============================================================================

/**
 * Build a CodeMirror theme extension from the given theme
 */
export function buildCMTheme(theme: Theme): Extension {
    const c = theme.colors;
    const isDark = theme.name === 'dark';

    return EditorView.theme({
        '&': {
            backgroundColor: c.bg,
            color: c.fg,
        },
        '.cm-content': {
            caretColor: c.accent,
        },
        '.cm-cursor': {
            borderLeftColor: c.accent,
            borderLeftWidth: '2px',
        },
        '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection': {
            backgroundColor: isDark ? 'rgba(112, 153, 170, 0.25)' : 'rgba(77, 118, 134, 0.25)',
        },
        '.cm-gutters': {
            backgroundColor: c.bg,
            color: c.fgSubtle,
            border: 'none',
        },
        '.cm-activeLineGutter': {
            backgroundColor: c.bgElevated,
            color: c.fgMuted,
        },
        '.cm-activeLine': {
            backgroundColor: c.bgHover,
        },
        // Syntax highlighting - dynamic colors for theme switching
        '.cm-cadence-keyword': {
            color: c.synKeyword,
            fontWeight: '700',
        },
        '.cm-cadence-keyword-control': {
            color: c.synKeywordControl,
            fontWeight: '600',
            fontStyle: 'italic',
        },
        '.cm-cadence-note': {
            color: c.synNote,
            fontWeight: '600',
        },
        '.cm-cadence-number': {
            color: c.synNumber,
        },
        '.cm-cadence-string': {
            color: c.synString,
        },
        '.cm-cadence-variable': {
            color: c.synVariable,
        },
        '.cm-cadence-operator': {
            color: c.synOperator,
        },
        '.cm-cadence-punctuation': {
            color: c.synPunctuation,
        },
        '.cm-cadence-comment': {
            color: c.synComment,
            fontStyle: 'italic',
        },
    }, { dark: isDark });
}
