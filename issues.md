---

## Editor: Emoji Position Mismatch ✅ Fixed

**Status**: Resolved via UTF-16 offset tracking

**Problem**: When code contains emojis (e.g., `// Welcome 🎵`), span positions from Rust were off by 1+ characters in JavaScript.

**Root cause**:
- Rust `Vec<char>` counts emoji as 1 character
- JavaScript strings count emoji as 2 UTF-16 code units (surrogate pair)

**Fix Applied**:
- Added `utf16_offset` and `utf16_len` fields to Rust `Span` struct
- Lexer now tracks UTF-16 position alongside char position
- WASM bindings expose UTF-16 offsets in `HighlightSpan` and `SpanInfoJS`
- TypeScript editor uses UTF-16 positions for highlighting and property edits

**Impact**: Property edits now insert at correct positions even with emoji in code.