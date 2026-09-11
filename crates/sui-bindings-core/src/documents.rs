use crate::application::normalized_option_name;
use std::fmt;
use sui::RichAttachment;
use sui::RichDocumentModel;
use sui::RichDocumentStatus;
use sui::RichDocumentUpdate;
use sui::RichExtensionBlock;

#[derive(Clone)]
pub struct BindingRichDocument {
    pub(crate) inner: RichDocumentModel,
}

impl BindingRichDocument {
    pub fn new(markdown: impl Into<String>) -> Self {
        Self {
            inner: RichDocumentModel::from_markdown(markdown),
        }
    }

    pub fn revision(&self) -> u64 {
        self.inner.revision()
    }

    pub fn markdown(&self) -> String {
        self.inner.markdown()
    }

    pub fn set_markdown(&self, markdown: impl Into<String>) -> bool {
        self.inner.set_markdown(markdown)
    }

    pub fn append_markdown(&self, fragment: &str) -> bool {
        self.inner.append_markdown(fragment)
    }

    pub fn last_update(&self) -> BindingRichDocumentUpdate {
        self.inner.last_update().into()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_attachment(
        &self,
        name: impl Into<String>,
        media_type: Option<String>,
        source: Option<String>,
        size_bytes: Option<u64>,
        description: Option<String>,
    ) -> u64 {
        let mut attachment = RichAttachment::new(name);
        attachment.media_type = media_type;
        attachment.source = source;
        attachment.size_bytes = size_bytes;
        attachment.description = description;
        self.inner.append_attachment(attachment).get()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn append_extension(
        &self,
        renderer: impl Into<String>,
        title: impl Into<String>,
        summary: Option<String>,
        body: impl Into<String>,
        status: &str,
        initially_expanded: bool,
        metadata: Vec<(String, String)>,
    ) -> Result<u64, String> {
        let mut extension = RichExtensionBlock::new(renderer, title);
        extension.summary = summary;
        extension.body = body.into();
        extension.status = binding_rich_document_status(status)?;
        extension.initially_expanded = initially_expanded;
        extension.metadata = metadata;
        Ok(self.inner.append_extension(extension).get())
    }
}

impl fmt::Debug for BindingRichDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingRichDocument")
            .field("revision", &self.revision())
            .field("markdown_len", &self.markdown().len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRichDocumentUpdate {
    pub revision: u64,
    pub reparsed_start: usize,
    pub reparsed_end: usize,
    pub reused_prefix_blocks: usize,
    pub changed_block_ids: Vec<u64>,
    pub append_only: bool,
}

impl From<RichDocumentUpdate> for BindingRichDocumentUpdate {
    fn from(value: RichDocumentUpdate) -> Self {
        Self {
            revision: value.revision,
            reparsed_start: value.reparsed_source.start,
            reparsed_end: value.reparsed_source.end,
            reused_prefix_blocks: value.reused_prefix_blocks,
            changed_block_ids: value
                .changed_block_ids
                .into_iter()
                .map(|id| id.get())
                .collect(),
            append_only: value.append_only,
        }
    }
}

pub(crate) fn binding_rich_document_status(value: &str) -> Result<RichDocumentStatus, String> {
    match normalized_option_name(value).as_str() {
        "neutral" => Ok(RichDocumentStatus::Neutral),
        "pending" => Ok(RichDocumentStatus::Pending),
        "running" | "active" => Ok(RichDocumentStatus::Running),
        "success" | "complete" => Ok(RichDocumentStatus::Success),
        "warning" | "warn" => Ok(RichDocumentStatus::Warning),
        "error" | "failed" => Ok(RichDocumentStatus::Error),
        _ => Err(format!(
            "rich document status must be neutral, pending, running, success, warning, or error; got '{value}'"
        )),
    }
}
