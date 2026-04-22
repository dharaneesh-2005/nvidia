use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use super::groq::GroqClient;

pub struct McqHandler {
    groq: Arc<GroqClient>,
    client: Client,
}

impl McqHandler {
    pub fn new(groq: Arc<GroqClient>) -> Self {
        Self { 
            groq,
            client: Client::new(),
        }
    }
    
    fn get_api_key(&self) -> String {
        std::env::var("GROQ_API_KEY")
            .or_else(|_| std::fs::read_to_string("config.json")
                .ok()
                .and_then(|content| {
                    serde_json::from_str::<serde_json::Value>(&content).ok()
                })
                .and_then(|json| {
                    json.get("groq_api_key")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .ok_or_else(|| "API key not found".to_string())
            )
            .unwrap_or_default()
    }
    
    /// Analyze MCQ screenshot using Scout model with MCQ-specific prompt
    pub async fn analyze_mcq_image(&self, image_base64: &str) -> Result<(String, bool), String> {
        let system_prompt = "You are Scout, a technical interview vision assistant specialized in MCQ (Multiple Choice Questions). Extract information from this screenshot in JSON format.\n\nFirst, identify the TYPE:\n- MCQ_CODING: Multiple choice question with code snippet that needs to be executed\n- MCQ_THEORY: Multiple choice question about concepts/theory\n- MCQ_OUTPUT: Multiple choice question asking for code output\n- MCQ_DEBUG: Multiple choice question about finding errors in code\n- GENERAL: Other content\n\nFor MCQ_CODING (code that needs execution):\n{\n  \"type\": \"MCQ_CODING\",\n  \"question\": \"full question text\",\n  \"code\": \"extract the exact code snippet\",\n  \"language\": \"python/cpp/java/javascript\",\n  \"options\": [\"A) ...\", \"B) ...\", \"C) ...\", \"D) ...\"],\n  \"needs_execution\": true,\n  \"confidence\": 0.95\n}\n\nFor MCQ_THEORY:\n{\n  \"type\": \"MCQ_THEORY\",\n  \"question\": \"full question text\",\n  \"options\": [\"A) ...\", \"B) ...\", \"C) ...\", \"D) ...\"],\n  \"topic\": \"topic area like algorithms, databases, networking, etc.\",\n  \"needs_web_search\": true,\n  \"confidence\": 0.90\n}\n\nFor MCQ_OUTPUT:\n{\n  \"type\": \"MCQ_OUTPUT\",\n  \"question\": \"what will be the output\",\n  \"code\": \"extract the exact code\",\n  \"language\": \"python/cpp/java/javascript\",\n  \"options\": [\"A) ...\", \"B) ...\", \"C) ...\", \"D) ...\"],\n  \"needs_execution\": true,\n  \"confidence\": 0.95\n}\n\nFor MCQ_DEBUG:\n{\n  \"type\": \"MCQ_DEBUG\",\n  \"question\": \"find the error or issue\",\n  \"code\": \"extract the code with potential error\",\n  \"language\": \"python/cpp/java/javascript\",\n  \"options\": [\"A) ...\", \"B) ...\", \"C) ...\", \"D) ...\"],\n  \"confidence\": 0.90\n}\n\nIMPORTANT:\n- Extract ALL visible text accurately\n- Extract ALL options (A, B, C, D, etc.)\n- If there's code, extract it EXACTLY as shown\n- Include confidence score (0.0-1.0)\n- If confidence < 0.7, set \"needs_recapture\": true\n- Return ONLY valid JSON, no extra text";
        
        let content_items = vec![
            json!({
                "type": "text",
                "text": system_prompt
            }),
            json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", image_base64)
                }
            })
        ];
        
        let payload = json!({
            "model": "meta-llama/llama-4-scout-17b-16e-instruct",
            "messages": [
                {
                    "role": "user",
                    "content": content_items
                }
            ],
            "temperature": 0.3,
            "max_tokens": 2000
        });
        
        let api_key = self.get_api_key();
        
        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
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
            let confidence = parsed["confidence"].as_f64().unwrap_or(1.0);
            let needs_recapture = parsed["needs_recapture"].as_bool().unwrap_or(false);
            
            if needs_recapture || confidence < 0.7 {
                return Ok((format!("{{\"needs_recapture\": true, \"reason\": \"Low confidence ({:.0}%). Please recapture with better quality.\"}}", confidence * 100.0), false));
            }
            
            let is_coding = parsed["needs_execution"].as_bool().unwrap_or(false);
            Ok((content, is_coding))
        } else {
            Ok((content, false))
        }
    }
    
    /// Solve MCQ using OSS-120B with code interpreter and web search tools
    pub async fn solve_mcq(&self, mcq_data: &str) -> Result<String, String> {
        eprintln!("🔍 [MCQ] Starting MCQ solving with data: {}", &mcq_data[..mcq_data.len().min(200)]);
        
        let system_prompt = "You are an expert MCQ solver with access to browser_search and code_interpreter tools.\n\n**AVAILABLE TOOLS:**\n- browser_search: Search the web for information to verify facts\n- code_interpreter: Execute code to determine output\n\n**RESPONSE FORMAT:**\n\n## QUESTION ANALYSIS\n[Briefly explain what the question is asking - 2-3 sentences]\n\n## SOLUTION APPROACH\n\n**For MCQ with Code:**\n1. Use code_interpreter to execute the code\n2. Analyze the actual output\n3. Match with the given options\n\n**For Theory MCQ:**\n1. Use browser_search to verify facts if needed\n2. Analyze each option carefully\n3. Identify the correct answer\n\n## DETAILED ANALYSIS\n\n**Option A:** [Explain why this is correct/incorrect]\n\n**Option B:** [Explain why this is correct/incorrect]\n\n**Option C:** [Explain why this is correct/incorrect]\n\n**Option D:** [Explain why this is correct/incorrect]\n\n## CORRECT ANSWER\n\n**Answer: [Letter]**\n\n**Reasoning:**\n[Provide clear, concise reasoning for why this is the correct answer - 3-4 sentences]\n\n**Verification:**\n[If code was executed, show the output. If web search was used, cite the source]\n\n---\n\n**CRITICAL RULES:**\n1. **Tool Usage:**\n   - Use code_interpreter for questions with code to get actual output\n   - Use browser_search for theory questions to verify facts\n   - Show tool results in your verification section\n\n2. **Answer Style:**\n   - Be definitive - state the correct answer clearly\n   - Explain WHY other options are wrong\n   - Use simple, clear language\n   - Show your reasoning process\n\n3. **Accuracy:**\n   - Use tools to verify, don't guess\n   - Double-check your logic\n   - Ensure accuracy for both code and theory questions";
        
        let messages = vec![
            json!({
                "role": "system",
                "content": system_prompt
            }),
            json!({
                "role": "user",
                "content": format!("Solve this MCQ:\n\n{}", mcq_data)
            })
        ];
        
        let payload = json!({
            "model": "openai/gpt-oss-120b",
            "messages": messages,
            "temperature": 0.3,
            "max_completion_tokens": 3000,
            "top_p": 1,
            "stream": false,
            "reasoning_effort": "medium",
            "tools": [
                {
                    "type": "browser_search"
                },
                {
                    "type": "code_interpreter"
                }
            ]
        });
        
        eprintln!("🔍 [MCQ] Sending request to OSS-120B...");
        
        let api_key = self.get_api_key();
        
        let mut retries = 0;
        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(15),
                self.client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
            ).await {
                Ok(Ok(response)) => {
                    let status = response.status();
                    eprintln!("🔍 [MCQ] Response status: {}", status);
                    
                    if status.is_success() {
                        let response_text = response.text().await.map_err(|e| e.to_string())?;
                        eprintln!("🔍 [MCQ] Raw response: {}", &response_text[..response_text.len().min(500)]);
                        
                        let json: serde_json::Value = serde_json::from_str(&response_text)
                            .map_err(|e| format!("JSON parse error: {}", e))?;
                        
                        let content = json["choices"][0]["message"]["content"]
                            .as_str()
                            .unwrap_or("");
                        
                        eprintln!("🔍 [MCQ] Content length: {}", content.len());
                        
                        if content.is_empty() {
                            eprintln!("🔍 [MCQ] Empty content received! Full JSON: {}", json);
                            return Err("Empty response from API".to_string());
                        }
                        
                        eprintln!("🔍 [MCQ] Content preview: {}", &content[..content.len().min(200)]);
                        return Ok(content.to_string());
                    } else if status.is_server_error() && retries < 3 {
                        eprintln!("🔍 [MCQ] Server error, retrying... (attempt {})", retries + 1);
                        retries += 1;
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    } else {
                        let error_text = response.text().await.unwrap_or_default();
                        eprintln!("🔍 [MCQ] API error: {} - {}", status, error_text);
                        return Err(format!("API error: {} - {}", status, error_text));
                    }
                }
                Ok(Err(e)) if retries < 3 => {
                    eprintln!("🔍 [MCQ] Request failed (attempt {}): {}", retries + 1, e);
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Ok(Err(e)) => {
                    eprintln!("🔍 [MCQ] Request failed after retries: {}", e);
                    return Err(format!("API request failed: {}", e));
                }
                Err(_) if retries < 3 => {
                    eprintln!("🔍 [MCQ] Request timed out (attempt {})", retries + 1);
                    retries += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
                Err(_) => {
                    eprintln!("🔍 [MCQ] Request timed out after retries");
                    return Err("Request timed out after 15 seconds".to_string());
                }
            }
        }
    }
}
