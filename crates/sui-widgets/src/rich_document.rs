use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt,
    ops::Range,
    rc::Rc,
    sync::Arc,
};

use pulldown_cmark::{
    CodeBlockKind, Event as MarkdownEvent, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use sui_reactive::{Observable, Observer, Signal, SourceId, Subscription};

mod view;

pub use view::{
    RichDocumentRenderContext, RichDocumentRendererRegistry, RichDocumentView,
    RichDocumentViewState,
};

/// Stable identity for a block in a [`RichDocumentModel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RichBlockId(u64);

impl RichBlockId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RichBlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichInlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichLink {
    pub destination: String,
    pub title: Option<String>,
}

impl RichLink {
    pub fn new(destination: impl Into<String>) -> Self {
        Self {
            destination: destination.into(),
            title: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichInlineImage {
    pub source: String,
    pub alt: String,
    pub title: Option<String>,
}

impl RichInlineImage {
    pub fn new(source: impl Into<String>, alt: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            alt: alt.into(),
            title: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichInlineKind {
    Text,
    Code,
    Link(RichLink),
    Image(RichInlineImage),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichDocumentSpan {
    pub text: String,
    pub source_range: Range<usize>,
    pub style: RichInlineStyle,
    pub kind: RichInlineKind,
}

impl RichDocumentSpan {
    pub fn text(text: impl Into<String>, source_range: Range<usize>) -> Self {
        Self {
            text: text.into(),
            source_range,
            style: RichInlineStyle::default(),
            kind: RichInlineKind::Text,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichListItem {
    pub source_range: Range<usize>,
    pub spans: Vec<RichDocumentSpan>,
    pub checked: Option<bool>,
}

impl RichListItem {
    pub fn plain_text(&self) -> String {
        spans_plain_text(&self.spans)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RichDocumentStatus {
    #[default]
    Neutral,
    Pending,
    Running,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichAttachment {
    pub name: String,
    pub media_type: Option<String>,
    pub source: Option<String>,
    pub size_bytes: Option<u64>,
    pub description: Option<String>,
}

impl RichAttachment {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            media_type: None,
            source: None,
            size_bytes: None,
            description: None,
        }
    }
}

/// Application-defined structured block rendered through a registry entry.
///
/// `renderer` is an architecture-neutral dispatch key such as `tool-call`,
/// `diff`, or `operation-log`. The fallback renderer remains usable when an
/// application does not register a specialized widget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichExtensionBlock {
    pub renderer: String,
    pub title: String,
    pub summary: Option<String>,
    pub body: String,
    pub status: RichDocumentStatus,
    pub initially_expanded: bool,
    pub metadata: Vec<(String, String)>,
}

impl RichExtensionBlock {
    pub fn new(renderer: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            renderer: renderer.into(),
            title: title.into(),
            summary: None,
            body: String::new(),
            status: RichDocumentStatus::Neutral,
            initially_expanded: false,
            metadata: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RichDocumentBlockKind {
    Paragraph {
        spans: Vec<RichDocumentSpan>,
    },
    Heading {
        level: u8,
        spans: Vec<RichDocumentSpan>,
    },
    BlockQuote {
        label: Option<String>,
        spans: Vec<RichDocumentSpan>,
    },
    List {
        start: Option<u64>,
        items: Vec<RichListItem>,
    },
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    ThematicBreak,
    Attachment(RichAttachment),
    Extension(RichExtensionBlock),
}

impl RichDocumentBlockKind {
    pub fn renderer_key(&self) -> String {
        match self {
            Self::Paragraph { .. } => "paragraph".to_string(),
            Self::Heading { .. } => "heading".to_string(),
            Self::BlockQuote { .. } => "block-quote".to_string(),
            Self::List { .. } => "list".to_string(),
            Self::CodeBlock { .. } => "code-block".to_string(),
            Self::ThematicBreak => "thematic-break".to_string(),
            Self::Attachment(_) => "attachment".to_string(),
            Self::Extension(extension) => format!("extension:{}", extension.renderer),
        }
    }

    pub fn plain_text(&self) -> String {
        match self {
            Self::Paragraph { spans }
            | Self::Heading { spans, .. }
            | Self::BlockQuote { spans, .. } => spans_plain_text(spans),
            Self::List { start, items } => {
                let mut next = start.unwrap_or(1);
                items
                    .iter()
                    .map(|item| {
                        let marker = if start.is_some() {
                            let marker = format!("{next}. ");
                            next = next.saturating_add(1);
                            marker
                        } else {
                            "- ".to_string()
                        };
                        format!("{marker}{}", item.plain_text())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Self::CodeBlock { code, .. } => code.clone(),
            Self::ThematicBreak => String::new(),
            Self::Attachment(attachment) => attachment.name.clone(),
            Self::Extension(extension) => {
                if extension.body.is_empty() {
                    extension.title.clone()
                } else {
                    format!("{}\n{}", extension.title, extension.body)
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichDocumentBlock {
    pub id: RichBlockId,
    pub source_range: Range<usize>,
    pub kind: RichDocumentBlockKind,
}

impl RichDocumentBlock {
    pub fn plain_text(&self) -> String {
        self.kind.plain_text()
    }

    pub fn renderer_key(&self) -> String {
        self.kind.renderer_key()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichDocumentUpdate {
    pub revision: u64,
    pub reparsed_source: Range<usize>,
    pub reused_prefix_blocks: usize,
    pub changed_block_ids: Vec<RichBlockId>,
    pub append_only: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RichDocumentSnapshot {
    pub revision: u64,
    pub markdown: String,
    pub blocks: Vec<RichDocumentBlock>,
    pub last_update: RichDocumentUpdate,
}

#[derive(Clone)]
pub struct RichDocumentModel {
    inner: Rc<RefCell<RichDocumentModelInner>>,
    revision: Signal<u64>,
    structure_revision: Signal<u64>,
}

struct RichDocumentModelInner {
    markdown: String,
    segments: Vec<RichDocumentSegment>,
    block_signals: HashMap<RichBlockId, Signal<RichDocumentBlock>>,
    ids_by_origin: HashMap<(usize, &'static str), RichBlockId>,
    next_id: u64,
    revision: u64,
    last_update: RichDocumentUpdate,
}

enum RichDocumentSegment {
    Markdown {
        source_range: Range<usize>,
        blocks: Vec<RichDocumentBlock>,
    },
    Structured(RichDocumentBlock),
}

impl RichDocumentSegment {
    fn block_count(&self) -> usize {
        match self {
            Self::Markdown { blocks, .. } => blocks.len(),
            Self::Structured(_) => 1,
        }
    }
}

impl RichDocumentModel {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(RichDocumentModelInner {
                markdown: String::new(),
                segments: Vec::new(),
                block_signals: HashMap::new(),
                ids_by_origin: HashMap::new(),
                next_id: 1,
                revision: 0,
                last_update: RichDocumentUpdate::default(),
            })),
            revision: Signal::named("RichDocumentModel", 0),
            structure_revision: Signal::named("RichDocumentModel::structure", 0),
        }
    }

    pub fn from_markdown(markdown: impl Into<String>) -> Self {
        let model = Self::new();
        model.set_markdown(markdown);
        model
    }

    pub fn revision(&self) -> u64 {
        self.inner.borrow().revision
    }

    pub fn markdown(&self) -> String {
        self.inner.borrow().markdown.clone()
    }

    pub fn blocks(&self) -> Vec<RichDocumentBlock> {
        flatten_blocks(&self.inner.borrow())
    }

    pub fn snapshot(&self) -> RichDocumentSnapshot {
        let inner = self.inner.borrow();
        RichDocumentSnapshot {
            revision: inner.revision,
            markdown: inner.markdown.clone(),
            blocks: flatten_blocks(&inner),
            last_update: inner.last_update.clone(),
        }
    }

    pub fn last_update(&self) -> RichDocumentUpdate {
        self.inner.borrow().last_update.clone()
    }

    pub(crate) fn structure_observable(&self) -> &Signal<u64> {
        &self.structure_revision
    }

    pub(crate) fn block_signal(&self, id: RichBlockId) -> Option<Signal<RichDocumentBlock>> {
        self.inner.borrow().block_signals.get(&id).cloned()
    }

    pub fn set_markdown(&self, markdown: impl Into<String>) -> bool {
        let markdown = markdown.into();
        let (changed, structure_changed) = {
            let mut inner = self.inner.borrow_mut();
            if inner.markdown == markdown {
                return false;
            }
            let old = flatten_blocks(&inner);
            inner.markdown = markdown;
            let source_len = inner.markdown.len();
            let parsed = parse_markdown_blocks(&inner.markdown, 0);
            let markdown_blocks = assign_block_ids(&mut inner, parsed);
            let structured = inner
                .segments
                .drain(..)
                .filter_map(|segment| match segment {
                    RichDocumentSegment::Structured(mut block) => {
                        block.source_range = source_len..source_len;
                        Some(block)
                    }
                    RichDocumentSegment::Markdown { .. } => None,
                })
                .map(RichDocumentSegment::Structured)
                .collect::<Vec<_>>();
            inner.segments = Vec::with_capacity(structured.len() + 1);
            if source_len > 0 {
                inner.segments.push(RichDocumentSegment::Markdown {
                    source_range: 0..source_len,
                    blocks: markdown_blocks,
                });
            }
            inner.segments.extend(structured);
            let structure_changed = commit_update(&mut inner, old, 0..source_len, 0, false);
            (inner.revision, structure_changed)
        };
        let _ = self.revision.set(changed);
        if structure_changed {
            let _ = self.structure_revision.set(changed);
        }
        true
    }

    /// Append a streaming Markdown fragment and reparse only the mutable tail.
    ///
    /// All blocks before the final Markdown block are retained byte-for-byte,
    /// including their stable IDs and any widget-local state keyed by them.
    pub fn append_markdown(&self, fragment: &str) -> bool {
        if fragment.is_empty() {
            return false;
        }
        let (changed, structure_changed) = {
            let mut inner = self.inner.borrow_mut();
            let old = flatten_blocks(&inner);
            let old_source_len = inner.markdown.len();
            let last_markdown = inner.segments.last().and_then(|segment| match segment {
                RichDocumentSegment::Markdown {
                    source_range,
                    blocks,
                } => Some((source_range.clone(), blocks.clone())),
                RichDocumentSegment::Structured(_) => None,
            });
            inner.markdown.push_str(fragment);
            let source_len = inner.markdown.len();
            let (reparse_start, reused_prefix_blocks) = if let Some((range, blocks)) = last_markdown
            {
                let reparse_start = blocks
                    .last()
                    .map(|block| block.source_range.start)
                    .unwrap_or(range.start)
                    .min(old_source_len);
                let local_prefix = blocks
                    .iter()
                    .take_while(|block| block.source_range.end <= reparse_start)
                    .count();
                let segment_prefix = inner.segments[..inner.segments.len().saturating_sub(1)]
                    .iter()
                    .map(RichDocumentSegment::block_count)
                    .sum::<usize>();
                let parsed = parse_markdown_blocks(&inner.markdown[reparse_start..], reparse_start);
                let mut next = blocks[..local_prefix].to_vec();
                next.extend(assign_block_ids(&mut inner, parsed));
                if let Some(RichDocumentSegment::Markdown {
                    source_range,
                    blocks,
                }) = inner.segments.last_mut()
                {
                    source_range.end = source_len;
                    *blocks = next;
                }
                (reparse_start, segment_prefix + local_prefix)
            } else {
                let reused = old.len();
                let parsed = parse_markdown_blocks(fragment, old_source_len);
                let blocks = assign_block_ids(&mut inner, parsed);
                inner.segments.push(RichDocumentSegment::Markdown {
                    source_range: old_source_len..source_len,
                    blocks,
                });
                (old_source_len, reused)
            };
            let structure_changed = commit_update(
                &mut inner,
                old,
                reparse_start..source_len,
                reused_prefix_blocks,
                true,
            );
            (inner.revision, structure_changed)
        };
        let _ = self.revision.set(changed);
        if structure_changed {
            let _ = self.structure_revision.set(changed);
        }
        true
    }

    pub fn append_attachment(&self, attachment: RichAttachment) -> RichBlockId {
        self.append_structured(RichDocumentBlockKind::Attachment(attachment))
    }

    pub fn append_extension(&self, extension: RichExtensionBlock) -> RichBlockId {
        self.append_structured(RichDocumentBlockKind::Extension(extension))
    }

    pub fn update_structured(&self, id: RichBlockId, kind: RichDocumentBlockKind) -> bool {
        if !matches!(
            kind,
            RichDocumentBlockKind::Attachment(_) | RichDocumentBlockKind::Extension(_)
        ) {
            return false;
        }
        let (changed, structure_changed) = {
            let mut inner = self.inner.borrow_mut();
            let old = flatten_blocks(&inner);
            let Some(segment_index) = inner.segments.iter().position(
                |segment| matches!(segment, RichDocumentSegment::Structured(block) if block.id == id),
            )
            else {
                return false;
            };
            let reused_prefix_blocks = inner.segments[..segment_index]
                .iter()
                .map(RichDocumentSegment::block_count)
                .sum();
            let RichDocumentSegment::Structured(block) = &mut inner.segments[segment_index] else {
                unreachable!("structured segment index changed")
            };
            if block.kind == kind {
                return false;
            }
            block.kind = kind;
            inner.revision = inner.revision.wrapping_add(1);
            inner.last_update = RichDocumentUpdate {
                revision: inner.revision,
                reparsed_source: inner.markdown.len()..inner.markdown.len(),
                reused_prefix_blocks,
                changed_block_ids: vec![id],
                append_only: false,
            };
            let structure_changed = reconcile_block_signals(&mut inner, &old);
            (inner.revision, structure_changed)
        };
        let _ = self.revision.set(changed);
        if structure_changed {
            let _ = self.structure_revision.set(changed);
        }
        true
    }

    fn append_structured(&self, kind: RichDocumentBlockKind) -> RichBlockId {
        let (id, revision, structure_changed) = {
            let mut inner = self.inner.borrow_mut();
            let old = flatten_blocks(&inner);
            let id = RichBlockId::new(inner.next_id);
            inner.next_id = inner.next_id.wrapping_add(1).max(1);
            let end = inner.markdown.len();
            inner
                .segments
                .push(RichDocumentSegment::Structured(RichDocumentBlock {
                    id,
                    source_range: end..end,
                    kind,
                }));
            inner.revision = inner.revision.wrapping_add(1);
            inner.last_update = RichDocumentUpdate {
                revision: inner.revision,
                reparsed_source: end..end,
                reused_prefix_blocks: inner
                    .segments
                    .iter()
                    .take(inner.segments.len().saturating_sub(1))
                    .map(RichDocumentSegment::block_count)
                    .sum(),
                changed_block_ids: vec![id],
                append_only: true,
            };
            let structure_changed = reconcile_block_signals(&mut inner, &old);
            (id, inner.revision, structure_changed)
        };
        let _ = self.revision.set(revision);
        if structure_changed {
            let _ = self.structure_revision.set(revision);
        }
        id
    }
}

impl Default for RichDocumentModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Observable<u64> for RichDocumentModel {
    fn source_id(&self) -> SourceId {
        self.revision.source_id()
    }

    fn source_name(&self) -> Arc<str> {
        self.revision.source_name()
    }

    fn get(&self) -> u64 {
        self.revision.get()
    }

    fn subscribe(&self, observer: Observer) -> Subscription {
        self.revision.subscribe(observer)
    }
}

fn flatten_blocks(inner: &RichDocumentModelInner) -> Vec<RichDocumentBlock> {
    let mut blocks = Vec::new();
    for segment in &inner.segments {
        match segment {
            RichDocumentSegment::Markdown {
                blocks: markdown, ..
            } => blocks.extend(markdown.iter().cloned()),
            RichDocumentSegment::Structured(block) => blocks.push(block.clone()),
        }
    }
    blocks
}

fn assign_block_ids(
    inner: &mut RichDocumentModelInner,
    parsed: Vec<ParsedBlock>,
) -> Vec<RichDocumentBlock> {
    parsed
        .into_iter()
        .map(|parsed| {
            let origin = (parsed.source_range.start, block_discriminant(&parsed.kind));
            let id = *inner.ids_by_origin.entry(origin).or_insert_with(|| {
                let id = RichBlockId::new(inner.next_id);
                inner.next_id = inner.next_id.wrapping_add(1).max(1);
                id
            });
            RichDocumentBlock {
                id,
                source_range: parsed.source_range,
                kind: parsed.kind,
            }
        })
        .collect()
}

fn commit_update(
    inner: &mut RichDocumentModelInner,
    old: Vec<RichDocumentBlock>,
    reparsed_source: Range<usize>,
    reused_prefix_blocks: usize,
    append_only: bool,
) -> bool {
    inner.revision = inner.revision.wrapping_add(1);
    let old_by_id = old
        .iter()
        .cloned()
        .map(|block| (block.id, block))
        .collect::<HashMap<_, _>>();
    let next = flatten_blocks(inner);
    let next_ids = next.iter().map(|block| block.id).collect::<HashSet<_>>();
    let mut changed_block_ids = next
        .iter()
        .filter(|block| old_by_id.get(&block.id) != Some(block))
        .map(|block| block.id)
        .collect::<Vec<_>>();
    changed_block_ids.extend(
        old.iter()
            .filter(|block| !next_ids.contains(&block.id))
            .map(|block| block.id),
    );
    inner.last_update = RichDocumentUpdate {
        revision: inner.revision,
        reparsed_source,
        reused_prefix_blocks,
        changed_block_ids,
        append_only,
    };
    reconcile_block_signals(inner, &old)
}

fn reconcile_block_signals(inner: &mut RichDocumentModelInner, old: &[RichDocumentBlock]) -> bool {
    let next = flatten_blocks(inner);
    let old_structure = old
        .iter()
        .map(|block| (block.id, block.renderer_key()))
        .collect::<Vec<_>>();
    let next_structure = next
        .iter()
        .map(|block| (block.id, block.renderer_key()))
        .collect::<Vec<_>>();
    let live = next.iter().map(|block| block.id).collect::<HashSet<_>>();
    inner.block_signals.retain(|id, _| live.contains(id));
    for block in next {
        if let Some(signal) = inner.block_signals.get(&block.id) {
            let _ = signal.set(block);
        } else {
            inner.block_signals.insert(
                block.id,
                Signal::named(format!("RichDocumentBlock({})", block.id), block),
            );
        }
    }
    old_structure != next_structure
}

fn block_discriminant(kind: &RichDocumentBlockKind) -> &'static str {
    match kind {
        RichDocumentBlockKind::Paragraph { .. } => "paragraph",
        RichDocumentBlockKind::Heading { .. } => "heading",
        RichDocumentBlockKind::BlockQuote { .. } => "quote",
        RichDocumentBlockKind::List { .. } => "list",
        RichDocumentBlockKind::CodeBlock { .. } => "code",
        RichDocumentBlockKind::ThematicBreak => "rule",
        RichDocumentBlockKind::Attachment(_) => "attachment",
        RichDocumentBlockKind::Extension(_) => "extension",
    }
}

struct ParsedBlock {
    source_range: Range<usize>,
    kind: RichDocumentBlockKind,
}

#[derive(Clone, Default)]
struct InlineContext {
    strong: usize,
    emphasis: usize,
    strikethrough: usize,
    code: usize,
    link: Option<RichLink>,
    image: Option<RichInlineImage>,
}

enum ParseBlock {
    Text {
        start: usize,
        end: usize,
        kind: TextBlockKind,
        spans: Vec<RichDocumentSpan>,
    },
    Code {
        start: usize,
        end: usize,
        language: Option<String>,
        code: String,
    },
    List {
        start: usize,
        end: usize,
        ordered_start: Option<u64>,
        items: Vec<RichListItem>,
        current_item: Option<RichListItem>,
    },
}

#[derive(Clone)]
enum TextBlockKind {
    Paragraph,
    Heading(u8),
    Quote(Option<String>),
}

fn parse_markdown_blocks(markdown: &str, base: usize) -> Vec<ParsedBlock> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_GFM);
    let mut blocks = Vec::new();
    let mut current: Option<ParseBlock> = None;
    let mut inline = InlineContext::default();
    let mut quote_label: Option<String> = None;

    for (event, local_range) in Parser::new_ext(markdown, options).into_offset_iter() {
        let range = base + local_range.start..base + local_range.end;
        match event {
            MarkdownEvent::Start(Tag::Paragraph) => {
                if current.is_none() {
                    current = Some(ParseBlock::Text {
                        start: range.start,
                        end: range.end,
                        kind: TextBlockKind::Paragraph,
                        spans: Vec::new(),
                    });
                }
            }
            MarkdownEvent::End(TagEnd::Paragraph) => {
                extend_end(&mut current, range.end);
                if !matches!(current, Some(ParseBlock::List { .. })) {
                    finish_block(&mut current, &mut blocks);
                }
            }
            MarkdownEvent::Start(Tag::Heading { level, .. }) => {
                finish_block(&mut current, &mut blocks);
                current = Some(ParseBlock::Text {
                    start: range.start,
                    end: range.end,
                    kind: TextBlockKind::Heading(heading_level(level)),
                    spans: Vec::new(),
                });
            }
            MarkdownEvent::End(TagEnd::Heading(_)) => {
                extend_end(&mut current, range.end);
                finish_block(&mut current, &mut blocks);
            }
            MarkdownEvent::Start(Tag::BlockQuote(kind)) => {
                finish_block(&mut current, &mut blocks);
                quote_label = kind.map(|kind| format!("{kind:?}"));
                current = Some(ParseBlock::Text {
                    start: range.start,
                    end: range.end,
                    kind: TextBlockKind::Quote(quote_label.clone()),
                    spans: Vec::new(),
                });
            }
            MarkdownEvent::End(TagEnd::BlockQuote(_)) => {
                extend_end(&mut current, range.end);
                finish_block(&mut current, &mut blocks);
                quote_label = None;
            }
            MarkdownEvent::Start(Tag::CodeBlock(kind)) => {
                finish_block(&mut current, &mut blocks);
                current = Some(ParseBlock::Code {
                    start: range.start,
                    end: range.end,
                    language: match kind {
                        CodeBlockKind::Indented => None,
                        CodeBlockKind::Fenced(language) => {
                            let language = language.trim();
                            (!language.is_empty()).then(|| language.to_string())
                        }
                    },
                    code: String::new(),
                });
            }
            MarkdownEvent::End(TagEnd::CodeBlock) => {
                extend_end(&mut current, range.end);
                finish_block(&mut current, &mut blocks);
            }
            MarkdownEvent::Start(Tag::List(start)) => {
                if !matches!(current, Some(ParseBlock::List { .. })) {
                    finish_block(&mut current, &mut blocks);
                    current = Some(ParseBlock::List {
                        start: range.start,
                        end: range.end,
                        ordered_start: start,
                        items: Vec::new(),
                        current_item: None,
                    });
                }
            }
            MarkdownEvent::End(TagEnd::List(_)) => {
                finish_list_item(&mut current, range.end);
                extend_end(&mut current, range.end);
                finish_block(&mut current, &mut blocks);
            }
            MarkdownEvent::Start(Tag::Item) => {
                finish_list_item(&mut current, range.start);
                if let Some(ParseBlock::List { current_item, .. }) = &mut current {
                    *current_item = Some(RichListItem {
                        source_range: range.clone(),
                        spans: Vec::new(),
                        checked: None,
                    });
                }
            }
            MarkdownEvent::End(TagEnd::Item) => finish_list_item(&mut current, range.end),
            MarkdownEvent::Start(Tag::Emphasis) => inline.emphasis += 1,
            MarkdownEvent::End(TagEnd::Emphasis) => {
                inline.emphasis = inline.emphasis.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Strong) => inline.strong += 1,
            MarkdownEvent::End(TagEnd::Strong) => {
                inline.strong = inline.strong.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Strikethrough) => inline.strikethrough += 1,
            MarkdownEvent::End(TagEnd::Strikethrough) => {
                inline.strikethrough = inline.strikethrough.saturating_sub(1);
            }
            MarkdownEvent::Start(Tag::Link {
                dest_url, title, ..
            }) => {
                inline.link = Some(RichLink {
                    destination: dest_url.to_string(),
                    title: (!title.is_empty()).then(|| title.to_string()),
                });
            }
            MarkdownEvent::End(TagEnd::Link) => inline.link = None,
            MarkdownEvent::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                inline.image = Some(RichInlineImage {
                    source: dest_url.to_string(),
                    alt: String::new(),
                    title: (!title.is_empty()).then(|| title.to_string()),
                });
            }
            MarkdownEvent::End(TagEnd::Image) => {
                if let Some(image) = inline.image.take() {
                    let display = if image.alt.is_empty() {
                        "Image".to_string()
                    } else {
                        image.alt.clone()
                    };
                    push_span(&mut current, display, range, &inline, Some(image));
                }
            }
            MarkdownEvent::Text(text) => {
                if let Some(image) = &mut inline.image {
                    image.alt.push_str(&text);
                } else if let Some(ParseBlock::Code { code, end, .. }) = &mut current {
                    code.push_str(&text);
                    *end = (*end).max(range.end);
                } else {
                    ensure_text_block(&mut current, range.start, quote_label.clone());
                    push_span(&mut current, text.to_string(), range, &inline, None);
                }
            }
            MarkdownEvent::Code(code) => {
                ensure_text_block(&mut current, range.start, quote_label.clone());
                inline.code += 1;
                push_span(&mut current, code.to_string(), range, &inline, None);
                inline.code = inline.code.saturating_sub(1);
            }
            MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => {
                ensure_text_block(&mut current, range.start, quote_label.clone());
                push_span(&mut current, "\n".to_string(), range, &inline, None);
            }
            MarkdownEvent::TaskListMarker(checked) => {
                if let Some(ParseBlock::List {
                    current_item: Some(item),
                    ..
                }) = &mut current
                {
                    item.checked = Some(checked);
                }
            }
            MarkdownEvent::Rule => {
                finish_block(&mut current, &mut blocks);
                blocks.push(ParsedBlock {
                    source_range: range,
                    kind: RichDocumentBlockKind::ThematicBreak,
                });
            }
            MarkdownEvent::Html(html) | MarkdownEvent::InlineHtml(html) => {
                ensure_text_block(&mut current, range.start, quote_label.clone());
                push_span(&mut current, html.to_string(), range, &inline, None);
            }
            MarkdownEvent::FootnoteReference(reference) => {
                ensure_text_block(&mut current, range.start, quote_label.clone());
                push_span(&mut current, format!("[{reference}]"), range, &inline, None);
            }
            _ => {}
        }
    }
    finish_block(&mut current, &mut blocks);
    blocks
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn ensure_text_block(current: &mut Option<ParseBlock>, start: usize, quote: Option<String>) {
    if current.is_none() {
        *current = Some(ParseBlock::Text {
            start,
            end: start,
            kind: quote.map_or(TextBlockKind::Paragraph, |label| {
                TextBlockKind::Quote(Some(label))
            }),
            spans: Vec::new(),
        });
    }
}

fn push_span(
    current: &mut Option<ParseBlock>,
    text: String,
    source_range: Range<usize>,
    inline: &InlineContext,
    image: Option<RichInlineImage>,
) {
    let style = RichInlineStyle {
        strong: inline.strong > 0,
        emphasis: inline.emphasis > 0,
        strikethrough: inline.strikethrough > 0,
    };
    let kind = if let Some(image) = image {
        RichInlineKind::Image(image)
    } else if inline.code > 0 {
        RichInlineKind::Code
    } else if let Some(link) = &inline.link {
        RichInlineKind::Link(link.clone())
    } else {
        RichInlineKind::Text
    };
    let span = RichDocumentSpan {
        text,
        source_range: source_range.clone(),
        style,
        kind,
    };
    match current {
        Some(ParseBlock::Text { spans, end, .. }) => {
            spans.push(span);
            *end = (*end).max(source_range.end);
        }
        Some(ParseBlock::List {
            current_item, end, ..
        }) => {
            if current_item.is_none() {
                *current_item = Some(RichListItem {
                    source_range: source_range.clone(),
                    spans: Vec::new(),
                    checked: None,
                });
            }
            if let Some(item) = current_item {
                item.source_range.end = item.source_range.end.max(source_range.end);
                item.spans.push(span);
            }
            *end = (*end).max(source_range.end);
        }
        Some(ParseBlock::Code { .. }) | None => {}
    }
}

fn finish_list_item(current: &mut Option<ParseBlock>, end: usize) {
    if let Some(ParseBlock::List {
        items,
        current_item,
        ..
    }) = current
        && let Some(mut item) = current_item.take()
    {
        item.source_range.end = item.source_range.end.max(end);
        items.push(item);
    }
}

fn extend_end(current: &mut Option<ParseBlock>, end: usize) {
    match current {
        Some(ParseBlock::Text { end: current, .. })
        | Some(ParseBlock::Code { end: current, .. })
        | Some(ParseBlock::List { end: current, .. }) => *current = (*current).max(end),
        None => {}
    }
}

fn finish_block(current: &mut Option<ParseBlock>, blocks: &mut Vec<ParsedBlock>) {
    let Some(block) = current.take() else {
        return;
    };
    let parsed = match block {
        ParseBlock::Text {
            start,
            end,
            kind,
            spans,
        } => {
            if spans.is_empty() {
                return;
            }
            let kind = match kind {
                TextBlockKind::Paragraph => RichDocumentBlockKind::Paragraph { spans },
                TextBlockKind::Heading(level) => RichDocumentBlockKind::Heading { level, spans },
                TextBlockKind::Quote(label) => RichDocumentBlockKind::BlockQuote { label, spans },
            };
            ParsedBlock {
                source_range: start..end.max(start),
                kind,
            }
        }
        ParseBlock::Code {
            start,
            end,
            language,
            code,
        } => ParsedBlock {
            source_range: start..end.max(start),
            kind: RichDocumentBlockKind::CodeBlock { language, code },
        },
        ParseBlock::List {
            start,
            end,
            ordered_start,
            mut items,
            current_item,
        } => {
            if let Some(item) = current_item {
                items.push(item);
            }
            ParsedBlock {
                source_range: start..end.max(start),
                kind: RichDocumentBlockKind::List {
                    start: ordered_start,
                    items,
                },
            }
        }
    };
    blocks.push(parsed);
}

fn spans_plain_text(spans: &[RichDocumentSpan]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichSyntaxTokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Type,
    Function,
    Property,
    Added,
    Removed,
    Header,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichSyntaxSpan {
    pub range: Range<usize>,
    pub kind: RichSyntaxTokenKind,
}

pub trait RichSyntaxHighlighter {
    fn highlight(&self, language: Option<&str>, source: &str) -> Vec<RichSyntaxSpan>;
}

impl<F> RichSyntaxHighlighter for F
where
    F: Fn(Option<&str>, &str) -> Vec<RichSyntaxSpan>,
{
    fn highlight(&self, language: Option<&str>, source: &str) -> Vec<RichSyntaxSpan> {
        self(language, source)
    }
}

/// Lightweight dependency-free lexer used by the default code-block renderer.
///
/// It deliberately recognizes broad token classes rather than implementing a
/// language grammar. Applications can replace it with tree-sitter, syntect, or
/// an LSP-backed highlighter through [`RichSyntaxHighlighter`].
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicSyntaxHighlighter;

impl RichSyntaxHighlighter for BasicSyntaxHighlighter {
    fn highlight(&self, language: Option<&str>, source: &str) -> Vec<RichSyntaxSpan> {
        let language = language.unwrap_or_default().to_ascii_lowercase();
        if matches!(language.as_str(), "diff" | "patch") {
            return highlight_diff(source);
        }
        highlight_code(&language, source)
    }
}

fn highlight_diff(source: &str) -> Vec<RichSyntaxSpan> {
    let mut spans = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let kind = if line.starts_with("@@") || line.starts_with("diff ") {
            Some(RichSyntaxTokenKind::Header)
        } else if line.starts_with('+') && !line.starts_with("+++") {
            Some(RichSyntaxTokenKind::Added)
        } else if line.starts_with('-') && !line.starts_with("---") {
            Some(RichSyntaxTokenKind::Removed)
        } else {
            None
        };
        if let Some(kind) = kind {
            spans.push(RichSyntaxSpan {
                range: offset..offset + line.len(),
                kind,
            });
        }
        offset += line.len();
    }
    spans
}

fn highlight_code(language: &str, source: &str) -> Vec<RichSyntaxSpan> {
    let mut spans = Vec::new();
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if starts_line_comment(language, bytes, index) {
            let end = source[index..]
                .find('\n')
                .map_or(source.len(), |relative| index + relative);
            spans.push(RichSyntaxSpan {
                range: index..end,
                kind: RichSyntaxTokenKind::Comment,
            });
            index = end;
            continue;
        }
        if matches!(bytes[index], b'\'' | b'"' | b'`') {
            let quote = bytes[index];
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == quote {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            spans.push(RichSyntaxSpan {
                range: start..index,
                kind: RichSyntaxTokenKind::String,
            });
            continue;
        }
        if bytes[index].is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || matches!(bytes[index], b'.' | b'_' | b'x' | b'o' | b'b'))
            {
                index += 1;
            }
            spans.push(RichSyntaxSpan {
                range: start..index,
                kind: RichSyntaxTokenKind::Number,
            });
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let word = &source[start..index];
            if is_keyword(language, word) {
                spans.push(RichSyntaxSpan {
                    range: start..index,
                    kind: RichSyntaxTokenKind::Keyword,
                });
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                spans.push(RichSyntaxSpan {
                    range: start..index,
                    kind: RichSyntaxTokenKind::Type,
                });
            }
            continue;
        }
        index += source[index..].chars().next().map_or(1, char::len_utf8);
    }
    spans
}

fn starts_line_comment(language: &str, bytes: &[u8], index: usize) -> bool {
    if bytes.get(index..index + 2) == Some(b"//") {
        return true;
    }
    bytes[index] == b'#'
        && matches!(
            language,
            "python" | "py" | "shell" | "sh" | "bash" | "zsh" | "toml" | "yaml" | "yml"
        )
}

fn is_keyword(language: &str, word: &str) -> bool {
    const COMMON: &[&str] = &[
        "as",
        "async",
        "await",
        "break",
        "case",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "fn",
        "for",
        "from",
        "function",
        "if",
        "impl",
        "import",
        "in",
        "interface",
        "let",
        "loop",
        "match",
        "mod",
        "move",
        "mut",
        "new",
        "none",
        "null",
        "pub",
        "return",
        "self",
        "static",
        "struct",
        "super",
        "switch",
        "trait",
        "true",
        "try",
        "type",
        "use",
        "var",
        "where",
        "while",
        "yield",
    ];
    COMMON.contains(&word)
        || (matches!(language, "python" | "py")
            && matches!(
                word,
                "def" | "elif" | "except" | "finally" | "lambda" | "with"
            ))
}

#[cfg(test)]
mod model_tests {
    use super::*;

    #[test]
    fn markdown_parser_preserves_structure_and_inline_actions() {
        let model = RichDocumentModel::from_markdown(
            "# Report\n\nRead [docs](https://example.test) and `code`.\n\n- one\n- [x] two\n\n```rust\nlet n = 42;\n```",
        );
        let blocks = model.blocks();
        assert!(matches!(
            blocks[0].kind,
            RichDocumentBlockKind::Heading { level: 1, .. }
        ));
        assert!(matches!(
            blocks[1].kind,
            RichDocumentBlockKind::Paragraph { ref spans }
                if spans.iter().any(|span| matches!(span.kind, RichInlineKind::Link(_)))
                    && spans.iter().any(|span| matches!(span.kind, RichInlineKind::Code))
        ));
        assert!(matches!(
            blocks[2].kind,
            RichDocumentBlockKind::List { ref items, .. }
                if items.len() == 2 && items[1].checked == Some(true)
        ));
        assert!(matches!(
            blocks[3].kind,
            RichDocumentBlockKind::CodeBlock { ref language, ref code }
                if language.as_deref() == Some("rust") && code.contains("let n")
        ));
    }

    #[test]
    fn streaming_append_reparses_only_tail_and_preserves_prefix_ids() {
        let model = RichDocumentModel::from_markdown("# Stable\n\nStreaming");
        let before = model.blocks();
        model.append_markdown(" text\n\n- next");
        let after = model.blocks();
        assert_eq!(before[0].id, after[0].id);
        assert_eq!(before[1].id, after[1].id);
        assert_eq!(after[1].plain_text(), "Streaming text");
        let update = model.last_update();
        assert!(update.append_only);
        assert_eq!(update.reused_prefix_blocks, 1);
        assert_eq!(update.reparsed_source.start, before[1].source_range.start);
    }

    #[test]
    fn structured_blocks_update_in_place() {
        let model = RichDocumentModel::new();
        let id = model.append_extension(RichExtensionBlock::new("tool-call", "Search"));
        let mut changed = RichExtensionBlock::new("tool-call", "Search");
        changed.status = RichDocumentStatus::Success;
        changed.body = "3 results".to_string();
        assert!(model.update_structured(id, RichDocumentBlockKind::Extension(changed)));
        assert_eq!(model.blocks()[0].id, id);
        assert_eq!(model.last_update().changed_block_ids, vec![id]);
    }

    #[test]
    fn markdown_segments_remain_ordered_around_structured_blocks() {
        let model = RichDocumentModel::from_markdown("Before");
        let before_id = model.blocks()[0].id;
        let extension_id =
            model.append_extension(RichExtensionBlock::new("tool-call", "Operation"));

        model.append_markdown("\n\nAfter");
        let first = model.blocks();
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].id, before_id);
        assert_eq!(first[1].id, extension_id);
        assert_eq!(first[2].plain_text(), "After");
        let after_id = first[2].id;

        model.append_markdown(" continued");
        let second = model.blocks();
        assert_eq!(second[0].id, before_id);
        assert_eq!(second[1].id, extension_id);
        assert_eq!(second[2].id, after_id);
        assert_eq!(second[2].plain_text(), "After continued");
        assert_eq!(model.last_update().reused_prefix_blocks, 2);
    }

    #[test]
    fn replacement_diagnostics_include_removed_block_ids() {
        let model = RichDocumentModel::from_markdown("# Heading\n\nParagraph");
        let removed = model.blocks()[1].id;
        model.set_markdown("# Heading");
        assert!(model.last_update().changed_block_ids.contains(&removed));
    }

    #[test]
    fn basic_highlighter_handles_code_and_diff_tokens() {
        let rust = BasicSyntaxHighlighter.highlight(Some("rust"), "let value = 42; // count");
        assert!(
            rust.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Keyword)
        );
        assert!(
            rust.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Number)
        );
        assert!(
            rust.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Comment)
        );

        let diff = BasicSyntaxHighlighter.highlight(Some("diff"), "@@ -1 +1 @@\n-old\n+new\n");
        assert!(
            diff.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Header)
        );
        assert!(
            diff.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Removed)
        );
        assert!(
            diff.iter()
                .any(|span| span.kind == RichSyntaxTokenKind::Added)
        );
    }
}
