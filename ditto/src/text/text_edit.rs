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
        if self.should_merge(idx, len) {
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

    // Checks whether the new edit should be merged into
    // the TextEdit. Returns true if the edits are overlapping
    // and the current edit has not yet hit a new line.
    fn should_merge(&self, idx: usize, len: usize) -> bool {
        self.overlaps_with(idx, len) && !self.text.ends_with("\n")
    }

    // Checks whether the TextEdit and new edit overlap.
    fn overlaps_with(&self, idx: usize, len: usize) -> bool {
        (idx + len >= self.idx) && (idx <= self.idx + self.text.char_len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_no_overlap() {
        let mut edit = new(3, 0, "hello");
        let merged_edit1 = edit.merge_or_replace(0, 0, "goodbye");
        let merged_edit2 = edit.merge_or_replace(20, 0, "goodbye");
        assert!(merged_edit1 == new(0, 0, "goodbye"));
        assert!(merged_edit2 == new(20, 0, "goodbye"));
        assert!(edit == new(20, 0, "goodbye"));
    }

    #[test]
    fn merge_newline() {
        let mut edit = new(0, 0, "hello\n");
        let merged_edit = edit.merge_or_replace(0, 5, "goodbye");
        assert!(merged_edit == new(0, 5, "goodbye"));
        assert!(edit == new(0, 5, "goodbye"));
    }

    #[test]
    fn merge_prefix_insert() {
        let mut edit = new(0, 0, "hello");
        let merged_edit = edit.merge_or_replace(0, 0, "goodbye");
        assert!(merged_edit == new(0, 5, "goodbyehello"));
        assert!(edit == new(0, 5, "goodbyehello"));
    }

    #[test]
    fn merge_inside_insert() {
        let mut edit = new(0, 0, "hello");
        let merged_edit = edit.merge_or_replace(1, 0, "goodbye");
        assert!(merged_edit == new(0, 5, "hgoodbyeello"));
    }

    #[test]
    fn merge_postfix_insert() {
        let mut edit = new(0, 0, "hello");
        let merged_edit = edit.merge_or_replace(5, 0, "goodbye");
        assert!(merged_edit == new(0, 5, "hellogoodbye"));
    }

    #[test]
    fn merge_prefix_delete() {
        let mut edit = new(3, 2, "hello");
        let merged_edit = edit.merge_or_replace(0, 3, "");
        assert!(merged_edit == new(0, 8, "hello"));
    }

    #[test]
    fn merge_preoverlapping_delete() {
        let mut edit = new(3, 2, "hello");
        let merged_edit = edit.merge_or_replace(1, 5, "");
        assert!(merged_edit == new(1,7, "lo"));
    }

    #[test]
    fn merge_internal_delete() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(4, 6, "");
        assert!(merged_edit == new(3, 11, "hrld!"));
    }

    #[test]
    fn merge_postoverlapping_delete() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(7, 10, "");
        assert!(merged_edit == new(3, 14, "hell"));
    }

    #[test]
    fn merge_postfix_delete() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(14, 2, "");
        assert!(merged_edit == new(3, 13, "helloworld!"));
    }

    #[test]
    fn merge_wrapping_delete() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(1, 20, "");
        assert!(merged_edit == new(1, 20, ""));
    }

    #[test]
    fn merge_prefix_replacement() {
        let mut edit = new(3, 2, "hello");
        let merged_edit = edit.merge_or_replace(0, 3, "x∆∅");
        assert!(merged_edit == new(0, 8, "x∆∅hello"));
    }

    #[test]
    fn merge_preoverlapping_replacement() {
        let mut edit = new(3, 2, "hello");
        let merged_edit = edit.merge_or_replace(1, 5, "x∆∅");
        assert!(merged_edit == new(1, 7, "x∆∅lo"));
    }

    #[test]
    fn merge_internal_replacement() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(4, 6, "x∆∅");
        assert!(merged_edit == new(3, 11, "hx∆∅rld!"));
    }

    #[test]
    fn merge_postoverlapping_replacement() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(7, 10, "x∆∅");
        assert!(merged_edit == new(3, 14, "hellx∆∅"));
    }

    #[test]
    fn merge_postfix_replacement() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(14, 2, "x∆∅");
        assert!(merged_edit == new(3, 13, "helloworld!x∆∅"));
    }

    #[test]
    fn merge_wrapping_replacement() {
        let mut edit = new(3, 2, "helloworld!");
        let merged_edit = edit.merge_or_replace(1, 20, "x∆∅");
        println!("{:?}", merged_edit);
        assert!(merged_edit == new(1, 20, "x∆∅"));
    }

    #[test]
    fn shift_insert_before() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(3, 0, "abcdefg").unwrap();
        assert!(edit.idx == 17);
    }

    #[test]
    fn shift_delete_before() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(3, 4, "").unwrap();
        assert!(edit.idx == 6);
    }

    #[test]
    fn shift_replace_before() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(3, 4, "xyz").unwrap();
        assert!(edit.idx == 9);
    }

    #[test]
    fn shift_insert_prefix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(10, 0, "abcdefg").unwrap();
        assert!(edit.idx == 17);
    }

    #[test]
    fn shift_delete_prefix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(6, 4, "").unwrap();
        assert!(edit.idx == 6);
    }

    #[test]
    fn shift_replace_prefix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(8, 2, "Δƍ🤡").unwrap();
        assert!(edit.idx == 11);
    }

    #[test]
    fn shift_delete_preoverlapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(8, 3, "") == None);
    }

    #[test]
    fn shift_replace_preoverlapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(8, 3, "Δƍ🤡") == None);
    }

    #[test]
    fn shift_insert_internal() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(11, 0, "Δƍ🤡") == None);
    }

    #[test]
    fn shift_delete_internal() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(11, 3, "") == None);
    }

    #[test]
    fn shift_replace_internal() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(11, 3, "abc") == None);
    }

    #[test]
    fn shift_delete_postoverlapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(15, 10, "") == None);
    }

    #[test]
    fn shift_replace_postoverlapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(15, 10, "abc") == None);
    }

    #[test]
    fn shift_insert_postfix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(21, 0, "abc").unwrap();
        assert!(edit.idx == 10);
    }

    #[test]
    fn shift_delete_postfix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(21, 412, "").unwrap();
        assert!(edit.idx == 10);
    }

    #[test]
    fn shift_replace_postfix() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(21, 999, "abc").unwrap();
        assert!(edit.idx == 10);
    }

    #[test]
    fn shift_delete_wrapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(9, 13, "") == None);
    }

    #[test]
    fn shift_replace_wrapping() {
        let edit = new(10, 0, "helloworld!");
        assert!(edit.shift_or_destroy(10, 11, "wazza?") == None);
    }

    #[test]
    fn shift_insert_after() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(22, 0, "wazza?").unwrap();
        assert!(edit.idx == 10);
    }

    #[test]
    fn shift_delete_after() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(22, 501, "").unwrap();
        assert!(edit.idx == 10);
    }

    #[test]
    fn shift_replace_after() {
        let edit = new(10, 0, "helloworld!");
        let edit = edit.shift_or_destroy(22, 501, "wazza?").unwrap();
        assert!(edit.idx == 10);
    }

    fn new(idx: usize, len: usize, text: &str) -> TextEdit {
        TextEdit{idx, len, text: text.into()}
    }
}
