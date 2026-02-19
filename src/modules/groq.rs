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
        
        let message_lower = message.to_lowercase();
        let last_msg = history.last().map(|m| m.content.to_lowercase()).unwrap_or_default();
        let topic_hint = format!("{} {}", last_msg, message_lower);
        let is_multithreading = topic_hint.contains("thread") || topic_hint.contains("concurren") || 
                                topic_hint.contains("mutex") || topic_hint.contains("lock") || 
                                topic_hint.contains("race condition") || topic_hint.contains("deadlock") ||
                                topic_hint.contains("synchroniz") || topic_hint.contains("parallel");
        let is_system_design = topic_hint.contains("design") && (topic_hint.contains("system") || 
                                topic_hint.contains("architect") || topic_hint.contains("scale"));
        
        let system_prompt = if !self.user_profile.is_empty() {
            if is_multithreading {
                format!("You are answering AS this candidate in a technical interview. This is YOUR profile:\n\n{}\n\nAnswer multithreading questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain how it works in 3-4 sentences\n- Give a quick practical example if helpful\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nCRITICAL RULES:\n- When asked about YOUR experience, internships, or background: You MUST use the information from YOUR profile above. This is YOUR real experience.\n- NEVER say \"I don't have experience\" if it's listed in YOUR profile above.\n- For technical concept questions (not about YOU personally), just explain the concept.\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.", self.user_profile)
            } else if is_system_design {
                format!("You are answering AS this candidate in a technical interview. This is YOUR profile:\n\n{}\n\nAnswer system design questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain the key components in 3-4 sentences\n- Mention how they work together briefly\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nCRITICAL RULES:\n- When asked about YOUR experience, internships, or background: You MUST use the information from YOUR profile above. This is YOUR real experience.\n- NEVER say \"I don't have experience\" if it's listed in YOUR profile above.\n- For technical concept questions (not about YOU personally), just explain the concept.\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.", self.user_profile)
            } else {
                format!("You are answering AS this candidate in a technical interview. This is YOUR complete profile in JSON format:\n\n{}\n\nAnswer questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain how it works in 3-4 sentences\n- Give a quick example if helpful\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nCRITICAL RULES:\n- When asked about YOUR experience, internships, projects, or background: You MUST use the information from YOUR profile JSON above. This is YOUR real experience.\n- NEVER say \"I don't have experience\" if it's in YOUR profile JSON above.\n- For technical concept questions (not about YOU personally), just explain the concept.\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.", self.user_profile)
            }
        } else {
            if is_multithreading {
                "You are a candidate in a technical interview. Answer multithreading questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain how it works in 3-4 sentences\n- Give a quick practical example if helpful\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.".to_string()
            } else if is_system_design {
                "You are a candidate in a technical interview. Answer system design questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain the key components in 3-4 sentences\n- Mention how they work together briefly\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.".to_string()
            } else {
                "You are a candidate in a technical interview. Answer questions in a simple, conversational way - like explaining to a friend or classmate. Use Indian English style.\n\nKEEP IT CONVERSATIONAL:\n- Start with a simple 1-line explanation of what it is\n- Then explain how it works in 3-4 sentences\n- Give a quick example if helpful\n- Use everyday language, not textbook words\n- Speak naturally like you're having a conversation\n\nNEVER use tables. Use bullet points only when listing things. Keep it natural and speakable.".to_string()
            }
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
    
    async fn analyze_images_internal(&self, images: &[&str], history: &[ConversationMessage]) -> Result<(String, bool), String> {
        if images.is_empty() {
            return Err("No images provided".to_string());
        }
        let recent_history: Vec<_> = history.iter().rev().take(20).rev().collect();
        
        let mut system_prompt = String::from("You are Scout, a technical interview vision assistant. Extract information from this screenshot in JSON format.\n\nFirst, identify the TYPE:\n- DSA_PROBLEM: Coding problem (LeetCode/HackerRank style) with input/output examples\n- SYSTEM_DESIGN: Architecture/design question\n- LOGICAL_PUZZLE: Text-based reasoning problem\n- DEBUG_ERROR: Code with error messages\n- GENERAL: Other technical content\n\nFor DSA_PROBLEM, extract:\n{\n  \"type\": \"DSA_PROBLEM\",\n  \"title\": \"problem name\",\n  \"description\": \"full problem statement\",\n  \"input_format\": \"how input is given\",\n  \"output_format\": \"expected output format\",\n  \"constraints\": [\"list of constraints\"],\n  \"examples\": [{\"input\": \"...\", \"output\": \"...\", \"explanation\": \"...\"}],\n  \"confidence\": 0.95\n}\n\nFor SYSTEM_DESIGN:\n{\n  \"type\": \"SYSTEM_DESIGN\",\n  \"question\": \"design question\",\n  \"requirements\": [\"list of requirements\"],\n  \"confidence\": 0.90\n}\n\nFor DEBUG_ERROR:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"code\": \"extracted code only\",\n  \"error_message\": \"error text\",\n  \"confidence\": 0.85\n}\n\nFor GENERAL:\n{\n  \"type\": \"GENERAL\",\n  \"content\": \"description of screenshot\",\n  \"confidence\": 0.80\n}\n\nIMPORTANT:\n- Extract ALL visible text accurately\n- Include confidence score (0.0-1.0)\n- If confidence < 0.7, set \"needs_recapture\": true\n- Ignore UI elements, focus on problem content\n- Return ONLY valid JSON, no extra text");
        
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
    
    pub async fn answer_mcq_direct(&self, image_base64: &str) -> Result<String, String> {
        let prompt = "Always mention option number along with the answer";
        
        let payload = json!({
            "model": "meta-llama/llama-4-maverick-17b-128e-instruct",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
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
            "temperature": 0.5,
            "max_tokens": 1000,
            "top_p": 1,
            "stream": false
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
    
    pub async fn solve_coding_problem(&self, problem: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(20).rev().collect();
        
        let system_prompt = "You are a technical interview coding expert. Explain solutions in simple, conversational Indian English - like explaining to a classmate.\n\nProvide the solution in this EXACT format:\n\nBRUTE FORCE APPROACH\nIntuition: [Explain the basic idea in 2-3 simple sentences. What's the straightforward way to solve this?]\nTime: O(...)\nSpace: O(...)\n```cpp\nclass Solution {\npublic:\n    // Complete brute force implementation\n};\n```\n\nOPTIMAL APPROACH\nIntuition: [Explain the better idea in 2-3 simple sentences. What's the key insight that makes it faster?]\nTime: O(...)\nSpace: O(...)\n```cpp\nclass Solution {\npublic:\n    // Complete optimal implementation\n};\n```\n\nSUMMARY\n[In 2-3 sentences: Compare both approaches. Why is brute force slow? Why is optimal better?]\n\nIMPORTANT:\n- Keep intuition simple and conversational - speak naturally\n- Write complete, working C++ code\n- Use clear variable names\n- Add brief comments in code if helpful\n- Make it easy to understand and speak out loud";
        
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
            "max_tokens": 2000,
            "tools": [{"type": "code_interpreter"}, {"type": "browser_search"}]
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
    
    pub async fn debug_code_error(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let recent_history: Vec<_> = history.iter().rev().take(20).rev().collect();
        
        let vision_prompt = "Extract code and error from this screenshot in JSON format:\n{\n  \"type\": \"DEBUG_ERROR\",\n  \"error_category\": \"COMPILATION/RUNTIME/TLE/WRONG_OUTPUT\",\n  \"language\": \"C++/Python/Java\",\n  \"code\": \"extracted code only, ignore UI\",\n  \"error_message\": \"exact error text\",\n  \"test_case_info\": \"if TLE or wrong output\",\n  \"confidence\": 0.90\n}\n\nIMPORTANT: Extract ONLY the code, ignore buttons, menus, UI elements. Return valid JSON only.";
        
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
        
        let debug_prompt = format!("Error analysis:\n{}\n{}\n\nProvide fix in this format:\n\nERROR IDENTIFIED\n[Brief explanation]\nFIX REQUIRED\n[Specific changes]\nCORRECTED CODE\n```cpp\n// Fixed C++ code\nclass Solution {{\npublic:\n    // Corrected implementation\n}};\n```\n\nIMPORTANT:\n- NEVER use tables, always use bullet points\n- Keep minimal spacing\n- Format as bullet points for easy verbal delivery", error_description, context);
        
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
            "max_tokens": 18801,
            "tools": [{"type": "code_interpreter"}, {"type": "browser_search"}]
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
            "max_tokens": 18801,
            "tools": [{"type": "code_interpreter"}, {"type": "browser_search"}]
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