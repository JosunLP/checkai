//! Internationalization (i18n) support for CheckAI.
//!
//! Provides locale detection and extraction from HTTP requests,
//! environment variables, and system locale settings.
//!
//! Supported languages: en, de, fr, es, zh-CN, ja, pt, ru.

use actix_web::HttpRequest;

/// All locales supported by CheckAI.
pub const SUPPORTED_LOCALES: &[&str] = &["en", "de", "fr", "es", "zh-CN", "ja", "pt", "ru"];

/// Detects the best locale from the system environment.
///
/// Priority:
/// 1. `CHECKAI_LANG` environment variable
/// 2. System locale (via `sys-locale` crate)
/// 3. Fallback to `"en"`
pub fn detect_system_locale() -> String {
    // 1. Explicit environment variable
    if let Ok(lang) = std::env::var("CHECKAI_LANG")
        && let Some(locale) = normalize_locale(&lang)
    {
        return locale;
    }

    // 2. System locale
    if let Some(locale_str) = sys_locale::get_locale()
        && let Some(locale) = normalize_locale(&locale_str)
    {
        return locale;
    }

    // 3. Fallback
    "en".to_string()
}

/// Extracts the locale from an HTTP request.
///
/// Priority:
/// 1. `?lang=xx` query parameter
/// 2. `Accept-Language` header
/// 3. Fallback to `"en"`
pub fn extract_locale_from_request(req: &HttpRequest) -> String {
    // 1. Query parameter ?lang=xx
    if let Some(lang) = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            if parts.next() == Some("lang") {
                parts.next().and_then(normalize_locale)
            } else {
                None
            }
        })
    {
        return lang;
    }

    // 2. Accept-Language header (simplified parsing)
    if let Some(accept_lang) = req.headers().get("Accept-Language")
        && let Ok(value) = accept_lang.to_str()
    {
        // Parse comma-separated language tags, pick the first supported one
        for entry in value.split(',') {
            let tag = entry.split(';').next().unwrap_or("").trim();
            if let Some(locale) = normalize_locale(tag) {
                return locale;
            }
        }
    }

    // 3. Fallback
    "en".to_string()
}

/// Normalizes a locale string to one of the supported locales.
///
/// Accepts common formats: "en-US", "de_DE.UTF-8", "zh-CN", "ja", etc.
/// Returns `None` if the language is not supported.
pub fn normalize_locale(input: &str) -> Option<String> {
    let lower = input.to_lowercase();
    // Strip encoding suffix (e.g. ".utf-8")
    let tag = lower.split('.').next().unwrap_or(&lower);
    // Normalize separator
    let tag = tag.replace('_', "-");

    if tag.starts_with("zh") {
        Some("zh-CN".to_string())
    } else if tag.starts_with("ja") {
        Some("ja".to_string())
    } else if tag.starts_with("de") {
        Some("de".to_string())
    } else if tag.starts_with("fr") {
        Some("fr".to_string())
    } else if tag.starts_with("es") {
        Some("es".to_string())
    } else if tag.starts_with("pt") {
        Some("pt".to_string())
    } else if tag.starts_with("ru") {
        Some("ru".to_string())
    } else if tag.starts_with("en") {
        Some("en".to_string())
    } else {
        None
    }
}
