# Silence Timeout Fix - Prevents Question Splitting

## Problem
Interviewer's questions were being split into multiple parts when they paused naturally during speech. A single question like:

> "Can you explain... [pause] ...how dependency injection works?"

Was being treated as two separate questions:
1. "Can you explain"
2. "how dependency injection works"

## Root Cause
The silence timeout was too short (700ms - 1200ms), which is less than natural pauses in human speech.

## Solution
Increased all silence-related timeouts to **2 seconds** (2000ms).

## Changes Made

### 1. Main Audio Processing (`src/main.rs`)

#### Before:
```rust
const SILENCE_TIMEOUT: Duration = Duration::from_millis(1200);
const ENERGY_DROP_TIMEOUT: Duration = Duration::from_millis(350);
```

#### After:
```rust
const SILENCE_TIMEOUT: Duration = Duration::from_secs(2); // 2 seconds of silence before processing
const ENERGY_DROP_TIMEOUT: Duration = Duration::from_millis(800); // Increased to allow natural pauses
```

### 2. Audio Processor (`src/modules/audio_processor.rs`)

#### Before:
```rust
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_millis(700);
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_millis(1200);
```

#### After:
```rust
const BASE_SILENCE_TIMEOUT: Duration = Duration::from_secs(2); // 2 seconds for short pauses
const EXTENDED_SILENCE_TIMEOUT: Duration = Duration::from_secs(2); // 2 seconds for longer pauses
```

## How It Works Now

### Timing Flow
```
Interviewer speaks: "Can you explain..."
                    ↓
                [pause - 1 second]
                    ↓
Still listening... (< 2 seconds)
                    ↓
Interviewer continues: "how dependency injection works?"
                    ↓
                [silence - 2 seconds]
                    ↓
Process complete question ✓
Send to AI ✓
```

### Previous Behavior (BAD)
```
Interviewer: "Can you explain..."
[700ms pause] ← TOO SHORT!
Process partial question ❌ "Can you explain"
Send to AI ❌

Interviewer continues: "how dependency injection works?"
[700ms pause]
Process second part ❌ "how dependency injection works"
Send to AI ❌
```

### New Behavior (GOOD)
```
Interviewer: "Can you explain... [pause 1s] ...how dependency injection works?"
[2 second silence] ← WAITS FOR COMPLETE QUESTION
Process full question ✓ "Can you explain how dependency injection works?"
Send to AI ✓
```

## Benefits

1. **Complete Questions**: No more split questions
2. **Natural Pauses**: Allows for thinking pauses, "um", "uh", etc.
3. **Better AI Responses**: AI gets the full context in one go
4. **More Professional**: Mimics human listening behavior

## Trade-offs

- **Slight Delay**: Responses come 0.8-1 second later (2s vs 1.2s)
- **Worth It**: Much better question understanding outweighs the small delay

## Testing

To verify the fix works:

1. **Build the backend:**
   ```bash
   cd interview-helper
   cargo build --release --bin nvidia
   ```

2. **Start the application:**
   ```bash
   target/release/nvidia.exe
   ```

3. **Test with pauses:**
   - Speak: "Can you explain..." [pause 1 second] "...how caching works?"
   - Should be captured as ONE complete question
   - Check the UI - should show the full question in one box

4. **Test with longer pauses:**
   - Speak: "Tell me about..." [pause 1.5 seconds] "...your experience with databases"
   - Should still be one question

5. **End of question:**
   - After speaking, wait 2 full seconds
   - Question should be sent to AI after 2 seconds of silence

## Configuration

If you want to adjust the timeout further, edit these constants:

**For stricter (faster but more splits):**
```rust
const SILENCE_TIMEOUT: Duration = Duration::from_millis(1500); // 1.5 seconds
```

**For more lenient (slower but never splits):**
```rust
const SILENCE_TIMEOUT: Duration = Duration::from_millis(3000); // 3 seconds
```

Current setting of **2 seconds** is optimal for most natural speech patterns.

## Files Modified

1. ✅ `src/main.rs` - Main silence timeout and energy drop timeout
2. ✅ `src/modules/audio_processor.rs` - Base and extended silence timeouts

## Status

✅ **Complete** - Ready to rebuild and test

---

**Fix Date**: June 5, 2026  
**Issue**: Question splitting on natural pauses  
**Solution**: Increased silence timeout from 1.2s to 2.0s  
**Impact**: Minimal delay, much better question understanding
