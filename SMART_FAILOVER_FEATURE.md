# Smart Groq → Cerebras Failover System

## Overview

Intelligent failover system that automatically switches between Groq (fast, primary) and Cerebras (reliable backup) based on performance and availability.

## Strategy

```
┌─────────────┐
│ Start: Groq │ (Primary - Fast & Reliable)
└──────┬──────┘
       │
       ├─ Success → Continue with Groq ✓
       │
       └─ Failure/Timeout (3s) 
          │
          ▼
    ┌──────────────────┐
    │ Cerebras Fallback│ (5 responses)
    │ with Key Rotation│
    └────────┬─────────┘
             │
             ├─ After 5 responses → Return to Groq
             │
             └─ If Groq fails 3 times total
                │
                ▼
          ┌────────────────┐
          │ Cerebras       │ (Permanent for session)
          │ Permanent Mode │
          └────────────────┘
```

## Configuration

### Constants (in `groq.rs`)
```rust
const GROQ_TIMEOUT: Duration = Duration::from_secs(3);  // 3 second timeout
const CEREBRAS_FALLBACK_WINDOW: usize = 5;              // 5 responses
const MAX_GROQ_FAILURES: usize = 3;                     // 3 failures = permanent switch
```

## Behavior Details

### Scenario 1: Single Groq Timeout
```
Request 1:  Groq timeout (>3s) → Cerebras (fallback 1/5) | Failures: 1
Request 2:  Cerebras (fallback 2/5)
Request 3:  Cerebras (fallback 3/5)  
Request 4:  Cerebras (fallback 4/5)
Request 5:  Cerebras (fallback 5/5)
Request 6:  Back to Groq ✓           | Failures: reset to 0 on success
Request 7:  Groq ✓
...
```

### Scenario 2: Multiple Groq Timeouts (< 3)
```
Request 1:  Groq timeout → Cerebras (1/5)              | Failures: 1
Request 6:  Groq SUCCESS ✓                             | Failures: 0 (reset!)
Request 7:  Groq timeout → Cerebras (1/5)              | Failures: 1 (fresh count)
Request 12: Groq SUCCESS ✓                             | Failures: 0
...
```

### Scenario 3: Three Groq Failures → Permanent Switch
```
Request 1:  Groq timeout → Cerebras (1/5)              | Failures: 1
Request 6:  Groq timeout → Cerebras (1/5)              | Failures: 2
Request 11: Groq timeout → Cerebras PERMANENT          | Failures: 3 ⚠️
Request 12: Cerebras (permanent mode)
Request 13: Cerebras (permanent mode)
... all future requests use Cerebras until app restart
```

## State Management

### State Variables (GroqClient)
```rust
groq_consecutive_failures: Arc<AtomicUsize>      // Tracks consecutive failures
use_cerebras_permanently: Arc<AtomicBool>        // Permanent switch flag
cerebras_fallback_count: Arc<AtomicUsize>        // Count for 5-response window
```

### State Transitions

#### Groq Success
- Reset `groq_consecutive_failures` to 0
- Broadcast "Groq" to UI indicator

#### Groq Failure/Timeout
- Increment `groq_consecutive_failures`
- If < 3: Start temporary Cerebras fallback (5 responses)
- If >= 3: Set `use_cerebras_permanently` = true

#### Cerebras Response
- If permanent mode: Continue with Cerebras
- If temporary mode: Increment counter
  - Counter > 5: Reset and return to Groq

## UI Indicators

### Display Names
```javascript
// Cerebras keys (existing behavior - shows key name)
"Shinchan", "Gyan", "Hattori", etc.

// Groq (new)
"Groq"

// Permanent Cerebras (new)  
"Cerebras (Permanent)"
```

### Indicator Updates
```json
{
    "type": "active_api_key",
    "name": "Groq"  // or key name, or "Cerebras (Permanent)"
}
```

## Implementation Details

### Helper Methods

#### `should_use_cerebras() -> (bool, bool)`
```rust
// Returns: (use_cerebras, should_try_groq_first)
// Checks:
// 1. Permanent mode flag
// 2. Temporary fallback counter (1-5)
// 3. Default: try Groq
```

#### `handle_groq_failure()`
```rust
// 1. Increment failure counter
// 2. Check if >= 3 failures
// 3. If yes: set permanent flag + notify UI
// 4. If no: start temporary fallback
```

#### `handle_groq_success()`
```rust
// 1. Reset failure counter to 0
// 2. Notify UI (display "Groq")
```

#### `increment_cerebras_fallback()`
```rust
// 1. Increment fallback counter
// 2. If > 5: reset counter (return to Groq next time)
```

### API Call Flow

```rust
// 1. Check failover state
let (should_use_cerebras, should_try_groq) = self.should_use_cerebras();

// 2. Try Groq first (if appropriate)
if should_try_groq && !should_use_cerebras {
    match timeout(3s, groq_request()).await {
        Success => {
            handle_groq_success();
            return response;
        }
        Failure/Timeout => {
            handle_groq_failure();
            // Fall through to Cerebras
        }
    }
}

// 3. Use Cerebras
increment_cerebras_fallback();  // Only if temporary mode
return cerebras_request_with_rotation();
```

## Cerebras Key Rotation

Existing rotation logic continues to work:
- Every 2 requests → next key
- On 429 error → immediately try next key
- Rotation wraps around all available keys

## Edge Cases

### No Cerebras Keys
- Fallback disabled
- Groq used without timeout
- Errors returned directly

### All Cerebras Keys Fail
- Return error to user
- Stay in Cerebras mode (don't switch back)
- Next request tries Cerebras again

### Groq Success During Fallback
- Success resets failure counter
- But doesn't exit fallback window early
- Must complete all 5 fallback responses

## Logging

```
[Failover] Groq failure #1
[Failover] → Temporary Cerebras fallback (5 responses)
[Failover] ✓ Groq success - reset failure counter (was 1)
[Failover] Groq failure #3
[Failover] ⚠ 3 Groq failures - PERMANENTLY switching to Cerebras for this session
[Failover] ← Returning to Groq after 5 Cerebras responses
```

## Benefits

1. **Speed**: Groq is tried first (fastest API)
2. **Reliability**: Auto-fallback prevents complete failures
3. **Recovery**: Automatic return to Groq after temporary issues
4. **Smart Switching**: Permanent switch only after repeated failures
5. **Session Scope**: Reset on restart (no persistent state)

## Testing

### Test 1: Single Timeout
```rust
// Given: Groq times out once
// When: Make 6 requests
// Then: 
//   - Request 1: Groq timeout → Cerebras
//   - Requests 2-5: Cerebras
//   - Request 6: Back to Groq
```

### Test 2: Groq Success Resets Counter
```rust
// Given: Groq timeout → success → timeout
// When: Check failure counter
// Then: Counter = 1 (reset after success)
```

### Test 3: Permanent Switch
```rust
// Given: Groq fails 3 times
// When: Make more requests
// Then: All future requests use Cerebras
```

### Test 4: Cerebras Rotation
```rust
// Given: In Cerebras mode with 3 keys
// When: Make 6 requests
// Then: Keys rotate: 1→1→2→2→3→3
```

## Files Modified

1. ✅ `src/modules/groq.rs`
   - Added state fields
   - Added helper methods
   - Updated `chat_with_context()` with failover logic

## Rebuild Required

```bash
cd interview-helper
cargo build --release --bin nvidia
```

## Status

✅ **Complete and Ready for Testing**

---

**Feature Date**: June 5, 2026  
**Primary Goal**: Maximize speed (Groq) while ensuring reliability (Cerebras)  
**Session Scope**: Failover state resets on app restart
