use autumnus::{HtmlLinkedBuilder, formatter::Formatter, languages::Language, themes};

/// A syntax highlighter using autumnus (tree-sitter based).
pub struct SyntaxHighlighter {
    /// Theme name for CSS generation (used by generate_css).
    #[allow(dead_code)]
    theme_name: String,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter with the given theme.
    pub fn new(theme_name: &str) -> Self {
        Self {
            theme_name: theme_name.to_string(),
        }
    }

    /// Highlight code and return HTML with CSS classes.
    /// Returns the original code wrapped in a plain `<code>` if the language is not supported.
    pub fn highlight(&self, code: &str, language: &str) -> String {
        // Use Language::guess which handles language detection from name or extension
        let lang = Language::guess(language, code);

        // Check if it's the Plaintext/unknown fallback
        if matches!(lang, Language::PlainText)
            && !language.is_empty()
            && language != "plaintext"
            && language != "text"
        {
            // Language wasn't recognized, use plain code block
            return Self::plain_code_block(code, language);
        }

        let formatter = HtmlLinkedBuilder::new().source(code).lang(lang).build();

        match formatter {
            Ok(f) => {
                let mut output: Vec<u8> = Vec::new();
                if f.format(&mut output).is_ok() {
                    String::from_utf8(output)
                        .unwrap_or_else(|_| Self::plain_code_block(code, language))
                } else {
                    Self::plain_code_block(code, language)
                }
            }
            Err(_) => Self::plain_code_block(code, language),
        }
    }

    /// Generate CSS for the current theme.
    #[allow(dead_code)]
    pub fn generate_css(&self) -> Option<String> {
        let theme = themes::get(&self.theme_name).ok()?;
        Some(theme.css(false)) // false = don't enable italic
    }

    /// Create a plain code block without highlighting.
    fn plain_code_block(code: &str, language: &str) -> String {
        let escaped = html_escape(code);
        if language.is_empty() {
            format!("<pre><code>{}</code></pre>", escaped)
        } else {
            format!(
                "<pre><code class=\"language-{}\">{}</code></pre>",
                language, escaped
            )
        }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        // Use a nice default theme
        Self::new("github-dark")
    }
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust() {
        let highlighter = SyntaxHighlighter::default();
        let code = "fn main() {}";
        let result = highlighter.highlight(code, "rust");
        // Should contain highlighted spans
        assert!(result.contains("<pre"));
        assert!(result.contains("</pre>"));
    }

    #[test]
    fn test_highlight_unknown_language() {
        let highlighter = SyntaxHighlighter::default();
        let result = highlighter.highlight("some code", "unknown_lang_xyz");
        // Should fall back to plain code block
        assert!(result.contains("<pre><code"));
        assert!(result.contains("some code"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<div>&</div>"), "&lt;div&gt;&amp;&lt;/div&gt;");
    }

    #[test]
    fn test_generate_css() {
        let highlighter = SyntaxHighlighter::new("dracula");
        let css = highlighter.generate_css();
        assert!(css.is_some());
        // CSS should contain style definitions
        let css_str = css.unwrap();
        assert!(!css_str.is_empty());
    }
}
