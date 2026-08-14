use crate::error::ApiError;

pub fn validate_version(version: &str) -> Result<String, ApiError> {
    let version = version.trim();

    if version.is_empty() {
        return Err(ApiError::Validation("version is required".to_string()));
    }

    if version.len() > 100 {
        return Err(ApiError::Validation(
            "version must not exceed 100 characters".to_string(),
        ));
    }

    if version.eq_ignore_ascii_case("latest") {
        return Err(ApiError::Validation(
            "'latest' cannot be used as a release version".to_string(),
        ));
    }

    let mut characters = version.chars();

    let Some(first) = characters.next() else {
        return Err(ApiError::Validation("version is required".to_string()));
    };

    if !first.is_ascii_alphanumeric() {
        return Err(ApiError::Validation(
            "version must start with a letter or number".to_string(),
        ));
    }

    let valid = characters
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'));

    if !valid {
        return Err(ApiError::Validation(
            "version contains unsupported characters".to_string(),
        ));
    }

    Ok(version.to_string())
}

pub fn valid_git_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
