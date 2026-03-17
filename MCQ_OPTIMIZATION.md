# MCQ Performance Optimization

## Changes Made

### 1. **Removed Conversation History**
- MCQ functions no longer send conversation history to API
- Reduces payload size significantly
- Faster API response times

### 2. **Reduced Token Limits**
All MCQ functions now use minimal tokens:
- `answer_mcq_direct()`: 1500 extract + 500 solve = **2000 total**
- `answer_mcq_picture()`: **800 tokens**
- `answer_interview_direct()`: **600 tokens**
- `analyze_images_internal()`: **2000 tokens**

Previous limits were 4000-6000 tokens per call.

### 3. **Simplified Prompts**
- Removed verbose instructions
- Focused on essential information only
- Clearer, more concise output format requirements

### 4. **Model Selection**
- `answer_mcq_direct()`: Scout (extract) → Scout (solve) - **2 API calls**
- `answer_mcq_picture()`: Scout only - **1 API call** ⚡ FASTEST
- `answer_interview_direct()`: Maverick only - **1 API call** ⚡ FASTEST

## Performance Targets

| Function | API Calls | Expected Time | Use Case |
|----------|-----------|---------------|----------|
| `answer_mcq_direct()` | 2 | 3-4 seconds | Text-based MCQs |
| `answer_mcq_picture()` | 1 | 2-3 seconds | Visual MCQs (patterns, diagrams) |
| `answer_interview_direct()` | 1 | 1-2 seconds | CCAT questions |

## Recommended Usage

### For Text MCQs:
```rust
groq.answer_mcq_direct(&image_base64).await
```

### For Visual MCQs (Patterns, Diagrams, Charts):
```rust
groq.answer_mcq_picture(&image_base64).await  // ⚡ FASTEST
```

### For CCAT/Interview Questions:
```rust
groq.answer_interview_direct(&image_base64).await  // ⚡ FASTEST
```

## UI Buttons Mapping

- `capture_mcq` → `answer_mcq_direct()` (2-step, slower)
- `capture_mcq_picture` → `answer_mcq_picture()` (1-step, faster) ⚡
- `capture_interview_snip` → `answer_interview_direct()` (1-step, fastest) ⚡

## What Was Removed

1. ❌ Conversation history (20 messages)
2. ❌ Verbose prompts (500+ words)
3. ❌ Excessive token limits (4000-6000)
4. ❌ Unnecessary context passing

## What Was Kept

1. ✅ Core functionality
2. ✅ Accuracy
3. ✅ Error handling
4. ✅ Retry logic
5. ✅ JSON parsing

## Expected Results

- **Before**: 5-7 seconds per MCQ
- **After**: 2-4 seconds per MCQ
- **Best case** (picture/interview modes): 1-3 seconds

All responses should complete within your 5-second target.
