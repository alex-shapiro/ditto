use char_fns::CharFns;
use std::cmp::min;

#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub idx:  usize,
    pub len:  usize,
    pub text: String,
}

impl TextEdit {
    /// Merges a contiguous new edit into the TextEdit or, if
    /// the new edit and TextEdit are not contiguous, replaces
    /// the TextEdit's values with the new edit's values. Then
    /// it returns a NewEdit with the TextEdit's updated values.
    pub fn merge_or_replace(&mut self, idx: usize, len: usize, text: &str) -> Self {
        if self.overlaps_with(idx, len) {
            let deletes_before = self.idx.saturating_sub(idx);
            let insert_idx = idx.saturating_sub(self.idx);

            let mut deletes_after = len - deletes_before;
            let text_len          = self.text.char_len();
            let text_delete_len   = min(deletes_after, text_len - insert_idx);
            deletes_after         = deletes_after.saturating_sub(text_delete_len);

            self.idx = min(self.idx, idx);
            self.len = deletes_before + text_len + deletes_after;
            self.text = self.text.char_replace(insert_idx, text_delete_len, text);
        } else {
            self.idx = idx;
            self.len = len;
            self.text = text.into();
        }

        self.clone()
    }

    /// Shifts the TextEdit's index if the new edit is
    /// discontiguous, otherwise it returns None.
    pub fn shift_or_destroy(mut self, idx: usize, len: usize, text: &str) -> Option<TextEdit> {
        if idx + len <= self.idx {
            self.idx -= len;
            self.idx += text.char_len();
            Some(self)
        } else if idx >= self.idx + self.text.len() {
            Some(self)
        } else {
            None
        }
    }

    /// Checks whether the TextEdit and new edit overlap.
    fn overlaps_with(&self, idx: usize, len: usize) -> bool {
        (idx + len >= self.idx) && (idx <= self.idx + self.text.char_len())
    }
}
