/**
 * Toolbar Component
 * 
 * Header controls for the Cadence editor: play/stop, tempo, theme toggle.
 */

import { getTheme, toggleTheme, onThemeChange } from '../theme';

export interface ToolbarCallbacks {
    onPlay: () => void;
    onStop: () => void;
    onTempoChange: (bpm: number) => void;
}

/**
 * Toolbar manages the header controls
 */
export class Toolbar {
    private playBtn: HTMLElement | null;
    private stopBtn: HTMLElement | null;
    private tempoSlider: HTMLInputElement | null;
    private tempoValue: HTMLElement | null;
    private themeToggle: HTMLElement | null;
    private callbacks: ToolbarCallbacks | null = null;

    constructor() {
        this.playBtn = document.getElementById('play-btn');
        this.stopBtn = document.getElementById('stop-btn');
        this.tempoSlider = document.getElementById('tempo') as HTMLInputElement;
        this.tempoValue = document.getElementById('tempo-value');
        this.themeToggle = document.getElementById('theme-toggle');

        this.setupEventListeners();
        this.updateThemeIcon();

        // Subscribe to theme changes
        onThemeChange(() => this.updateThemeIcon());
    }

    /**
     * Set callbacks for toolbar actions
     */
    setCallbacks(callbacks: ToolbarCallbacks): void {
        this.callbacks = callbacks;
    }

    /**
     * Get current tempo value
     */
    getTempo(): number {
        return this.tempoSlider ? parseInt(this.tempoSlider.value, 10) : 120;
    }

    /**
     * Set tempo value programmatically
     */
    setTempo(bpm: number): void {
        if (this.tempoSlider) {
            this.tempoSlider.value = String(bpm);
        }
        if (this.tempoValue) {
            this.tempoValue.textContent = String(bpm);
        }
    }

    /**
     * Update play button state (e.g., show pause when playing)
     */
    setPlaying(isPlaying: boolean): void {
        if (this.playBtn) {
            this.playBtn.classList.toggle('playing', isPlaying);
        }
    }

    private setupEventListeners(): void {
        // Play button
        this.playBtn?.addEventListener('click', () => {
            this.callbacks?.onPlay();
        });

        // Stop button
        this.stopBtn?.addEventListener('click', () => {
            this.callbacks?.onStop();
        });

        // Tempo slider
        this.tempoSlider?.addEventListener('input', () => {
            const bpm = this.getTempo();
            if (this.tempoValue) {
                this.tempoValue.textContent = String(bpm);
            }
            this.callbacks?.onTempoChange(bpm);
        });

        // Theme toggle
        this.themeToggle?.addEventListener('click', () => {
            toggleTheme();
        });
    }

    private updateThemeIcon(): void {
        if (!this.themeToggle) return;

        const isDark = getTheme().name === 'dark';
        const sunIcon = this.themeToggle.querySelector('.icon-sun') as HTMLElement;
        const moonIcon = this.themeToggle.querySelector('.icon-moon') as HTMLElement;

        if (sunIcon) sunIcon.style.display = isDark ? 'block' : 'none';
        if (moonIcon) moonIcon.style.display = isDark ? 'none' : 'block';
    }
}
