use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use leptos::prelude::*;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

use crate::content::course::Course;
use crate::diag::Diagnostic;
use crate::widgets::{resolve, WidgetInstance, WidgetRegistry};

const PATH_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'/');

#[derive(Debug, Clone)]
pub struct WidgetTag {
    pub kind: String,
    pub id: String,
}

pub struct MarkdownCtx<'a> {
    pub course: &'a Course,
    pub source: &'a Path,
    pub lessons_by_file: &'a HashMap<String, String>,
}

pub struct MarkdownOutput {
    pub views: Vec<AnyView>,
    pub widget_kinds: BTreeSet<String>,
}

#[derive(Debug)]
enum Node {
    Inline(AnyView),
    Widget(AnyView),
}

struct MdParser<'a> {
    events: std::iter::Peekable<Parser<'a>>,
    ctx: &'a MarkdownCtx<'a>,
    registry: &'a WidgetRegistry,
    diags: &'a mut Vec<Diagnostic>,
    kinds: BTreeSet<String>,
    heading_text: Option<String>,
}

pub fn render_markdown(
    src: &str,
    ctx: &MarkdownCtx,
    registry: &WidgetRegistry,
    diags: &mut Vec<Diagnostic>,
) -> MarkdownOutput {
    let mut opts = Options::empty();
    opts.insert(
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_MATH,
    );
    let parser = Parser::new_ext(src, opts);
    let mut p = MdParser {
        events: parser.peekable(),
        ctx,
        registry,
        diags,
        kinds: BTreeSet::new(),
        heading_text: None,
    };
    let views = p.blocks();
    MarkdownOutput { views, widget_kinds: p.kinds }
}

fn paragraph(inline: Vec<AnyView>) -> AnyView {
    view! { <p>{inline}</p> }.into_any()
}

/// Tags pulldown-cmark emits as inline containers. They can appear without a
/// wrapping `Paragraph` event inside tight list items, where they must not be
/// treated as block boundaries.
fn inline_tag(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
    )
}

fn into_views(nodes: Vec<Node>) -> Vec<AnyView> {
    nodes
        .into_iter()
        .map(|n| match n {
            Node::Inline(v) | Node::Widget(v) => v,
        })
        .collect()
}

fn partition(nodes: Vec<Node>) -> Vec<AnyView> {
    let mut out: Vec<AnyView> = Vec::new();
    let mut cur: Vec<AnyView> = Vec::new();
    for node in nodes {
        match node {
            Node::Inline(v) => cur.push(v),
            Node::Widget(v) => {
                if !cur.is_empty() {
                    out.push(paragraph(std::mem::take(&mut cur)));
                }
                out.push(v);
            }
        }
    }
    if !cur.is_empty() {
        out.push(paragraph(cur));
    }
    out
}

fn heading(level: HeadingLevel, id: Option<String>, nodes: Vec<AnyView>) -> AnyView {
    match level {
        HeadingLevel::H1 => view! { <h1 id={id}>{nodes}</h1> }.into_any(),
        HeadingLevel::H2 => view! { <h2 id={id}>{nodes}</h2> }.into_any(),
        HeadingLevel::H3 => view! { <h3 id={id}>{nodes}</h3> }.into_any(),
        HeadingLevel::H4 => view! { <h4 id={id}>{nodes}</h4> }.into_any(),
        HeadingLevel::H5 => view! { <h5 id={id}>{nodes}</h5> }.into_any(),
        HeadingLevel::H6 => view! { <h6 id={id}>{nodes}</h6> }.into_any(),
    }
}

fn align_style(a: &Alignment) -> Option<&'static str> {
    match a {
        Alignment::Center => Some("text-align: center"),
        Alignment::Right => Some("text-align: right"),
        _ => None,
    }
}

enum LinkResolution {
    Url(String, bool),
    MissingLesson,
}

fn resolve_link(
    dest: &str,
    ctx: &MarkdownCtx,
    source: &Path,
    diags: &mut Vec<Diagnostic>,
) -> LinkResolution {
    let d = dest.trim();
    if d.starts_with('#')
        || d.starts_with("mailto:")
        || d.starts_with("http://")
        || d.starts_with("https://")
    {
        let external = d.starts_with("http://") || d.starts_with("https://");
        return LinkResolution::Url(d.to_string(), external);
    }
    let (base, frag) = match d.split_once('#') {
        Some((b, f)) => (b, Some(f)),
        None => (d, None),
    };
    if base.to_ascii_lowercase().ends_with(".md") {
        let fname = Path::new(base)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if let Some(slug) = ctx.lessons_by_file.get(fname) {
            let mut url = format!("/course/{}/{}", ctx.course.slug, slug);
            if let Some(f) = frag {
                url.push('#');
                url.push_str(f);
            }
            return LinkResolution::Url(url, false);
        }
        diags.push(Diagnostic::warning(
            source,
            format!("markdown link '{dest}' does not match any lesson file in this course"),
        ));
        return LinkResolution::MissingLesson;
    }
    LinkResolution::Url(
        format!("/content/{}/{}", ctx.course.slug, encode_path(base)),
        false,
    )
}

fn encode_path(dest: &str) -> String {
    let decoded = percent_decode_str(dest).decode_utf8_lossy().into_owned();
    utf8_percent_encode(&decoded, PATH_SAFE).to_string()
}

pub fn strip_title_heading(body: &str, title: &str) -> String {
    let mut offset = 0usize;
    for line in body.split_inclusive('\n') {
        let t = line.trim();
        if t.is_empty() {
            offset += line.len();
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ")
            && rest.trim() == title.trim() {
                offset += line.len();
                return body[offset..]
                    .trim_start_matches(['\n', '\r'])
                    .to_string();
            }
        break;
    }
    body.to_string()
}

pub fn scan_widget_tags(
    chunk: &str,
    source: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Vec<WidgetTag> {
    let lower = chunk.to_ascii_lowercase();
    let mut tags = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("<widget") {
        let start = search + rel;
        let after = start + "<widget".len();
        let boundary = lower[after..]
            .chars()
            .next()
            .is_none_or(|c| c.is_ascii_whitespace() || c == '>' || c == '/');
        if boundary
            && let Some(tag) = parse_widget_tag(chunk, after, source, diags) {
                tags.push(tag);
            }
        search = after;
    }
    tags
}

/// Widget tags actually reachable by the renderer: runs the markdown parser
/// and scans only HTML events, so `<Widget ... />` inside fenced code blocks
/// or inline code is ignored.
pub fn scan_widget_tags_in_markdown(
    src: &str,
    source: &Path,
) -> Vec<WidgetTag> {
    let mut opts = Options::empty();
    opts.insert(
        Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_MATH,
    );
    let mut scratch = Vec::new();
    let mut tags = Vec::new();
    for event in Parser::new_ext(src, opts) {
        match event {
            Event::Html(h) | Event::InlineHtml(h) => {
                tags.extend(scan_widget_tags(&h, source, &mut scratch));
            }
            _ => {}
        }
    }
    tags
}

fn parse_widget_tag(
    chunk: &str,
    mut pos: usize,
    source: &Path,
    diags: &mut Vec<Diagnostic>,
) -> Option<WidgetTag> {
    let bytes = chunk.as_bytes();
    let mut kind: Option<String> = None;
    let mut id: Option<String> = None;
    let mut self_closing = false;
    loop {
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let Some(&b) = bytes.get(pos) else {
            diags.push(Diagnostic::warning(
                source,
                "widget tag is unterminated; write it as a single self-closing tag \
                 <Widget type=\"..\" id=\"..\" />",
            ));
            return None;
        };
        match b {
            b'>' => break,
            b'/' if bytes.get(pos + 1) == Some(&b'>') => {
                self_closing = true;
                break;
            }
            b'/' => {
                pos += 1;
            }
            _ => {
                let name_start = pos;
                while pos < bytes.len() {
                    let c = bytes[pos];
                    if c.is_ascii_whitespace() || c == b'=' || c == b'>' || c >= 128 {
                        break;
                    }
                    pos += 1;
                }
                let name = chunk[name_start..pos].to_ascii_lowercase();
                while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                }
                let value = if bytes.get(pos) == Some(&b'=') {
                    pos += 1;
                    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                        pos += 1;
                    }
                    match bytes.get(pos) {
                        Some(&q) if q == b'"' || q == b'\'' => {
                            pos += 1;
                            let vstart = pos;
                            match bytes[pos..].iter().position(|&c| c == q) {
                                Some(off) => {
                                    let v = chunk[vstart..vstart + off].to_string();
                                    pos = vstart + off + 1;
                                    v
                                }
                                None => {
                                    diags.push(Diagnostic::warning(
                                        source,
                                        "widget attribute value is missing its closing quote",
                                    ));
                                    return None;
                                }
                            }
                        }
                        _ => {
                            let vstart = pos;
                            while pos < bytes.len() {
                                let c = bytes[pos];
                                if c.is_ascii_whitespace() || c == b'>' {
                                    break;
                                }
                                pos += 1;
                            }
                            chunk[vstart..pos].to_string()
                        }
                    }
                } else {
                    String::new()
                };
                match name.as_str() {
                    "type" => kind = Some(value),
                    "id" => id = Some(value),
                    _ => {}
                }
            }
        }
    }
    if !self_closing {
        diags.push(Diagnostic::error(
            source,
            "widget tag must be self-closing: <Widget type=\"...\" id=\"...\" />",
        ));
        return None;
    }
    match (kind, id) {
        (Some(kind), Some(id))
            if !kind.trim().is_empty() && !id.trim().is_empty() =>
        {
            Some(WidgetTag {
                kind: kind.trim().to_ascii_lowercase(),
                id: id.trim().to_string(),
            })
        }
        _ => {
            diags.push(Diagnostic::error(
                source,
                "widget tag requires non-empty 'type' and 'id' attributes",
            ));
            None
        }
    }
}

impl<'a> MdParser<'a> {
    fn widget_view(&mut self, tag: WidgetTag) -> AnyView {
        let config = self.ctx.course.widget_config(&tag.id);
        let instance = WidgetInstance {
            kind: tag.kind.clone(),
            id: tag.id.clone(),
            config,
        };
        let (view, used_kind, warning) = resolve(&instance, self.registry);
        if let Some(msg) = warning {
            self.diags.push(Diagnostic::warning(self.ctx.source, msg));
        }
        if let Some(k) = used_kind {
            self.kinds.insert(k);
        }
        view
    }

    fn html_widgets(&mut self, chunk: &str) -> Vec<AnyView> {
        let tags = scan_widget_tags(chunk, self.ctx.source, self.diags);
        let mut out = Vec::new();
        for tag in tags {
            out.push(self.widget_view(tag));
        }
        if out.is_empty() {
            tracing::debug!(
                file = %self.ctx.source.display(),
                "ignored raw HTML in markdown: {}",
                chunk.trim()
            );
        }
        out
    }

    fn blocks(&mut self) -> Vec<AnyView> {
        let mut out: Vec<AnyView> = Vec::new();
        let mut pending: Vec<AnyView> = Vec::new();
        loop {
            if matches!(
                self.events.peek(),
                None | Some(Event::End(_))
            ) {
                break;
            }
            let ev = self.events.next().unwrap();
            match ev {
                Event::Start(tag) => {
                    if inline_tag(&tag) {
                        // Tight list items (and similar containers) emit
                        // inline runs with no wrapping Paragraph event; if
                        // they were dispatched to start_tag, each would be
                        // split into its own <p>. Keep them in one paragraph.
                        for v in self.start_tag(tag) {
                            pending.push(v);
                        }
                    } else {
                        if !pending.is_empty() {
                            out.push(paragraph(std::mem::take(&mut pending)));
                        }
                        for v in self.start_tag(tag) {
                            out.push(v);
                        }
                    }
                }
                Event::Text(t) => pending.push(t.to_string().into_any()),
                Event::Code(c) => {
                    pending.push(view! { <code>{c.to_string()}</code> }.into_any())
                }
                Event::SoftBreak => pending.push(" ".into_any()),
                Event::HardBreak => pending.push(view! { <br /> }.into_any()),
                Event::Rule => {
                    if !pending.is_empty() {
                        out.push(paragraph(std::mem::take(&mut pending)));
                    }
                    out.push(view! { <hr /> }.into_any());
                }
                Event::Html(h) | Event::InlineHtml(h) => {
                    let widgets = self.html_widgets(&h);
                    if !widgets.is_empty() {
                        if !pending.is_empty() {
                            out.push(paragraph(std::mem::take(&mut pending)));
                        }
                        out.extend(widgets);
                    }
                }
                Event::DisplayMath(tex) => {
                    if !pending.is_empty() {
                        out.push(paragraph(std::mem::take(&mut pending)));
                    }
                    let tex = tex.to_string();
                    out.push(
                        view! {
                            <div class="widget widget-math">
                                <div class="math-display" data-tex={tex}></div>
                            </div>
                        }
                        .into_any(),
                    );
                    self.kinds.insert("math".to_string());
                }
                Event::InlineMath(tex) => {
                    let tex = tex.to_string();
                    pending.push(
                        view! { <span class="math-inline" data-tex={tex}></span> }.into_any(),
                    );
                    self.kinds.insert("math".to_string());
                }
                Event::TaskListMarker(_)
                | Event::FootnoteReference(_) => {}
                Event::End(_) => break,
            }
        }
        if !pending.is_empty() {
            out.push(paragraph(pending));
        }
        out
    }

    fn inline(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();
        loop {
            if matches!(self.events.peek(), None | Some(Event::End(_))) {
                break;
            }
            let ev = self.events.next().unwrap();
            if let Some(buf) = &mut self.heading_text {
                match &ev {
                    Event::Text(t) | Event::Code(t) => buf.push_str(t),
                    Event::SoftBreak | Event::HardBreak => buf.push(' '),
                    Event::InlineMath(tex) | Event::DisplayMath(tex) => buf.push_str(tex),
                    _ => {}
                }
            }
            match ev {
                Event::Text(t) => nodes.push(Node::Inline(t.to_string().into_any())),
                Event::Code(c) => nodes.push(Node::Inline(
                    view! { <code>{c.to_string()}</code> }.into_any(),
                )),
                Event::SoftBreak => nodes.push(Node::Inline(" ".into_any())),
                Event::HardBreak => {
                    nodes.push(Node::Inline(view! { <br /> }.into_any()))
                }
                Event::Html(h) | Event::InlineHtml(h) => {
                    for w in self.html_widgets(&h) {
                        nodes.push(Node::Widget(w));
                    }
                }
                Event::InlineMath(tex) => {
                    let tex = tex.to_string();
                    nodes.push(Node::Inline(
                        view! { <span class="math-inline" data-tex={tex}></span> }.into_any(),
                    ));
                    self.kinds.insert("math".to_string());
                }
                Event::DisplayMath(tex) => {
                    let tex = tex.to_string();
                    nodes.push(Node::Widget(
                        view! {
                            <div class="widget widget-math">
                                <div class="math-display" data-tex={tex}></div>
                            </div>
                        }
                        .into_any(),
                    ));
                    self.kinds.insert("math".to_string());
                }
                Event::Start(tag) => {
                    for v in self.start_tag(tag) {
                        nodes.push(Node::Inline(v));
                    }
                }
                _ => {}
            }
        }
        nodes
    }

    fn inline_plain(&mut self) -> String {
        let mut out = String::new();
        loop {
            if matches!(self.events.peek(), None | Some(Event::End(_))) {
                break;
            }
            match self.events.next().unwrap() {
                Event::Text(t) | Event::Code(t) => out.push_str(&t),
                Event::SoftBreak | Event::HardBreak => out.push(' '),
                Event::Start(_) => {
                    out.push_str(&self.inline_plain());
                    self.events.next();
                }
                _ => {}
            }
        }
        out
    }

    fn list_item(&mut self) -> AnyView {
        let checked = if matches!(self.events.peek(), Some(Event::TaskListMarker(_))) {
            match self.events.next() {
                Some(Event::TaskListMarker(b)) => Some(b),
                _ => None,
            }
        } else {
            None
        };
        let inner = self.blocks();
        match checked {
            Some(state) => {
                let checkbox: AnyView = if state {
                    view! { <input type="checkbox" checked disabled /> }.into_any()
                } else {
                    view! { <input type="checkbox" disabled /> }.into_any()
                };
                view! {
                    <li class="task-item">
                        <div class="task-row">{checkbox} {inner}</div>
                    </li>
                }
                .into_any()
            }
            None => view! { <li>{inner}</li> }.into_any(),
        }
    }

    fn consume_until_end(&mut self) {
        loop {
            match self.events.next() {
                None | Some(Event::End(_)) => break,
                Some(Event::Start(_)) => self.consume_until_end(),
                Some(_) => {}
            }
        }
    }

    fn start_tag(&mut self, tag: Tag) -> Vec<AnyView> {
        match tag {
            Tag::Paragraph => {
                let nodes = self.inline();
                self.events.next();
                partition(nodes)
            }
            Tag::Heading { level, .. } => {
                self.heading_text = Some(String::new());
                let nodes = into_views(self.inline());
                self.events.next();
                let text = self.heading_text.take().unwrap_or_default();
                let id = crate::content::course::slugify(&text);
                let id = if id == "unnamed" { None } else { Some(id) };
                vec![heading(level, id, nodes)]
            }
            Tag::BlockQuote(_) => {
                let inner = self.blocks();
                self.events.next();
                vec![view! { <blockquote>{inner}</blockquote> }.into_any()]
            }
            Tag::CodeBlock(kind) => {
                let mut text = String::new();
                loop {
                    match self.events.next() {
                        Some(Event::Text(t)) => text.push_str(&t),
                        Some(Event::End(_)) | None => break,
                        Some(_) => {}
                    }
                }
                let class = match &kind {
                    CodeBlockKind::Fenced(l) if !l.is_empty() => {
                        Some(format!("language-{l}"))
                    }
                    _ => None,
                };
                vec![view! {
                    <pre><code class=class>{text}</code></pre>
                }
                .into_any()]
            }
            Tag::HtmlBlock => {
                let mut chunk = String::new();
                loop {
                    match self.events.next() {
                        Some(Event::Html(h)) | Some(Event::Text(h)) => chunk.push_str(&h),
                        Some(Event::End(_)) | None => break,
                        Some(_) => {}
                    }
                }
                self.html_widgets(&chunk)
            }
            Tag::List(start) => {
                let mut items: Vec<AnyView> = Vec::new();
                loop {
                    match self.events.peek() {
                        Some(Event::Start(Tag::Item)) => {
                            self.events.next();
                            let item = self.list_item();
                            self.events.next();
                            items.push(item);
                        }
                        Some(Event::End(TagEnd::List(_))) => {
                            self.events.next();
                            break;
                        }
                        None | Some(Event::End(_)) => break,
                        _ => {
                            self.events.next();
                        }
                    }
                }
                match start {
                    Some(n) => {
                        let start_attr = n.to_string();
                        vec![view! { <ol start=start_attr>{items}</ol> }.into_any()]
                    }
                    None => vec![view! { <ul>{items}</ul> }.into_any()],
                }
            }
            Tag::Table(aligns) => {
                let mut head_cells: Vec<AnyView> = Vec::new();
                if matches!(self.events.peek(), Some(Event::Start(Tag::TableHead))) {
                    self.events.next();
                    loop {
                        match self.events.next() {
                            Some(Event::Start(Tag::TableCell)) => {
                                let nodes = into_views(self.inline());
                                self.events.next();
                                let align = aligns.get(head_cells.len());
                                head_cells.push(table_cell("th", align, nodes));
                            }
                            Some(Event::End(TagEnd::TableHead)) | None => break,
                            Some(_) => {}
                        }
                    }
                }
                let mut rows: Vec<AnyView> = Vec::new();
                loop {
                    match self.events.peek() {
                        Some(Event::Start(Tag::TableRow)) => {
                            self.events.next();
                            let mut cells: Vec<AnyView> = Vec::new();
                            loop {
                                match self.events.next() {
                                    Some(Event::Start(Tag::TableCell)) => {
                                        let nodes = into_views(self.inline());
                                        self.events.next();
                                        let align = aligns.get(cells.len());
                                        cells.push(table_cell("td", align, nodes));
                                    }
                                    Some(Event::End(TagEnd::TableRow)) | None => break,
                                    Some(_) => {}
                                }
                            }
                            rows.push(view! { <tr>{cells}</tr> }.into_any());
                        }
                        Some(Event::End(TagEnd::Table)) => {
                            self.events.next();
                            break;
                        }
                        None | Some(Event::End(_)) => break,
                        _ => {
                            self.events.next();
                        }
                    }
                }
                let head = view! { <thead><tr>{head_cells}</tr></thead> }.into_any();
                let body = view! { <tbody>{rows}</tbody> }.into_any();
                vec![view! {
                    <div class="table-wrap"><table>{head} {body}</table></div>
                }
                .into_any()]
            }
            Tag::Emphasis => {
                let inner = into_views(self.inline());
                self.events.next();
                vec![view! { <em>{inner}</em> }.into_any()]
            }
            Tag::Strong => {
                let inner = into_views(self.inline());
                self.events.next();
                vec![view! { <strong>{inner}</strong> }.into_any()]
            }
            Tag::Strikethrough => {
                let inner = into_views(self.inline());
                self.events.next();
                vec![view! { <s>{inner}</s> }.into_any()]
            }
            Tag::Link { dest_url, title, .. } => {
                let inner = into_views(self.inline());
                self.events.next();
                vec![self.link_view(&dest_url, &title, inner)]
            }
            Tag::Image { dest_url, title, .. } => {
                let alt = self.inline_plain();
                self.events.next();
                vec![self.image_view(&dest_url, &title, alt)]
            }
            Tag::FootnoteDefinition(_) | Tag::MetadataBlock(_) => {
                self.consume_until_end();
                vec![]
            }
            other => {
                tracing::debug!("generic handling for markdown tag: {other:?}");
                let inner = self.blocks();
                self.events.next();
                vec![view! { <div>{inner}</div> }.into_any()]
            }
        }
    }

    fn link_view(&mut self, dest: &str, title: &str, inner: Vec<AnyView>) -> AnyView {
        match resolve_link(dest, self.ctx, self.ctx.source, self.diags) {
            LinkResolution::Url(url, external) => {
                let title_attr = if title.is_empty() {
                    None
                } else {
                    Some(title.to_string())
                };
                if external {
                    view! {
                        <a href={url} target="_blank" rel="noopener" title=title_attr>{inner}</a>
                    }
                    .into_any()
                } else {
                    view! { <a href={url} title=title_attr>{inner}</a> }.into_any()
                }
            }
            LinkResolution::MissingLesson => {
                view! { <span class="dead-link">{inner}</span> }.into_any()
            }
        }
    }

    fn image_view(&mut self, dest: &str, title: &str, alt: String) -> AnyView {
        match resolve_link(dest, self.ctx, self.ctx.source, self.diags) {
            LinkResolution::Url(src, _) => {
                let title_attr = if title.is_empty() {
                    None
                } else {
                    Some(title.to_string())
                };
                view! {
                    <img src={src} alt={alt} title=title_attr loading="lazy" />
                }
                .into_any()
            }
            LinkResolution::MissingLesson => {
                view! { <span class="dead-link">{alt}</span> }.into_any()
            }
        }
    }
}

fn table_cell(tag: &str, align: Option<&Alignment>, nodes: Vec<AnyView>) -> AnyView {
    let style = align.and_then(align_style);
    match (tag, style) {
        ("th", Some(s)) => view! { <th style={s}>{nodes}</th> }.into_any(),
        ("th", None) => view! { <th>{nodes}</th> }.into_any(),
        (_, Some(s)) => view! { <td style={s}>{nodes}</td> }.into_any(),
        (_, None) => view! { <td>{nodes}</td> }.into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::course::load_course_dir;
    use crate::diag::Severity;

    fn fixture_course() -> (tempfile::TempDir, Course, std::collections::HashMap<String, String>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("01-a.md"), "---\ntitle: A\n---\nbody").unwrap();
        std::fs::write(dir.path().join("02-b.md"), "---\ntitle: B\n---\nnext").unwrap();
        let mut diags = Vec::new();
        let course = load_course_dir(dir.path(), &mut diags).unwrap();
        let by_file = course
            .lessons
            .iter()
            .map(|l| (l.file_name.clone(), l.slug.clone()))
            .collect();
        (dir, course, by_file)
    }

    fn render_doc(src: &str, course: &Course, by_file: &std::collections::HashMap<String, String>) -> String {
        let registry = WidgetRegistry::with_builtins();
        let ctx = MarkdownCtx {
            course,
            source: Path::new("01-a.md"),
            lessons_by_file: by_file,
        };
        let mut diags = Vec::new();
        render_markdown(src, &ctx, &registry, &mut diags)
            .views
            .into_iter()
            .map(|v| crate::render::render_html(move || v))
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn tight_list_items_render_one_paragraph_per_item() {
        let (_dir, course, by_file) = fixture_course();
        let html = render_doc(
            "- **Bold lead.** rest of item\n- second [link](02-b.md) item\n",
            &course,
            &by_file,
        );
        assert!(
            html.contains("<li><p><strong>Bold lead.</strong> rest of item</p></li>"),
            "bold lead must stay inside the item's paragraph: {html}"
        );
        let link_html = format!(
            "<li><p>second <a href=\"/course/{}/b\">link</a> item</p></li>",
            course.slug
        );
        assert!(
            html.contains(&link_html),
            "links must stay inside the item's paragraph: {html}"
        );
        assert!(!html.contains("</strong><p>"), "{html}");
        assert!(!html.contains("</p><a"), "{html}");
    }

    #[test]
    fn numbered_tight_list_items_render_one_paragraph_per_item() {
        let (_dir, course, by_file) = fixture_course();
        let html = render_doc(
            "1. **First rule.** text one\n2. **Second rule.** text two\n",
            &course,
            &by_file,
        );
        assert!(
            html.contains("<li><p><strong>First rule.</strong> text one</p></li>"),
            "{html}"
        );
        assert!(
            html.contains("<li><p><strong>Second rule.</strong> text two</p></li>"),
            "{html}"
        );
    }

    #[test]
    fn loose_list_items_keep_their_paragraphs() {
        let (_dir, course, by_file) = fixture_course();
        let html = render_doc(
            "- first paragraph\n\n  second paragraph\n",
            &course,
            &by_file,
        );
        assert!(html.contains("<li><p>first paragraph</p><p>second paragraph</p></li>"), "{html}");
    }

    fn tags(chunk: &str) -> Vec<WidgetTag> {
        let mut diags = Vec::new();
        scan_widget_tags(chunk, Path::new("test.md"), &mut diags)
    }

    #[test]
    fn scans_standard_widget_tag() {
        let t = tags(r#"<Widget type="quiz" id="com.example.q1" />"#);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].kind, "quiz");
        assert_eq!(t[0].id, "com.example.q1");
    }

    #[test]
    fn scans_mixed_case_and_single_quotes() {
        let t = tags("<widget TYPE='slider' id=\"x.y\"/>");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].kind, "slider");
        assert_eq!(t[0].id, "x.y");
    }

    #[test]
    fn handles_gt_inside_quoted_value() {
        let t = tags(r#"<Widget type="quiz" id="a>b.c" />"#);
        assert_eq!(t[0].id, "a>b.c");
    }

    #[test]
    fn finds_multiple_tags_in_chunk() {
        let t = tags(
            "<p>\n<Widget type=\"quiz\" id=\"a.b\" />\n<Widget type=\"plot\" id=\"c.d\" />\n</p>",
        );
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].kind, "quiz");
        assert_eq!(t[1].kind, "plot");
    }

    #[test]
    fn rejects_non_self_closing_tag() {
        let mut diags = Vec::new();
        let t = scan_widget_tags("<Widget type=\"quiz\" id=\"a.b\">", Path::new("t"), &mut diags);
        assert!(t.is_empty());
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn rejects_missing_attributes() {
        assert!(tags("<Widget type=\"quiz\" />").is_empty());
        assert!(tags("<Widget id=\"a.b\" />").is_empty());
    }

    #[test]
    fn ignores_lookalike_tags() {
        assert!(tags("<widgets type=\"quiz\" id=\"a.b\" />").is_empty());
        assert!(tags("</Widget>").is_empty());
        assert!(tags("<div>hello</div>").is_empty());
    }

    #[test]
    fn unterminated_tag_is_dropped_with_warning() {
        let mut diags = Vec::new();
        let t = scan_widget_tags("<Widget type=\"quiz\"", Path::new("t"), &mut diags);
        assert!(t.is_empty());
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn strips_duplicated_h1() {
        assert_eq!(strip_title_heading("# My Title\n\nBody", "My Title"), "Body");
        assert_eq!(strip_title_heading("# Other\n\nBody", "My Title"), "# Other\n\nBody");
        assert_eq!(strip_title_heading("No heading", "x"), "No heading");
    }

    #[test]
    fn link_resolution_rules() {
        let (_dir, course, by_file) = fixture_course();
        let src = Path::new("01-a.md");
        let ctx = MarkdownCtx {
            course: &course,
            source: src,
            lessons_by_file: &by_file,
        };
        let mut empty = Vec::new();

        assert!(matches!(
            resolve_link("02-b.md", &ctx, src, &mut empty),
            LinkResolution::Url(ref u, false) if *u == format!("/course/{}/b", course.slug)
        ));
        assert!(matches!(
            resolve_link("02-b.md#section", &ctx, src, &mut empty),
            LinkResolution::Url(ref u, false) if *u == format!("/course/{}/b#section", course.slug)
        ));
        assert!(matches!(
            resolve_link("https://x.y", &ctx, src, &mut empty),
            LinkResolution::Url(_, true)
        ));
        assert!(matches!(
            resolve_link("pic.png", &ctx, src, &mut empty),
            LinkResolution::Url(ref u, false) if *u == format!("/content/{}/pic.png", course.slug)
        ));
        assert!(matches!(
            resolve_link("missing.md", &ctx, src, &mut empty),
            LinkResolution::MissingLesson
        ));
    }
}
