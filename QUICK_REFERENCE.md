# Quick Reference Guide

## What Changed?

### ✅ ADDED:
- Scout model for MCQ extraction
- OSS-120B model for MCQ solving
- Two-step workflow (extract → solve)

### ❌ REMOVED:
- Interview tab
- Debug button (🐛)
- Mic button (🎤)
- Maverick model (deprecated)
- Fireworks API

### 🔄 MODIFIED:
- MCQ workflow now uses Scout + OSS-120B
- UI simplified to MCQ-only mode

---

## Current UI:

```
┌─────────────────────────────────────┐
│ 🎯 Nvidia                      ● │
├─────────────────────────────────────┤
│ [MCQ]                               │
├─────────────────────────────────────┤
│ [📸 Capture] [🗑️ Clear]             │
└─────────────────────────────────────┘
```

---

## How to Use:

1. Click **📸 Capture**
2. Select MCQ area on screen
3. Wait for Scout to extract
4. Wait for OSS-120B to solve
5. View answer in chat

---

## Models Used:

| Model | Purpose | API |
|-------|---------|-----|
| Scout (llama-4-scout-17b-16e-instruct) | Extract MCQ from image | Groq |
| OSS-120B (gpt-oss-120b) | Solve MCQ with reasoning | Groq |

---

## Workflow:

```
Image → Scout → OSS-120B → Answer
```

**Scout**: Extracts question + options
**OSS-120B**: Provides answer + reasoning

---

## Files Modified:

1. `src/modules/groq.rs` - MCQ method updated
2. `static/index.html` - UI simplified

---

## Build & Run:

```bash
# Stop running app first
cargo build --release
# Start app
./target/release/nvidia.exe
```

---

## Testing:

1. Open app in browser
2. Click "📸 Capture"
3. Select MCQ question
4. Verify answer appears

---

## Troubleshooting:

**Q: Build fails with "Access denied"**
A: Stop the running application first

**Q: No answer appears**
A: Check console for errors, verify API key

**Q: Wrong answer**
A: Scout + OSS-120B provides better accuracy than old system

---

## Documentation Files:

- `MCQ_CHANGES_SUMMARY.md` - Detailed changes
- `MCQ_WORKFLOW_DIAGRAM.md` - Visual workflow
- `WORKFLOW_ANSWER.md` - Answer to your question
- `QUICK_REFERENCE.md` - This file
