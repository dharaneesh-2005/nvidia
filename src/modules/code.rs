use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;
use serde_json::{json, Value};

pub struct CodeManager {
    project_path: PathBuf,
}

impl CodeManager {
    pub fn new(path: String) -> Self {
        Self {
            project_path: PathBuf::from(path),
        }
    }
    
    pub fn get_file_tree(&self) -> Value {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(&self.project_path)
            .max_depth(10)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy();
                    if Self::is_code_file(&ext_str) {
                        if let Ok(rel_path) = path.strip_prefix(&self.project_path) {
                            files.push(json!({
                                "path": rel_path.to_string_lossy(),
                                "name": path.file_name().unwrap().to_string_lossy(),
                                "type": ext_str,
                            }));
                        }
                    }
                }
            }
        }
        
        json!({ "files": files })
    }
    
    pub fn get_file_content(&self, relative_path: &str) -> String {
        let full_path = self.project_path.join(relative_path);
        fs::read_to_string(full_path).unwrap_or_else(|_| "Error reading file".to_string())
    }
    
    pub fn get_relevant_context(&self, query: &str) -> String {
        let mut context = String::new();
        let query_lower = query.to_lowercase();
        
        for entry in WalkDir::new(&self.project_path)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    if content.to_lowercase().contains(&query_lower) {
                        if let Ok(rel_path) = path.strip_prefix(&self.project_path) {
                            context.push_str(&format!("\n\n=== {} ===\n", rel_path.display()));
                            context.push_str(&content);
                            
                            if context.len() > 8000 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        if context.is_empty() {
            context = "No relevant code found.".to_string();
        }
        
        context
    }
    
    fn is_code_file(ext: &str) -> bool {
        matches!(
            ext,
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "c" | "cpp" | "h" | "hpp" |
            "go" | "rb" | "php" | "cs" | "swift" | "kt" | "scala" | "sh" | "bash" | "sql" |
            "html" | "css" | "json" | "yaml" | "yml" | "toml" | "xml" | "md"
        )
    }
}
