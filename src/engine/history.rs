// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Undo / redo.
//!
//! Every entry stores the *other* state of whatever it changed, so applying an entry
//! swaps the document between "before" and "after". Undo and redo are the same operation
//! applied to entries moving between the two stacks.

use tiny_skia::{IntRect, Pixmap};

use super::document::{Document, LayerId};
use super::raster;

enum Change {
    /// A rectangle of one layer's pixels.
    Pixels { layer: LayerId, x: i32, y: i32, patch: Pixmap },
    /// The whole document (used for structural changes such as resizing or layer edits).
    Document(Box<Document>),
}

impl Change {
    fn apply(&mut self, doc: &mut Document) {
        match self {
            Change::Pixels { layer, x, y, patch } => {
                if let Some(l) = doc.layer_mut(*layer) {
                    raster::swap_region(&mut l.pixels, patch, *x, *y);
                }
            }
            Change::Document(other) => std::mem::swap(doc, other),
        }
    }

    fn byte_size(&self) -> usize {
        match self {
            Change::Pixels { patch, .. } => patch.data().len(),
            Change::Document(d) => d.byte_size(),
        }
    }
}

struct Entry {
    id: u64,
    label: String,
    change: Change,
}

pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    next_id: u64,
    max_bytes: usize,
    max_entries: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            max_bytes: 1024 * 1024 * 1024,
            max_entries: 200,
        }
    }

    /// Records a change to a region of one layer. `before` holds the region's pixels
    /// prior to the edit; the layer must already contain the new pixels.
    pub fn push_pixels(&mut self, label: &str, layer: LayerId, region: IntRect, before: Pixmap) {
        debug_assert_eq!((before.width(), before.height()), (region.width(), region.height()));
        self.push(
            label,
            Change::Pixels {
                layer,
                x: region.x(),
                y: region.y(),
                patch: before,
            },
        );
    }

    /// Records a change that may have touched anything; `before` is the prior document.
    pub fn push_document(&mut self, label: &str, before: Document) {
        self.push(label, Change::Document(Box::new(before)));
    }

    fn push(&mut self, label: &str, change: Change) {
        let id = self.next_id;
        self.next_id += 1;
        self.undo.push(Entry {
            id,
            label: label.to_owned(),
            change,
        });
        self.redo.clear();
        self.trim();
    }

    fn trim(&mut self) {
        let mut total: usize = self.undo.iter().map(|e| e.change.byte_size()).sum();
        while self.undo.len() > 1 && (total > self.max_bytes || self.undo.len() > self.max_entries) {
            let dropped = self.undo.remove(0);
            total -= dropped.change.byte_size();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|e| e.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|e| e.label.as_str())
    }

    /// Reverts the most recent change. Returns its label.
    pub fn undo(&mut self, doc: &mut Document) -> Option<String> {
        let mut entry = self.undo.pop()?;
        entry.change.apply(doc);
        let label = entry.label.clone();
        self.redo.push(entry);
        Some(label)
    }

    /// Re-applies the most recently undone change. Returns its label.
    pub fn redo(&mut self, doc: &mut Document) -> Option<String> {
        let mut entry = self.redo.pop()?;
        entry.change.apply(doc);
        let label = entry.label.clone();
        self.undo.push(entry);
        Some(label)
    }

    /// Identifies the current position in history; compare with a value saved
    /// earlier to tell whether the document has changed since then.
    pub fn state_id(&self) -> u64 {
        self.undo.last().map_or(0, |e| e.id)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::color::Rgba;
    use crate::engine::raster::{copy_region, fill_region, get_pixel};

    fn paint_black(doc: &mut Document, history: &mut History, r: IntRect) {
        let layer = doc.active_layer().id;
        let before = copy_region(&doc.active_layer().pixels, r);
        fill_region(&mut doc.active_layer_mut().pixels, r, Rgba::BLACK);
        history.push_pixels("Paint", layer, r, before);
    }

    #[test]
    fn undo_redo_pixels() {
        let mut doc = Document::new(8, 8, Rgba::WHITE);
        let mut h = History::new();
        let r = IntRect::from_xywh(2, 2, 3, 3).unwrap();
        paint_black(&mut doc, &mut h, r);
        assert_eq!(get_pixel(&doc.flatten(), 3, 3), Some(Rgba::BLACK));

        assert_eq!(h.undo(&mut doc).as_deref(), Some("Paint"));
        assert_eq!(get_pixel(&doc.flatten(), 3, 3), Some(Rgba::WHITE));
        assert!(!h.can_undo());

        assert_eq!(h.redo(&mut doc).as_deref(), Some("Paint"));
        assert_eq!(get_pixel(&doc.flatten(), 3, 3), Some(Rgba::BLACK));
        assert!(!h.can_redo());
    }

    #[test]
    fn new_change_clears_redo() {
        let mut doc = Document::new(8, 8, Rgba::WHITE);
        let mut h = History::new();
        paint_black(&mut doc, &mut h, IntRect::from_xywh(0, 0, 1, 1).unwrap());
        h.undo(&mut doc);
        assert!(h.can_redo());
        paint_black(&mut doc, &mut h, IntRect::from_xywh(1, 1, 1, 1).unwrap());
        assert!(!h.can_redo());
    }

    #[test]
    fn undo_document_change() {
        let mut doc = Document::new(4, 4, Rgba::WHITE);
        let mut h = History::new();
        let before = doc.clone();
        doc.resize_canvas(10, 10, 0, 0, Rgba::WHITE);
        doc.add_layer();
        h.push_document("Resize", before);
        h.undo(&mut doc);
        assert_eq!((doc.width(), doc.height(), doc.layers.len()), (4, 4, 1));
        h.redo(&mut doc);
        assert_eq!((doc.width(), doc.height(), doc.layers.len()), (10, 10, 2));
    }

    #[test]
    fn mixed_changes_undo_in_order() {
        let mut doc = Document::new(4, 4, Rgba::WHITE);
        let mut h = History::new();
        paint_black(&mut doc, &mut h, IntRect::from_xywh(0, 0, 2, 2).unwrap());
        let before = doc.clone();
        doc.resize_canvas(6, 6, 0, 0, Rgba::WHITE);
        h.push_document("Resize", before);
        paint_black(&mut doc, &mut h, IntRect::from_xywh(4, 4, 2, 2).unwrap());

        h.undo(&mut doc);
        h.undo(&mut doc);
        assert_eq!(doc.width(), 4);
        assert_eq!(get_pixel(&doc.flatten(), 0, 0), Some(Rgba::BLACK));
        h.undo(&mut doc);
        assert_eq!(get_pixel(&doc.flatten(), 0, 0), Some(Rgba::WHITE));
        h.redo(&mut doc);
        h.redo(&mut doc);
        h.redo(&mut doc);
        assert_eq!(get_pixel(&doc.flatten(), 5, 5), Some(Rgba::BLACK));
    }

    #[test]
    fn state_id_tracks_saved_position() {
        let mut doc = Document::new(4, 4, Rgba::WHITE);
        let mut h = History::new();
        let saved = h.state_id();
        paint_black(&mut doc, &mut h, IntRect::from_xywh(0, 0, 1, 1).unwrap());
        assert_ne!(h.state_id(), saved);
        h.undo(&mut doc);
        assert_eq!(h.state_id(), saved);
    }

    #[test]
    fn trims_old_entries() {
        let mut doc = Document::new(4, 4, Rgba::WHITE);
        let mut h = History::new();
        h.max_entries = 3;
        for _ in 0..10 {
            paint_black(&mut doc, &mut h, IntRect::from_xywh(0, 0, 1, 1).unwrap());
        }
        assert_eq!(h.undo.len(), 3);
    }
}
