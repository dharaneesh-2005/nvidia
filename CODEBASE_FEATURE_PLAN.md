# CODEBASE INDEXING & ANALYSIS - FEATURE PLAN

## OVERVIEW
Separate standalone feature for codebase indexing, analysis, and code corrections.
Completely independent from the normal interview workflow.

---

## ARCHITECTURE

```
┌─────────────────────────────────────────────────────────┐
│                  MAIN APPLICATION                        │
│                                                          │
│  ┌──────────────────┐         ┌────────────────────┐   │
│  │  INTERVIEW MODE  │         │  CODEBASE MODE     │   │
│  │  (Current)       │         │  (New Feature)     │   │
│  │                  │         │                    │   │
│  │  - Voice Q&A     │         │  - Index Projects  │   │
│  │  - Screenshots   │         │  - Ask Questions   │   │
│  │  - Debug Code    │         │  - Code Analysis   │   │
│  │  - Manual Input  │         │  - Fix Errors      │   │
│  └──────────────────┘         └────────────────────┘   │
│                                                          │
│  Toggle: Ctrl+Shift+M (Mode Switch)                     │
└─────────────────────────────────────────────────────────┘
```

---

## USER FLOW

### 1. MODE SWITCHING
- **Hotkey**: `Ctrl+Shift+M` toggles between Interview Mode and Codebase Mode
- **Visual Indicator**: Different background color/header for each mode
- **State Isolation**: Each mode has separate conversation history

### 2. CODEBASE MODE FEATURES

#### A. PROJECT INDEXING
```
User Action: Click "Index Project" button
↓
Select project directory (file picker)
↓
Background indexing starts
↓
Progress indicator shows: "Indexing... 45/120 files"
↓
Completion: "✓ Indexed 120 files (2.3MB)"
```

**What gets indexed:**
- All code files (.js, .ts, .py, .java, .rs, .cpp, etc.)
- File structure and relationships
- Function/class signatures
- Import/export statements
- Comments and documentation

**Storage:**
- SQLite database: `codebase_index.db`
- Stores: file paths, content, metadata, embeddings (optional)

#### B. QUESTION ANSWERING
```
User types: "How does authentication work?"
↓
System searches indexed codebase
↓
Finds relevant files (auth.ts, middleware.ts)
↓
Sends to AI with context
↓
AI responds with file references and line numbers
```

**Response Format:**
```
Authentication is handled in:

📄 server/auth.ts (Lines 15-45)
   15: export function generateToken(user: User) {
   16:   return jwt.sign({ id: user.id }, SECRET);
   17: }
   ...

📄 server/middleware.ts (Lines 8-20)
   8: export function authMiddleware(req, res, next) {
   9:   const token = req.headers.authorization;
   ...
```

#### C. CODE ANALYSIS
```
User: "Review the database connection code"
↓
System finds db.ts
↓
AI analyzes for:
  - Security issues
  - Performance problems
  - Best practices
  - Potential bugs
↓
Returns detailed analysis with suggestions
```

#### D. ERROR FIXING
```
User: "Fix the error in payment.ts line 45"
↓
System loads payment.ts with context
↓
AI identifies issue
↓
Provides:
  - Error explanation
  - Fixed code
  - Line-by-line diff
```

---

## TECHNICAL IMPLEMENTATION

### 1. DATABASE SCHEMA
```sql
CREATE TABLE indexed_files (
    id INTEGER PRIMARY KEY,
    project_path TEXT,
    file_path TEXT,
    content TEXT,
    language TEXT,
    size INTEGER,
    last_modified INTEGER,
    indexed_at INTEGER
);

CREATE TABLE file_functions (
    id INTEGER PRIMARY KEY,
    file_id INTEGER,
    name TEXT,
    start_line INTEGER,
    end_line INTEGER,
    signature TEXT
);

CREATE INDEX idx_file_path ON indexed_files(file_path);
CREATE INDEX idx_content_search ON indexed_files(content);
```

### 2. RUST MODULES

**New files to create:**
```
src/modules/
  ├── indexer.rs       # File scanning and indexing
  ├── codebase_db.rs   # SQLite database operations
  ├── code_search.rs   # Search and retrieval
  └── code_ai.rs       # AI interactions for code
```

**Key functions:**
```rust
// indexer.rs
pub fn index_project(path: &str) -> Result<IndexStats, Error>
pub fn update_index(path: &str) -> Result<(), Error>

// codebase_db.rs
pub fn store_file(file: &CodeFile) -> Result<(), Error>
pub fn search_files(query: &str) -> Vec<CodeFile>
pub fn get_file_content(path: &str) -> Option<String>

// code_search.rs
pub fn find_relevant_files(question: &str) -> Vec<FileMatch>
pub fn get_context_for_question(question: &str) -> String

// code_ai.rs
pub async fn answer_code_question(question: &str, context: &str) -> Result<String, Error>
pub async fn analyze_code(file_path: &str) -> Result<Analysis, Error>
pub async fn fix_code_error(file_path: &str, line: usize) -> Result<Fix, Error>
```

### 3. UI CHANGES

**Add to index.html:**
```html
<!-- Mode Toggle -->
<div class="mode-selector">
  <button id="interview-mode" class="active">💬 Interview</button>
  <button id="codebase-mode">📁 Codebase</button>
</div>

<!-- Codebase Mode UI (hidden by default) -->
<div id="codebase-panel" style="display: none;">
  <div class="index-controls">
    <button onclick="selectProject()">📂 Select Project</button>
    <button onclick="indexProject()">🔄 Index</button>
    <span id="index-status"></span>
  </div>
  
  <div class="indexed-files">
    <h3>Indexed Files (0)</h3>
    <ul id="file-list"></ul>
  </div>
  
  <div class="code-chat">
    <!-- Q&A interface for code questions -->
  </div>
</div>
```

### 4. API ENDPOINTS

**New routes:**
```rust
.route("/api/codebase/index", post(index_project))
.route("/api/codebase/files", get(get_indexed_files))
.route("/api/codebase/search", post(search_code))
.route("/api/codebase/ask", post(ask_code_question))
.route("/api/codebase/analyze", post(analyze_file))
.route("/api/codebase/fix", post(fix_error))
```

---

## ADVANTAGES OF THIS APPROACH

✅ **Clean Separation**: Interview mode stays simple and fast
✅ **No Interference**: Codebase mode doesn't affect normal workflow
✅ **Persistent Index**: Index once, query many times (fast)
✅ **Offline Capable**: Works without re-scanning files
✅ **Scalable**: Can handle large codebases efficiently
✅ **Focused**: Each mode has clear, specific purpose

---

## IMPLEMENTATION PHASES

### Phase 1: Basic Indexing (Week 1)
- [ ] Create database schema
- [ ] Implement file scanner
- [ ] Store files in SQLite
- [ ] Basic UI for project selection

### Phase 2: Search & Retrieval (Week 1)
- [ ] Implement keyword search
- [ ] Context extraction
- [ ] File relevance ranking

### Phase 3: AI Integration (Week 2)
- [ ] Connect to Groq API
- [ ] Question answering with context
- [ ] Code analysis
- [ ] Error fixing

### Phase 4: UI Polish (Week 2)
- [ ] Mode switching
- [ ] Progress indicators
- [ ] File browser
- [ ] Syntax highlighting

---

## NEXT STEPS

1. **Confirm this approach** - Does this match your vision?
2. **Start with Phase 1** - Basic indexing infrastructure
3. **Iterate quickly** - Get feedback at each phase

Ready to proceed?
