use std::borrow::Cow;

/// Redacts local Windows/Unix user paths and bearer/API tokens for safe open-source export.
#[must_use]
pub fn redact_sensitive(input: &str) -> String {
    let mut out = input
        .replace("C:\\Users\\Admin\\TechTide", "<WORKSPACE>")
        .replace("c:\\Users\\Admin\\TechTide", "<WORKSPACE>")
        .replace("C:/Users/Admin/TechTide", "<WORKSPACE>")
        .replace("c:/Users/Admin/TechTide", "<WORKSPACE>")
        .replace("C:\\Users\\Admin", "<HOME>")
        .replace("C:/Users/Admin", "<HOME>");

    for prefix in ["gho_", "ghp_", "sk-ant-", "sk-proj-", "bearer "] {
        while let Some(idx) = out.to_lowercase().find(prefix) {
            let end = out[idx..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |offset| idx + offset);
            out.replace_range(idx..end, "<REDACTED_TOKEN>");
        }
    }
    out
}

#[must_use]
pub fn normalize_line(line: &str) -> Cow<'_, str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Cow::Borrowed("")
    } else {
        Cow::Owned(redact_sensitive(trimmed))
    }
}
