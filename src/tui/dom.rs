//! DOM element tree for component composition.
//!
//! Mirrors the ref's `dom.ts` with element types, text nodes, and tree
//! manipulation operations (append, insert, remove, mark dirty).

use crate::tui::layout::{ElementType, LayoutStyle};
use crate::tui::style::TextStyles;

/// Unique node identifier.
pub type NodeId = u64;

/// A DOM node: either an element or a text node.
#[derive(Debug)]
pub enum DomNode {
    Element(DomElement),
    Text(TextNode),
}

impl DomNode {
    /// Get the parent node ID.
    pub fn parent_id(&self) -> Option<NodeId> {
        match self {
            DomNode::Element(e) => e.parent_id,
            DomNode::Text(t) => t.parent_id,
        }
    }

    /// Set the parent node ID.
    pub fn set_parent_id(&mut self, id: Option<NodeId>) {
        match self {
            DomNode::Element(e) => e.parent_id = id,
            DomNode::Text(t) => t.parent_id = id,
        }
    }

    /// Is this node dirty?
    pub fn is_dirty(&self) -> bool {
        match self {
            DomNode::Element(e) => e.dirty,
            DomNode::Text(_) => false,
        }
    }

    /// Get this node's id.
    pub fn id(&self) -> NodeId {
        match self {
            DomNode::Element(e) => e.id,
            DomNode::Text(t) => t.id,
        }
    }
}

/// An element in the DOM tree. Corresponds to the ref's `DOMElement`.
#[derive(Debug)]
pub struct DomElement {
    /// Unique identifier for this element.
    pub id: NodeId,
    /// Element type (root, box, text, etc.).
    pub element_type: ElementType,
    /// Child node IDs in order.
    pub children: Vec<NodeId>,
    /// Layout style properties.
    pub layout_style: LayoutStyle,
    /// Text styling (for text elements).
    pub text_styles: Option<TextStyles>,
    /// Arbitrary string attributes.
    pub attributes: std::collections::HashMap<String, DomNodeAttribute>,
    /// Parent element ID.
    pub parent_id: Option<NodeId>,
    /// Taffy layout node ID, if this element participates in layout.
    pub layout_node: Option<taffy::NodeId>,
    /// Whether this node needs re-rendering.
    pub dirty: bool,
    /// Whether this node is hidden (reconciler hide/unhide).
    pub is_hidden: bool,

    // Scroll state
    pub scroll_top: Option<i32>,
    pub pending_scroll_delta: Option<i32>,
    pub scroll_height: Option<i32>,
    pub scroll_viewport_height: Option<i32>,
    pub scroll_viewport_top: Option<i32>,
    pub sticky_scroll: bool,
}

impl DomElement {
    /// Create a new element of the given type.
    pub fn new(id: NodeId, element_type: ElementType) -> Self {
        Self {
            id,
            element_type,
            children: Vec::new(),
            layout_style: LayoutStyle::default(),
            text_styles: None,
            attributes: std::collections::HashMap::new(),
            parent_id: None,
            layout_node: None,
            dirty: false,
            is_hidden: false,
            scroll_top: None,
            pending_scroll_delta: None,
            scroll_height: None,
            scroll_viewport_height: None,
            scroll_viewport_top: None,
            sticky_scroll: false,
        }
    }
}

/// A text node in the DOM tree. Corresponds to the ref's `TextNode`.
#[derive(Debug)]
pub struct TextNode {
    /// Unique identifier.
    pub id: NodeId,
    /// Text content.
    pub value: String,
    /// Parent element ID.
    pub parent_id: Option<NodeId>,
}

impl TextNode {
    pub fn new(id: NodeId, value: String) -> Self {
        Self {
            id,
            value,
            parent_id: None,
        }
    }
}

/// Attribute value type.
#[derive(Debug, Clone, PartialEq)]
pub enum DomNodeAttribute {
    Bool(bool),
    String(String),
    Number(f64),
}

/// DOM tree that owns all nodes and provides tree operations.
pub struct DomTree {
    nodes: std::collections::HashMap<NodeId, DomNode>,
    next_id: NodeId,
}

impl DomTree {
    pub fn new() -> Self {
        Self {
            nodes: std::collections::HashMap::new(),
            next_id: 1,
        }
    }

    /// Allocate a new unique node ID.
    fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new element node.
    pub fn create_element(&mut self, element_type: ElementType) -> NodeId {
        let id = self.alloc_id();
        let elem = DomElement::new(id, element_type);
        self.nodes.insert(id, DomNode::Element(elem));
        id
    }

    /// Create a new text node.
    pub fn create_text_node(&mut self, text: &str) -> NodeId {
        let id = self.alloc_id();
        let node = TextNode::new(id, text.into());
        self.nodes.insert(id, DomNode::Text(node));
        id
    }

    /// Get a reference to a node.
    pub fn get(&self, id: NodeId) -> Option<&DomNode> {
        self.nodes.get(&id)
    }

    /// Get a mutable reference to a node.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut DomNode> {
        self.nodes.get_mut(&id)
    }

    /// Get a reference to an element node. Panics if not an element.
    pub fn element(&self, id: NodeId) -> Option<&DomElement> {
        match self.nodes.get(&id) {
            Some(DomNode::Element(e)) => Some(e),
            _ => None,
        }
    }

    /// Get a mutable reference to an element node.
    pub fn element_mut(&mut self, id: NodeId) -> Option<&mut DomElement> {
        match self.nodes.get_mut(&id) {
            Some(DomNode::Element(e)) => Some(e),
            _ => None,
        }
    }

    /// Append a child node to a parent element.
    pub fn append_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        // Remove from old parent if any
        if let Some(old_parent_id) = self.get(child_id).and_then(|n| n.parent_id()) {
            self.remove_child_from(old_parent_id, child_id);
        }

        // Set parent
        if let Some(child) = self.get_mut(child_id) {
            child.set_parent_id(Some(parent_id));
        }

        // Add to children list
        if let Some(parent) = self.element_mut(parent_id) {
            parent.children.push(child_id);
        }

        self.mark_dirty(parent_id);
    }

    /// Insert a child before another child.
    pub fn insert_before(
        &mut self,
        parent_id: NodeId,
        new_child_id: NodeId,
        before_child_id: NodeId,
    ) {
        // Remove from old parent if any
        if let Some(old_parent_id) = self.get(new_child_id).and_then(|n| n.parent_id()) {
            self.remove_child_from(old_parent_id, new_child_id);
        }

        // Set parent
        if let Some(child) = self.get_mut(new_child_id) {
            child.set_parent_id(Some(parent_id));
        }

        // Insert at correct position
        if let Some(parent) = self.element_mut(parent_id) {
            let idx = parent
                .children
                .iter()
                .position(|&id| id == before_child_id);
            match idx {
                Some(i) => parent.children.insert(i, new_child_id),
                None => parent.children.push(new_child_id),
            }
        }

        self.mark_dirty(parent_id);
    }

    /// Remove a child node from a parent.
    pub fn remove_child(&mut self, parent_id: NodeId, child_id: NodeId) {
        self.remove_child_from(parent_id, child_id);
        if let Some(child) = self.get_mut(child_id) {
            child.set_parent_id(None);
        }
        self.mark_dirty(parent_id);
    }

    /// Set an attribute on an element.
    pub fn set_attribute(&mut self, node_id: NodeId, key: &str, value: DomNodeAttribute) {
        if key == "children" {
            return; // React handles children via append/remove
        }
        if let Some(elem) = self.element_mut(node_id) {
            let existing = elem.attributes.get(key);
            if existing == Some(&value) {
                return;
            }
            elem.attributes.insert(key.into(), value);
            elem.dirty = true;
        }
        self.mark_dirty(node_id);
    }

    /// Set the layout style of an element.
    pub fn set_style(&mut self, node_id: NodeId, style: LayoutStyle) {
        if let Some(elem) = self.element_mut(node_id) {
            elem.layout_style = style;
            elem.dirty = true;
        }
    }

    /// Set text styles on an element.
    pub fn set_text_styles(&mut self, node_id: NodeId, styles: TextStyles) {
        if let Some(elem) = self.element_mut(node_id) {
            if elem.text_styles.as_ref() == Some(&styles) {
                return;
            }
            elem.text_styles = Some(styles);
            elem.dirty = true;
        }
    }

    /// Set the text value of a text node.
    pub fn set_text_value(&mut self, node_id: NodeId, value: &str) {
        if let Some(DomNode::Text(t)) = self.get_mut(node_id) {
            if t.value == value {
                return;
            }
            t.value = value.into();
        }
        // Mark parent dirty
        if let Some(parent_id) = self.get(node_id).and_then(|n| n.parent_id()) {
            self.mark_dirty(parent_id);
        }
    }

    /// Mark a node and all its ancestors as dirty.
    pub fn mark_dirty(&mut self, node_id: NodeId) {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if let Some(DomNode::Element(elem)) = self.nodes.get_mut(&id) {
                if elem.dirty {
                    break; // Already dirty up the chain
                }
                elem.dirty = true;
                current = elem.parent_id;
            } else {
                // Text node -- just go to parent
                current = self.nodes.get(&id).and_then(|n| n.parent_id());
            }
        }
    }

    /// Clear dirty flags on a subtree.
    pub fn clear_dirty(&mut self, node_id: NodeId) {
        if let Some(DomNode::Element(elem)) = self.nodes.get_mut(&node_id) {
            elem.dirty = false;
            let children = elem.children.clone();
            for child_id in children {
                self.clear_dirty(child_id);
            }
        }
    }

    // --- internal ---

    fn remove_child_from(&mut self, parent_id: NodeId, child_id: NodeId) {
        if let Some(parent) = self.element_mut(parent_id) {
            parent.children.retain(|&id| id != child_id);
        }
    }
}

impl Default for DomTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_element() {
        let mut tree = DomTree::new();
        let id = tree.create_element(ElementType::Box);
        assert!(tree.element(id).is_some());
        assert_eq!(tree.element(id).unwrap().element_type, ElementType::Box);
    }

    #[test]
    fn test_create_text_node() {
        let mut tree = DomTree::new();
        let id = tree.create_text_node("hello");
        match tree.get(id) {
            Some(DomNode::Text(t)) => assert_eq!(t.value, "hello"),
            _ => panic!("expected text node"),
        }
    }

    #[test]
    fn test_append_and_remove_child() {
        let mut tree = DomTree::new();
        let root = tree.create_element(ElementType::Root);
        let child = tree.create_element(ElementType::Box);

        tree.append_child(root, child);
        assert_eq!(tree.element(root).unwrap().children.len(), 1);

        tree.remove_child(root, child);
        assert!(tree.element(root).unwrap().children.is_empty());
    }

    #[test]
    fn test_insert_before() {
        let mut tree = DomTree::new();
        let root = tree.create_element(ElementType::Root);
        let a = tree.create_element(ElementType::Box);
        let b = tree.create_element(ElementType::Box);
        let c = tree.create_element(ElementType::Box);

        tree.append_child(root, a);
        tree.append_child(root, c);
        tree.insert_before(root, b, c);

        let children = &tree.element(root).unwrap().children;
        assert_eq!(children, &[a, b, c]);
    }

    #[test]
    fn test_mark_dirty_propagates() {
        let mut tree = DomTree::new();
        let root = tree.create_element(ElementType::Root);
        let child = tree.create_element(ElementType::Box);
        let grandchild = tree.create_element(ElementType::Text);

        tree.append_child(root, child);
        tree.append_child(child, grandchild);

        // Clear all dirty flags
        tree.clear_dirty(root);
        assert!(!tree.element(root).unwrap().dirty);

        // Mark grandchild dirty
        tree.mark_dirty(grandchild);
        assert!(tree.element(grandchild).unwrap().dirty);
        assert!(tree.element(child).unwrap().dirty);
        assert!(tree.element(root).unwrap().dirty);
    }

    #[test]
    fn test_set_attribute() {
        let mut tree = DomTree::new();
        let id = tree.create_element(ElementType::Box);
        tree.set_attribute(id, "width", DomNodeAttribute::Number(80.0));
        let attr = tree.element(id).unwrap().attributes.get("width");
        assert_eq!(attr, Some(&DomNodeAttribute::Number(80.0)));
    }

    #[test]
    fn test_set_text_value() {
        let mut tree = DomTree::new();
        let root = tree.create_element(ElementType::Root);
        let text = tree.create_text_node("hello");
        tree.append_child(root, text);

        tree.clear_dirty(root);
        tree.set_text_value(text, "world");

        match tree.get(text) {
            Some(DomNode::Text(t)) => assert_eq!(t.value, "world"),
            _ => panic!("expected text node"),
        }
        // Parent should be dirty
        assert!(tree.element(root).unwrap().dirty);
    }
}
