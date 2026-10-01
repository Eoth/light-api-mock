//! Messages of the server that people read: errors of the management API shown by the UI, hints of the rule
//! tester, errors of rule scripts, availability-test errors. Same rule as the UI: the English sentence is written
//! once, where it is used, and is the key of `src/locales/<language>.json`; a language adds a catalogue and nothing
//! else. The language is the one the request asks for in `Accept-Language` (the UI sends its own), English
//! otherwise. Responses of mocked services and the server's logs stay in English.
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    French,
}

static FRENCH: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| serde_json::from_str(include_str!("locales/fr.json")).unwrap_or_default());

tokio::task_local! {
    static LANGUAGE: Language;
}

impl Language {
    /// The supported language with the highest preference in an `Accept-Language` value (`fr-FR,fr;q=0.9,en;q=0.8`).
    pub fn from_accept_language(header: Option<&str>) -> Self {
        let mut best = (Language::English, -1.0_f32);
        for (position, entry) in header.unwrap_or_default().split(',').enumerate() {
            let mut parts = entry.split(';');
            let tag = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
            let quality = parts
                .find_map(|p| p.trim().strip_prefix("q="))
                .and_then(|q| q.parse::<f32>().ok())
                .unwrap_or(1.0);
            let language = match tag.split('-').next().unwrap_or_default() {
                "fr" => Some(Language::French),
                "en" => Some(Language::English),
                _ => None,
            };
            // Earlier entries win ties, as the header lists preferences in order.
            let score = quality - position as f32 * 1e-4;
            if let Some(language) = language
                && quality > 0.0
                && score > best.1
            {
                best = (language, score);
            }
        }
        best.0
    }
}

/// The language of the request being handled, English outside of one.
pub fn current() -> Language {
    LANGUAGE.try_with(|l| *l).unwrap_or(Language::English)
}

/// `message` in the language of the current request, its `{0}`, `{1}`… replaced by `args`.
pub fn tr(message: &'static str, args: &[&dyn Display]) -> String {
    let translated = match current() {
        Language::English => message,
        Language::French => FRENCH.get(message).map(String::as_str).unwrap_or(message),
    };
    let mut out = translated.to_string();
    for (index, arg) in args.iter().enumerate() {
        out = out.replace(&format!("{{{index}}}"), &arg.to_string());
    }
    out
}

/// Runs the rest of the request in the language it asks for.
pub async fn language_scope(req: Request, next: Next) -> Response {
    let language = Language::from_accept_language(
        req.headers()
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok()),
    );
    LANGUAGE.scope(language, next.run(req)).await
}

#[cfg(test)]
pub async fn in_language<F: std::future::Future>(language: Language, f: F) -> F::Output {
    LANGUAGE.scope(language, f).await
}

#[cfg(test)]
mod tests;
