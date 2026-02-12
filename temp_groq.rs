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
        match fs::read_to_string("profile.json") {
            Ok(content) => {
                if let Ok(profile) = serde_json::from_str::<serde_json::Value>(&content) {
                    Self::format_profile(&profile)
                } else {
                    eprintln!("⚠️  WARNING: profile.json is invalid JSON. Profile features disabled.");
                    String::new()
                }
            }
            Err(_) => {
                eprintln!("⚠️  WARNING: profile.json not found. Profile features disabled.");
                String::new()
            },
        }
    }
    
    fn format_profile(profile: &serde_json::Value) -> String {
        let mut context = String::from("CANDIDATE PROFILE:\n\n");
        
        if let Some(personal) = profile.get("personal") {
            context.push_str(&format!("Name: {}\n", personal["name"].as_str().unwrap_or("")));
            context.push_str(&format!("Location: {}\n", personal["location"].as_str().unwrap_or("")));
        }
        
        if let Some(summary) = profile.get("summary").and_then(|s| s.as_str()) {
            context.push_str(&format!("\nSummary: {}\n", summary));
        }
        
        if let Some(education) = profile.get("education").and_then(|e| e.as_array()) {
            context.push_str("\nEducation:\n");
            for edu in education {
                context.push_str(&format!("- {} from {} ({}), GPA: {}\n",
                    edu["degree"].as_str().unwrap_or(""),
                    edu["university"].as_str().unwrap_or(""),
                    edu["graduation"].as_str().unwrap_or(""),
                    edu["gpa"].as_str().unwrap_or("")));
            }
        }
        
        if let Some(experience) = profile.get("experience").and_then(|e| e.as_array()) {
            context.push_str("\nWork Experience:\n");
            for exp in experience {
                context.push_str(&format!("- {} at {} ({})\n",
                    exp["title"].as_str().unwrap_or(""),
                    exp["company"].as_str().unwrap_or(""),
                    exp["duration"].as_str().unwrap_or("")));
            }
        }
        
        if let Some(skills) = profile.get("skills") {
            context.push_str("\nSkills:\n");
            if let Some(langs) = skills["programming_languages"].as_array() {
                let lang_str: Vec<String> = langs.iter().filter_map(|l| l.as_str().map(String::from)).collect();
                context.push_str(&format!("- Languages: {}\n", lang_str.join(", ")));
            }
            if let Some(frameworks) = skills["frameworks"].as_array() {
                let fw_str: Vec<String> = frameworks.iter().filter_map(|f| f.as_str().map(String::from)).collect();
                context.push_str(&format!("- Frameworks: {}\n", fw_str.join(", ")));
            }
            if let Some(dbs) = skills["databases"].as_array() {
                let db_str: Vec<String> = dbs.iter().filter_map(|d| d.as_str().map(String::from)).collect();
                context.push_str(&format!("- Databases: {}\n", db_str.join(", ")));
            }
        }
        
        if let Some(projects) = profile.get("projects").and_then(|p| p.as_array()) {
            context.push_str("\nKey Projects:\n");
            for proj in projects {
                context.push_str(&format!("- {}: {}\n",
                    proj["name"].as_str().unwrap_or(""),
                    proj["description"].as_str().unwrap_or("")));
            }
        }
        
        context
    }

    fn is_profile_question(message_lower: &str) -> bool {
        let explicit_phrases = [
            "tell me about yourself",
            "introduce yourself",
            "walk me through your",
            "about you",
            "your background",
            "your experience",
            "your projects",
            "your project",
            "describe your project",
            "your work",
            "your resume",
            "your cv",
            "your profile",
            "your achievements",
            "your accomplishment",
            "your strengths",
            "your weakness",
            "your role",
            "your responsibilities",
            "your internship",
            "your education",
            "your degree",
            "your company",
            "where did you work",
            "where have you worked",
            "what did you do at",
            "what have you built",
            "what did you build",
            "what have you worked on",
            "what did you work on",
            "project you built",
            "project you worked on",
            "portfolio",
            "github",
            "most proud",
            "why should we hire you",
            "why do you want to join",
            "why do you want this role",
        ];

        if explicit_phrases.iter().any(|t| message_lower.contains(t)) {
            return true;
        }

        let has_you = message_lower.contains(" you ") ||
                      message_lower.contains(" your ") ||
                      message_lower.starts_with("you ") ||
                      message_lower.starts_with("your ") ||
                      message_lower.contains(" u ") ||
                      message_lower.starts_with("u ");

        let metric_keywords = [
            "how many",
            "how much",
            "how long",
            "problems",
            "problem",
            "questions",
            "leetcode",
            "codeforces",
            "codechef",
            "hackerrank",
            "contest",
            "rating",
            "rank",
            "score",
            "stars",
            "solved",
        ];

        let has_metric_intent = (message_lower.contains("how many") || message_lower.contains("how much") || message_lower.contains("how long")) &&
                                has_you &&
                                metric_keywords.iter().any(|k| message_lower.contains(k));

        if has_metric_intent {
            return true;
        }

        let personal_keywords = [
            "experience",
            "project",
            "projects",
            "work",
            "resume",
            "cv",
            "profile",
            "achievement",
            "accomplishment",
            "education",
            "degree",
            "internship",
            "company",
            "role",
            "responsibilities",
            "built",
        ];

        has_you && personal_keywords.iter().any(|k| message_lower.contains(k))
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
        let message_lower = message.to_lowercase();
        let last_msg = history.last().map(|m| m.content.to_lowercase()).unwrap_or_default();
        let topic_hint = format!("{} {}", last_msg, message_lower);
        let is_multithreading = topic_hint.contains("thread") || topic_hint.contains("concurren") || 
                                topic_hint.contains("mutex") || topic_hint.contains("lock") || 
                                topic_hint.contains("race condition") || topic_hint.contains("deadlock") ||
                                topic_hint.contains("synchroniz") || topic_hint.contains("parallel");
        let is_system_design = topic_hint.contains("design") && (topic_hint.contains("system") || 
                                topic_hint.contains("architect") || topic_hint.contains("scale"));
        let is_profile_question = Self::is_profile_question(&message_lower);
        let use_profile = !self.user_profile.is_empty() && is_profile_question;
        
        let system_prompt = if !use_profile {
            if is_multithreading {
                "You are a candidate in a technical interview answering multithreading/concurrency questions. Focus on:

- Thread safety mechanisms (mutexes, semaphores, locks)
- Race conditions and how to prevent them
- Deadlock scenarios and prevention strategies
- Synchronization primitives (monitors, condition variables)
- Thread lifecycle and management
- Concurrent data structures
- Producer-consumer patterns
- Read-write locks and atomic operations

Provide practical examples with code snippets. Be conversational and direct. Answer in a neutral, factual style. Do not mention personal background, achievements, or projects. If asked about personal details, say they are not provided. Never invent personal details. Never mention you are an AI.".to_string()
            } else if is_system_design {
                "You are a candidate in a technical interview answering system design questions. Provide:

1. High-level architecture description
2. Component breakdown with responsibilities
3. Data flow and communication patterns
4. Scalability considerations
5. Technology choices with justification
6. ASCII diagram representation when helpful

Use simple text diagrams like:
```
[Client] --> [Load Balancer] --> [App Servers]
                                      |
                                      v
                                 [Database]
```

Be conversational and direct. Answer in a neutral, factual style. Do not mention personal background, achievements, or projects. If asked about personal details, say they are not provided. Never invent personal details. Never mention you are an AI.".to_string()
            } else {
                "You are a candidate in a technical interview. Answer questions directly as yourself. When answering:

- For OOP/OOPS: Explain Object-Oriented Programming principles
- For DBMS: Discuss Database Management Systems concepts
- For DSA: Explain Data Structures and Algorithms
- For OS: Discuss Operating Systems concepts
- For System Design: Explain architecture patterns

Be conversational and direct. Answer in a neutral, factual style. Do not mention personal background, achievements, or projects. If asked about personal details, say they are not provided. Never invent personal details. Never mention you are an AI or assistant.".to_string()
            }
        } else {
            if is_multithreading {
                format!("You are this candidate in a technical interview:\n\n{}\n\nAnswer multithreading/concurrency questions as this person. Use only facts explicitly present in the profile. If a personal detail is not in the profile, say it is not provided. Focus on:

- Thread safety mechanisms (mutexes, semaphores, locks)
- Race conditions and how to prevent them
- Deadlock scenarios and prevention strategies
- Synchronization primitives
- Concurrent data structures
- Producer-consumer patterns

Provide practical examples with code. Use first person (I, my, me). Never say you are an AI.", self.user_profile)
            } else if is_system_design {
                format!("You are this candidate in a technical interview:\n\n{}\n\nAnswer system design questions as this person. Use only facts explicitly present in the profile. If a personal detail is not in the profile, say it is not provided. Provide:

1. High-level architecture description
2. Component breakdown
3. Data flow patterns
4. Scalability considerations
5. ASCII diagram representation:
```
[Client] --> [Load Balancer] --> [Servers]
                                      |
                                      v
                                 [Database]
```

Use first person (I, my, me). Never say you are an AI.", self.user_profile)
            } else {
                format!("You are this candidate in a technical interview:\n\n{}\n\nAnswer questions about yourself, projects, or experience using only the profile above. If a personal detail is not in the profile, say it is not provided. For purely technical questions, answer directly without adding personal details. Use first person (I, my, me) only when answering about yourself. Never say you are ChatGPT, an AI, or an assistant.\n\nFor technical questions:
- For OOP/OOPS: Explain Object-Oriented Programming principles
- For DBMS: Discuss Database Management Systems concepts
- For DSA: Explain Data Structures and Algorithms
- For OS: Discuss Operating Systems concepts

Be conversational and natural like a real candidate.", self.user_profile)
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
            "temperature": 0.3,
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
            "max_tokens": 5000
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
        let recent_history: Vec<_> = history.iter().rev().take(20).rev().collect();
        
        let system_prompt = "You are a technical interview coding expert. Solve problems using C++ with this EXACT format:\n\nPROBLEM UNDERSTANDING\n[Brief explanation of what the problem asks]\n\nBRUTE FORCE APPROACH\nExplanation: [How brute force works]\nTime Complexity: O(...)\nSpace Complexity: O(...)\n\n```cpp\n// Brute force C++ code with clear comments\n// Each line should be properly indented\nclass Solution {\npublic:\n    // Function implementation here\n};\n```\n\nOPTIMAL APPROACH\nExplanation: [How optimal solution works, why it's better]\nTime Complexity: O(...)\nSpace Complexity: O(...)\n\n```cpp\n// Optimal C++ code with clear comments\n// Each line should be properly indented\nclass Solution {\npublic:\n    // Function implementation here\n};\n```\n\nIMPORTANT: \n- Use proper C++ indentation (4 spaces per level)\n- Include complete class structure\n- Add meaningful comments\n- Ensure code is properly formatted with line breaks\n- Use standard LeetCode-style class structure";
        
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
            "max_tokens": 2500
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
        
        let debug_prompt = format!("Error analysis:\n{}\n{}\n\nProvide fix in this format:\n\nERROR IDENTIFIED\n[Brief explanation]\n\nFIX REQUIRED\n[Specific changes needed]\n\nCORRECTED CODE\n```cpp\n// Fixed C++ code with proper indentation\n// Each line should be properly formatted\nclass Solution {{\npublic:\n    // Corrected function implementation\n}};\n```\n\nIMPORTANT: \n- Use proper C++ indentation (4 spaces per level)\n- Include complete corrected code\n- Add comments explaining the fix\n- Ensure code is properly formatted with line breaks", error_description, context);
        
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
        
        let system_prompt = if self.user_profile.is_empty() {
            format!("You are a technical interview assistant analyzing a codebase. {}\n\nProvide precise answers for bug fixes, integration tasks, or code understanding questions.", codebase_context)
        } else {
            format!("You are a technical interview assistant helping this candidate:\n\n{}\n\n{}\n\nProvide precise answers for bug fixes, integration tasks, or code understanding questions.", self.user_profile, codebase_context)
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
