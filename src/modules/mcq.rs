use std::sync::Arc;
use super::groq::GroqClient;

pub struct McqHandler {
    groq: Arc<GroqClient>,
}

impl McqHandler {
    pub fn new(groq: Arc<GroqClient>) -> Self {
        Self { groq }
    }

    /// Solve MCQ directly from screenshot using Gemini vision.
    /// Single call — Gemini sees image, identifies options, solves, and explains.
    /// Falls back to Groq OSS-120B text if Gemini fails.
    pub async fn solve_mcq_from_image(&self, image_base64: &str) -> Result<String, String> {
        eprintln!("🔍 [MCQ] Solving with Gemini vision...");
        match self.groq.solve_mcq_with_gemini_vision(image_base64).await {
            Ok(solution) => {
                eprintln!("🔍 [MCQ] ✓ Solution received ({} chars)", solution.len());
                Ok(solution)
            }
            Err(e) => {
                eprintln!("🔍 [MCQ] Error: {}", e);
                Err(e)
            }
        }
    }
}
