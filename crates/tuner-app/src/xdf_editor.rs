use tuner_core::Endianness;
use tuner_xdf::{
    Dimensions, NumericKind, ParameterKind, XdfAuthoringOperation, XdfCategory, XdfDocument,
    XdfParameterDraft,
};

#[derive(Clone, Debug)]
pub(crate) struct XdfEditorForm {
    pub header_title: String,
    pub header_description: String,
    pub author: String,
    pub version: String,
    pub base_offset_text: String,
    pub base_offset_subtract: bool,
    pub categories: Vec<XdfCategory>,
    pub definition: XdfParameterDraft,
    pub address_text: String,
}

impl Default for XdfEditorForm {
    fn default() -> Self {
        Self {
            header_title: "New XDF".into(),
            header_description: String::new(),
            author: String::new(),
            version: "1.70".into(),
            base_offset_text: "0".into(),
            base_offset_subtract: false,
            categories: Vec::new(),
            address_text: "0".into(),
            definition: XdfParameterDraft {
                unique_id: None,
                kind: ParameterKind::Table,
                title: "New Table".into(),
                description: String::new(),
                category: None,
                category_memberships: Vec::new(),
                xdf_address: 0,
                element_width_bits: 8,
                dimensions: Dimensions {
                    rows: 1,
                    columns: 1,
                },
                signed: false,
                endianness: Endianness::Little,
                numeric_kind: NumericKind::Integer,
                column_major: false,
                row_stride_bits: 8,
                column_stride_bits: 8,
                conversion: None,
                bit_offset: None,
                bit_width: None,
                bit_mask: None,
                unknown_type_flags: 0,
                axes: Vec::new(),
            },
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct XdfEditorState {
    pub open: bool,
    pub focus_requested: bool,
    pub draft: Option<XdfDocument>,
    pub form: XdfEditorForm,
    pub selected_semantic_id: Option<String>,
    pub adding: bool,
    pub dirty: bool,
    pub form_dirty: bool,
    pub close_prompt: bool,
    pub close_after_save: bool,
    pub workspace_preview_active: bool,
    pub save_pending: bool,
    pub pending_new_xdf: bool,
    pub revision: u64,
    pub undo: Vec<XdfDocument>,
    pub redo: Vec<XdfDocument>,
    pub task_id: Option<String>,
}

impl XdfEditorState {
    const MAX_UNDO: usize = 100;

    pub fn open_document(
        &mut self,
        document: XdfDocument,
        selected_semantic_id: Option<String>,
    ) -> Result<(), String> {
        let form = form_for_document(&document, selected_semantic_id.as_deref())?;
        self.open = true;
        self.focus_requested = true;
        self.draft = Some(document);
        self.form = form;
        self.selected_semantic_id = selected_semantic_id;
        self.adding = false;
        self.dirty = false;
        self.form_dirty = false;
        self.close_prompt = false;
        self.close_after_save = false;
        self.workspace_preview_active = false;
        self.save_pending = false;
        self.pending_new_xdf = false;
        self.revision = 0;
        self.undo.clear();
        self.redo.clear();
        self.task_id = None;
        Ok(())
    }

    pub fn begin_new_definition(&mut self) {
        self.selected_semantic_id = None;
        self.adding = true;
        self.form.definition = XdfEditorForm::default().definition;
        self.form.address_text = "0".into();
        self.form.form_title_for_new();
        self.form_dirty = true;
    }

    pub fn select_definition(&mut self, semantic_id: &str) -> Result<(), String> {
        if self.save_pending {
            return Err(
                "Wait for XDF Save As to finish before selecting another definition.".into(),
            );
        }
        if self.form_dirty {
            return Err(
                "Apply or discard the current form edits before selecting another definition."
                    .into(),
            );
        }
        let document = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?;
        self.form = form_for_document(document, Some(semantic_id))?;
        self.selected_semantic_id = Some(semantic_id.to_string());
        self.adding = false;
        self.form_dirty = false;
        Ok(())
    }

    pub fn apply_current_definition(&mut self) -> Result<(), String> {
        if self.save_pending {
            return Err("Wait for XDF Save As to finish before editing the draft.".into());
        }
        let document = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?;
        let selected_index = self.selected_semantic_id.as_deref().and_then(|id| {
            document
                .parameters
                .iter()
                .position(|item| item.semantic_id == id)
        });
        let adding = self.adding;
        let updated = self.preview_current_definition()?;
        self.commit_document(updated)?;
        let document = self.draft.as_ref().expect("draft remains open");
        let selected_index = if adding {
            document.parameters.len().checked_sub(1)
        } else {
            selected_index
        };
        self.selected_semantic_id = selected_index
            .and_then(|index| document.parameters.get(index))
            .map(|parameter| parameter.semantic_id.clone());
        self.adding = false;
        self.refresh_form()?;
        self.form_dirty = false;
        Ok(())
    }

    pub fn preview_current_definition(&self) -> Result<XdfDocument, String> {
        let document = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?;
        let mut definition = self.form.definition.clone();
        if self.adding || self.selected_semantic_id.is_some() {
            definition.xdf_address = parse_address(&self.form.address_text)?;
        }
        let mut header = document.header.clone();
        header.title = non_empty(&self.form.header_title);
        header.description = non_empty(&self.form.header_description);
        header.author = non_empty(&self.form.author);
        header.version = non_empty(&self.form.version);
        header.base_offset.offset = parse_address(&self.form.base_offset_text)?;
        header.base_offset.subtract = self.form.base_offset_subtract;

        let mut operations = vec![
            XdfAuthoringOperation::SetHeader { header },
            XdfAuthoringOperation::SetCategories {
                categories: self.form.categories.clone(),
            },
        ];
        if self.adding {
            operations.push(XdfAuthoringOperation::AddParameter { definition });
        } else if let Some(semantic_id) = self.selected_semantic_id.clone() {
            operations.push(XdfAuthoringOperation::ReplaceParameter {
                semantic_id,
                definition,
            });
        }
        self.preview_operations(&operations)
    }

    pub fn apply_operations(&mut self, operations: &[XdfAuthoringOperation]) -> Result<(), String> {
        if self.save_pending {
            return Err("Wait for XDF Save As to finish before changing the draft.".into());
        }
        if self.form_dirty {
            return Err(
                "Apply or discard the current form edits before changing the draft.".into(),
            );
        }
        let updated = self.preview_operations(operations)?;
        self.commit_document(updated)
    }

    pub fn preview_operations(
        &self,
        operations: &[XdfAuthoringOperation],
    ) -> Result<XdfDocument, String> {
        let current = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?;
        let mut updated = current.clone();
        updated
            .apply_authoring_operations(operations)
            .map_err(|error| error.to_string())?;
        Ok(updated)
    }

    fn commit_document(&mut self, updated: XdfDocument) -> Result<(), String> {
        let current = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?
            .clone();
        self.push_undo(current);
        self.draft = Some(updated);
        self.redo.clear();
        self.dirty = true;
        self.revision = self.revision.saturating_add(1);
        self.form_dirty = false;
        self.refresh_form()?;
        Ok(())
    }

    pub fn duplicate_selected(&mut self) -> Result<(), String> {
        if self.form_dirty {
            return Err(
                "Apply or discard the current form edits before duplicating a definition.".into(),
            );
        }
        let semantic_id = self
            .selected_semantic_id
            .clone()
            .ok_or_else(|| "Select a definition to duplicate.".to_string())?;
        let document = self
            .draft
            .as_ref()
            .ok_or_else(|| "Open the XDF editor first.".to_string())?;
        let mut definition = document
            .authoring_draft(&semantic_id)
            .map_err(|error| error.to_string())?;
        let count = document.parameters.len() + 1;
        let original_id = definition
            .unique_id
            .clone()
            .unwrap_or_else(|| "parameter".into());
        definition.unique_id = Some(format!("{original_id}-copy-{count}"));
        definition.title = format!("{} copy", definition.title);
        self.adding = true;
        self.selected_semantic_id = None;
        self.form.definition = definition;
        self.form.address_text = format!("{:#X}", self.form.definition.xdf_address);
        self.form_dirty = true;
        Ok(())
    }

    pub fn delete_selected(&mut self) -> Result<(), String> {
        let semantic_id = self
            .selected_semantic_id
            .clone()
            .ok_or_else(|| "Select a definition to delete.".to_string())?;
        self.apply_operations(&[XdfAuthoringOperation::DeleteParameter { semantic_id }])?;
        self.selected_semantic_id = None;
        self.adding = false;
        self.form = form_for_document(self.draft.as_ref().expect("draft remains open"), None)?;
        Ok(())
    }

    pub fn reorder_selected(&mut self, index: usize) -> Result<(), String> {
        let semantic_id = self
            .selected_semantic_id
            .clone()
            .ok_or_else(|| "Select a definition to reorder.".to_string())?;
        self.apply_operations(&[XdfAuthoringOperation::ReorderParameter { semantic_id, index }])
    }

    pub fn undo(&mut self) -> bool {
        if self.form_dirty || self.save_pending {
            return false;
        }
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        if let Some(current) = self.draft.replace(previous) {
            self.redo.push(current);
        }
        self.dirty = true;
        self.form_dirty = false;
        self.revision = self.revision.saturating_add(1);
        let _ = self.refresh_form();
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.form_dirty || self.save_pending {
            return false;
        }
        let Some(next) = self.redo.pop() else {
            return false;
        };
        if let Some(current) = self.draft.replace(next) {
            self.push_undo(current);
        }
        self.dirty = true;
        self.form_dirty = false;
        self.revision = self.revision.saturating_add(1);
        let _ = self.refresh_form();
        true
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.form_dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.form_dirty
    }

    /// Returns true when the editor must remain open for a save/discard decision.
    pub fn request_close(&mut self) -> bool {
        if self.is_dirty() {
            self.close_prompt = true;
            self.open = true;
            true
        } else {
            self.open = false;
            self.close_prompt = false;
            false
        }
    }

    pub fn discard_and_close(&mut self) {
        self.open = false;
        self.focus_requested = false;
        self.close_prompt = false;
        self.close_after_save = false;
        self.workspace_preview_active = false;
        self.save_pending = false;
        self.pending_new_xdf = false;
        self.draft = None;
        self.selected_semantic_id = None;
        self.undo.clear();
        self.redo.clear();
        self.dirty = false;
        self.form_dirty = false;
        self.task_id = None;
    }

    fn push_undo(&mut self, document: XdfDocument) {
        if self.undo.len() == Self::MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(document);
    }

    fn refresh_form(&mut self) -> Result<(), String> {
        let Some(document) = self.draft.as_ref() else {
            return Ok(());
        };
        if self
            .selected_semantic_id
            .as_deref()
            .is_some_and(|id| document.parameter(id).is_none())
        {
            self.selected_semantic_id = None;
        }
        self.form = form_for_document(document, self.selected_semantic_id.as_deref())?;
        Ok(())
    }
}

impl XdfEditorForm {
    fn form_title_for_new(&mut self) {
        if self.header_title.trim().is_empty() {
            self.header_title = "New XDF".into();
        }
    }
}

fn form_for_document(
    document: &XdfDocument,
    selected_semantic_id: Option<&str>,
) -> Result<XdfEditorForm, String> {
    let mut form = XdfEditorForm::default();
    form.header_title = document.header.title.clone().unwrap_or_default();
    form.header_description = document.header.description.clone().unwrap_or_default();
    form.author = document.header.author.clone().unwrap_or_default();
    form.version = document.header.version.clone().unwrap_or_default();
    form.base_offset_text = format!("{:#X}", document.header.base_offset.offset);
    form.base_offset_subtract = document.header.base_offset.subtract;
    form.categories = document.categories.clone();
    if let Some(semantic_id) = selected_semantic_id {
        form.definition = document
            .authoring_draft(semantic_id)
            .map_err(|error| error.to_string())?;
        form.address_text = format!("{:#X}", form.definition.xdf_address);
    }
    Ok(form)
}

fn parse_address(value: &str) -> Result<u64, String> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
            .map_err(|_| format!("Invalid hexadecimal XDF address: {value}"))
    } else if value
        .chars()
        .any(|character| character.is_ascii_alphabetic())
    {
        u64::from_str_radix(value, 16).map_err(|_| format!("Invalid XDF address: {value}"))
    } else {
        value
            .parse()
            .map_err(|_| format!("Invalid XDF address: {value}"))
    }
}

fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuner_xdf::XdfDocument;

    #[test]
    fn authoring_draft_supports_undo_redo_and_clean_save_state() {
        let mut editor = XdfEditorState::default();
        editor
            .open_document(XdfDocument::new("Draft").unwrap(), None)
            .unwrap();
        editor.begin_new_definition();
        editor.form.definition.title = "Test table".into();
        editor.form.address_text = "0x20".into();
        editor.apply_current_definition().unwrap();
        assert_eq!(editor.draft.as_ref().unwrap().parameters.len(), 1);
        assert!(editor.is_dirty());

        assert!(editor.undo());
        assert!(editor.draft.as_ref().unwrap().parameters.is_empty());
        assert!(editor.redo());
        assert_eq!(
            editor.draft.as_ref().unwrap().parameters[0].title,
            "Test table"
        );

        editor.mark_saved();
        assert!(!editor.is_dirty());
        assert!(!editor.request_close());
        assert!(!editor.open);
    }

    #[test]
    fn header_only_changes_can_be_applied_without_selecting_a_parameter() {
        let mut editor = XdfEditorState::default();
        editor
            .open_document(XdfDocument::new("Old title").unwrap(), None)
            .unwrap();
        editor.form.header_title = "New title".into();
        editor.form_dirty = true;

        editor.apply_current_definition().unwrap();
        assert_eq!(
            editor.draft.as_ref().unwrap().header.title.as_deref(),
            Some("New title")
        );
        assert!(editor.draft.as_ref().unwrap().parameters.is_empty());
        assert!(editor.selected_semantic_id.is_none());
    }

    #[test]
    fn pending_form_changes_cannot_be_lost_by_navigation_or_history_actions() {
        let mut editor = XdfEditorState::default();
        editor
            .open_document(XdfDocument::new("Draft").unwrap(), None)
            .unwrap();
        editor.begin_new_definition();
        editor.apply_current_definition().unwrap();
        let semantic_id = editor.selected_semantic_id.clone().unwrap();
        editor.form.definition.title = "Unapplied title".into();
        editor.form_dirty = true;

        assert!(editor.select_definition(&semantic_id).is_err());
        assert!(!editor.undo());
        assert_eq!(editor.form.definition.title, "Unapplied title");
        assert!(editor.is_dirty());
    }

    #[test]
    fn pending_save_blocks_draft_mutations_until_completion() {
        let mut editor = XdfEditorState::default();
        editor
            .open_document(XdfDocument::new("Draft").unwrap(), None)
            .unwrap();
        editor.save_pending = true;
        let mut header = editor.draft.as_ref().unwrap().header.clone();
        header.title = Some("Changed during save".into());

        assert!(editor
            .apply_operations(&[XdfAuthoringOperation::SetHeader { header }])
            .is_err());
        assert_eq!(
            editor.draft.as_ref().unwrap().header.title.as_deref(),
            Some("Draft")
        );
        assert_eq!(editor.revision, 0);
    }
}
