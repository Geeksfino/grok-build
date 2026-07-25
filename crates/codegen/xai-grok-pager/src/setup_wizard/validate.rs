use std::time::Duration;

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateRequest {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub api_backend: String,
    pub auth_not_required: bool,
    pub extra_headers: IndexMap<String, String>,
}

pub async fn validate_provider_endpoint(request: &ValidateRequest) -> Result<()> {
    let base_url = request.base_url.trim();
    if base_url.is_empty() {
        bail!("Provider base URL is required");
    }
    let model = request.model.trim();
    if model.is_empty() {
        bail!("Provider model is required");
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("failed to build provider validation client")?;
    let headers = build_headers(request)?;
    let models_url = format!("{}/models", base_url.trim_end_matches('/'));
    let response = client
        .get(&models_url)
        .headers(headers.clone())
        .send()
        .await
        .with_context(|| format!("failed to reach {models_url}"))?;

    if response.status().is_success() {
        return Ok(());
    }

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        probe_generation_endpoint(&client, request, headers).await?;
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("Validation failed with {}: {}", status, body_snippet(&body));
}

fn build_headers(request: &ValidateRequest) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    for (name, value) in &request.extra_headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .with_context(|| format!("bad header name {name}"))?;
        let header_value =
            HeaderValue::from_str(value).with_context(|| format!("bad header value for {name}"))?;
        headers.insert(header_name, header_value);
    }
    if request.auth_not_required {
        return Ok(headers);
    }
    let Some(api_key) = request
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    else {
        bail!("API key required to validate this provider");
    };
    if request.api_backend == "messages" {
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(api_key).context("invalid API key header value")?,
        );
    } else {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .context("invalid Authorization header value")?,
        );
    }
    Ok(headers)
}

async fn probe_generation_endpoint(
    client: &reqwest::Client,
    request: &ValidateRequest,
    headers: HeaderMap,
) -> Result<()> {
    let base_url = request.base_url.trim_end_matches('/');
    let model = request.model.trim();
    let (path, body) = match request.api_backend.as_str() {
        "messages" => (
            "messages",
            serde_json::json!({
                "model": model,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "ping" }],
            }),
        ),
        "responses" => (
            "responses",
            serde_json::json!({
                "model": model,
                "max_output_tokens": 1,
                "input": [{ "role": "user", "content": "ping" }],
            }),
        ),
        _ => (
            "chat/completions",
            serde_json::json!({
                "model": model,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "ping" }],
            }),
        ),
    };
    let url = format!("{base_url}/{path}");
    let response = client
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to reach {url}"))?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("Validation failed with {}: {}", status, body_snippet(&body));
}

fn body_snippet(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty response body>".to_string();
    }
    let max_len = 200usize;
    if trimmed.chars().count() <= max_len {
        trimmed.to_string()
    } else {
        let mut snippet = String::new();
        for ch in trimmed.chars().take(max_len) {
            snippet.push(ch);
        }
        snippet.push_str("...");
        snippet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use serde_json::json;
    use xai_grok_test_support::{MockInferenceServer, ScriptedResponse};

    #[tokio::test]
    async fn validate_provider_endpoint_uses_responses_probe_for_responses_backend() {
        let server = MockInferenceServer::start().await.unwrap();
        server.enqueue_response(
            "/v1/chat/completions",
            ScriptedResponse::json(500, json!({ "error": "wrong endpoint" })),
        );
        let request = ValidateRequest {
            base_url: server.url(),
            model: "test-model".into(),
            api_key: Some("sk-test".into()),
            api_backend: "responses".into(),
            auth_not_required: false,
            extra_headers: IndexMap::new(),
        };
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let headers = build_headers(&request).unwrap();

        probe_generation_endpoint(&client, &request, headers)
            .await
            .expect("responses backend should probe /responses");

        assert!(
            server.has_responses_request(),
            "responses backend should hit /v1/responses"
        );
        assert!(
            !server.has_chat_completion_request(),
            "responses backend must not fall back to /v1/chat/completions"
        );
    }
}
