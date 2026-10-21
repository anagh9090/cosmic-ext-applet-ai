// Available models that can be selected via the dropdown.
// When the endpoint supports /models, the list is fetched live;
// otherwise a sensible hardcoded default is used.
pub fn available_models() -> Vec<String> {
    let settings = crate::Settings::load();

    crate::api::list_models(&settings.api_endpoint, &settings.api_key)
        .map(|models| models.into_iter().filter(|id| !id.is_empty()).collect())
        .unwrap_or_else(|_| {
            vec![
                "gpt-4o-mini".to_string(),
                "gpt-4o".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-3.5-turbo".to_string(),
            ]
        })
}
