use crate::settings::RemoteSpeechProvider;
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};

/// Reuse the transport error-diagnostics helpers that the LLM client already
/// implements (kinds, source chain, URL sanitization, log-safe reporting).
/// They are shared across the two API clients so diagnostics stay consistent.
pub(crate) use crate::llm_client::{report_reqwest_error, sanitized_url, sanitized_url_for_log};

/// Transcribe recorded audio through an OpenAI-compatible speech endpoint
/// (the `/v1/audio/transcriptions` route, i.e. the Whisper HTTP API).
///
/// `audio` is 16 kHz mono f32 PCM. It is encoded to 16-bit WAV in memory and
/// POSTed as multipart/form-data to `{base_url}/audio/transcriptions`. The
/// OpenAI-compatible response is `{ "text": "..." }`, which is what this
/// returns.
pub async fn transcribe_audio(
    provider: &RemoteSpeechProvider,
    api_key: &str,
    model: &str,
    audio: Vec<f32>,
    language: Option<String>,
) -> Result<String, String> {
    if audio.is_empty() {
        debug!("Remote speech transcription skipped: no audio samples");
        return Ok(String::new());
    }
    if model.trim().is_empty() {
        return Err(
            "Remote speech model is not configured. Set a model (e.g. whisper-large-v3) in Settings."
                .to_string(),
        );
    }

    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{base_url}/audio/transcriptions");
    debug!(
        "Sending remote speech transcription request to: {}",
        sanitized_url_for_log(&url)
    );

    let wav_bytes = encode_wav(audio.as_slice());

    // Some servers validate the upload by its MIME type rather than the bytes,
    // so we set an explicit `audio/wav` content type on the file part.
    let audio_part = reqwest::multipart::Part::bytes(wav_bytes)
        .mime_str("audio/wav")
        .map_err(|e| format!("Invalid MIME type for audio part: {e}"))?
        .file_name("audio.wav");

    let mut form = reqwest::multipart::Form::new()
        .part("file", audio_part)
        .text("model", model.trim().to_string());

    // Only send `language` when the user pinned one; omit it for "auto" so the
    // server can detect language itself.
    if let Some(lang) = language.filter(|l| !l.trim().is_empty() && l.trim() != "auto") {
        form = form.text("language", lang.trim().to_string());
    }

    let client = build_client(api_key)?;

    let response = client
        .post(&url)
        .multipart(form)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| report_reqwest_error("HTTP request failed", &e))?;
    let status = response.status();
    debug!(
        "Remote speech response received with status {} over {:?} from {}",
        status,
        response.version(),
        sanitized_url(response.url())
    );

    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| report_reqwest_error("Failed to read API error response", &e));
        return Err(format!(
            "Remote speech API request failed with status {status}: {error_text}"
        ));
    }

    // OpenAI returns a plain JSON object with a `text` field. Some servers
    // return a bare string instead, so be tolerant of both.
    let body = response
        .text()
        .await
        .map_err(|e| report_reqwest_error("Failed to read transcription response", &e))?;

    let transcription = parse_transcription(&body)
        .ok_or_else(|| "Failed to parse remote speech API response".to_string())?;

    let text = transcription.trim().to_string();
    debug!(
        "Remote speech transcription succeeded. Output length: {} chars",
        text.len()
    );
    Ok(text)
}

/// Fetch available models from an OpenAI-compatible `/v1/models` endpoint so the
/// settings UI can offer a model picker. Returns model IDs, or an error if the
/// endpoint is unavailable.
pub async fn fetch_models(
    provider: &RemoteSpeechProvider,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{base_url}/models");
    debug!(
        "Fetching remote speech models from: {}",
        sanitized_url_for_log(&url)
    );

    let client = build_client(api_key)?;
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| report_reqwest_error("Failed to fetch models", &e))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| report_reqwest_error("Failed to read model list error", &e));
        return Err(format!(
            "Model list request failed ({status}): {error_text}"
        ));
    }

    let parsed: serde_json::Value = response
        .json()
        .await
        .map_err(|e| report_reqwest_error("Failed to parse model list response", &e))?;

    let mut models = Vec::new();
    // OpenAI shape: { "data": [ { "id": "..." }, ... ] }
    if let Some(data) = parsed.get("data").and_then(|d| d.as_array()) {
        for entry in data {
            if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                models.push(id.to_string());
            }
        }
    }
    // Fallback: a bare array of strings.
    else if let Some(array) = parsed.as_array() {
        for entry in array {
            if let Some(model) = entry.as_str() {
                models.push(model.to_string());
            }
        }
    }

    Ok(models)
}

fn build_client(api_key: &str) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Handy/1.0 (+https://github.com/cjpais/Handy)"),
    );
    if !api_key.trim().is_empty() {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
                .map_err(|e| format!("Invalid authorization header value: {e}"))?,
        );
    }
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| report_reqwest_error("Failed to build HTTP client", &e))
}

/// Parse the OpenAI-compatible transcription response. Accepts the documented
/// `{ "text": "..." }` object as well as a bare JSON string.
fn parse_transcription(body: &str) -> Option<String> {
    let value: serde_json::Result<serde_json::Value> = serde_json::from_str(body.trim());

    match value {
        Ok(serde_json::Value::String(s)) => Some(s),
        Ok(obj) => obj.get("text").and_then(|t| t.as_str()).map(str::to_string),
        Err(_) => None,
    }
}

/// Encode 16 kHz mono f32 samples as a 16-bit little-endian PCM WAV file in
/// memory. Uses a hand-written RIFF/WAV writer so the client has no extra
/// runtime dependency (the app already has `hound`, but it only writes to a
/// path, not to a `Vec<u8>`).
pub(crate) fn encode_wav(samples: &[f32]) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 16_000;
    const NUM_CHANNELS: u16 = 1;
    const BITS_PER_SAMPLE: u16 = 16;

    let data_bytes = samples.len() * (BITS_PER_SAMPLE as usize / 8);
    let byte_rate = (SAMPLE_RATE * NUM_CHANNELS as u32 * BITS_PER_SAMPLE as u32 / 8) as u32;
    let block_align = (NUM_CHANNELS as u32 * BITS_PER_SAMPLE as u32 / 8) as u16;

    let mut out = Vec::with_capacity(44 + data_bytes);
    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format: PCM
    out.extend_from_slice(&NUM_CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0f32, 1.0f32);
        let i16_value = (clamped * i16::MAX as f32) as i16;
        out.extend_from_slice(&i16_value.to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn provider(base_url: &str) -> RemoteSpeechProvider {
        RemoteSpeechProvider {
            id: "custom".to_string(),
            label: "Custom".to_string(),
            base_url: base_url.to_string(),
        }
    }

    /// Read an HTTP request (headers + exactly Content-Length body bytes) so we
    /// never deadlock waiting on a client that is itself waiting for our
    /// response. Returns the raw bytes received.
    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut tmp = [0_u8; 4096];
        let mut body_left: Option<u64> = None;

        loop {
            let n = stream.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);

            if body_left.is_none() {
                if let Some(header_end) = find_header_end(&buf) {
                    let headers = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
                    if let Some(pos) = headers.find("content-length:") {
                        let rest = &headers[pos + "content-length:".len()..];
                        // The value runs to the end of its line.
                        let value = rest.split('\n').next().unwrap_or("").trim();
                        if let Ok(cl) = value.parse::<u64>() {
                            body_left = Some(cl);
                        }
                    }
                }
            }

            if let Some(cl) = body_left {
                if let Some(header_end) = find_header_end(&buf) {
                    let body_have = (buf.len() as u64) - ((header_end + 4) as u64);
                    if body_have >= cl {
                        break;
                    }
                }
            }
        }
        buf
    }

    fn find_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    /// Serve one fixed HTTP response on an ephemeral port and return the base
    /// URL (no path) the client should hit.
    async fn serve_response(status: &str, content_type: &str, body: &str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = read_request(&mut stream).await;
            let _ = stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}")
    }

    #[test]
    fn encode_wav_produces_valid_header() {
        let samples = vec![0.0f32, 0.5, -0.5, 1.0, -1.0];
        let wav = encode_wav(&samples);

        // "RIFF" + size + "WAVE" + "fmt " + 16 + 16-byte fmt + "data" + size
        let expected_data = samples.len() * 2;
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        // total data bytes
        assert_eq!(wav.len(), 44 + expected_data);
        // sample rate (16000) little-endian at offset 24
        let rate = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
        assert_eq!(rate, 16_000);

        // 0.5 -> 16383, -0.5 -> -16384 (approx), check first two samples
        let s0 = i16::from_le_bytes([wav[44], wav[45]]);
        let s1 = i16::from_le_bytes([wav[46], wav[47]]);
        assert_eq!(s0, 0);
        assert!((s1 as i32 - 16383).abs() <= 2);
    }

    #[test]
    fn parse_transcription_handles_object() {
        assert_eq!(
            parse_transcription(r#"{"text":"hello world","task":"transcribe"}"#),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn parse_transcription_handles_bare_string() {
        assert_eq!(
            parse_transcription(r#""just text""#),
            Some("just text".to_string())
        );
    }

    #[test]
    fn parse_transcription_rejects_unexpected() {
        assert_eq!(parse_transcription("not json"), None);
        assert_eq!(parse_transcription("null"), None);
        assert_eq!(parse_transcription(r#"{"foo":"bar"}"#), None);
    }

    #[tokio::test]
    async fn transcribe_audio_hits_endpoint_and_parses_text() {
        let base_url =
            serve_response("200 OK", "application/json", r#"{"text":"the answer"}"#).await;
        let provider = provider(&base_url);

        let result = transcribe_audio(
            &provider,
            "",
            "whisper-large-v3",
            vec![0.0; 16000],
            Some("auto".to_string()),
        )
        .await
        .expect("transcription should succeed");

        assert_eq!(result, "the answer");
    }

    #[tokio::test]
    async fn transcribe_audio_sends_multipart_and_auth() {
        // Capture the raw request so we can assert the multipart + auth shape.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response =
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\nConnection: close\r\n\r\n{\"text\":\"ok\"}";
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let buf = read_request(&mut stream).await;
            let head = String::from_utf8_lossy(&buf).to_string();
            let _ = tx.send(head);
            let _ = stream.write_all(response.as_bytes()).await.unwrap();
        });
        let base_url = format!("http://{address}");
        let provider = provider(&base_url);

        let result = transcribe_audio(
            &provider,
            "sk-test-key-123",
            "whisper-large-v3",
            vec![0.1; 3200],
            Some("en".to_string()),
        )
        .await
        .expect("transcription should succeed");
        assert_eq!(result, "ok");

        let request_head = rx.await.unwrap();
        // Compare case-insensitively: reqwest emits header names in lowercase.
        let lower = request_head.to_lowercase();
        // Multipart body with a file part and the model field
        assert!(lower.contains("multipart/form-data; boundary="));
        assert!(lower.contains("name=\"file\""));
        assert!(lower.contains("name=\"model\""));
        assert!(lower.contains("name=\"language\""));
        // The language form value is sent as a plain text part ("en"), not JSON.
        assert!(lower.contains("en"));
        assert!(lower.contains("whisper-large-v3"));
        assert!(lower.contains("authorization: bearer sk-test-key-123"));
        assert!(lower.contains("audio.wav"));
    }

    #[tokio::test]
    async fn transcribe_audio_surfaces_http_errors() {
        let base_url =
            serve_response("404 Not Found", "application/json", r#"{"error":"nope"}"#).await;
        let provider = provider(&base_url);

        let err = transcribe_audio(&provider, "", "m", vec![0.0; 16000], None)
            .await
            .expect_err("should surface the 404");
        assert!(err.contains("404"), "error should mention status: {err}");
    }

    #[tokio::test]
    async fn transcribe_audio_rejects_missing_model() {
        let provider = provider("http://127.0.0.1:1");
        let err = transcribe_audio(&provider, "", "  ", vec![0.0; 16000], None)
            .await
            .expect_err("empty model must be rejected before any request");
        assert!(
            err.contains("model"),
            "error should mention the model: {err}"
        );
    }
}
