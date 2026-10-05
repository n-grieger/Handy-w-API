use crate::settings::{get_settings, write_settings, AppSettings};
use tauri::AppHandle;

fn validate_provider_exists(settings: &AppSettings, provider_id: &str) -> Result<(), String> {
    if !settings
        .remote_speech_providers
        .iter()
        .any(|provider| provider.id == provider_id)
    {
        return Err(format!("Provider '{provider_id}' not found"));
    }
    Ok(())
}

/// Select which remote-speech provider to use for transcription. The special
/// "none" provider switches back to local models.
#[tauri::command]
#[specta::specta]
pub fn set_remote_speech_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let mut settings = get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.remote_speech_provider_id = provider_id;
    write_settings(&app, settings);
    Ok(())
}

/// Update the base URL for the "custom" remote-speech provider.
#[tauri::command]
#[specta::specta]
pub fn change_remote_speech_base_url(
    app: AppHandle,
    provider_id: String,
    base_url: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;

    let label = settings
        .remote_speech_provider(&provider_id)
        .map(|provider| provider.label.clone());
    let provider = settings
        .remote_speech_provider_mut(&provider_id)
        .expect("provider validated above");

    // Only the custom endpoint is user-editable; presets (e.g. "none") are not.
    if provider.id != "custom" {
        return Err(format!(
            "Provider '{}' does not allow editing the base URL",
            label.unwrap_or_default()
        ));
    }

    provider.base_url = base_url;
    write_settings(&app, settings);
    Ok(())
}

/// Store the API key for a remote-speech provider.
#[tauri::command]
#[specta::specta]
pub fn change_remote_speech_api_key(
    app: AppHandle,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.remote_speech_api_keys.insert(provider_id, api_key);
    write_settings(&app, settings);
    Ok(())
}

/// Store the model name for a remote-speech provider.
#[tauri::command]
#[specta::specta]
pub fn change_remote_speech_model(
    app: AppHandle,
    provider_id: String,
    model: String,
) -> Result<(), String> {
    let mut settings = get_settings(&app);
    validate_provider_exists(&settings, &provider_id)?;
    settings.remote_speech_models.insert(provider_id, model);
    write_settings(&app, settings);
    Ok(())
}

/// List models from a remote-speech provider's `/v1/models` endpoint so the
/// settings UI can offer a picker.
#[tauri::command]
#[specta::specta]
pub async fn fetch_remote_speech_models(
    app: AppHandle,
    provider_id: String,
) -> Result<Vec<String>, String> {
    let settings = get_settings(&app);
    let provider = settings
        .remote_speech_providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Provider '{provider_id}' not found"))?;

    if provider.base_url.trim().is_empty() {
        return Err(
            "Base URL is required to list models. Enter your endpoint's URL first.".to_string(),
        );
    }

    let api_key = settings
        .remote_speech_api_keys
        .get(&provider_id)
        .cloned()
        .unwrap_or_default();

    crate::speech_client::fetch_models(provider, &api_key).await
}
