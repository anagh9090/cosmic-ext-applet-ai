// Available models that can be selected via dropdown.
// These are common OpenAI-compatible models.
pub fn available_models() -> Vec<String> {
    vec![
        "gpt-4o-mini".to_string(),
        "gpt-4o".to_string(),
        "gpt-4-turbo".to_string(),
        "gpt-3.5-turbo".to_string(),
    ]
}
