use regex::Regex;

/// Sanitize log content to remove sensitive information
#[allow(dead_code)]
pub fn sanitize_log(content: &str) -> String {
    let patterns = [
        // API Keys
        r"sk-[a-zA-Z0-9]{20,}",
        r"key-[a-zA-Z0-9]{20,}",
        r#"api[_-]?key[=:]\s*['"]?[a-zA-Z0-9_-]+['"]?"#,
        // Anthropic
        r"ANTHROPIC_API_KEY=[^\s]+",
        // OpenAI
        r"OPENAI_API_KEY=[^\s]+",
        // Generic secrets
        r#"(password|secret|token)[=:]\s*['"]?[^\s'"]+['"]?"#,
    ];

    let mut result = content.to_string();
    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, "[REDACTED]").to_string();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_api_keys() {
        let input = "Using key sk-abcdefghijklmnopqrstuvwxyz123456";
        let output = sanitize_log(input);
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("sk-"));
    }

    #[test]
    fn test_sanitize_env_vars() {
        let input = "ANTHROPIC_API_KEY=secret123 OPENAI_API_KEY=key456";
        let output = sanitize_log(input);
        assert_eq!(output.matches("[REDACTED]").count(), 2);
    }
}
