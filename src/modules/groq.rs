use reqwest::Client;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

/// A named Gemini API key
#[derive(Clone, Debug)]
pub struct NamedKey {
    pub name: String,
    pub key: String,
}

// Gemini model
const GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

// Groq fallback model
const GROQ_FALLBACK_MODEL: &str = "openai/gpt-oss-120b";
const GROQ_BASE_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

pub struct GroqClient {
    client: Client,
    groq_key: String,           // Groq key for fallback + Whisper
    gemini_keys: Vec<NamedKey>, // Rotating Gemini keys (primary)
    gemini_counter: Arc<AtomicUsize>,
    user_profile: String,
    tx: Option<broadcast::Sender<String>>,
}

impl GroqClient {
    pub fn new(groq_key: String) -> Self {
        Self::new_with_gemini(groq_key, Vec::new())
    }

    pub fn new_with_gemini(groq_key: String, gemini_keys: Vec<NamedKey>) -> Self {
        let user_profile = Self::load_profile();
        let keys: Vec<NamedKey> = gemini_keys.into_iter().filter(|k| !k.key.is_empty()).collect();
        if !keys.is_empty() {
            let names: Vec<&str> = keys.iter().map(|k| k.name.as_str()).collect();
            eprintln!("[Gemini] {} key(s) loaded: {:?}, rotating every 2 requests", keys.len(), names);
        } else {
            eprintln!("[Gemini] No keys — Groq-only mode");
        }
        Self {
            client: Client::new(),
            groq_key,
            gemini_keys: keys,
            gemini_counter: Arc::new(AtomicUsize::new(0)),
            user_profile,
            tx: None,
        }
    }

    pub fn set_broadcast(&mut self, tx: broadcast::Sender<String>) {
        self.tx = Some(tx);
    }

    // ── Key rotation (same logic as old Cerebras) ─────────────────────────────

    fn get_gemini_key(&self) -> Option<(String, String)> {
        if self.gemini_keys.is_empty() { return None; }
        let count = self.gemini_counter.fetch_add(1, Ordering::SeqCst);
        let index = (count / 2) % self.gemini_keys.len();
        let named = &self.gemini_keys[index];
        if let Some(ref tx) = self.tx {
            let _ = tx.send(json!({
                "type": "active_api_key",
                "name": named.name
            }).to_string());
        }
        Some((named.name.clone(), named.key.clone()))
    }

    fn get_gemini_key_at(&self, index: usize) -> Option<(String, String)> {
        if self.gemini_keys.is_empty() { return None; }
        let idx = index % self.gemini_keys.len();
        let named = &self.gemini_keys[idx];
        if let Some(ref tx) = self.tx {
            let _ = tx.send(json!({
                "type": "active_api_key",
                "name": named.name
            }).to_string());
        }
        Some((named.name.clone(), named.key.clone()))
    }

    fn current_gemini_key_index(&self) -> usize {
        let count = self.gemini_counter.load(Ordering::SeqCst);
        if self.gemini_keys.is_empty() { 0 } else { (count / 2) % self.gemini_keys.len() }
    }

    // ── Profile loading ────────────────────────────────────────────────────────

    fn load_profile() -> String {
        let profile_paths = [
            "profile.json",
            "interview-helper/profile.json",
            "../profile.json",
            "./interview-helper/profile.json",
            "e:/project/newphonewrtc/interview-helper/profile.json",
            "e:\\project\\newphonewrtc\\interview-helper\\profile.json",
        ];
        let content = profile_paths.iter().find_map(|path| {
            if let Ok(c) = fs::read_to_string(path) {
                eprintln!("✅ Profile loaded from: {}", path);
                Some(c)
            } else { None }
        });
        match content {
            Some(c) => { eprintln!("📋 Profile length: {} chars", c.len()); c }
            None => {
                eprintln!("⚠️  profile.json not found — profile features disabled.");
                String::new()
            }
        }
    }

    // ── Helpers ────────────────────────────────────────────────────────────────

    /// Parse Gemini response and collect all text parts
    fn parse_gemini_response(json: &serde_json::Value) -> String {
        let mut result = String::new();
        if let Some(candidates) = json["candidates"].as_array() {
            if let Some(first) = candidates.first() {
                if let Some(parts) = first["content"]["parts"].as_array() {
                    for part in parts {
                        if let Some(text) = part["text"].as_str() {
                            result.push_str(text);
                        }
                        // Code execution output
                        if let Some(code_result) = part["codeExecutionResult"].as_object() {
                            if let Some(output) = code_result.get("output").and_then(|o| o.as_str()) {
                                if !output.is_empty() {
                                    result.push_str(&format!("\n\n**Code Execution Output:**\n```\n{}\n```\n", output));
                                }
                            }
                        }
                        // Executable code shown (only when result is still empty to avoid duplication)
                        if let Some(exec_code) = part["executableCode"].as_object() {
                            if let Some(code) = exec_code.get("code").and_then(|c| c.as_str()) {
                                if !code.is_empty() && result.is_empty() {
                                    result.push_str(&format!("```python\n{}\n```\n", code));
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }

    /// Convert ConversationMessage history to Gemini `contents` format
    fn build_gemini_contents(
        history: &[ConversationMessage],
        current_text: &str,
        images: Option<&[String]>,
    ) -> Vec<serde_json::Value> {
        let mut contents: Vec<serde_json::Value> = Vec::new();

        // Add prior history (map user/assistant → user/model)
        let recent: Vec<_> = history.iter().rev().take(20).rev().collect();
        for msg in recent {
            let role = if msg.role == "assistant" { "model" } else { "user" };
            contents.push(json!({
                "role": role,
                "parts": [{ "text": msg.content }]
            }));
        }

        // Build current user turn with optional images
        let mut current_parts: Vec<serde_json::Value> = Vec::new();
        if let Some(imgs) = images {
            for img_b64 in imgs {
                current_parts.push(json!({
                    "inlineData": {
                        "mimeType": "image/png",
                        "data": img_b64
                    }
                }));
            }
        }
        current_parts.push(json!({ "text": current_text }));
        contents.push(json!({
            "role": "user",
            "parts": current_parts
        }));

        contents
    }

    /// Build system prompt for interview Q&A
    fn build_interview_system_prompt(&self, candidate_context: Option<&[String]>) -> String {
        let candidate_section = if let Some(ctx) = candidate_context {
            if !ctx.is_empty() {
                let recent: Vec<_> = ctx.iter().rev().take(5).collect();
                format!(
                    "\n\nCANDIDATE'S ACTUAL ANSWERS IN THIS INTERVIEW:\n{}\n",
                    recent.iter().rev().enumerate()
                        .map(|(i, a)| format!("{}. {}", i + 1, a))
                        .collect::<Vec<_>>().join("\n")
                )
            } else { String::new() }
        } else { String::new() };

        if !self.user_profile.is_empty() {
            format!(
                "You are answering interview questions for Dharaneesh (final-year CS student). \
                He reads your answer DIRECTLY from screen while speaking. Make it EASY TO READ ALOUD.\n\n\
                PROFILE:\n{}\
                {}\
                === CRITICAL: USE CANDIDATE'S ACTUAL RESPONSES ===\n\
                The 'CANDIDATE'S ACTUAL ANSWERS' section above contains what Dharaneesh ACTUALLY SAID.\n\
                - If interviewer asks about something Dharaneesh already mentioned, REFERENCE IT\n\
                - NEVER contradict what the candidate already said\n\
                - NEVER say 'I don't have' if candidate mentioned they DO have it\n\n\
                === STRICT RULES ===\n\n\
                1. NEVER give code unless interviewer EXPLICITLY asks to write code or solve a coding problem.\n\
                   - 'Explain event-driven architecture' → NO code, just explanation\n\
                   - 'Write a program to sort a stack' → YES, give code\n\
                   - 'How does sorting work?' → NO code, explain the concept\n\n\
                2. FORMAT for easy reading:\n\
                   - First line: **one sentence direct answer** (bold)\n\
                   - Then 3-5 short lines explaining it\n\
                   - Each line = one idea, max 15 words\n\
                   - Use → arrows or dashes to separate points\n\
                   - Leave blank lines between points for breathing room\n\n\
                3. LANGUAGE:\n\
                   - Speak like a student, not a textbook\n\
                   - NO words like: 'Furthermore', 'paradigm', 'encompasses', 'facilitates'\n\
                   - YES words like: 'basically', 'so what happens is', 'the main idea is'\n\
                   - Every line must be readable in one breath\n\n\
                4. ONLY mention your projects/profile if interviewer asks about YOUR experience.",
                self.user_profile, candidate_section
            )
        } else {
            format!(
                "You answer interview questions for a CS student who reads your answer DIRECTLY from screen.\
                {}\
                RULES:\n\
                1. NEVER give code unless explicitly asked to write code\n\
                2. First line: **one sentence direct answer** (bold)\n\
                3. Then 3-5 short lines, one idea each, max 15 words per line\n\
                4. Leave blank lines between points\n\
                5. Simple words only — speakable in one breath per line\n\
                6. No projects/profile unless asked about YOUR experience",
                candidate_section
            )
        }
    }

    /// Build system prompt for DSA/coding problems
    fn build_coding_system_prompt(candidate_section: &str) -> String {
        format!(
            "You are a technical interview expert specializing in DSA problems with code execution capability.\n\
            {}\n\
            **OUTPUT FORMAT FOR DSA PROBLEMS:**\n\n\
            ## PROBLEM UNDERSTANDING\n\
            [Explain what the problem is asking in simple words - 2-3 sentences]\n\n\
            ## APPROACH 1: BRUTE FORCE\n\n\
            **Intuition:**\n\
            [Explain the straightforward approach - 3-4 sentences conversationally]\n\n\
            **How it works:**\n\
            - Step 1: [Explain first step]\n\
            - Step 2: [Explain second step]\n\n\
            **Code:**\n\
            ```cpp\n\
            #include <bits/stdc++.h>\n\
            using namespace std;\n\
            // Brute force implementation\n\
            ```\n\n\
            **Complexity Analysis:**\n\
            - Time Complexity: O(n²) where n = size of input array\n\
            - Space Complexity: O(1)\n\n\
            ---\n\n\
            ## APPROACH 2: OPTIMAL SOLUTION\n\n\
            **Intuition:**\n\
            [Explain the optimized approach - what's the key insight? 3-4 sentences]\n\n\
            **How it works:**\n\
            - Step 1: [Explain step 1]\n\
            - Step 2: [Explain step 2]\n\n\
            **Code:**\n\
            ```cpp\n\
            #include <bits/stdc++.h>\n\
            using namespace std;\n\
            // Optimal implementation\n\
            ```\n\n\
            **Complexity Analysis:**\n\
            - Time Complexity: O(n) where n = size of input\n\
            - Space Complexity: O(n)\n\n\
            ---\n\n\
            ## COMPARISON\n\
            [Compare both — which is better and when to use which? 2-3 sentences]\n\n\
            **CRITICAL RULES:**\n\
            1. Use C++ ONLY with #include <bits/stdc++.h> and using namespace std;\n\
            2. Do NOT use ios::sync_with_stdio(false), cin.tie(NULL), or fast I/O\n\
            3. If predefined structure exists (class Solution), use it EXACTLY\n\
            4. If NO predefined structure: int main() {{ int t; cin >> t; while(t--) {{ }} return 0; }}\n\
            5. NEVER mix class-based and main()-based approaches\n\
            6. ALWAYS explain what each variable in O() notation means\n\
            7. If candidate mentioned an approach, PRIORITIZE it\n\
            8. Use code execution to verify your solution against the examples",
            candidate_section
        )
    }

    // ── Gemini REST call (primary) ─────────────────────────────────────────────

    /// Make a Gemini API call with retry + key rotation on 429
    async fn call_gemini(
        &self,
        contents: Vec<serde_json::Value>,
        system_instruction: &str,
        use_code_execution: bool,
        temperature: f64,
    ) -> Result<String, String> {
        let initial_key = self.get_gemini_key();
        if initial_key.is_none() {
            return Err("No Gemini keys configured".to_string());
        }
        let (_name, initial_api_key) = initial_key.unwrap();

        let mut tools: Vec<serde_json::Value> = Vec::new();
        if use_code_execution {
            tools.push(json!({ "codeExecution": {} }));
        }

        let mut payload = json!({
            "contents": contents,
            "systemInstruction": {
                "parts": [{ "text": system_instruction }]
            },
            "generationConfig": {
                "temperature": temperature,
                "maxOutputTokens": 1536
            }
        });

        if !tools.is_empty() {
            payload["tools"] = json!(tools);
        }

        let max_retries = self.gemini_keys.len() + 1;
        let mut current_key = initial_api_key;
        let mut retry_index = self.current_gemini_key_index();

        for attempt in 0..max_retries {
            let url = format!(
                "{}/{}:generateContent?key={}",
                GEMINI_BASE_URL, GEMINI_MODEL, current_key
            );

            let response = match self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    if attempt < max_retries - 1 {
                        retry_index += 1;
                        if let Some((_n, k)) = self.get_gemini_key_at(retry_index) {
                            current_key = k;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                    return Err(format!("Gemini request failed: {}", e));
                }
            };

            let status = response.status();

            if status.is_success() {
                let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;

                // Check for API-level error in body
                if let Some(err) = json["error"].as_object() {
                    let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown");
                    return Err(format!("Gemini API error: {}", msg));
                }

                let result = Self::parse_gemini_response(&json);
                if !result.is_empty() {
                    return Ok(result);
                }

                return Err(format!("Gemini empty response. JSON: {}", &json.to_string()[..json.to_string().len().min(300)]));

            } else if status.as_u16() == 429 && attempt < max_retries - 1 {
                // Rate limited — rotate to next key
                retry_index += 1;
                eprintln!("[Gemini] 429 on attempt {}, rotating key...", attempt + 1);
                if let Some((_n, k)) = self.get_gemini_key_at(retry_index) {
                    current_key = k;
                }
                // No sleep on 429 — rotate immediately
                continue;
            } else {
                let body = response.text().await.unwrap_or_default();
                if attempt < max_retries - 1 {
                    eprintln!("[Gemini] Error {}, retrying... body: {}", status, &body[..body.len().min(200)]);
                    retry_index += 1;
                    if let Some((_n, k)) = self.get_gemini_key_at(retry_index) {
                        current_key = k;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                return Err(format!("Gemini API error {}: {}", status, &body[..body.len().min(300)]));
            }
        }

        Err("All Gemini keys exhausted".to_string())
    }

    /// Groq fallback call (text-only, OpenAI-compatible)
    async fn call_groq_fallback(
        &self,
        messages: Vec<serde_json::Value>,
        temperature: f64,
        max_tokens: u32,
    ) -> Result<String, String> {
        eprintln!("[Fallback] Gemini failed → trying Groq OSS-120B...");
        if let Some(ref tx) = self.tx {
            let _ = tx.send(json!({ "type": "active_api_key", "name": "Groq Fallback" }).to_string());
        }

        let payload = json!({
            "model": GROQ_FALLBACK_MODEL,
            "messages": messages,
            "temperature": temperature,
            "max_completion_tokens": max_tokens
        });

        let mut retries = 0;
        loop {
            match tokio::time::timeout(
                Duration::from_secs(30),
                self.client
                    .post(GROQ_BASE_URL)
                    .header("Authorization", format!("Bearer {}", self.groq_key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
            ).await {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        return Ok(json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string());
                    } else if response.status().is_server_error() && retries < 2 {
                        retries += 1;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        return Err(format!("Groq fallback error: {}", &body[..body.len().min(200)]));
                    }
                }
                Ok(Err(_e)) if retries < 2 => { retries += 1; tokio::time::sleep(Duration::from_secs(1)).await; continue; }
                Ok(Err(e)) => return Err(format!("Groq fallback request failed: {}", e)),
                Err(_) if retries < 2 => { retries += 1; tokio::time::sleep(Duration::from_secs(1)).await; continue; }
                Err(_) => return Err("Groq fallback timed out".to_string()),
            }
        }
    }

    // ── Public API ─────────────────────────────────────────────────────────────

    pub async fn transcribe(&self, audio_data: &[u8]) -> Result<String, String> {
        self.transcribe_with_options(audio_data, "whisper-large-v3", None).await
    }

    pub async fn transcribe_with_options(&self, audio_data: &[u8], model: &str, prompt: Option<&str>) -> Result<String, String> {
        let max_retries = 3;
        let mut retry_count = 0;
        let mut last_error = String::new();

        while retry_count < max_retries {
            let mut form = reqwest::multipart::Form::new()
                .part("file", reqwest::multipart::Part::bytes(audio_data.to_vec())
                    .file_name("audio.wav")
                    .mime_str("audio/wav").map_err(|e| e.to_string())?)
                .text("model", model.to_string())
                .text("language", "en")
                .text("temperature", "0.0")
                .text("response_format", "text");

            if let Some(p) = prompt {
                if !p.is_empty() {
                    form = form.text("prompt", p.to_string());
                }
            } else {
                form = form.text("prompt", "Interview conversation in Indian English accent.");
            }

            let result = self.client
                .post("https://api.groq.com/openai/v1/audio/transcriptions")
                .header("Authorization", format!("Bearer {}", self.groq_key))
                .multipart(form)
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        return response.text().await.map_err(|e| e.to_string());
                    } else {
                        let status = response.status();
                        let text = response.text().await.unwrap_or_default();
                        last_error = format!("Status: {}, Body: {}", status, text);
                        if status.is_server_error() {
                            retry_count += 1;
                            tokio::time::sleep(Duration::from_millis(500 * retry_count as u64)).await;
                            continue;
                        } else {
                            return Err(last_error);
                        }
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    retry_count += 1;
                    tokio::time::sleep(Duration::from_millis(500 * retry_count as u64)).await;
                }
            }
        }
        Err(format!("Transcription failed after {} retries: {}", max_retries, last_error))
    }

    /// Chat: Gemini primary → Groq fallback. Maintains full conversation context.
    pub async fn chat_with_history(&self, message: &str, history: &[ConversationMessage]) -> Result<String, String> {
        self.chat_with_context(message, history, &[]).await
    }

    pub async fn chat(&self, message: &str) -> Result<String, String> {
        self.chat_with_history(message, &[]).await
    }

    /// Full context chat: Gemini primary → Groq fallback.
    pub async fn chat_with_context(&self, message: &str, history: &[ConversationMessage], candidate_context: &[String]) -> Result<String, String> {
        let system_prompt = self.build_interview_system_prompt(Some(candidate_context));
        let contents = Self::build_gemini_contents(history, message, None);

        // Try Gemini
        match self.call_gemini(contents, &system_prompt, false, 0.5).await {
            Ok(answer) => return Ok(answer),
            Err(e) => eprintln!("[Gemini] chat_with_context failed: {}", e),
        }

        // Fallback: Groq OSS-120B
        let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
        let recent: Vec<_> = history.iter().rev().take(10).rev().collect();
        for msg in recent {
            messages.push(json!({ "role": msg.role, "content": msg.content }));
        }
        messages.push(json!({ "role": "user", "content": message }));
        self.call_groq_fallback(messages, 0.5, 8192).await
    }

    /// Solve coding problem: Gemini primary (with code execution) → Groq fallback
    pub async fn solve_coding_problem(&self, problem: &str, history: &[ConversationMessage], candidate_context: &[String]) -> Result<String, String> {
        let candidate_section = if !candidate_context.is_empty() {
            let recent: Vec<_> = candidate_context.iter().rev().take(5).collect();
            format!(
                "\nCANDIDATE HAS MENTIONED (use their stated approach if relevant):\n{}\n",
                recent.iter().rev().enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>().join("\n")
            )
        } else { String::new() };

        let system_prompt = Self::build_coding_system_prompt(&candidate_section);
        let contents = Self::build_gemini_contents(history, problem, None);

        // Try Gemini with code execution
        match self.call_gemini(contents, &system_prompt, true, 0.4).await {
            Ok(solution) => return Ok(solution),
            Err(e) => eprintln!("[Gemini] solve_coding_problem failed: {}", e),
        }

        // Fallback: Groq OSS-120B
        let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
        let recent: Vec<_> = history.iter().rev().take(10).rev().collect();
        for msg in recent {
            messages.push(json!({ "role": msg.role, "content": msg.content }));
        }
        messages.push(json!({ "role": "user", "content": problem }));
        self.call_groq_fallback(messages, 0.4, 8192).await
    }

    /// Solve directly from screenshot(s): Gemini vision → Groq text fallback
    /// This replaces the old 2-step Scout→Cerebras pipeline entirely.
    pub async fn solve_with_gemini_vision(
        &self,
        images: &[String],
        history: &[ConversationMessage],
        candidate_context: &[String],
    ) -> Result<String, String> {
        let candidate_section = if !candidate_context.is_empty() {
            let recent: Vec<_> = candidate_context.iter().rev().take(5).collect();
            format!(
                "\nCANDIDATE HAS MENTIONED:\n{}\n",
                recent.iter().rev().enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>().join("\n")
            )
        } else { String::new() };

        let prompt_text = format!(
            "Analyze the screenshot(s) carefully and solve the problem shown.\n\
            {}\n\
            INSTRUCTIONS:\n\
            1. First identify what type of problem this is (DSA coding, system design, MCQ, debug, or general question)\n\
            2. For DSA CODING problems:\n\
               - Give BRUTE FORCE approach + code\n\
               - Give OPTIMAL approach + code\n\
               - Use code execution to verify against the examples\n\
               - Follow the EXACT output format below\n\
            3. For SYSTEM DESIGN: explain architecture clearly\n\
            4. For GENERAL/CONCEPTUAL: answer directly in speakable format\n\
            5. For DEBUG: identify bug, fix with minimal changes\n\n\
            OUTPUT FORMAT FOR DSA PROBLEMS:\n\n\
            ## PROBLEM UNDERSTANDING\n\
            [2-3 sentences]\n\n\
            ## APPROACH 1: BRUTE FORCE\n\
            **Intuition:** [3-4 sentences]\n\
            **How it works:**\n- Step 1...\n\
            **Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n// implementation\n```\n\
            **Complexity:** Time O(...) where ..., Space O(...)\n\n\
            ---\n\n\
            ## APPROACH 2: OPTIMAL\n\
            **Intuition:** [3-4 sentences]\n\
            **How it works:**\n- Step 1...\n\
            **Code:**\n```cpp\n// optimal implementation\n```\n\
            **Complexity:** Time O(...) where ..., Space O(...)\n\n\
            ---\n\n\
            ## COMPARISON\n\
            [2-3 sentences comparing both approaches]\n\n\
            CRITICAL RULES:\n\
            - C++ only, #include <bits/stdc++.h>, using namespace std;\n\
            - NO ios::sync_with_stdio or cin.tie\n\
            - If predefined structure (class Solution) exists in screenshot, use it EXACTLY\n\
            - If no predefined structure: int main() {{ int t; cin >> t; while(t--) {{ }} return 0; }}\n\
            - NEVER mix class-based and main()-based\n\
            - Verify with code execution on the examples from the screenshot",
            candidate_section
        );

        let contents = Self::build_gemini_contents(history, &prompt_text, Some(images));

        let system_prompt = format!(
            "You are a world-class competitive programming and technical interview expert with code execution capability.{}\n\
            Always verify DSA solutions against examples using code execution before presenting them.",
            candidate_section
        );

        // Try Gemini vision
        match self.call_gemini(contents, &system_prompt, true, 0.4).await {
            Ok(solution) => return Ok(solution),
            Err(e) => eprintln!("[Gemini Vision] failed: {} — falling back to text", e),
        }

        // Fallback: Groq text-only (can't send images, ask user to describe)
        let fallback_msg = format!(
            "I captured a screenshot but Gemini is temporarily unavailable. \
            Based on the conversation context, please describe what you see and I'll help solve it.\n\n\
            Context: {}\n\
            Candidate info: {}",
            if history.is_empty() { "No prior context".to_string() }
            else { history.last().map(|m| m.content.chars().take(300).collect::<String>()).unwrap_or_default() },
            candidate_section
        );

        let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
        let recent: Vec<_> = history.iter().rev().take(6).rev().collect();
        for msg in recent {
            messages.push(json!({ "role": msg.role, "content": msg.content }));
        }
        messages.push(json!({ "role": "user", "content": fallback_msg }));
        self.call_groq_fallback(messages, 0.4, 8192).await
    }

    /// Debug code error from screenshot: Gemini vision (direct) → Groq fallback
    pub async fn debug_with_gemini(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<String, String> {
        // Find previous code from history for context
        let last_code = history.iter().rev()
            .find(|m| m.role == "assistant" && (m.content.contains("```") || m.content.contains("#include")))
            .map(|m| format!("\n\nPREVIOUSLY GENERATED CODE (make MINIMAL changes to THIS):\n{}\n", m.content))
            .unwrap_or_default();

        let prompt_text = format!(
            "Look at this screenshot carefully. It shows code with an error/bug.\n\
            {}\n\
            INSTRUCTIONS:\n\
            1. Identify the EXACT bug from the screenshot\n\
            2. Fix with MINIMAL changes only — do NOT rewrite the entire code\n\
            3. Mark every change with: // FIX: [what changed]\n\
            4. Use code execution to VERIFY the fix works\n\n\
            FORMAT:\n\
            ## BUG\n\
            [1-2 sentences: what exactly is wrong]\n\n\
            ## FIX\n\
            ```cpp\n\
            // Full code with ONLY the bug fixed\n\
            // Mark changes with: // FIX: [explanation]\n\
            ```\n\n\
            ## WHAT CHANGED\n\
            - [old code] → [new code] (reason)\n\n\
            RULES:\n\
            - Do NOT change the algorithm\n\
            - Do NOT rewrite the solution\n\
            - ONLY fix the specific error shown\n\
            - Keep the same variable names, structure, approach",
            last_code
        );

        let system_prompt = "You are a C++ debugging expert with code execution capability. \
            Your job is to identify bugs from screenshots and fix them with minimal changes. \
            Always verify your fix works using code execution.";

        let contents = Self::build_gemini_contents(history, &prompt_text, Some(&[image_base64.to_string()]));

        // Try Gemini vision
        match self.call_gemini(contents, system_prompt, true, 0.3).await {
            Ok(fix) => return Ok(fix),
            Err(e) => eprintln!("[Gemini Debug] vision failed: {}", e),
        }

        // Fallback: Groq text-only debug
        let fallback_prompt = format!(
            "Debug error analysis from screenshot (Gemini unavailable):\n\
            Based on the prior conversation context, help identify and fix the code error shown.\n\
            {}",
            last_code
        );
        let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
        let recent: Vec<_> = history.iter().rev().take(10).rev().collect();
        for msg in recent {
            messages.push(json!({ "role": msg.role, "content": msg.content }));
        }
        messages.push(json!({ "role": "user", "content": fallback_prompt }));
        self.call_groq_fallback(messages, 0.3, 4000).await
    }

    /// MCQ: Gemini vision (direct solve from screenshot) → Groq fallback
    pub async fn solve_mcq_with_gemini_vision(&self, image_base64: &str) -> Result<String, String> {
        let prompt_text = "Look at this MCQ (Multiple Choice Question) screenshot carefully.\n\n\
            INSTRUCTIONS:\n\
            1. Read the question and ALL options exactly as shown\n\
            2. For code-based MCQs: use code execution to determine the exact output\n\
            3. For theory MCQs: reason through each option carefully\n\
            4. Identify the correct answer definitively\n\n\
            FORMAT:\n\
            ## QUESTION ANALYSIS\n\
            [2-3 sentences: what is this question asking?]\n\n\
            ## ANALYSIS\n\
            **Option A:** [Why correct/incorrect]\n\
            **Option B:** [Why correct/incorrect]\n\
            **Option C:** [Why correct/incorrect]\n\
            **Option D:** [Why correct/incorrect]\n\n\
            ## CORRECT ANSWER\n\
            **Answer: [Letter]**\n\n\
            **Reasoning:**\n\
            [3-4 sentences explaining why this is correct]\n\n\
            **Verification:**\n\
            [If code was executed, show the output. If theory, cite the key concept]\n\n\
            CRITICAL: Be definitive. State exactly which option is correct and why the others are wrong.";

        let system_prompt = "You are an expert MCQ solver with code execution capability. \
            For code-based questions, always execute the code to get the actual output. \
            Be definitive and accurate in your answers.";

        let contents = Self::build_gemini_contents(&[], prompt_text, Some(&[image_base64.to_string()]));

        // Try Gemini vision
        match self.call_gemini(contents, system_prompt, true, 0.3).await {
            Ok(solution) => return Ok(solution),
            Err(e) => eprintln!("[Gemini MCQ] vision failed: {}", e),
        }

        // Fallback: Groq text
        let messages = vec![
            json!({ "role": "system", "content": system_prompt }),
            json!({ "role": "user", "content": "MCQ screenshot analysis failed — Gemini unavailable. Please help solve the MCQ based on any prior context." })
        ];
        self.call_groq_fallback(messages, 0.3, 3000).await
    }

    pub async fn analyze_codebase_with_question(&self, question: &str, codebase_files: Vec<(String, String)>, history: &[ConversationMessage]) -> Result<String, String> {
        let mut codebase_context = String::from("\n\nCODEBASE CONTEXT:\n\n");
        for (file_path, content) in codebase_files.iter() {
            codebase_context.push_str(&format!("=== {} ===\n{}\n\n", file_path, content));
        }

        let system_prompt = if !self.user_profile.is_empty() {
            format!("You are a technical interview assistant.\n\nCANDIDATE PROFILE:\n{}\n\n{}\n\nIMPORTANT: For technical questions, focus on the code. Only reference candidate profile if asked about their experience.",
                self.user_profile, codebase_context)
        } else {
            format!("You are a technical interview assistant analyzing a codebase. {}\n\nProvide precise answers for bug fixes, integration tasks, or code understanding questions.", codebase_context)
        };

        let contents = Self::build_gemini_contents(history, question, None);

        match self.call_gemini(contents, &system_prompt, false, 0.4).await {
            Ok(answer) => return Ok(answer),
            Err(e) => eprintln!("[Gemini] analyze_codebase failed: {}", e),
        }

        let mut messages = vec![json!({ "role": "system", "content": system_prompt })];
        for msg in history {
            messages.push(json!({ "role": msg.role, "content": msg.content }));
        }
        messages.push(json!({ "role": "user", "content": question }));
        self.call_groq_fallback(messages, 0.4, 4000).await
    }
}

pub fn read_codebase_files(repo_path: &str) -> Vec<(String, String)> {
    use std::path::Path;
    use walkdir::WalkDir;

    let mut files = Vec::new();
    let path = Path::new(repo_path);

    if !path.exists() { return files; }

    let code_extensions = [
        "rs", "py", "js", "ts", "java", "cpp", "c", "h", "hpp",
        "go", "rb", "php", "cs", "swift", "kt", "scala", "sql",
    ];

    for entry in WalkDir::new(path).max_depth(5).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if code_extensions.contains(&ext.to_str().unwrap_or("")) {
                    if let Ok(content) = fs::read_to_string(path) {
                        if content.len() < 50000 {
                            let relative_path = path.strip_prefix(repo_path)
                                .unwrap_or(path)
                                .to_string_lossy()
                                .to_string();
                            files.push((relative_path, content));
                            if files.len() >= 20 { break; }
                        }
                    }
                }
            }
        }
    }
    files
}