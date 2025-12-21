use serde::Deserialize;

/// Converts a text potentially containing a mix of markdown and HTML into HTML.
///
/// Returns `None` if the text fails to parse.
pub(crate) fn to_html<T: AsRef<str>>(text: T) -> Option<String> {
    let mut options = markdown::Options::gfm();
    options.compile.allow_dangerous_html = true; // Preserve existing HTML.

    let html = markdown::to_html_with_options(text.as_ref(), &options).ok()?;

    let document = dom_query::Document::fragment(html);

    remove_empty_anchors(&document);

    Some(document.html_root().inner_html().to_string())
}

// Removes empty anchor elements.
//
// In combination with markdown's autolinks feature something like
//
//   <a href="https://bar.foo.tld">https://foo.tld</a>
//
// produces
//
//   <a href="https://bar.foo.tld"></a><a href="https://foo.tld">https://foo.tld</a>
//
// We'll clean that up to
//
//   <a href="https://foo.tld">https://foo.tld</a>
fn remove_empty_anchors(document: &dom_query::Document) {
    for node in document.select("a").iter() {
        if node.inner_html().is_empty() {
            node.remove();
        }
    }
}

/// A custom deserializer to automatically convert a markdown text to HTML.
pub(crate) fn deserialize_to_html<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    String::deserialize(deserializer).map(to_html)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_to_html {
        ($given:expr, $expected:expr $(,)?) => {
            assert_eq!(to_html($given).unwrap(), $expected);
        };
    }

    #[test]
    fn basic() {
        assert_to_html!("foo\nbar\n\nbaz", "<p>foo\nbar</p>\n<p>baz</p>");

        assert_to_html!(
            "arbitrary<br />html<div></div>",
            "<p>arbitrary<br>html</p><div></div><p></p>"
        );
    }

    #[test]
    fn links() {
        assert_to_html!(
            "contains <a href=\"\">a link</a>",
            "<p>contains <a href=\"\">a link</a></p>",
        );

        assert_to_html!(
            "A link to https://musikundkultur.de",
            "<p>A link to <a href=\"https://musikundkultur.de\">https://musikundkultur.de</a></p>",
        );

        assert_to_html!(
            "A link to <https://musikundkultur.de>",
            "<p>A link to <a href=\"https://musikundkultur.de\">https://musikundkultur.de</a></p>",
        );

        assert_to_html!(
            "A link to [somewhere](https://musikundkultur.de)",
            "<p>A link to <a href=\"https://musikundkultur.de\">somewhere</a></p>",
        );

        assert_to_html!(
            "<a href=\"https://foo.musikundkultur.de\">https://musikundkultur.de</a>",
            "<p><a href=\"https://musikundkultur.de\">https://musikundkultur.de</a></p>",
        );

        assert_to_html!(
            "<a href=\"https://foo.musikundkultur.de\">https://musikundkultur.de</a> foo\n\n<a href=\"https://bar.musikundkultur.de\">https://musikundkultur.de</a>",
            "<p><a href=\"https://musikundkultur.de\">https://musikundkultur.de</a> foo</p>\n<p><a href=\"https://musikundkultur.de\">https://musikundkultur.de</a></p>",
        );

        assert_to_html!(
            "<a href=\"https://foo.musikundkultur.de\"><a href=\"https://bar.musikundkultur.de\">https://musikundkultur.de</a></a>",
            "<p><a href=\"https://musikundkultur.de\">https://musikundkultur.de</a></p>",
        );

        assert_to_html!(
            "<a href=\"https://foo.musikundkultur.de\"><a href=\"https://bar.musikundkultur.de\">Link</a></a>",
            "<p><a href=\"https://bar.musikundkultur.de\">Link</a></p>",
        );
    }

    #[test]
    fn advanced() {
        assert_to_html!(
            "<div style=\"background: #f00\">this is red</div> and this is not\n\nanother paragraph",
            "<div style=\"background: #f00\">this is red</div> and this is not\n<p>another paragraph</p>"
        );
    }

    #[test]
    fn unclosed() {
        assert_to_html!("<br>", "<br>");
        assert_to_html!("<div>", "<div></div>");
    }

    #[test]
    fn script() {
        assert_to_html!(
            "<script src=\"https://some.external.resource\" />",
            "&lt;script src=\"https://some.external.resource\" /&gt;"
        );
        assert_to_html!("<script></script>", "&lt;script&gt;&lt;/script&gt;");
    }
}
