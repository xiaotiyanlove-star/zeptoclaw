use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ActiveTag {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
    Pre,
    Anchor(String),
    Spoiler,
    Blockquote,
}

impl ActiveTag {
    pub fn open_tag(&self) -> String {
        match self {
            ActiveTag::Bold => "<b>".to_string(),
            ActiveTag::Italic => "<i>".to_string(),
            ActiveTag::Underline => "<u>".to_string(),
            ActiveTag::Strikethrough => "<s>".to_string(),
            ActiveTag::Code => "<code>".to_string(),
            ActiveTag::Pre => "<pre>".to_string(),
            ActiveTag::Anchor(url) => format!("<a href=\"{}\">", html_escape(url)),
            ActiveTag::Spoiler => "<tg-spoiler>".to_string(),
            ActiveTag::Blockquote => "<blockquote>".to_string(),
        }
    }

    pub fn close_tag(&self) -> &'static str {
        match self {
            ActiveTag::Bold => "</b>",
            ActiveTag::Italic => "</i>",
            ActiveTag::Underline => "</u>",
            ActiveTag::Strikethrough => "</s>",
            ActiveTag::Code => "</code>",
            ActiveTag::Pre => "</pre>",
            ActiveTag::Anchor(_) => "</a>",
            ActiveTag::Spoiler => "</tg-spoiler>",
            ActiveTag::Blockquote => "</blockquote>",
        }
    }
}

pub fn html_escape(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    for c in content.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn render_and_chunk_telegram_markdown(content: &str, chunk_size: usize) -> Vec<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(content, options);

    let mut chunks = Vec::new();
    let mut current_chunk = String::with_capacity(chunk_size + 1024);
    let mut active_tags: Vec<ActiveTag> = Vec::new();
    let mut in_spoiler = false;
    let mut in_code_block = false;

    // Helper to close all active tags
    let close_all_tags = |tags: &Vec<ActiveTag>| -> String {
        tags.iter().rev().map(|t| t.close_tag()).collect::<String>()
    };

    // Helper to reopen all active tags
    let open_all_tags =
        |tags: &Vec<ActiveTag>| -> String { tags.iter().map(|t| t.open_tag()).collect::<String>() };

    let mut list_index: Option<u64> = None;

    for event in parser {
        let mut event_str = String::new();

        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {}
                Tag::Heading { .. } => {
                    event_str.push_str(&ActiveTag::Bold.open_tag());
                    active_tags.push(ActiveTag::Bold);
                }
                Tag::BlockQuote => {
                    event_str.push_str(&ActiveTag::Blockquote.open_tag());
                    active_tags.push(ActiveTag::Blockquote);
                }
                Tag::CodeBlock(_) => {
                    event_str.push_str(&ActiveTag::Pre.open_tag());
                    active_tags.push(ActiveTag::Pre);
                    in_code_block = true;
                }
                Tag::List(Some(start_num)) => {
                    list_index = Some(start_num);
                }
                Tag::List(None) => {
                    list_index = None;
                }
                Tag::Item => {
                    if let Some(ref mut num) = list_index {
                        event_str.push_str(&format!("{}. ", num));
                        *num += 1;
                    } else {
                        event_str.push_str("• ");
                    }
                }
                Tag::Strong => {
                    event_str.push_str(&ActiveTag::Bold.open_tag());
                    active_tags.push(ActiveTag::Bold);
                }
                Tag::Emphasis => {
                    event_str.push_str(&ActiveTag::Italic.open_tag());
                    active_tags.push(ActiveTag::Italic);
                }
                Tag::Strikethrough => {
                    event_str.push_str(&ActiveTag::Strikethrough.open_tag());
                    active_tags.push(ActiveTag::Strikethrough);
                }
                Tag::Link { dest_url, .. } => {
                    let tag = ActiveTag::Anchor(dest_url.to_string());
                    event_str.push_str(&tag.open_tag());
                    active_tags.push(tag);
                }
                Tag::Table(_) => {
                    event_str.push_str(&ActiveTag::Pre.open_tag());
                    active_tags.push(ActiveTag::Pre);
                }
                Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Paragraph => {
                    event_str.push_str("\n\n");
                }
                TagEnd::Heading { .. } => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Bold))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Bold.close_tag());
                        event_str.push_str("\n\n");
                    }
                }
                TagEnd::BlockQuote => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Blockquote))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Blockquote.close_tag());
                        event_str.push_str("\n");
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Pre))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Pre.close_tag());
                        event_str.push_str("\n\n");
                    }
                    in_code_block = false;
                }
                TagEnd::Item | TagEnd::List(_) => {
                    event_str.push_str("\n");
                }
                TagEnd::Strong => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Bold))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Bold.close_tag());
                    }
                }
                TagEnd::Emphasis => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Italic))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Italic.close_tag());
                    }
                }
                TagEnd::Strikethrough => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Strikethrough))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Strikethrough.close_tag());
                    }
                }
                TagEnd::Link { .. } => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Anchor(_)))
                    {
                        let tag = active_tags.remove(idx);
                        event_str.push_str(tag.close_tag());
                    }
                }
                TagEnd::Table => {
                    if let Some(idx) = active_tags
                        .iter()
                        .rposition(|t| matches!(t, ActiveTag::Pre))
                    {
                        active_tags.remove(idx);
                        event_str.push_str(ActiveTag::Pre.close_tag());
                        event_str.push_str("\n\n");
                    }
                }
                TagEnd::TableRow => {
                    event_str.push_str("\n");
                }
                TagEnd::TableCell => {
                    event_str.push_str(" | ");
                }
                _ => {}
            },
            Event::Text(text) => {
                if in_code_block {
                    event_str.push_str(&html_escape(&text));
                } else {
                    let pieces: Vec<&str> = text.split("||").collect();
                    for (i, piece) in pieces.iter().enumerate() {
                        if i > 0 {
                            if in_spoiler {
                                if let Some(idx) = active_tags
                                    .iter()
                                    .rposition(|t| matches!(t, ActiveTag::Spoiler))
                                {
                                    active_tags.remove(idx);
                                    event_str.push_str(ActiveTag::Spoiler.close_tag());
                                }
                            } else {
                                event_str.push_str(&ActiveTag::Spoiler.open_tag());
                                active_tags.push(ActiveTag::Spoiler.clone());
                            }
                            in_spoiler = !in_spoiler;
                        }
                        event_str.push_str(&html_escape(piece));
                    }
                }
            }
            Event::Code(text) => {
                event_str.push_str(&ActiveTag::Code.open_tag());
                event_str.push_str(&html_escape(&text));
                event_str.push_str(ActiveTag::Code.close_tag());
            }
            Event::SoftBreak | Event::HardBreak => {
                event_str.push_str("\n");
            }
            _ => {}
        }

        current_chunk.push_str(&event_str);

        if current_chunk.len() >= chunk_size {
            chunks.push(format!("{}{}", current_chunk, close_all_tags(&active_tags)));
            current_chunk.clear();
            current_chunk.push_str(&open_all_tags(&active_tags));
        }
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(format!(
            "{}{}",
            current_chunk.trim_end(),
            close_all_tags(&active_tags)
        ));
    }

    if chunks.is_empty() {
        chunks.push(String::new());
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_formatting() {
        let md = "**bold** *italic* ~~strike~~ `code` [link](https://example.com) ||spoiler||";
        let chunks = render_and_chunk_telegram_markdown(md, 4000);
        assert_eq!(chunks.len(), 1);
        let html = &chunks[0];

        assert!(html.contains("<b>bold</b>"));
        assert!(html.contains("<i>italic</i>"));
        assert!(html.contains("<s>strike</s>"));
        assert!(html.contains("<code>code</code>"));
        assert!(html.contains("<a href=\"https://example.com\">link</a>"));
        assert!(html.contains("<tg-spoiler>spoiler</tg-spoiler>"));
    }

    #[test]
    fn test_list_and_heading() {
        let md = "### Heading\n- Item 1\n- Item 2";
        let chunks = render_and_chunk_telegram_markdown(md, 4000);
        let html = &chunks[0];

        // headings should become bold
        assert!(html.contains("<b>Heading</b>"));
        // lists should become string bullets
        assert!(html.contains("• Item 1"));
        assert!(html.contains("• Item 2"));
    }

    #[test]
    fn test_chunking_with_tag_balancing() {
        let md = "**this is a very long string that we will chunk up into pieces so we can test tag balancing**";

        let chunks = render_and_chunk_telegram_markdown(md, 20);
        assert!(chunks.len() > 1);

        for chunk in chunks {
            let open_count = chunk.matches("<b>").count();
            let close_count = chunk.matches("</b>").count();
            assert_eq!(
                open_count, close_count,
                "Tags unbalanced in chunk: {}",
                chunk
            );
        }
    }
}
