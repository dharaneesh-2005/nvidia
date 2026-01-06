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
                    String::new()
                }
            }
            Err(_) => String::new(),
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
                form = form.text("prompt", "Technical interview question about programming, databases, algorithms, or computer science.");
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
    
    pub async fn chat_with_history(&self, _message: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let system_prompt = if self.user_profile.is_empty() {
            "You are a candidate in a technical interview. Answer questions directly as yourself. When answering:

- For OOP/OOPS: Explain Object-Oriented Programming principles
- For DBMS: Discuss Database Management Systems concepts
- For DSA: Explain Data Structures and Algorithms
- For OS: Discuss Operating Systems concepts
- For System Design: Explain architecture patterns

Be conversational and direct. Never mention you are an AI or assistant.".to_string()
        } else {
            format!("You are this candidate in a technical interview:\n\n{}\n\nAnswer all questions as this person. Use first person (I, my, me). When asked about yourself, projects, or experience, refer to the profile above. Never say you are ChatGPT, an AI, or an assistant.\n\nFor technical questions:
- For OOP/OOPS: Explain Object-Oriented Programming principles
- For DBMS: Discuss Database Management Systems concepts
- For DSA: Explain Data Structures and Algorithms
- For OS: Discuss Operating Systems concepts

Be conversational and natural like a real candidate.", self.user_profile)
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
    
    pub async fn analyze_image(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<(String, bool), String> {
        let system_prompt = if self.user_profile.is_empty() {
            "You are a technical interview assistant. Analyze this screenshot. If it contains a coding/programming problem, respond with 'CODING_PROBLEM:' followed by the problem description. Otherwise, provide a brief explanation.".to_string()
        } else {
            format!("You are a technical interview assistant helping this candidate:\n\n{}\n\nAnalyze this screenshot. If it contains a coding/programming problem, respond with 'CODING_PROBLEM:' followed by the problem description. Otherwise, provide a brief explanation.", self.user_profile)
        };
        
        let mut context = String::new();
        if !history.is_empty() {
            context.push_str("\n\nRecent conversation:\n");
            for msg in history {
                context.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
        }
        let full_prompt = format!("{}{}", system_prompt, context);
        
        let payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": full_prompt
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
            "temperature": 0.3,
            "max_tokens": 2000
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
        
        let is_coding = content.starts_with("CODING_PROBLEM:");
        Ok((content, is_coding))
    }
    
    pub async fn solve_coding_problem(&self, problem: &str, history: &[ConversationMessage]) -> Result<String, String> {
        let system_prompt = if self.user_profile.is_empty() {
            "You are a technical interview assistant solving coding problems. Provide 2 solutions in this exact format:

## Brute Force Approach
```python
# code here
```
**Time Complexity:** O(n)
**Space Complexity:** O(1)

## Optimal Approach
```python
# code here
```
**Time Complexity:** O(n)
**Space Complexity:** O(1)

Keep code concise and well-commented.".to_string()
        } else {
            format!("You are a technical interview assistant helping this candidate:\n\n{}\n\nSolve the coding problem with 2 solutions in this exact format:

## Brute Force Approach
```python
# code here
```
**Time Complexity:** O(n)
**Space Complexity:** O(1)

## Optimal Approach
```python
# code here
```
**Time Complexity:** O(n)
**Space Complexity:** O(1)

Keep code concise.", self.user_profile)
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
        
        messages.push(json!({
            "role": "user",
            "content": problem
        }));
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
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
        
        Ok(content)
    }
    
    pub async fn debug_code_error(&self, image_base64: &str, history: &[ConversationMessage]) -> Result<String, String> {
        // Step 1: Use Scout to extract error details from image
        let vision_prompt = "Extract the code and error message from this screenshot. Describe:
1. The programming language
2. The exact error message shown
3. The code that has the error
4. Any stack trace or line numbers

Be detailed and precise.";
        
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
            "max_tokens": 1500
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
        let system_prompt = if self.user_profile.is_empty() {
            "You are a technical interview assistant and code debugging expert. Provide precise fixes for code errors.".to_string()
        } else {
            format!("You are a technical interview assistant helping this candidate:\n\n{}\n\nProvide precise fixes for code errors.", self.user_profile)
        };
        
        let debug_prompt = format!("Based on this error analysis:\n\n{}\n\nProvide the fix in this format:

## Error Identified
[Brief explanation]

## Fix Required
[Specific changes needed]

## Corrected Code
```language
// Fixed code with comments
```

Be concise and precise.", error_description);
        
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
        
        messages.push(json!({
            "role": "user",
            "content": debug_prompt
        }));
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
            "temperature": 0.2,
            "max_tokens": 2000
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
            "temperature": 0.2,
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
