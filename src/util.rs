//! Shared utility functions.

/// Convert a slug to title case.
///
/// Splits on `-` and `_`, capitalizes each word.
/// "getting-started" -> "Getting Started"
/// "api_reference" -> "Api Reference"
pub fn title_case(s: &str) -> String {
    s.split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_case() {
        assert_eq!(title_case("getting-started"), "Getting Started");
        assert_eq!(title_case("installation"), "Installation");
        assert_eq!(title_case("api_reference"), "Api Reference");
        assert_eq!(title_case("README"), "README");
        assert_eq!(title_case("my-cool-feature"), "My Cool Feature");
    }
}
