/**
 * Centralized SVG Icon System for Cadence Editor
 * 
 * All icons as inline SVG strings for consistent styling and easy theming.
 * Icons use currentColor for fill/stroke to inherit from parent CSS.
 */

// =============================================================================
// Transport Icons
// =============================================================================

export const ICON_PLAY = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
  <polygon points="5,3 19,12 5,21"/>
</svg>`;

export const ICON_STOP = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
  <rect x="4" y="4" width="16" height="16" rx="2"/>
</svg>`;

export const ICON_PAUSE = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
  <rect x="6" y="4" width="4" height="16" rx="1"/>
  <rect x="14" y="4" width="4" height="16" rx="1"/>
</svg>`;

// =============================================================================
// Theme Icons
// =============================================================================

export const ICON_SUN = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <circle cx="12" cy="12" r="5"/>
  <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/>
</svg>`;

export const ICON_MOON = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
</svg>`;

// =============================================================================
// UI Icons
// =============================================================================

export const ICON_SETTINGS = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <circle cx="12" cy="12" r="3"/>
  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
</svg>`;

export const ICON_LOCK = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
  <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
</svg>`;

export const ICON_UNLOCK = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
  <path d="M7 11V7a5 5 0 0 1 9.9-1"/>
</svg>`;

export const ICON_MUSIC = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <path d="M9 18V5l12-2v13"/>
  <circle cx="18" cy="16" r="3"/>
</svg>`;

export const ICON_SPEAKER = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/>
  <path d="M15.54 8.46a5 5 0 0 1 0 7.07"/>
  <path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>
</svg>`;

export const ICON_PIANO = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <path d="M19 4H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2z"/>
  <path d="M9 4v16"/>
  <path d="M15 4v16"/>
  <path d="M7 4v9h2"/>
  <path d="M13 4v9h2"/>
</svg>`;

// =============================================================================
// Helper Function
// =============================================================================

/**
 * Get an icon by name with optional size override
 */
export function icon(name: keyof typeof ICONS, size?: number): string {
    const svg = ICONS[name];
    if (!svg) return '';

    if (size) {
        return svg
            .replace(/width="\d+"/, `width="${size}"`)
            .replace(/height="\d+"/, `height="${size}"`);
    }
    return svg;
}

// Icon lookup map
const ICONS = {
    play: ICON_PLAY,
    stop: ICON_STOP,
    pause: ICON_PAUSE,
    sun: ICON_SUN,
    moon: ICON_MOON,
    settings: ICON_SETTINGS,
    lock: ICON_LOCK,
    unlock: ICON_UNLOCK,
    music: ICON_MUSIC,
    speaker: ICON_SPEAKER,
    piano: ICON_PIANO,
} as const;

export type IconName = keyof typeof ICONS;
