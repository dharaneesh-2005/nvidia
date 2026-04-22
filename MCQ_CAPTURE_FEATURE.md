# MCQ Capture Feature

## Overview
A new "Capture MCQ" button has been added to handle Multiple Choice Questions (MCQ) differently from DSA coding problems.

## Hotkey
**Ctrl+Alt+Q** - Capture and analyze MCQ from screen

## Key Differences from Regular Capture

### Regular Capture Button (📸 Capture)
- Designed for DSA coding problems
- Uses conversation history for context
- Provides detailed solutions with brute force and optimal approaches
- Adds to conversation history

### MCQ Capture Button (📝 MCQ)
- Specialized for Multiple Choice Questions
- **Hotkey: Ctrl+Alt+Q**
- **No conversation history** - each MCQ is independent
- Uses code interpreter and web search tools
- Does **not** add to conversation history

## How It Works

### Step 1: Scout Model Analysis
The Scout model (llama-4-scout-17b-16e-instruct) analyzes the screenshot and extracts:

**For MCQ with Code:**
```json
{
  "type": "MCQ_CODING",
  "question": "full question text",
  "code": "exact code snippet",
  "language": "python/cpp/java/javascript",
  "options": ["A) ...", "B) ...", "C) ...", "D) ..."],
  "needs_execution": true
}
```

**For Theory MCQ:**
```json
{
  "type": "MCQ_THEORY",
  "question": "full question text",
  "options": ["A) ...", "B) ...", "C) ...", "D) ..."],
  "topic": "algorithms/databases/networking",
  "needs_web_search": true
}
```

### Step 2: OSS-120B Model Solution
The OSS-120B model solves the MCQ using:

**For Code MCQs:**
- `code_interpreter` tool to execute the code
- Analyzes the actual output
- Matches with given options

**For Theory MCQs:**
- `browser_search` tool to verify facts
- Searches for reliable information
- Validates against multiple sources

## Response Format

The MCQ solver provides:

1. **Question Analysis** - Brief explanation of what's being asked
2. **Solution Approach** - How to solve (execute code or search web)
3. **Detailed Analysis** - Explanation of each option (A, B, C, D)
4. **Correct Answer** - Clear statement of the right answer
5. **Verification** - Code output or web search results as proof

## Usage

1. Click the **📝 MCQ** button OR press **Ctrl+Alt+Q**
2. The system captures the screen
3. Scout model extracts the MCQ details
4. OSS-120B solves it using appropriate tools
5. Answer is displayed with reasoning

## Technical Implementation

### Files Modified/Created:
- `src/modules/mcq.rs` - New MCQ handler module
- `src/modules/mcq_hotkey.rs` - New MCQ hotkey listener (Ctrl+Alt+Q)
- `src/modules/mod.rs` - Added mcq and mcq_hotkey module exports
- `src/main.rs` - Added MCQ capture handler, processing, and hotkey listener
- `static/index.html` - Added MCQ button with hotkey tooltip and JavaScript function

### Key Functions:
- `handle_mcq_capture()` - Captures screen for MCQ
- `process_mcq_screenshot()` - Processes MCQ screenshot
- `McqHandler::analyze_mcq_image()` - Scout model analysis
- `McqHandler::solve_mcq()` - OSS-120B solution with tools

## System Prompts

### Scout Model (Vision)
- Specialized for MCQ extraction
- Identifies MCQ types (coding, theory, output, debug)
- Extracts all options accurately
- Determines if code execution or web search is needed

### OSS-120B Model (Solver)
- Uses code_interpreter for code-based MCQs
- Uses browser_search for theory MCQs
- No conversation history (independent questions)
- Provides definitive answers with verification

## Benefits

1. **Specialized Handling** - MCQs get appropriate treatment
2. **Tool Usage** - Automatic code execution and web search
3. **No Context Pollution** - MCQs don't clutter conversation history
4. **Accurate Verification** - Tools provide proof of correctness
5. **Fast Processing** - No need to maintain conversation context

## Example Use Cases

### Code Output MCQ
```
Question: What will be the output of this code?
[code snippet]
A) 5
B) 10
C) 15
D) Error

→ System executes code using code_interpreter
→ Verifies actual output
→ Provides correct answer with execution proof
```

### Theory MCQ
```
Question: Which sorting algorithm has O(n log n) worst-case time complexity?
A) Quick Sort
B) Merge Sort
C) Bubble Sort
D) Selection Sort

→ System searches web using browser_search
→ Verifies facts from reliable sources
→ Provides correct answer with citations
```

## Notes

- MCQ responses are **not** added to conversation history
- Each MCQ is treated as an independent question
- The system automatically chooses the right tool (code_interpreter or browser_search)
- Low confidence captures will prompt for recapture
