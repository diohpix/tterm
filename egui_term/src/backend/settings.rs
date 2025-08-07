use std::path::PathBuf;
use std::collections::HashMap;

const DEFAULT_SHELL: &str = "/bin/bash";

#[derive(Debug, Clone)]
pub struct BackendSettings {
    pub shell: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub env: HashMap<String, String>,
}

impl Default for BackendSettings {
    fn default() -> Self {
        let mut env = HashMap::new();
        
        // Ensure UTF-8 locale is properly set for Korean/Unicode support
        env.insert("LANG".to_string(), "en_US.UTF-8".to_string());
        env.insert("LC_ALL".to_string(), "en_US.UTF-8".to_string());
        env.insert("LC_CTYPE".to_string(), "en_US.UTF-8".to_string());
        
        Self {
            shell: DEFAULT_SHELL.to_string(),
            args: vec![],
            working_directory: None,
            env,
        }
    }
}
