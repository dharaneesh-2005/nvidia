# Visual Analysis & Debug Enhancement Plan

## Problem Statement
Current implementation lacks proper context extraction and conversation continuity between Scout (vision) and OSS-120B (reasoning) models. Need intelligent problem type detection and structured data extraction.

---

## Core Requirements

### 1. **Visual Analysis (Ctrl+Shift+A) - Enhanced Problem Detection**

#### Problem Types to Handle:
1. **DSA Coding Problems** (Most Common)
   - LeetCode/HackerRank/CodeForces style problems
   - Need: Problem statement, input format, output format, constraints, test cases
   
2. **System Design Problems**
   - Architecture diagrams, design questions
   - Need: Requirements, scale, components mentioned
   
3. **Logical/Puzzle Problems**
   - Text-based reasoning problems
   - Need: Problem statement, conditions, expected output
   
4. **General Technical Questions**
   - Theory questions, explanations
   - Need: Question text extraction

#### Two-Stage Pipeline:
```
Screenshot → Scout (Extract) → OSS-120B (Solve with tools)
```

**Stage 1: Scout Extraction (Structured)**
- Detect problem type (DSA/System Design/Logical/General)
- Extract structured data based on type
- Return JSON format for OSS-120B consumption

**Stage 2: OSS-120B Solution**
- Receive structured problem data
- Use code_interpreter for DSA problems
- Use browser_search for system design research
- Generate comprehensive solution

---

### 2. **Debug Analysis (Ctrl+Alt+D) - Enhanced Error Detection**

#### Scenarios to Handle:
1. **Code with Compilation/Runtime Error**
   - Extract: Code, error message, line number, stack trace
   
2. **Code with Wrong Output (Test Case Failure)**
   - Extract: Code, expected output, actual output, test case
   - Link to previous conversation if this is a retry
   
3. **Code with Time Limit Exceeded (TLE)**
   - Extract: Code, time limit, test case size
   - Suggest optimization based on previous solution context

#### Two-Stage Pipeline:
```
Screenshot → Scout (Extract Error) → OSS-120B (Fix with context)
```

**Stage 1: Scout Error Extraction**
- Detect error type (compilation/runtime/wrong output/TLE)
- Extract code cleanly (ignore UI elements)
- Extract error details or test case info
- Return structured JSON

**Stage 2: OSS-120B Fix**
- Receive error context + conversation history
- Check if this relates to previous coding problem
- Provide targeted fix with explanation

---

### 3. **Conversation Context Continuity**

#### Context Flow Requirements:
```
Visual Problem → OSS-120B Solution → Voice Question → 20B Answer
                     ↓                                    ↑
                     └────────── Context Shared ──────────┘
```

**Scenario Example:**
1. User captures DSA problem screenshot
2. OSS-120B generates solution with approach X
3. User asks via voice: "Why did you use dynamic programming here?"
4. 20B model needs access to:
   - Original problem statement
   - OSS-120B's solution
   - The approach used

**Implementation:**
- Store structured problem data in conversation history
- Tag messages with metadata (problem_type, solution_approach, etc.)
- 20B model receives full conversation including OSS-120B solutions
- Enable 20B to explain OSS-120B's reasoning

---

### 4. **Debug Context Linking**

**Scenario Example:**
1. User captures coding problem → OSS-120B solves it
2. User submits code → Gets TLE error
3. User captures TLE screenshot → Debug triggered
4. Scout extracts: "Previous solution, TLE on large input"
5. OSS-120B needs to know:
   - Original problem constraints
   - Previous solution approach
   - Why it's timing out

**Implementation:**
- Track "active coding problem" in conversation
- When debug triggered, check if related to recent problem
- Pass original problem + previous solution to OSS-120B
- Generate optimized solution

---

## Structured Data Formats

### DSA Problem Format (Scout → OSS-120B):
```json
{
  "type": "DSA_PROBLEM",
  "title": "Two Sum",
  "description": "Given an array of integers...",
  "input_format": "First line: n, Second line: array elements",
  "output_format": "Two space-separated indices",
  "constraints": ["1 <= n <= 10^5", "Time: O(n)"],
  "examples": [
    {"input": "4\n2 7 11 15\n9", "output": "0 1"}
  ]
}
```

### System Design Format:
```json
{
  "type": "SYSTEM_DESIGN",
  "question": "Design a URL shortener",
  "requirements": ["Handle 1M requests/day", "Low latency"],
  "components_mentioned": ["Database", "Cache", "Load Balancer"]
}
```

### Debug Error Format:
```json
{
  "type": "DEBUG_ERROR",
  "error_category": "TIME_LIMIT_EXCEEDED",
  "code": "def twoSum(nums, target):\n    for i in range(len(nums))...",
  "error_details": "TLE on test case with n=100000",
  "test_case_info": "Large input size",
  "related_to_previous": true
}
```

---

## Edge Cases to Handle

### 1. **Ambiguous Screenshots**
- Screenshot contains both code and problem statement
- Solution: Scout should extract both, mark as "problem + attempted solution"

### 2. **Multiple Problems in One Screenshot**
- User captures entire page with multiple problems
- Solution: Scout extracts first/main problem, warns about multiple

### 3. **Poor Quality Screenshots**
- Blurry, partial, or cut-off text
- Solution: Scout returns confidence score, asks for recapture if low

### 4. **Context Mismatch**
- User asks about "previous solution" but no recent coding problem
- Solution: 20B model gracefully handles missing context

### 5. **Language Detection**
- Code in different programming languages
- Solution: Scout detects language, OSS-120B adapts solution

### 6. **Incomplete Problem Statement**
- Constraints or examples missing
- Solution: OSS-120B makes reasonable assumptions, states them

### 7. **Non-Technical Screenshots**
- User accidentally captures wrong screen
- Solution: Scout detects non-technical content, returns friendly error

### 8. **Rapid Fire Debugging**
- User captures multiple debug screenshots quickly
- Solution: Queue processing, maintain order, link to correct problem

---

## Implementation Phases

### Phase 1: Enhanced Scout Prompts ✓ (Next)
- Create structured extraction prompts for each problem type
- Implement JSON response parsing
- Add problem type classification

### Phase 2: OSS-120B Context Integration ✓
- Modify solve_coding_problem to accept structured input
- Modify debug_code_error to check conversation history
- Add metadata tagging to conversation messages

### Phase 3: 20B Model Context Access ✓
- Ensure chat_with_history receives full conversation
- Add context-aware prompts for explaining previous solutions
- Test voice questions about visual solutions

### Phase 4: Error Handling & Edge Cases ✓
- Implement confidence scoring
- Add fallback mechanisms
- Handle ambiguous scenarios gracefully

---

## Success Metrics

1. **Accuracy**: Scout correctly identifies problem type 95%+ of time
2. **Completeness**: Extracts all key elements (statement, constraints, examples)
3. **Context Continuity**: 20B can answer questions about OSS-120B solutions
4. **Debug Linking**: Correctly links debug requests to previous problems
5. **User Experience**: Clear feedback on what's being processed

---

## Technical Implementation Notes

### Conversation Message Structure:
```rust
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub metadata: Option<MessageMetadata>, // NEW
}

pub struct MessageMetadata {
    pub message_type: String, // "dsa_problem", "system_design", "debug_error"
    pub problem_id: Option<String>, // Link related messages
    pub structured_data: Option<serde_json::Value>, // Store extracted JSON
}
```

### Scout Prompt Template:
```
Analyze this screenshot and extract information in JSON format.

First, identify the type:
- DSA_PROBLEM: Coding problem with input/output
- SYSTEM_DESIGN: Architecture/design question
- LOGICAL_PUZZLE: Text-based reasoning
- DEBUG_ERROR: Code with error message
- GENERAL: Other technical content

Then extract relevant fields based on type...
```

### OSS-120B Enhanced Prompt:
```
You are solving a technical interview problem.

Problem Data (structured):
{json_data}

Previous Context:
{conversation_history}

Use code_interpreter to test solutions.
Use browser_search for system design patterns.
Provide clear, interview-ready answer.
```

---

## Questions for Clarification

1. Should we store problem_id persistently across sessions?
2. Maximum conversation history to pass to models (token limits)?
3. Should Scout attempt OCR correction for poor quality images?
4. Timeout handling for long OSS-120B processing (code_interpreter)?
5. Should we support multi-language code solutions or stick to Python?

---

## Next Steps

1. Review this plan for completeness
2. Confirm edge cases are covered
3. Proceed with Phase 1 implementation
4. Test with real interview problem screenshots
