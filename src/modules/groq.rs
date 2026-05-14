use reqwest::Client;
use serde_json::json;
use std::fs;

#[derive(Clone, Debug)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

pub struct GroqClient {
    client: Client,
    api_key: String,
    user_profile: String,
}

impl GroqClient {
    pub fn new(api_key: String) -> Self {
        let user_profile = Self::load_profile();
        Self {
            client: Client::new(),
            api_key,
            user_profile,
        }
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
                form = form.text("prompt", p.to_string());
            } else {
                form = form.text("prompt", "Indian English accent. Technical interview question about programming, databases, algorithms, or computer science.");
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
                "You are helping a CS student named Dharaneesh answer questions in a technical interview. \
                Speak AS him, using his real background below.\n\n\
                PROFILE:\n{}\n\n\
                HOW TO ANSWER:\n\
                - Start with one clear sentence that directly answers the question\n\
                - Then explain in 3-5 natural sentences — like talking to someone face to face\n\
                - Use simple words. If you must use a technical term, explain it in the same breath\n\
                - Give one small real-world example if it makes it clearer\n\
                - Stop there. Do not summarize. Do not repeat.\n\n\
                VOICE STYLE:\n\
                - Speak like a confident final-year engineering student from Tamil Nadu\n\
                - Natural connectors: 'So basically', 'The thing is', 'What happens here is', 'In simple terms'\n\
                - Avoid: 'Furthermore', 'Moreover', 'It is worth noting', 'In conclusion'\n\
                - If asked about YOUR experience or projects: use ONLY what is in the profile above\n\
                - If it is a concept question: just explain the concept simply, no need to tie it to the profile\n\n\
                LENGTH RULE: Your answer must be speakable in under 100 seconds. \
                If it takes longer, you have said too much.",
                self.user_profile
            )
        } else {
            "You are helping a CS student answer questions in a technical interview.\n\n\
            HOW TO ANSWER:\n\
            - Start with one clear sentence that directly answers the question\n\
            - Then explain in 3-5 natural sentences — like talking to someone face to face\n\
            - Use simple words. If you must use a technical term, explain it in the same breath\n\
            - Give one small real-world example if it makes it clearer\n\
            - Stop there. Do not summarize. Do not repeat.\n\n\
            VOICE STYLE:\n\
            - Speak like a confident final-year engineering student from Tamil Nadu\n\
            - Natural connectors: 'So basically', 'The thing is', 'What happens here is', 'In simple terms'\n\
            - Avoid: 'Furthermore', 'Moreover', 'It is worth noting', 'In conclusion'\n\n\
            LENGTH RULE: Your answer must be speakable in under 100 seconds. \
            If it takes longer, you have said too much.".to_string()
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
        
        let payload = json!({
            "model": "openai/gpt-oss-20b",
            "messages": messages,
            "temperature": 0.5,
            "max_tokens": 1500
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
    
    pub async fn chat(&self, message: &str) -> Result<String, String> {
        self.chat_with_history(message, &[]).await
    }
    
    /// Chat with both conversation history AND candidate context
    /// Structures the prompt with separate windows:
    /// - Profile (ALWAYS FULL)
    /// - Interviewer questions summary + recent
    /// - Candidate answers summary + recent
    pub async fn chat_with_context(&self, message: &str, history: &[ConversationMessage], candidate_context: &[String]) -> Result<String, String> {
        // Separate interviewer questions from conversation history
        let interviewer_questions: Vec<&str> = history.iter()
            .filter(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .collect();
        
        // Build interviewer questions section (summarize if too many)
        let interviewer_section = if interviewer_questions.len() > 5 {
            let old_questions = &interviewer_questions[..interviewer_questions.len() - 5];
            let recent_questions = &interviewer_questions[interviewer_questions.len() - 5..];
            
            let old_summary = old_questions.iter()
                .map(|q| format!("- {}", q.chars().take(80).collect::<String>()))
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "INTERVIEWER'S PREVIOUS QUESTIONS (summary):\n{}\n\nINTERVIEWER'S RECENT QUESTIONS:\n{}",
                old_summary,
                recent_questions.iter()
                    .enumerate()
                    .map(|(i, q)| format!("{}. {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if !interviewer_questions.is_empty() {
            format!(
                "INTERVIEWER'S QUESTIONS SO FAR:\n{}",
                interviewer_questions.iter()
                    .enumerate()
                    .map(|(i, q)| format!("{}. {}", i + 1, q))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            String::new()
        };
        
        // Build candidate answers section (summarize if too many)
        let candidate_section = if candidate_context.len() > 5 {
            let old_answers = &candidate_context[..candidate_context.len() - 5];
            let recent_answers = &candidate_context[candidate_context.len() - 5..];
            
            let old_summary = old_answers.iter()
                .map(|a| format!("- {}", a.chars().take(80).collect::<String>()))
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "CANDIDATE'S PREVIOUS ANSWERS (summary):\n{}\n\nCANDIDATE'S RECENT ANSWERS:\n{}",
                old_summary,
                recent_answers.iter()
                    .enumerate()
                    .map(|(i, a)| format!("{}. {}", i + 1, a))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if !candidate_context.is_empty() {
            format!(
                "CANDIDATE'S ANSWERS SO FAR:\n{}",
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
                "You are helping a CS student named Dharaneesh answer questions in a technical interview. \
                Speak AS him, using his real background below.\n\n\
                PROFILE (FULL - use for any question about experience/projects):\n{}\n\
                {}\
                HOW TO ANSWER:\n\
                - CRITICAL: The candidate has already spoken the answers listed above. Build upon what they said.\n\
                - If the interviewer asks a follow-up, reference what the candidate already mentioned.\n\
                - Start with one clear sentence that directly answers the question\n\
                - Then explain in 3-5 natural sentences — like talking to someone face to face\n\
                - Use simple words. If you must use a technical term, explain it in the same breath\n\
                - Give one small real-world example if it makes it clearer\n\
                - Stop there. Do not summarize. Do not repeat.\n\n\
                VOICE STYLE:\n\
                - Speak like a confident final-year engineering student from Tamil Nadu\n\
                - Natural connectors: 'So basically', 'The thing is', 'What happens here is', 'In simple terms'\n\
                - Avoid: 'Furthermore', 'Moreover', 'It is worth noting', 'In conclusion'\n\
                - If asked about YOUR experience or projects: use ONLY what is in the profile above\n\
                - If it is a concept question: just explain the concept simply\n\n\
                LENGTH RULE: Your answer must be speakable in under 100 seconds. \
                If it takes longer, you have said too much.",
                self.user_profile,
                context_block
            )
        } else {
            format!(
                "You are helping a CS student answer questions in a technical interview.\n\
                {}\
                HOW TO ANSWER:\n\
                - CRITICAL: The candidate has already spoken the answers listed above. Build upon what they said.\n\
                - If the interviewer asks a follow-up, reference what the candidate already mentioned.\n\
                - Start with one clear sentence that directly answers the question\n\
                - Then explain in 3-5 natural sentences — like talking to someone face to face\n\
                - Use simple words. If you must use a technical term, explain it in the same breath\n\
                - Give one small real-world example if it makes it clearer\n\
                - Stop there. Do not summarize. Do not repeat.\n\n\
                VOICE STYLE:\n\
                - Speak like a confident final-year engineering student from Tamil Nadu\n\
                - Natural connectors: 'So basically', 'The thing is', 'What happens here is', 'In simple terms'\n\
                - Avoid: 'Furthermore', 'Moreover', 'It is worth noting', 'In conclusion'\n\n\
                LENGTH RULE: Your answer must be speakable in under 100 seconds. \
                If it takes longer, you have said too much.",
                context_block
            )
        };
        
        let mut messages = vec![json!({
            "role": "system",
            "content": system_prompt
        })];
        
        // Add only recent conversation history (last 6 exchanges to save tokens)
        let recent_history: Vec<_> = history.iter().rev().take(12).rev().collect();
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
        
        let payload = json!({
            "model": "openai/gpt-oss-20b",
            "messages": messages,
            "temperature": 0.5,
            "max_tokens": 1500
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
    
    pub async fn solve_coding_problem(&self, problem: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(25).rev().collect();
        
        let system_prompt = "You are a technical interview expert specializing in DSA problems. Provide clear, structured, and detailed explanations.\n\n**PRIMARY USE CASE: DSA PROBLEMS (90%)**\nFor coding/algorithm questions, use this EXACT format:\n\n## PROBLEM UNDERSTANDING\n[Explain what the problem is asking in simple words - 2-3 sentences]\n\n## APPROACH 1: BRUTE FORCE\n\n**Intuition:**\n[Explain the straightforward approach in simple, conversational language - like explaining to a friend. Use 3-4 sentences.]\n\n**How it works:**\n- Step 1: [Explain first step]\n- Step 2: [Explain second step]\n- Step 3: [Continue...]\n\n**Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// If predefined structure exists (class Solution), use it EXACTLY\n// Otherwise use: int main() { int t; cin >> t; while(t--) { } }\n```\n\n**Complexity Analysis:**\n- Time Complexity: O(n²) where n = size of input array\n- Space Complexity: O(1) where we use constant extra space\n\n---\n\n## APPROACH 2: OPTIMAL SOLUTION\n\n**Intuition:**\n[Explain the optimized approach in simple words. What's the key insight that makes it faster? 3-4 sentences.]\n\n**How it works:**\n- Step 1: [Explain optimization step 1]\n- Step 2: [Explain optimization step 2]\n- Step 3: [Continue...]\n\n**Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// Optimized implementation\n// Use EXACT predefined structure if given\n```\n\n**Complexity Analysis:**\n- Time Complexity: O(n) where n = size of input array\n- Space Complexity: O(n) where n = space used for hash map\n\n---\n\n## COMPARISON\n[Compare both approaches - which is better and why? When to use which? 2-3 sentences]\n\n---\n\n**EDGE CASES (10%): System Design / General Questions**\nIf the question is NOT a DSA problem (system design, conceptual, etc.), provide a clear conversational answer with:\n- Simple explanation\n- Key points as bullet points\n- Examples if helpful\n\n---\n\n**CRITICAL RULES:**\n1. **Code Structure:**\n   - Use #include <bits/stdc++.h> and using namespace std;\n   - Do NOT use ios::sync_with_stdio(false), cin.tie(NULL), or fast I/O\n   - If predefined structure exists (class Solution { public: ... }), use it EXACTLY\n   - If NO predefined structure, use: int main() { int t; cin >> t; while(t--) { } return 0; }\n   - NEVER mix class-based and main()-based approaches\n\n2. **Complexity Explanation:**\n   - ALWAYS explain what each variable in O() notation means\n   - Example: O(n*m) where n=rows, m=columns\n   - Example: O(V+E) where V=vertices, E=edges\n   - Use simple, clear language\n\n3. **Code Style:**\n   - Keep code SIMPLE and readable\n   - Use short variable names (n, m, i, j, x, y)\n   - Add brief comments for clarity\n   - Follow the predefined structure if given\n\n4. **Explanation Style:**\n   - Write like you're explaining to a friend\n   - Use conversational, simple language\n   - Break down complex ideas into steps\n   - Focus on WHY, not just WHAT\n\n5. **Tools:**\n   - Use browser_search to find optimal solutions\n   - Use code_interpreter to verify logic\n   - ALWAYS provide the MOST efficient solution";
        
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
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
            "temperature": 0.4,
            "max_tokens": 4000
        });
        
        let mut retries = 0;
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(8),
                self.client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", self.api_key))
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
                    eprintln!("Request timed out after 8 seconds (attempt {})", retries + 1);
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Err(_) => return Err("Request timed out after 8 seconds".to_string()),
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
        let error_description = vision_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        // Step 2: Use GPT-OSS-120B to provide fix
        let system_prompt = "You are a technical interview C++ debugging expert. Provide precise fixes.";
        
        let mut context = String::new();
        if !recent_history.is_empty() {
            context.push_str("\n\nRecent conversation (last 20 messages):\n");
            for msg in &recent_history {
                context.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
        }
        
        let debug_prompt = format!("Error analysis from screenshot:\n{}\n{}\n\nProvide debugging solution in this EXACT format:\n\n## WHY THE ERROR OCCURRED\n[Explain in simple words what caused this error - 2-3 sentences]\n\n## APPROACH 1: QUICK FIX (Minimal Changes)\n\n**What to change:**\n[Explain the small correction needed]\n\n**Corrected Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// CHANGED: [explain what changed] - OLD: [old code] -> NEW: [new code]\n// Rest of code remains same\n```\n\n**Complexity:**\n- Time: O(...) where [explain variables]\n- Space: O(...) where [explain variables]\n\n---\n\n## APPROACH 2: OPTIMAL FIX (Better Solution)\n\n**What to improve:**\n[Explain the better approach]\n\n**Optimized Code:**\n```cpp\n#include <bits/stdc++.h>\nusing namespace std;\n\n// Optimized implementation\n```\n\n**Complexity:**\n- Time: O(...) where [explain variables]\n- Space: O(...) where [explain variables]\n\n---\n\n## COMPARISON\n[Compare both fixes - when to use which]\n\n---\n\n**CRITICAL RULES:**\n1. **For Quick Fix:** Mark changes with comments like // CHANGED: [what changed]\n2. **Code Structure:**\n   - Use #include <bits/stdc++.h> and using namespace std;\n   - Do NOT use ios::sync_with_stdio(false), cin.tie(NULL), or fast I/O\n   - If predefined structure exists, use it EXACTLY\n   - Otherwise use: int main() {{ int t; cin >> t; while(t--) {{ }} }}\n3. **Explanation Style:**\n   - Explain WHY the error happened\n   - Show WHAT changed in the code\n   - Use simple, conversational language\n4. **Complexity:**\n   - ALWAYS explain what each variable means\n   - Example: O(n) where n = size of array", error_description, context);
        
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