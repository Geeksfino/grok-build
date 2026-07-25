use indexmap::IndexMap;
use std::io;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, value};

pub struct ProviderModelWrite {
    pub catalog_id: String,
    pub model: String,
    pub name: String,
    pub base_url: String,
    pub api_backend: String,
    pub env_key: Option<String>,
    pub api_key: Option<String>,
    pub auth_not_required: bool,
    pub extra_headers: IndexMap<String, String>,
}

pub fn write_provider_model_config(path: &Path, write: &ProviderModelWrite) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut doc = read_config_document_for_write(path)?;
    {
        let root = doc.as_table_mut();
        set_table_string(
            ensure_table(root, "auth"),
            "preferred_method",
            Some("api_key"),
        );
        set_table_string(
            ensure_table(root, "models"),
            "default",
            Some(write.catalog_id.as_str()),
        );

        let model_root = ensure_table(root, "model");
        let model_table = ensure_table(model_root, &write.catalog_id);
        set_table_string(model_table, "model", Some(write.model.as_str()));
        set_table_string(model_table, "name", Some(write.name.as_str()));
        set_table_string(model_table, "base_url", Some(write.base_url.as_str()));
        set_table_string(model_table, "api_backend", Some(write.api_backend.as_str()));
        set_table_string(model_table, "env_key", write.env_key.as_deref());
        set_table_string(model_table, "api_key", write.api_key.as_deref());
        set_table_bool(
            model_table,
            "auth_not_required",
            write.auth_not_required.then_some(true),
        );
        set_extra_headers(model_table, &write.extra_headers);
    }

    atomic_write_string(path, &doc.to_string())
}

fn read_config_document_for_write(path: &Path) -> io::Result<DocumentMut> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    if content.is_empty() {
        return Ok(DocumentMut::new());
    }

    content.parse::<DocumentMut>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "refusing to overwrite unparseable {}: {err}",
                path.display()
            ),
        )
    })
}

fn ensure_table<'a>(parent: &'a mut Table, key: &str) -> &'a mut Table {
    let needs_replacement = !matches!(parent.get(key), Some(Item::Table(_)));
    if needs_replacement {
        parent.insert(key, Item::Table(Table::new()));
    }

    parent[key]
        .as_table_mut()
        .expect("table entry should exist after insertion")
}

fn set_table_string(table: &mut Table, key: &str, value_to_set: Option<&str>) {
    match value_to_set {
        Some(value_to_set) => {
            table[key] = value(value_to_set);
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_table_bool(table: &mut Table, key: &str, value_to_set: Option<bool>) {
    match value_to_set {
        Some(value_to_set) => {
            table[key] = value(value_to_set);
        }
        None => {
            table.remove(key);
        }
    }
}

fn set_extra_headers(table: &mut Table, headers: &IndexMap<String, String>) {
    if headers.is_empty() {
        table.remove("extra_headers");
        return;
    }

    let headers_table = ensure_table(table, "extra_headers");
    headers_table.clear();
    for (key, value_to_set) in headers {
        headers_table[key] = value(value_to_set.as_str());
    }
}

fn atomic_write_string(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    let prior_mode: Option<u32> = match std::fs::metadata(path) {
        Ok(metadata) => {
            use std::os::unix::fs::PermissionsExt;
            Some(metadata.permissions().mode())
        }
        Err(_) => None,
    };
    #[cfg(not(unix))]
    let prior_mode: Option<u32> = None;

    let suffix = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        format!("toml.tmp.{}.{}", std::process::id(), nanos)
    };
    let tmp = path.with_extension(suffix);
    std::fs::write(&tmp, content)?;

    #[cfg(unix)]
    if let Some(mode) = prior_mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
    }
    let _ = prior_mode;

    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_auth_models_and_model_table() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        write_provider_model_config(
            &path,
            &ProviderModelWrite {
                catalog_id: "openai-gpt-4o".into(),
                model: "gpt-4o".into(),
                name: "GPT-4o".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_backend: "chat_completions".into(),
                env_key: Some("OPENAI_API_KEY".into()),
                api_key: None,
                auth_not_required: false,
                extra_headers: Default::default(),
            },
        )
        .unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("preferred_method") && body.contains("api_key"));
        assert!(body.contains("[models]"));
        assert!(body.contains("default = \"openai-gpt-4o\""));
        assert!(body.contains("[model.openai-gpt-4o]"));
        assert!(body.contains("base_url = \"https://api.openai.com/v1\""));
        assert!(body.contains("env_key = \"OPENAI_API_KEY\""));
    }

    #[test]
    fn merge_preserves_unrelated_tables() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[ui]\nvim_mode = true\n").unwrap();

        write_provider_model_config(
            &path,
            &ProviderModelWrite {
                catalog_id: "ollama-llama3".into(),
                model: "llama3".into(),
                name: "Llama 3".into(),
                base_url: "http://localhost:11434/v1".into(),
                api_backend: "chat_completions".into(),
                env_key: None,
                api_key: None,
                auth_not_required: true,
                extra_headers: Default::default(),
            },
        )
        .unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("vim_mode = true"));
        assert!(body.contains("auth_not_required = true"));
    }

    #[test]
    fn invalid_existing_config_returns_error_without_clobbering_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let invalid = "this is [not valid toml\n";
        std::fs::write(&path, invalid).unwrap();

        let err = write_provider_model_config(
            &path,
            &ProviderModelWrite {
                catalog_id: "openai-gpt-4o".into(),
                model: "gpt-4o".into(),
                name: "GPT-4o".into(),
                base_url: "https://api.openai.com/v1".into(),
                api_backend: "chat_completions".into(),
                env_key: Some("OPENAI_API_KEY".into()),
                api_key: None,
                auth_not_required: false,
                extra_headers: Default::default(),
            },
        )
        .expect_err("invalid TOML should not be overwritten");

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
    }
}
