use reqwest::Client;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

/// A named Cerebras API key
#[derive(Clone, Debug)]
pub struct NamedKey {
    pub name: String,
    pub key: String,
}

pub struct GroqClient {
    client: Client,
    api_key: String,
    cerebras_keys: Vec<NamedKey>,
    cerebras_counter: Arc<AtomicUsize>,
    user_profile: String,
    // Broadcast channel to notify UI which key is active
    tx: Option<broadcast::Sender<String>>,
}

impl GroqClient {
    pub fn new(api_key: String) -> Self {
        Self::new_with_cerebras(api_key, Vec::new())
    }
    
    pub fn new_with_cerebras(api_key: String, cerebras_keys: Vec<NamedKey>) -> Self {
        let user_profile = Self::load_profile();
        // Filter out empty keys
        let keys: Vec<NamedKey> = cerebras_keys.into_iter().filter(|k| !k.key.is_empty()).collect();
        if !keys.is_empty() {
            let names: Vec<&str> = keys.iter().map(|k| k.name.as_str()).collect();
            eprintln!("[Cerebras] {} key(s) loaded: {:?}, rotating every 3 requests", keys.len(), names);
        } else {
            eprintln!("[Cerebras] No keys - using Groq for all text/code");
        }
        Self {
            client: Client::new(),
            api_key,
            cerebras_keys: keys,
            cerebras_counter: Arc::new(AtomicUsize::new(0)),
            user_profile,
            tx: None,
        }
    }
    
    /// Attach a broadcast sender so the client can notify UI of active key
    pub fn set_broadcast(&mut self, tx: broadcast::Sender<String>) {
        self.tx = Some(tx);
    }
    
    /// Returns the next Cerebras (name, key) rotating every 2 requests, or None to use Groq
    fn get_cerebras_key(&self) -> Option<(String, String)> {
        if self.cerebras_keys.is_empty() {
            return None;
        }
        let count = self.cerebras_counter.fetch_add(1, Ordering::SeqCst);
        let index = (count / 2) % self.cerebras_keys.len();
        let named = &self.cerebras_keys[index];
        
        // Broadcast the active key name to UI
        if let Some(ref tx) = self.tx {
            let _ = tx.send(serde_json::json!({
                "type": "active_api_key",
                "name": named.name
            }).to_string());
        }
        
        Some((named.name.clone(), named.key.clone()))
    }
    
    /// Get a SPECIFIC key by index (for retry after 429)
    fn get_cerebras_key_at(&self, index: usize) -> Option<(String, String)> {
        if self.cerebras_keys.is_empty() {
            return None;
        }
        let idx = index % self.cerebras_keys.len();
        let named = &self.cerebras_keys[idx];
        
        if let Some(ref tx) = self.tx {
            let _ = tx.send(serde_json::json!({
                "type": "active_api_key",
                "name": named.name
            }).to_string());
        }
        
        Some((named.name.clone(), named.key.clone()))
    }
    
    /// Get current rotation index (for retry logic)
    fn current_key_index(&self) -> usize {
        let count = self.cerebras_counter.load(Ordering::SeqCst);
        if self.cerebras_keys.is_empty() { 0 } else { (count / 2) % self.cerebras_keys.len() }
    }
    
    fn load_profile() -> String {
        let profile_paths = [
            "profile.json",
            "interview-helper/profile.json",
            "../profile.json",
            "./interview-helper/profile.json",
            "e:/project/newphonewrtc/interview-helper/profile.json",
            "e:\\project\\newphonewrtc\\interview-helper\\profile.json"
        ];
        
        let content = profile_paths.iter()
            .find_map(|path| {
                if let Ok(c) = fs::read_to_string(path) {
                    eprintln!("✅ Profile loaded from: {}", path);
                    Some(c)
                } else {
                    None
                }
            });
        
        match content {
            Some(content) => {
                eprintln!("📋 Raw profile length: {} chars", content.len());
                content
            }
            None => {
                eprintln!("⚠️  WARNING: profile.json not found in any expected location. Profile features disabled.");
                String::new()
            }
        }
    }

    
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
                // Empty string = no prompt bias (for candidate mic)
            } else {
                // Default prompt for interviewer system audio only
                form = form.text("prompt", "Interview conversation in Indian English accent.");
            }
            
            let result = self.client
                .post("https://api.groq.com/openai/v1/audio/transcriptions")
                .header("Authorization", format!("Bearer {}", self.api_key))
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
                        // Retry on server errors
                        if status.is_server_error() {
                            retry_count += 1;
                            tokio::time::sleep(std::time::Duration::from_millis(500 * retry_count as u64)).await;
                            continue;
                        } else {
                            // Don't retry on 4xx errors
                            return Err(last_error);
                        }
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    retry_count += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500 * retry_count as u64)).await;
                }
            }
        }
        
        Err(format!("Failed after {} retries. Last error: {}", max_retries, last_error))
    }
    
    pub async fn chat_with_history(&self, message: &str, history: &[ConversationMessage]) -> Result<String, String> {
        eprintln!("🔍 DEBUG: user_profile length = {} chars", self.user_profile.len());
        eprintln!("🔍 DEBUG: user_profile empty? {}", self.user_profile.is_empty());
        if !self.user_profile.is_empty() {
            eprintln!("🔍 DEBUG: Profile preview: {}", &self.user_profile[..self.user_profile.len().min(300)]);
        }
        
        let system_prompt = if !self.user_profile.is_empty() {
            format!(
                "You are answering interview questions FOR a CS student named Dharaneesh. \
                He will READ your answer ALOUD word-for-word. Write EXACTLY how he should speak it.\n\n\
                PROFILE:\n{}\n\n\
                === HOW TO WRITE ===\n\
                FORMAT (Markdown for easy scanning):\n\
                - Start with a ONE-LINE direct answer in **bold**\n\
                - Break explanation into short bullet points or short paragraphs\n\
                - **Bold** the key terms so they stand out while speaking\n\
                - For code/DSA: approach in points, then a clean code block\n\n\
                LANGUAGE:\n\
                - SIMPLE everyday English. NO complex or fancy words.\n\
                - Every line directly speakable — no rewording needed\n\
                - Short sentences, one idea per line\n\n\
                LENGTH:\n\
                - Simple question: 2-4 points\n\
                - Technical/problem-solving: as long as needed, fully structured\n\
                - Behavioral: structured story in simple words\n\
                - Complete and clear beats short and vague\n\n\
                CONTENT:\n\
                - Real technical answers, no food/daily-life analogies unless asked\n\
                - For 'explain with example': give a code/technical example\n\
                - Only mention projects if asked about YOUR experience",
                self.user_profile
            )
        } else {
            "You are answering interview questions for a CS student who READS the answer ALOUD.\n\n\
            === HOW TO WRITE ===\n\
            - Start with a ONE-LINE direct answer in **bold**\n\
            - Break explanation into short bullet points\n\
            - **Bold** key terms\n\
            - For code/DSA: approach in points, then code block\n\n\
            LANGUAGE:\n\
            - SIMPLE everyday English, NO complex words\n\
            - Every line directly speakable\n\
            - Short sentences, one idea per line\n\n\
            LENGTH: short for simple questions, fully structured for technical ones. Complete over vague.\n\
            CONTENT: real technical answers, no analogies unless asked.".to_string()
        };
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        // Add ALL conversation history for full context
        for msg in history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }
        
        // 🔥 FIX: Add the current message as the latest user message
        messages.push(json!({
            "role": "user",
            "content": message
        }));
        
        // Use Cerebras if available (rotating keys) with 429 retry
        let initial_key = self.get_cerebras_key();
        let is_cerebras = initial_key.is_some();
        
        let (base_url, initial_api_key, model_name) = match initial_key {
            Some((_name, k)) => ("https://api.cerebras.ai/v1/chat/completions".to_string(), k, "gpt-oss-120b"),
            None => ("https://api.groq.com/openai/v1/chat/completions".to_string(), self.api_key.clone(), "openai/gpt-oss-120b"),
        };
        
        let payload = json!({
            "model": model_name,
            "messages": messages,
            "temperature": 0.5,
            "max_completion_tokens": 8192
        });
        
        let max_retries = if is_cerebras { self.cerebras_keys.len() + 1 } else { 2 };
        let mut current_key = initial_api_key;
        let mut retry_index = self.current_key_index();
        
        for attempt in 0..max_retries {
            let response = self.client
                .post(&base_url)
                .header("Authorization", format!("Bearer {}", current_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            
            if response.status().is_success() {
                let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                return Ok(json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string());
            } else if response.status().as_u16() == 429 && is_cerebras && attempt < max_retries - 1 {
                retry_index += 1;
                if let Some((_name, k)) = self.get_cerebras_key_at(retry_index) {
                    current_key = k;
                    continue;
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, &body[..body.len().min(200)]));
            }
        }
        
        Err("All keys exhausted (429 on all)".to_string())
    }
    
    pub async fn chat(&self, message: &str) -> Result<String, String> {
        self.chat_with_history(message, &[]).await
    }
    
    /// Chat with both conversation history AND candidate context
    /// Structures the prompt with separate windows:
    /// - Profile (ALWAYS FULL)
    /// - Interviewer questions: last 5 exact + older summarized
    /// - Candidate answers: last 5 exact + older summarized
    /// - Conversation history: last 10 messages (5 Q&A pairs)
    pub async fn chat_with_context(&self, message: &str, history: &[ConversationMessage], candidate_context: &[String]) -> Result<String, String> {
        // Separate interviewer questions from conversation history
        let interviewer_questions: Vec<&str> = history.iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .collect();
        
        // Build interviewer questions section
        // Keep last 5 exact, summarize older ones (up to 10 more)
        let interviewer_section = if interviewer_questions.len() > 5 {
            let split_at = interviewer_questions.len().saturating_sub(5);
            let old_questions = &interviewer_questions[..split_at.min(10)]; // Max 10 old ones
            let recent_questions = &interviewer_questions[interviewer_questions.len() - 5..];
            
            let old_summary = old_questions.iter()
                .map(|q| {
                    let truncated: String = q.chars().take(100).collect();
                    format!("- {}", truncated)
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "INTERVIEWER'S EARLIER QUESTIONS (summarized):\n{}\n\nINTERVIEWER'S RECENT QUESTIONS (exact):\n{}",
                old_summary,
                recent_questions.iter()
                    .enumerate()
                    .map(|(i, q)| format!("{}. {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if !interviewer_questions.is_empty() {
            format!(
                "INTERVIEWER'S QUESTIONS:\n{}",
                interviewer_questions.iter()
                    .enumerate()
                    .map(|(i, q)| format!("{}. {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };
        
        // Build candidate answers section
        // Keep last 5 exact, summarize older ones (up to 10 more)
        let candidate_section = if candidate_context.len() > 5 {
            let split_at = candidate_context.len().saturating_sub(5);
            let old_answers = &candidate_context[..split_at.min(10)]; // Max 10 old ones
            let recent_answers = &candidate_context[candidate_context.len() - 5..];
            
            let old_summary = old_answers.iter()
                .map(|a| {
                    let truncated: String = a.chars().take(100).collect();
                    format!("- {}", truncated)
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "CANDIDATE'S EARLIER ANSWERS (summarized):\n{}\n\nCANDIDATE'S RECENT ANSWERS (exact):\n{}",
                old_summary,
                recent_answers.iter()
                    .enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if !candidate_context.is_empty() {
            format!(
                "CANDIDATE'S ANSWERS:\n{}",
                candidate_context.iter()
                    .enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };
        
        // Build context block
        let context_block = match (interviewer_section.is_empty(), candidate_section.is_empty()) {
            (true, true) => String::new(),
            (false, true) => format!("\n\n{}\n", interviewer_section),
            (true, false) => format!("\n\n{}\n", candidate_section),
            (false, false) => format!("\n\n{}\n\n{}\n", interviewer_section, candidate_section),
        };
        
        // Build system prompt: Profile (FULL) + Context Windows + Instructions
        let system_prompt = if !self.user_profile.is_empty() {
            format!(
                "You are answering interview questions for Dharaneesh (final-year CS student). \
                He reads your answer DIRECTLY from screen while speaking. Make it EASY TO READ ALOUD.\n\n\
                PROFILE:\n{}\n\
                {}\
                === STRICT RULES ===\n\n\
                1. NEVER give code unless the interviewer EXPLICITLY asks to write code or solve a coding problem.\n\
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
                4. ONLY mention your projects/profile if interviewer asks about YOUR experience.\n\n\
                === EXAMPLE (Event-Driven Architecture) ===\n\n\
                **Event-driven architecture is when different parts of a system talk to each other through events instead of direct calls.**\n\n\
                → One service sends an event like 'order placed'\n\n\
                → Other services listen for that event and react on their own\n\n\
                → So the order service doesn't need to know about payment or shipping — they just pick up the event\n\n\
                → This makes the system loosely coupled — you can add new services without changing existing ones\n\n\
                → Common tools for this: Kafka, RabbitMQ, AWS SNS\n\n\
                (Notice: no code, short lines, easy to read aloud, blank lines between points)",
                self.user_profile,
                context_block
            )
        } else {
            format!(
                "You answer interview questions for a CS student who reads your answer DIRECTLY from screen.\n\
                {}\
                RULES:\n\
                1. NEVER give code unless explicitly asked to write code\n\
                2. First line: **one sentence direct answer** (bold)\n\
                3. Then 3-5 short lines, one idea each, max 15 words per line\n\
                4. Leave blank lines between points\n\
                5. Simple words only — speakable in one breath per line\n\
                6. No projects/profile unless asked about YOUR experience",
                context_block
            )
        };
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        // Only send last 10 messages (5 Q&A pairs) as chat history
        // This prevents token overflow while keeping recent conversation flow
        let recent_history: Vec<_> = history.iter().rev().take(10).rev().collect();
        for msg in recent_history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }
        
        // Add current question
        messages.push(json!({
            "role": "user",
            "content": message
        }));
        
        // Use Cerebras for voice answers (rotating keys) with 429 retry
        let initial_key = self.get_cerebras_key();
        let is_cerebras = initial_key.is_some();
        
        let (base_url, initial_api_key, model_name) = match initial_key {
            Some((_name, k)) => ("https://api.cerebras.ai/v1/chat/completions".to_string(), k, "gpt-oss-120b"),
            None => ("https://api.groq.com/openai/v1/chat/completions".to_string(), self.api_key.clone(), "openai/gpt-oss-120b"),
        };
        
        let payload = json!({
            "model": model_name,
            "messages": messages,
            "temperature": 0.5,
            "max_completion_tokens": 8192
        });
        
        // Retry loop: on 429, try next key (up to all keys)
        let max_retries = if is_cerebras { self.cerebras_keys.len() + 1 } else { 2 };
        let mut current_key = initial_api_key;
        let mut retry_index = self.current_key_index();
        
        for attempt in 0..max_retries {
            let response = self.client
                .post(&base_url)
                .header("Authorization", format!("Bearer {}", current_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            
            if response.status().is_success() {
                let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                return Ok(content);
            } else if response.status().as_u16() == 429 && is_cerebras && attempt < max_retries - 1 {
                // Rate limited - try next key immediately
                retry_index += 1;
                if let Some((_name, k)) = self.get_cerebras_key_at(retry_index) {
                    eprintln!("[Cerebras] 429 on attempt {}, switching to next key", attempt + 1);
                    current_key = k;
                    continue;
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(format!("API error {}: {}", status, &body[..body.len().min(200)]));
            }
        }
        
        Err("All Cerebras keys exhausted (429 on all)".to_string())
    }
    
    async fn analyze_images_internal(&self, images: &[&str], history: &[ConversationMessage]) -> Result<(String, bool), String> {
        if images.is_empty() {
            return Err("No images provided".to_string());
        }
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        let mut system_prompt = String::from("You are Scout, a technical interview vision assistant. Extract information from this screenshot in JSON format.\n\nFirst, identify the TYPE:\n- DSA_PROBLEM: Coding problem (LeetCode/HackerRank style) with input/output examples\n- SYSTEM_DESIGN: Architecture/design question\n- LOGICAL_PUZZLE: Text-based reasoning problem\n- DEBUG_ERROR: Code with error messages\n- GENERAL: Other technical content\n\nFor DSA_PROBLEM, extract:\n{\n  \"type\": \"DSA_PROBLEM\",\n  \"title\": \"problem name\",\n  \"description\": \"full problem statement\",\n  \"input_format\": \"how input is given\",\n  \"output_format\": \"expected output format\",\n  \"constraints\": [\"list of constraints\"],\n  \"examples\": [{\"input\": \"...\", \"output\": \"...\", \"explanation\": \"...\"}],\n  \"predefined_code\": \"if present, extract the exact function signature like 'class Solution { public: long long minimumPerimeter(long long neededApples) { } };'\",\n  \"confidence\": 0.95\n}\n\nFor SYSTEM_DESIGN:\n{\n  \"type\": \"SYSTEM_DESIGN\",\n  \"question\": \"design question\",\n  \"requirements\": [\"list of requirements\"],\n  \"confidence\": 0.90\n}\n\nFor DEBUG_ERROR:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"code\": \"extracted code only\",\n  \"predefined_code\": \"if present, extract the exact function signature\",\n  \"error_message\": \"error text\",\n  \"confidence\": 0.85\n}\n\nFor GENERAL:\n{\n  \"type\": \"GENERAL\",\n  \"content\": \"description of screenshot\",\n  \"confidence\": 0.80\n}\n\nIMPORTANT:\n- Extract ALL visible text accurately\n- If there's predefined code template (like class Solution with function signature), extract it EXACTLY\n- Include confidence score (0.0-1.0)\n- If confidence < 0.7, set \"needs_recapture\": true\n- Ignore UI elements, focus on problem content\n- Return ONLY valid JSON, no extra text");
        
        if images.len() > 1 {
            system_prompt.push_str("\n\nMULTI-IMAGE INSTRUCTIONS:\n- The following screenshots are consecutive parts of the same question.\n- Combine ALL visible text across images into a single coherent extraction.\n- If text overlaps between images, de-duplicate it.\n- Do not omit any sections, constraints, or examples.");
        }
        
        let mut context = String::new();
        if !recent_history.is_empty() {
            context.push_str("\n\nRecent conversation (last 20 messages):\n");
            for msg in recent_history {
                context.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
        }
        let full_prompt = format!("{}{}", system_prompt, context);
        
        let mut content_items = vec![json!({
            "type": "text",
            "text": full_prompt
        })];
        
        for image_base64 in images {
            content_items.push(json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", image_base64)
                }
            }));
        }
        
        let payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [
                {
                    "role": "user",
                    "content": content_items
                }
            ],
            "temperature": 0.3,
            "max_tokens": 3000
        });
        
        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Scout API error {}: {}", status, &body[..body.len().min(300)]));
        }
        
        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
            let problem_type = parsed["type"].as_str().unwrap_or("GENERAL");
            let confidence = parsed["confidence"].as_f64().unwrap_or(1.0);
            let needs_recapture = parsed["needs_recapture"].as_bool().unwrap_or(false);
            
            if needs_recapture || confidence < 0.7 {
                return Ok((format!("{{\"needs_recapture\": true, \"reason\": \"Low confidence ({:.0}%). Please recapture with better quality.\"}}", confidence * 100.0), false));
            }
            
            let is_coding = problem_type == "DSA_PROBLEM";
            Ok((content, is_coding))
        } else {
            let is_coding = content.contains("CODING_PROBLEM:");
            Ok((content, is_coding))
        }
    }

    pub async fn analyze_image(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<(String, bool), String> {
        self.analyze_images_internal(&[image_base64], history).await
    }

    pub async fn analyze_images(&self, images: &[String], history: &[ConversationMessage]) -> Result<(String, bool), String> {
        let refs: Vec<&str> = images.iter().map(|s| s.as_str()).collect();
        self.analyze_images_internal(&refs, history).await
    }
    
    pub async fn solve_coding_problem(&self, problem: &str, history: &[ConversationMessage], candidate_context: &[String]) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        // Build candidate context section for 120B
        let candidate_section = if !candidate_context.is_empty() {
            let recent: Vec<_> = candidate_context.iter().rev().take(5).collect();
            format!(
                "\n\nCANDIDATE HAS MENTIONED (use their stated approach if relevant):\n{}\n",
                recent.iter().rev()
                    .enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };
        
        let system_prompt = format!("You are a technical interview expert specializing in DSA problems. You have access to a CODE INTERPRETER tool.\n\n**TOOL USAGE:**\n- **code_interpreter** - Use this to:\n  - Run your solution against the provided test cases\n  - Verify correctness before presenting the answer\n  - Test edge cases (empty input, single element, large values)\n  - Confirm time complexity is acceptable\n\n**WORKFLOW:**\n1. Read the problem carefully\n2. Think of the optimal approach\n3. Write the solution\n4. USE code_interpreter to test it against the examples\n5. If it fails, fix and re-test\n6. Present the verified solution\n\nProvide clear, structured, and detailed explanations.{}\n\n**OUTPUT FORMAT FOR DSA PROBLEMS (90%):**\n\n## PROBLEM UNDERSTANDING\n[Explain what the problem is asking in simple words - 2-3 sentences]\n\n## APPROACH 1: BRUTE FORCE\n\n**Intuition:**\n[Explain the straightforward approach in simple, conversational language - like explaining to a friend. Use 3-4 sentences.]\n\n**How it works:**\n- Step 1: [Explain first step]\n- Step 2: [Explain second step]\n- Step 3: [Continue...]\n\n**Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// If predefined structure exists (class Solution), use it EXACTLY\n// Otherwise use: int main() {{ int t; cin >> t; while(t--) {{ }} }}\n```\n\n**Complexity Analysis:**\n- Time Complexity: O(n²) where n = size of input array\n- Space Complexity: O(1) where we use constant extra space\n\n---\n\n## APPROACH 2: OPTIMAL SOLUTION\n\n**Intuition:**\n[Explain the optimized approach in simple words. What's the key insight that makes it faster? 3-4 sentences.]\n\n**How it works:**\n- Step 1: [Explain optimization step 1]\n- Step 2: [Explain optimization step 2]\n- Step 3: [Continue...]\n\n**Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// Optimized implementation\n// Use EXACT predefined structure if given\n```\n\n**Complexity Analysis:**\n- Time Complexity: O(n) where n = size of input array\n- Space Complexity: O(n) where n = space used for hash map\n\n---\n\n## COMPARISON\n[Compare both approaches - which is better and why? When to use which? 2-3 sentences]\n\n---\n\n**EDGE CASES (10%): System Design / General Questions**\nIf the question is NOT a DSA problem (system design, conceptual, etc.), provide a clear conversational answer with:\n- Simple explanation\n- Key points as bullet points\n- Examples if helpful\n\n---\n\n**CRITICAL RULES:**\n1. **Code Structure:**\n   - Use #include <bits/stdc++.h> and using namespace std;\n   - Do NOT use ios::sync_with_stdio(false), cin.tie(NULL), or fast I/O\n   - If predefined structure exists (class Solution {{ public: ... }}), use it EXACTLY\n   - If NO predefined structure, use: int main() {{ int t; cin >> t; while(t--) {{ }} return 0; }}\n   - NEVER mix class-based and main()-based approaches\n\n2. **Complexity Explanation:**\n   - ALWAYS explain what each variable in O() notation means\n   - Example: O(n*m) where n=rows, m=columns\n   - Use simple, clear language\n\n3. **Code Style:**\n   - Keep code SIMPLE and readable\n   - Use short variable names (n, m, i, j, x, y)\n   - Add brief comments for clarity\n   - Follow the predefined structure if given\n\n4. **Explanation Style:**\n   - Write like you're explaining to a friend\n   - Use conversational, simple language\n   - Break down complex ideas into steps\n   - Focus on WHY, not just WHAT\n\n5. **If candidate mentioned an approach:**\n   - PRIORITIZE their stated approach as the optimal solution\n   - Show it works correctly\n   - Only suggest alternatives if their approach is suboptimal\n\n6. **VERIFY WITH CODE INTERPRETER:**\n   - ALWAYS test your optimal solution against the provided examples\n   - If test fails, debug and fix before presenting", candidate_section);
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        for msg in recent_history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }
        
        messages.push(json!({
            "role": "user",
            "content": problem
        }));
        
        // Use Cerebras for coding (rotating keys)
        let (url, key, model_name) = match self.get_cerebras_key() {
            Some((_name, k)) => ("https://api.cerebras.ai/v1/chat/completions".to_string(), k, "gpt-oss-120b"),
            None => ("https://api.groq.com/openai/v1/chat/completions".to_string(), self.api_key.clone(), "openai/gpt-oss-120b"),
        };
        
        // Use code_interpreter tool to verify solutions against test cases
        let payload = json!({
            "model": model_name,
            "messages": messages,
            "temperature": 0.4,
            "max_completion_tokens": 8192,
            "top_p": 1,
            "reasoning_effort": "medium"
        });
        
        let mut retries = 0;
        loop {
            // Increased timeout to 30s because code interpreter takes longer
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
            ).await {
                Ok(Ok(response)) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                        return Ok(content);
                    } else if response.status().is_server_error() && retries < 3 {
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    } else {
                        return Err(format!("API error: {}", response.status()));
                    }
                }
                Ok(Err(e)) if retries < 3 => {
                    eprintln!("API request failed (attempt {}): {}", retries + 1, e);
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Ok(Err(e)) => return Err(format!("API request failed: {}", e)),
                Err(_) if retries < 3 => {
                    eprintln!("Request timed out after 30 seconds (attempt {})", retries + 1);
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Err(_) => return Err("Request timed out after 30 seconds".to_string()),
            }
        }
    }
    
    /// Solve coding problem using Gemini 3.1 Flash via Google AI API
    /// Uses code execution tool to verify solutions against test cases
    pub async fn solve_with_gemini(&self, problem: &str, history: &[ConversationMessage], candidate_context: &[String], gemini_key: &str) -> Result<String, String> {
        let candidate_section = if !candidate_context.is_empty() {
            let recent: Vec<_> = candidate_context.iter().rev().take(5).collect();
            format!(
                "\nCANDIDATE HAS MENTIONED:\n{}\n",
                recent.iter().rev()
                    .enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };
        
        // Build conversation parts for Gemini format
        let mut parts_text = format!(
            "You are a world-class competitive programming expert with code execution capability.\n\
            {}\n\
            INSTRUCTIONS:\n\
            1. Solve the problem below with the OPTIMAL approach\n\
            2. Use code execution to VERIFY your solution against the provided test cases\n\
            3. If verification fails, fix and re-verify\n\
            4. Present the final verified solution\n\n\
            OUTPUT FORMAT (use this EXACT structure):\n\n\
            ## PROBLEM UNDERSTANDING\n\
            [Explain what the problem is asking in simple words - 2-3 sentences]\n\n\
            ## OPTIMAL SOLUTION\n\n\
            **Intuition:**\n\
            [Explain the optimal approach - what's the key insight? 3-5 sentences]\n\n\
            **How it works:**\n\
            - Step 1: [Explain step 1]\n\
            - Step 2: [Explain step 2]\n\
            - Step 3: [Continue...]\n\n\
            **Code:**\n\
            ```cpp\n\
            #include <bits/stdc++.h>\n\
            using namespace std;\n\
            // Optimized implementation\n\
            ```\n\n\
            **Complexity Analysis:**\n\
            - Time Complexity: O(...) where [explain variables]\n\
            - Space Complexity: O(...) where [explain variables]\n\n\
            CRITICAL RULES:\n\
            - Use C++ ONLY with #include <bits/stdc++.h> and using namespace std;\n\
            - Do NOT use ios::sync_with_stdio(false), cin.tie(NULL), or fast I/O\n\
            - If predefined structure exists (class Solution), use it EXACTLY\n\
            - If NO predefined structure, use: int main() {{ int t; cin >> t; while(t--) {{ }} return 0; }}\n\
            - NEVER mix class-based and main()-based approaches\n\
            - ALWAYS explain what each variable in O() notation means\n\
            - If candidate mentioned an approach, PRIORITIZE it\n\
            - Use code execution to verify your solution works on the examples\n\n\
            PROBLEM:\n{}",
            candidate_section, problem
        );
        
        // Add recent history context
        let recent: Vec<_> = history.iter().rev().take(6).rev().collect();
        if !recent.is_empty() {
            parts_text.push_str("\n\nRECENT CONTEXT:\n");
            for msg in recent {
                parts_text.push_str(&format!("{}: {}\n", msg.role, msg.content.chars().take(200).collect::<String>()));
            }
        }
        
        let payload = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": parts_text }]
            }],
            "generationConfig": {
                "thinkingConfig": { "thinkingLevel": "HIGH" }
            },
            "tools": [
                { "codeExecution": {} },
                { "googleSearch": {} }
            ]
        });
        
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite:generateContent?key={}",
            gemini_key
        );
        
        let mut retries = 0;
        loop {
            match self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        
                        // Gemini response format: candidates[0].content.parts[].text
                        // Also handles: functionCall, executableCode, codeExecutionResult
                        let mut result = String::new();
                        if let Some(candidates) = json["candidates"].as_array() {
                            if let Some(first) = candidates.first() {
                                if let Some(parts) = first["content"]["parts"].as_array() {
                                    for part in parts {
                                        if let Some(text) = part["text"].as_str() {
                                            result.push_str(text);
                                        }
                                        // Handle code execution results
                                        if let Some(code_result) = part["codeExecutionResult"].as_object() {
                                            if let Some(output) = code_result.get("output").and_then(|o| o.as_str()) {
                                                if !output.is_empty() {
                                                    result.push_str(&format!("\n\n**Code Execution Output:**\n```\n{}\n```\n", output));
                                                }
                                            }
                                        }
                                        // Handle executable code shown
                                        if let Some(exec_code) = part["executableCode"].as_object() {
                                            if let Some(code) = exec_code.get("code").and_then(|c| c.as_str()) {
                                                if !code.is_empty() && result.is_empty() {
                                                    result.push_str(&format!("```python\n{}\n```\n", code));
                                                }
                                            }
                                        }
                                        // Handle functionCall (code execution in progress)
                                        if let Some(func_call) = part["functionCall"].as_object() {
                                            if let Some(args) = func_call.get("args").and_then(|a| a.as_object()) {
                                                if let Some(code) = args.get("code").and_then(|c| c.as_str()) {
                                                    if result.is_empty() {
                                                        // Model is still thinking via code - return the code as context
                                                        result.push_str(&format!("**Gemini is solving via code execution...**\n\n```python\n{}\n```", code));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Check finish reason
                                let finish_reason = first["finishReason"].as_str().unwrap_or("");
                                if finish_reason == "STOP" && result.is_empty() {
                                    // Model finished but with function call only - extract from functionCall
                                    if let Some(parts) = first["content"]["parts"].as_array() {
                                        for part in parts {
                                            if let Some(func_call) = part["functionCall"].as_object() {
                                                if let Some(args) = func_call.get("args").and_then(|a| a.as_object()) {
                                                    if let Some(code) = args.get("code").and_then(|c| c.as_str()) {
                                                        result = format!("**Gemini Code Solution:**\n\n```python\n{}\n```", code);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        if !result.is_empty() {
                            return Ok(result);
                        }
                        
                        // Check for error
                        if let Some(err) = json["error"].as_object() {
                            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown");
                            return Err(format!("Gemini error: {}", msg));
                        }
                        
                        return Err(format!("Gemini empty response: {}", &json.to_string()[..json.to_string().len().min(300)]));
                    } else if retries < 3 {
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        continue;
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        return Err(format!("Gemini API error: {}", body));
                    }
                }
                Err(e) if retries < 3 => {
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    continue;
                }
                Err(e) => return Err(format!("Gemini request failed: {}", e)),
            }
        }
    }
    
    /// Debug code error using Gemini 3.1 Flash with code execution verification
    pub async fn debug_with_gemini(&self, image_base64: &str, history: &[ConversationMessage], gemini_key: &str) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        // Step 1: Scout extracts error info
        let vision_prompt = "Extract ALL debugging information from this screenshot in JSON format:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"language\": \"C++/Python/Java\",\n  \"code\": \"extracted code only, ignore UI\",\n  \"predefined_code\": \"if present, extract the exact function signature\",\n  \"error_message\": \"exact error text\",\n  \"input_test_case\": \"if visible, extract the input that caused the error\",\n  \"expected_output\": \"if visible\",\n  \"actual_output\": \"if visible\",\n  \"confidence\": 0.90\n}\n\nIMPORTANT:\n- Extract ONLY the code, ignore buttons, menus, UI elements\n- Extract input/expected/actual output if visible\n- Return valid JSON only";
        
        let vision_payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [{ "role": "user", "content": [
                { "type": "text", "text": vision_prompt },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{}", image_base64) } }
            ]}],
            "temperature": 0.2,
            "max_tokens": 2000
        });
        
        let vision_response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&vision_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        let vision_json: serde_json::Value = vision_response.json().await.map_err(|e| e.to_string())?;
        let error_raw = vision_json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        
        // Parse Scout output
        let error_description = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&error_raw.trim().trim_start_matches("```json").trim_end_matches("```").trim()) {
            let mut f = String::new();
            if let Some(v) = parsed["error_category"].as_str() { f.push_str(&format!("ERROR TYPE: {}\n", v)); }
            if let Some(v) = parsed["error_message"].as_str() { if !v.is_empty() { f.push_str(&format!("ERROR: {}\n", v)); } }
            if let Some(v) = parsed["code"].as_str() { if !v.is_empty() { f.push_str(&format!("\nCODE:\n```\n{}\n```\n", v)); } }
            if let Some(v) = parsed["input_test_case"].as_str() { if !v.is_empty() { f.push_str(&format!("INPUT: {}\n", v)); } }
            if let Some(v) = parsed["expected_output"].as_str() { if !v.is_empty() { f.push_str(&format!("EXPECTED: {}\n", v)); } }
            if let Some(v) = parsed["actual_output"].as_str() { if !v.is_empty() { f.push_str(&format!("ACTUAL: {}\n", v)); } }
            if f.is_empty() { error_raw } else { f }
        } else {
            error_raw
        };
        
        // Find previous code from history
        let mut context = String::new();
        let last_code = recent_history.iter().rev()
            .find(|m| m.role == "assistant" && (m.content.contains("```") || m.content.contains("#include")))
            .map(|m| m.content.as_str())
            .unwrap_or("");
        if !last_code.is_empty() {
            context.push_str(&format!("\nPREVIOUSLY GENERATED CODE (make MINIMAL changes to THIS):\n{}\n", last_code));
        }
        
        // Step 2: Gemini fixes the bug with code execution verification
        let debug_text = format!(
            "You are a C++ debugging expert with code execution capability.\n\n\
            {}\n{}\n\n\
            INSTRUCTIONS:\n\
            1. Identify the bug\n\
            2. Fix with MINIMAL changes only\n\
            3. Use code execution to VERIFY the fix works against the test cases\n\
            4. If verification fails, iterate until it passes\n\n\
            RULES:\n\
            - Do NOT rewrite the entire code\n\
            - Do NOT change the algorithm\n\
            - ONLY fix the specific bug\n\
            - Mark changes with: // FIX: [what changed]\n\n\
            FORMAT:\n\
            ## BUG\n\
            [1-2 sentences]\n\n\
            ## FIX\n\
            ```cpp\n\
            // Full code with only bug fixed\n\
            // Mark: // FIX: [explanation]\n\
            ```\n\n\
            ## CHANGED\n\
            - [old] → [new] (why)",
            error_description, context
        );
        
        let payload = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": debug_text }]
            }],
            "generationConfig": {
                "thinkingConfig": { "thinkingLevel": "HIGH" }
            },
            "tools": [
                { "codeExecution": {} }
            ]
        });
        
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite:generateContent?key={}",
            gemini_key
        );
        
        let mut retries = 0;
        loop {
            match self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        
                        let mut result = String::new();
                        if let Some(candidates) = json["candidates"].as_array() {
                            if let Some(first) = candidates.first() {
                                if let Some(parts) = first["content"]["parts"].as_array() {
                                    for part in parts {
                                        if let Some(text) = part["text"].as_str() {
                                            result.push_str(text);
                                        }
                                        if let Some(code_result) = part["codeExecutionResult"].as_object() {
                                            if let Some(output) = code_result.get("output").and_then(|o| o.as_str()) {
                                                if !output.is_empty() {
                                                    result.push_str(&format!("\n**Verification:**\n```\n{}\n```\n", output));
                                                }
                                            }
                                        }
                                        if let Some(exec_code) = part["executableCode"].as_object() {
                                            if let Some(code) = exec_code.get("code").and_then(|c| c.as_str()) {
                                                if !code.is_empty() && result.is_empty() {
                                                    result.push_str(&format!("```python\n{}\n```\n", code));
                                                }
                                            }
                                        }
                                        if let Some(func_call) = part["functionCall"].as_object() {
                                            if let Some(args) = func_call.get("args").and_then(|a| a.as_object()) {
                                                if let Some(code) = args.get("code").and_then(|c| c.as_str()) {
                                                    if result.is_empty() {
                                                        result.push_str(&format!("**Gemini Debug Solution:**\n\n```python\n{}\n```", code));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        if !result.is_empty() {
                            return Ok(result);
                        }
                        
                        if let Some(err) = json["error"].as_object() {
                            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown");
                            return Err(format!("Gemini debug error: {}", msg));
                        }
                        
                        return Err(format!("Gemini debug empty: {}", &json.to_string()[..json.to_string().len().min(300)]));
                    } else if retries < 3 {
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        continue;
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        return Err(format!("Gemini debug API error: {}", body));
                    }
                }
                Err(e) if retries < 3 => { retries += 1; tokio::time::sleep(std::time::Duration::from_secs(3)).await; continue; }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    
    pub async fn debug_code_error(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        let vision_prompt = "Extract ALL debugging information from this screenshot in JSON format:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"language\": \"C++/Python/Java\",\n  \"code\": \"extracted code only, ignore UI\",\n  \"predefined_code\": \"if present, extract the exact function signature like 'class Solution { public: long long minimumPerimeter(long long neededApples) { } };'\",\n  \"error_message\": \"exact error text\",\n  \"input_test_case\": \"if visible, extract the input that caused the error\",\n  \"expected_output\": \"if visible, extract what output was expected\",\n  \"actual_output\": \"if visible, extract what output your code produced\",\n  \"test_case_info\": \"any additional test case details\",\n  \"confidence\": 0.90\n}\n\nIMPORTANT:\n- Extract ONLY the code, ignore buttons, menus, UI elements\n- If predefined function signature exists, extract it EXACTLY\n- Extract input/expected/actual output if visible on screen\n- Return valid JSON only";
        
        let vision_payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": vision_prompt
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:image/png;base64,{}", image_base64)
                            }
                        }
                    ]
                }
            ],
            "temperature": 0.2,
            "max_tokens": 2000
        });
        
        let vision_response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&vision_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        let vision_json: serde_json::Value = vision_response.json().await.map_err(|e| e.to_string())?;
        let error_description_raw = vision_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        // Parse Scout's structured output for better formatting
        let error_description = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&error_description_raw.trim().trim_start_matches("```json").trim_end_matches("```").trim()) {
            let mut formatted = String::new();
            
            if let Some(category) = parsed["error_category"].as_str() {
                formatted.push_str(&format!("ERROR TYPE: {}\n", category));
            }
            if let Some(lang) = parsed["language"].as_str() {
                formatted.push_str(&format!("LANGUAGE: {}\n", lang));
            }
            if let Some(error_msg) = parsed["error_message"].as_str() {
                if !error_msg.is_empty() {
                    formatted.push_str(&format!("\nERROR MESSAGE:\n{}\n", error_msg));
                }
            }
            if let Some(code) = parsed["code"].as_str() {
                if !code.is_empty() {
                    formatted.push_str(&format!("\nCODE FROM SCREENSHOT:\n```\n{}\n```\n", code));
                }
            }
            if let Some(predefined) = parsed["predefined_code"].as_str() {
                if !predefined.is_empty() {
                    formatted.push_str(&format!("\nPREDEFINED STRUCTURE (must use):\n{}\n", predefined));
                }
            }
            if let Some(input) = parsed["input_test_case"].as_str() {
                if !input.is_empty() {
                    formatted.push_str(&format!("\nFAILING TEST CASE INPUT:\n{}\n", input));
                }
            }
            if let Some(expected) = parsed["expected_output"].as_str() {
                if !expected.is_empty() {
                    formatted.push_str(&format!("EXPECTED OUTPUT: {}\n", expected));
                }
            }
            if let Some(actual) = parsed["actual_output"].as_str() {
                if !actual.is_empty() {
                    formatted.push_str(&format!("ACTUAL OUTPUT: {}\n", actual));
                }
            }
            
            if formatted.is_empty() {
                error_description_raw
            } else {
                formatted
            }
        } else {
            error_description_raw
        };
        
        // Step 2: Use GPT-OSS-120B to provide fix
        let system_prompt = "You are a technical interview C++ debugging expert.\n\n\
CRITICAL RULE: You MUST make ONLY MINIMAL changes to fix the error.\n\
- Do NOT rewrite the entire code\n\
- Do NOT change the algorithm or approach\n\
- ONLY fix the specific bug/error shown in the screenshot\n\
- Mark every change with a comment: // FIX: [what was changed]\n\
- Keep ALL other code EXACTLY the same\n\
- If the previous solution is in conversation history, use THAT code as the base\n\
- If the screenshot shows code, use THAT as the base (it's what's currently in the editor)\n\
- Output the FULL corrected code with minimal changes highlighted";
        
        let mut context = String::new();
        if !recent_history.is_empty() {
            // Find the most recent assistant code response
            let last_code = recent_history.iter().rev()
                .find(|m| m.role == "assistant" && (m.content.contains("```") || m.content.contains("#include")))
                .map(|m| m.content.as_str())
                .unwrap_or("");
            
            if !last_code.is_empty() {
                context.push_str(&format!("\n\nPREVIOUSLY GENERATED CODE (make MINIMAL changes to THIS):\n{}\n", last_code));
            }
        }
        
        let debug_prompt = format!("Error analysis from screenshot:\n{}\n{}\n\nFix this error with MINIMAL changes only.\n\nFormat:\n\n## BUG IDENTIFIED\n[1-2 sentences: what exactly is wrong]\n\n## MINIMAL FIX\n```cpp\n// Output the FULL code with ONLY the bug fixed\n// Mark changes with: // FIX: [explanation]\n// Keep everything else EXACTLY the same\n```\n\n## WHAT CHANGED\n- Line X: [old code] → [new code] (reason)\n\nRULES:\n- Do NOT change the algorithm\n- Do NOT rewrite the solution\n- ONLY fix the specific error\n- Keep the same variable names, structure, approach", error_description, context);
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        for msg in &recent_history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }
        
        messages.push(json!({
            "role": "user",
            "content": debug_prompt
        }));
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
            "temperature": 0.4,
            "max_tokens": 4000
        });
        
        let mut retries = 0;
        loop {
            match self.client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
                .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
                        return Ok(content);
                    } else if response.status().is_server_error() && retries < 3 {
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    } else {
                        return Err(format!("API error: {}", response.status()));
                    }
                }
                Err(e) if retries < 3 => {
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    
    /// Debug code error using Ring 1T model via OpenRouter
    /// Same Scout extraction, but Ring model for the fix
    pub async fn debug_with_ring(&self, image_base64: &str, history: &[ConversationMessage], openrouter_key: &str) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        // Step 1: Scout extracts error info (same as debug_code_error)
        let vision_prompt = "Extract ALL debugging information from this screenshot in JSON format:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"language\": \"C++/Python/Java\",\n  \"code\": \"extracted code only, ignore UI\",\n  \"predefined_code\": \"if present, extract the exact function signature\",\n  \"error_message\": \"exact error text\",\n  \"input_test_case\": \"if visible, extract the input that caused the error\",\n  \"expected_output\": \"if visible\",\n  \"actual_output\": \"if visible\",\n  \"confidence\": 0.90\n}\n\nIMPORTANT:\n- Extract ONLY the code, ignore buttons, menus, UI elements\n- Extract input/expected/actual output if visible\n- Return valid JSON only";
        
        let vision_payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [{ "role": "user", "content": [
                { "type": "text", "text": vision_prompt },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{}", image_base64) } }
            ]}],
            "temperature": 0.2,
            "max_tokens": 2000
        });
        
        let vision_response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&vision_payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        let vision_json: serde_json::Value = vision_response.json().await.map_err(|e| e.to_string())?;
        let error_raw = vision_json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        
        // Parse Scout output
        let error_description = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&error_raw.trim().trim_start_matches("```json").trim_end_matches("```").trim()) {
            let mut f = String::new();
            if let Some(v) = parsed["error_category"].as_str() { f.push_str(&format!("ERROR TYPE: {}\n", v)); }
            if let Some(v) = parsed["error_message"].as_str() { if !v.is_empty() { f.push_str(&format!("ERROR: {}\n", v)); } }
            if let Some(v) = parsed["code"].as_str() { if !v.is_empty() { f.push_str(&format!("\nCODE:\n```\n{}\n```\n", v)); } }
            if let Some(v) = parsed["input_test_case"].as_str() { if !v.is_empty() { f.push_str(&format!("INPUT: {}\n", v)); } }
            if let Some(v) = parsed["expected_output"].as_str() { if !v.is_empty() { f.push_str(&format!("EXPECTED: {}\n", v)); } }
            if let Some(v) = parsed["actual_output"].as_str() { if !v.is_empty() { f.push_str(&format!("ACTUAL: {}\n", v)); } }
            if f.is_empty() { error_raw } else { f }
        } else {
            error_raw
        };
        
        // Find previous code from history
        let mut context = String::new();
        if !recent_history.is_empty() {
            let last_code = recent_history.iter().rev()
                .find(|m| m.role == "assistant" && (m.content.contains("```") || m.content.contains("#include")))
                .map(|m| m.content.as_str())
                .unwrap_or("");
            if !last_code.is_empty() {
                context.push_str(&format!("\n\nPREVIOUSLY GENERATED CODE (make MINIMAL changes to THIS):\n{}\n", last_code));
            }
        }
        
        // Step 2: Ring model fixes the bug
        let system_prompt = "You are a world-class C++ debugging expert.\n\n\
CRITICAL: Make ONLY MINIMAL changes to fix the error.\n\
- Do NOT rewrite the entire code\n\
- Do NOT change the algorithm\n\
- ONLY fix the specific bug\n\
- Mark changes with: // FIX: [what changed]\n\
- Output the FULL corrected code";
        
        let debug_prompt = format!("{}\n{}\n\nFix with MINIMAL changes.\n\n## BUG\n[1-2 sentences]\n\n## FIX\n```cpp\n// Full code with only bug fixed\n// Mark: // FIX: [explanation]\n```\n\n## CHANGED\n- [old] → [new] (why)", error_description, context);
        
        let mut messages = vec![json!({"role": "system", "content": system_prompt})];
        for msg in recent_history.iter().rev().take(6).rev() {
            messages.push(json!({"role": msg.role, "content": msg.content}));
        }
        messages.push(json!({"role": "user", "content": debug_prompt}));
        
        let payload = json!({
            "model": "inclusionai/ring-2.6-1t:free",
            "messages": messages,
            "temperature": 0.3
        });
        
        let mut retries = 0;
        loop {
            match self.client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", openrouter_key))
                .header("Content-Type", "application/json")
                .header("HTTP-Referer", "http://localhost:5000")
                .json(&payload)
                .send()
                .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
                        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("");
                        if !content.is_empty() { return Ok(content.to_string()); }
                        if let Some(r) = json["choices"][0]["message"]["reasoning"].as_str() {
                            if !r.is_empty() { return Ok(r.to_string()); }
                        }
                        if let Some(r) = json["choices"][0]["message"]["reasoning_content"].as_str() {
                            if !r.is_empty() { return Ok(r.to_string()); }
                        }
                        return Err(format!("Ring debug empty. Response: {}", &json.to_string()[..json.to_string().len().min(300)]));
                    } else if retries < 3 {
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        continue;
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        return Err(format!("Ring debug error: {}", body));
                    }
                }
                Err(e) if retries < 3 => { retries += 1; tokio::time::sleep(std::time::Duration::from_secs(3)).await; continue; }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    
    pub async fn analyze_codebase_with_question(&self, question: &str, codebase_files: Vec<(String, String)>, history: &[ConversationMessage]) -> Result<String, String> {
        let mut codebase_context = String::from("\n\nCODEBASE CONTEXT:\n\n");
        
        for (file_path, content) in codebase_files.iter() {
            codebase_context.push_str(&format!("=== {} ===\n{}\n\n", file_path, content));
        }
        
        let system_prompt = if !self.user_profile.is_empty() {
            format!("You are a technical interview assistant.\n\nCANDIDATE PROFILE (use ONLY when explicitly asked about the candidate's background, experience, or projects):\n{}\n\n{}\n\nIMPORTANT INSTRUCTIONS:\n- For technical questions: Focus purely on the code logic, algorithms, and best practices. DO NOT mention the candidate's projects or experience unless specifically asked.\n- For questions like 'tell me about yourself', 'your experience', 'which project', 'where did you use': Reference the candidate profile above.\n- For questions like 'how did you optimize', 'what was the performance': ONLY answer from the profile if it contains that specific information, otherwise say you need more context.", 
                self.user_profile, codebase_context)
        } else {
            format!("You are a technical interview assistant analyzing a codebase. {}\n\nProvide precise answers for bug fixes, integration tasks, or code understanding questions.", codebase_context)
        };
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        for msg in history {
            messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }
        
        messages.push(json!({
            "role": "user",
            "content": question
        }));
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
            "temperature": 0.4,
            "max_tokens": 4000
        });
        
        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        
        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        Ok(content)
    }
}

pub fn read_codebase_files(repo_path: &str) -> Vec<(String, String)> {
    use std::path::Path;
    use walkdir::WalkDir;
    
    let mut files = Vec::new();
    let path = Path::new(repo_path);
    
    if !path.exists() {
        return files;
    }
    
    let code_extensions = [
        "rs", "py", "js", "ts", "java", "cpp", "c", "h", "hpp",
        "go", "rb", "php", "cs", "swift", "kt", "scala", "sql"
    ];
    
    for entry in WalkDir::new(path)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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
                            
                            if files.len() >= 20 {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    
    files
}