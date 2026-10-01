use super::*;
use std::path::{Path, PathBuf};

fn rust_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        // The i18n module itself defines tr and tests it with sample calls: not messages of the server.
        if path.is_dir() {
            if !path.ends_with("i18n") {
                rust_sources(&path, files);
            }
        } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("i18n.rs") {
            files.push(path);
        }
    }
}

/// The string literal starting at `source[at]` (a `"`), unescaped, or `None` when there is none.
fn literal_at(source: &str, at: usize) -> Option<String> {
    let mut chars = source[at..].chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut value = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                't' => value.push('\t'),
                other => value.push(other),
            },
            other => value.push(other),
        }
    }
    None
}

/// Every message given to `tr`, and the calls whose message is not a string literal (which no catalogue can know).
fn extract() -> (HashMap<String, String>, Vec<String>) {
    let mut files = Vec::new();
    rust_sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut messages = HashMap::new();
    let mut unextractable = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        let mut from = 0;
        while let Some(found) = source[from..].find("tr(") {
            let at = from + found;
            from = at + 3;
            let before = source[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            let start = at + 3 + source[at + 3..].len() - source[at + 3..].trim_start().len();
            let place = format!("{}:{}", file.display(), source[..at].lines().count());
            match literal_at(&source, start) {
                Some(message) => {
                    messages.entry(message).or_insert(place);
                }
                None => unextractable.push(place),
            }
        }
    }
    (messages, unextractable)
}

fn placeholders(text: &str) -> Vec<String> {
    let mut found: Vec<String> = text
        .match_indices('{')
        .filter_map(|(i, _)| {
            let rest = &text[i + 1..];
            let end = rest.find('}')?;
            rest[..end]
                .chars()
                .all(|c| c.is_ascii_digit())
                .then(|| rest[..end].to_string())
                .filter(|n| !n.is_empty())
        })
        .collect();
    found.sort();
    found
}

#[test]
fn every_message_is_a_literal_and_the_french_catalogue_matches_the_code() {
    let (messages, unextractable) = extract();
    assert!(
        unextractable.is_empty(),
        "tr() needs a string literal: {unextractable:?}"
    );
    let catalogue: HashMap<String, String> =
        serde_json::from_str(include_str!("../locales/fr.json")).unwrap();
    let mut missing: Vec<_> = messages
        .keys()
        .filter(|m| !catalogue.contains_key(*m))
        .collect();
    let mut extra: Vec<_> = catalogue
        .keys()
        .filter(|m| !messages.contains_key(*m))
        .collect();
    missing.sort();
    extra.sort();
    assert!(missing.is_empty(), "untranslated: {missing:#?}");
    assert!(extra.is_empty(), "no longer used: {extra:#?}");
    let broken: Vec<_> = catalogue
        .iter()
        .filter(|(source, target)| {
            target.trim().is_empty() || placeholders(source) != placeholders(target)
        })
        .collect();
    assert!(broken.is_empty(), "placeholders lost: {broken:#?}");
}

#[test]
fn picks_the_preferred_supported_language() {
    use Language::*;
    for (header, expected) in [
        (None, English),
        (Some(""), English),
        (Some("fr-FR,fr;q=0.9,en;q=0.8"), French),
        (Some("en-US,en;q=0.9,fr;q=0.8"), English),
        (Some("de-DE,fr;q=0.5"), French),
        (Some("de-DE,es"), English),
        (Some("en;q=0.2, fr;q=0.7"), French),
        (Some("fr;q=0"), English),
    ] {
        assert_eq!(
            Language::from_accept_language(header),
            expected,
            "{header:?}"
        );
    }
}

#[tokio::test]
async fn translates_in_the_language_of_the_request() {
    let english = tr("The group \"{0}\" does not exist.", &[&"ops"]);
    assert_eq!(english, "The group \"ops\" does not exist.");
    let french = in_language(Language::French, async {
        tr("The group \"{0}\" does not exist.", &[&"ops"])
    })
    .await;
    assert_eq!(french, "Le groupe \"ops\" n'existe pas.");
}

#[tokio::test]
async fn api_errors_follow_the_language_the_request_asks_for() {
    use crate::server::test_support::{
        auth_disabled, mock_service, serve, temp_data_dir, test_state,
    };
    let data_dir = temp_data_dir("i18n");
    let state = test_state(
        &data_dir,
        crate::models::MockConfig::empty(),
        auth_disabled(),
    )
    .await;
    let root = serve(crate::server::build_router(state, &data_dir)).await;
    let client = reqwest::Client::new();
    let reserved = mock_service("api", "x");
    let error = |language: Option<&'static str>| {
        let mut request = client.post(format!("{root}/api/services")).json(&reserved);
        if let Some(language) = language {
            request = request.header("accept-language", language);
        }
        async move {
            let body: serde_json::Value = request.send().await.unwrap().json().await.unwrap();
            body["error"].as_str().unwrap().to_string()
        }
    };
    assert!(error(None).await.contains("is reserved by Mimicway"));
    assert!(
        error(Some("fr-FR,fr;q=0.9"))
            .await
            .contains("est reserve par Mimicway")
    );
    assert!(
        error(Some("de-DE"))
            .await
            .contains("is reserved by Mimicway")
    );
}
