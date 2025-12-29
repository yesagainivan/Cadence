# Deprecated Audio Modules

These modules have been replaced by `event_dispatcher.rs` as of December 2024.

## Why Deprecated

The original architecture had two separate systems:
- **PlaybackEngine**: Handled looping patterns with queue support
- **Scheduler**: Handled one-shot scheduled events

This led to complexity and "fighting" between systems. The unified `EventDispatcher` consolidates both into a single system inspired by Sonic Pi and TidalCycles.

## Archived Modules

- `playback_engine.rs` - Full-featured progression player with queue support
- `scheduler.rs` - Virtual time-based scheduler for one-shot events

## Notes

The `QueueMode` functionality from PlaybackEngine is NOT yet implemented in EventDispatcher. If queue support is needed, either:
1. Implement it in EventDispatcher
2. Temporarily restore these modules

## Restoration

To restore a module:
1. Move it back to `src/audio/`
2. Add `pub mod <module_name>;` to `src/audio/mod.rs`
