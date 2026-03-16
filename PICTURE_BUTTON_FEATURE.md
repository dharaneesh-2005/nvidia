# Picture Button Feature - Documentation

## Changes Made

### 1. **Token Limits Updated**

#### Normal Capture Mode:
- **Scout Model**: 3000 tokens (increased from 2000)
- **OSS-120B Model**: 2000 tokens

#### Picture Mode:
- **Scout Model**: 6000 tokens (double the normal mode)
- **OSS-120B Model**: 2000 tokens

---

### 2. **New Picture Button Added**

**Location**: Next to Capture button in UI

**Icon**: 🖼️ Picture

**Purpose**: For MCQ questions that contain images, diagrams, charts, or visual patterns

---

## How It Works

### **Normal Capture Button (📸 Capture)**

**Use for**: Text-based MCQ questions

**Workflow**:
```
1. User clicks "📸 Capture"
2. User selects region
3. Scout extracts question + options (3000 tokens)
4. OSS-120B solves the MCQ (2000 tokens)
5. Answer displayed
```

**Scout Prompt**: Simple extraction
- Extract question text
- Extract options (A, B, C, D, E)
- Extract context

---

### **Picture Button (🖼️ Picture)**

**Use for**: Image-based MCQ questions (diagrams, charts, patterns, visual questions)

**Workflow**:
```
1. User clicks "🖼️ Picture"
2. User selects region
3. Scout analyzes image in EXTREME detail (6000 tokens)
4. OSS-120B solves based on detailed description (2000 tokens)
5. Answer displayed
```

**Scout Prompt**: Exhaustive visual analysis
- Describe EVERY visual element in extreme detail
- For diagrams: All components, connections, relationships
- For charts: Axes, data points, trends, labels
- For patterns: Sequence, transformations, rules
- For images: Objects, arrangement, spatial relationships
- Describe each option in complete detail

---

## When to Use Which Button?

### Use **📸 Capture** for:
- ✅ Text-only MCQ questions
- ✅ Questions with text options
- ✅ Simple questions without images
- ✅ Verbal/Math questions with text

**Example**:
```
Question: What is the capital of France?
A) London
B) Paris
C) Berlin
D) Madrid
```

---

### Use **🖼️ Picture** for:
- ✅ Questions with diagrams
- ✅ Questions with charts/graphs
- ✅ Pattern recognition questions
- ✅ Visual spatial questions
- ✅ Questions where options are images
- ✅ Questions requiring visual analysis

**Example**:
```
Question: Which shape completes the pattern?
[Shows a sequence of rotating shapes]
Options: [4 different shape images]
```

---

## Technical Details

### Code Changes

#### 1. **groq.rs** - Added two methods:

```rust
// Normal mode (3000 tokens)
pub async fn answer_mcq_direct(&self, image_base64: &str) -> Result<String, String>

// Picture mode (6000 tokens)
pub async fn answer_mcq_picture(&self, image_base64: &str) -> Result<String, String>

// Internal method with mode parameter
async fn answer_mcq_internal(&self, image_base64: &str, is_picture_mode: bool) -> Result<String, String>
```

#### 2. **main.rs** - Added handlers:

```rust
// Normal MCQ snip handler
async fn handle_mcq_snip_capture(...)

// Picture MCQ snip handler
async fn handle_mcq_picture_snip_capture(...)
```

#### 3. **index.html** - Added UI button:

```html
<button class="btn" onclick="capturePicture()">🖼️ Picture</button>
```

---

## Token Allocation Strategy

### Why Different Token Limits?

**Normal Mode (3000 tokens)**:
- Text extraction is straightforward
- Question + options + context = ~500-1500 tokens
- 3000 tokens provides comfortable buffer

**Picture Mode (6000 tokens)**:
- Visual descriptions are verbose
- Need to describe shapes, colors, positions, relationships
- Each option might need 200-500 tokens of description
- Complex diagrams need detailed explanation
- 6000 tokens ensures Scout can be exhaustive

---

## UI Layout

```
┌─────────────────────────────────────────────────────────┐
│ 🎯 Nvidia                                          ●    │
├─────────────────────────────────────────────────────────┤
│ [MCQ]                                                   │
├─────────────────────────────────────────────────────────┤
│ [📸 Capture] [🖼️ Picture] [🗑️ Clear]                   │
└─────────────────────────────────────────────────────────┘
```

---

## Example Prompts

### Normal Capture Prompt (Scout):
```
"Extract the question, all options, and any relevant context. 
Format as JSON with question, options array, and context."
```

### Picture Capture Prompt (Scout):
```
"Describe EVERYTHING in extreme detail so another AI can solve 
it without seeing the image.

VISUAL ELEMENTS (Be extremely detailed):
- Describe every shape, object, diagram, chart, or pattern
- Mention colors, sizes, positions, orientations
- For graphs/charts: Describe axes, data points, trends, labels
- For diagrams: Describe all components, connections, relationships
- For patterns: Describe the sequence, transformations, rules

OPTIONS: For EACH option:
- If image/diagram: Describe it in complete detail
- Explain what makes each option unique

CRITICAL: The downstream AI cannot see the image. Your 
description must be so detailed that someone could solve 
the question from your words alone."
```

---

## Testing

### Test Normal Capture:
1. Find a text-based MCQ
2. Click "📸 Capture"
3. Select the question area
4. Verify answer is correct

### Test Picture Capture:
1. Find an image-based MCQ (diagram/chart/pattern)
2. Click "🖼️ Picture"
3. Select the question area
4. Verify Scout provides detailed visual description
5. Verify OSS-120B solves correctly

---

## Performance

### Normal Mode:
- **Scout**: ~2-3 seconds (3000 tokens)
- **OSS-120B**: ~2-3 seconds (2000 tokens)
- **Total**: ~4-6 seconds

### Picture Mode:
- **Scout**: ~4-6 seconds (6000 tokens)
- **OSS-120B**: ~2-3 seconds (2000 tokens)
- **Total**: ~6-9 seconds

---

## Summary

| Feature | Normal Capture | Picture Capture |
|---------|---------------|-----------------|
| **Button** | 📸 Capture | 🖼️ Picture |
| **Scout Tokens** | 3000 | 6000 |
| **OSS-120B Tokens** | 2000 | 2000 |
| **Use Case** | Text MCQs | Image MCQs |
| **Prompt** | Simple extraction | Exhaustive visual analysis |
| **Speed** | ~4-6 seconds | ~6-9 seconds |

---

## Files Modified

1. `src/modules/groq.rs` - Added picture mode methods
2. `src/main.rs` - Added picture mode handlers
3. `static/index.html` - Added Picture button
