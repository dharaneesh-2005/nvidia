# MCQ Workflow Diagram

## NEW WORKFLOW (Current Implementation)

```
┌─────────────────────────────────────────────────────────────────┐
│                         USER INTERFACE                          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Header: 🎯 Nvidia                                       │  │
│  │  Mode: [MCQ]                                             │  │
│  │  Buttons: [📸 Capture] [🗑️ Clear]                        │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ User clicks "📸 Capture"
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      SNIP TOOL (Region Selection)               │
│                                                                 │
│  User selects MCQ area on screen                               │
│  Screenshot captured as base64 image                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Image data (base64)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    STEP 1: SCOUT MODEL                          │
│                                                                 │
│  Model: meta-llama/llama-4-scout-17b-16e-instruct              │
│  API: Groq API                                                  │
│  Temperature: 0.2                                               │
│  Max Tokens: 2000                                               │
│                                                                 │
│  Task: Extract MCQ details from image                           │
│  Output: JSON with question, options, context                   │
│                                                                 │
│  Example Output:                                                │
│  {                                                              │
│    "question": "What is the capital of France?",                │
│    "options": [                                                 │
│      {"label": "A", "text": "London"},                          │
│      {"label": "B", "text": "Paris"},                           │
│      {"label": "C", "text": "Berlin"}                           │
│    ],                                                           │
│    "context": "Geography question"                              │
│  }                                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Extracted JSON data
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    STEP 2: OSS-120B MODEL                       │
│                                                                 │
│  Model: openai/gpt-oss-120b                                     │
│  API: Groq API                                                  │
│  Temperature: 0.4                                               │
│  Max Tokens: 2000                                               │
│                                                                 │
│  Task: Solve MCQ with reasoning                                 │
│  Input: Extracted question + options from Scout                 │
│  Output: Answer with reasoning                                  │
│                                                                 │
│  Example Output:                                                │
│  "The correct answer is B: Paris.                               │
│   Paris is the capital and largest city of France.              │
│   It has been the capital since the 12th century."              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Final answer
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DISPLAY TO USER                            │
│                                                                 │
│  Answer shown in chat interface with:                           │
│  - Question (Q box)                                             │
│  - Answer with reasoning (A box)                                │
│  - Markdown formatting                                          │
│  - Code highlighting (if applicable)                            │
└─────────────────────────────────────────────────────────────────┘
```

## OLD WORKFLOW (Deprecated - Using Maverick)

```
┌─────────────────────────────────────────────────────────────────┐
│                         USER INTERFACE                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      SNIP TOOL                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              SINGLE STEP: MAVERICK MODEL (DEPRECATED)           │
│                                                                 │
│  Model: meta-llama/llama-4-maverick-17b-128e-instruct          │
│  API: Groq API                                                  │
│  Status: ❌ DEPRECATED - NO LONGER AVAILABLE                    │
│                                                                 │
│  Task: Direct image → answer (single step)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      DISPLAY TO USER                            │
└─────────────────────────────────────────────────────────────────┘
```

## COMPARISON

| Aspect | Old (Maverick) | New (Scout + OSS-120B) |
|--------|----------------|------------------------|
| **Steps** | 1 step | 2 steps |
| **Models** | Maverick (deprecated) | Scout + OSS-120B |
| **Status** | ❌ Not available | ✅ Active |
| **Extraction** | Direct | Scout extracts details |
| **Reasoning** | Limited | OSS-120B provides reasoning |
| **Accuracy** | Lower | Higher |
| **API** | Groq | Groq |

## WHY TWO STEPS?

1. **Scout Model Strengths**:
   - Excellent at vision tasks
   - Accurately extracts text from images
   - Handles complex layouts
   - Structured output (JSON)

2. **OSS-120B Model Strengths**:
   - Excellent at reasoning
   - Solves complex problems
   - Provides detailed explanations
   - Better at logical thinking

3. **Combined Benefits**:
   - Scout extracts → OSS-120B solves
   - Better accuracy than single-step
   - Clear reasoning in answers
   - Handles edge cases better

## ANSWER TO YOUR QUESTION

**Q: Does Scout answer the MCQ or pass to OSS-120B?**

**A: Scout PASSES to OSS-120B**

- Scout extracts the complete MCQ details from the image
- Scout returns structured JSON with question, options, context
- OSS-120B receives this extracted data
- OSS-120B analyzes and provides the final answer with reasoning

This is the same workflow as the Interview mode was using, but now applied to MCQ questions.
