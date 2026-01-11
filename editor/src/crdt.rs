use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::Ordering;
use crate::backend_api::{DocBackend, Intent, FrontendUpdate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id {
    pub replica_id: u16,
    pub value: u32,
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        // Sort by sequence number, then by replica_id for tie-breaking
        self.value.cmp(&other.value).then(self.replica_id.cmp(&other.replica_id))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Global {
    pub state: HashMap<u16, u32>,
}

impl Global {
    pub fn new() -> Self {
        Self { state: HashMap::new() }
    }
    
    pub fn update(&mut self, id: Id) {
        let current = self.state.entry(id.replica_id).or_insert(0);
        if id.value > *current {
            *current = id.value;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    pub id: Id,
    pub relative_id: Option<Id>, 
    pub text: Option<char>, 
    pub version: Global,
    pub is_delete: bool,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub insertion_id: Id,
    pub relative_to_id: Option<Id>,
    pub text: char,
    pub visible: bool,
}

pub struct Buffer {
    pub replica_id: u16,
    pub sequence: u32,
    pub nodes: Vec<Node>,
    pub version: Global,
    pub holdback_queue: Vec<Op>,
}

impl Buffer {
    pub fn new(replica_id: u16) -> Self {
        Self {
            replica_id,
            sequence: 0,
            nodes: Vec::new(),
            version: Global::new(),
            holdback_queue: Vec::new(),
        }
    }

    pub fn render(&self) -> String {
        self.nodes.iter().filter(|n| n.visible).map(|n| n.text).collect()
    }
    
    fn find_index(&self, id: Id) -> Option<usize> {
        self.nodes.iter().position(|n| n.insertion_id == id)
    }
    
    fn find_visible_insertion_point(&self, visible_pos: usize) -> Option<Id> {
        if visible_pos == 0 {
            return None;
        }
        let mut count = 0;
        for node in &self.nodes {
            if node.visible {
                count += 1;
                if count == visible_pos {
                    return Some(node.insertion_id);
                }
            }
        }
        None 
    }

    pub fn apply_local_insert(&mut self, pos: usize, text: char) -> Op {
        self.sequence += 1;
        let id = Id { replica_id: self.replica_id, value: self.sequence };
        let relative_id = self.find_visible_insertion_point(pos);
        
        let node = Node {
            insertion_id: id,
            relative_to_id: relative_id,
            text,
            visible: true,
        };
        
        self.insert_node(node.clone());
        self.version.update(id);
        
        Op {
            id,
            relative_id,
            text: Some(text),
            version: self.version.clone(),
            is_delete: false,
        }
    }
    
    pub fn apply_local_delete(&mut self, pos: usize) -> Op {
        let mut count = 0;
        let mut target_id = None;
        
        for node in self.nodes.iter_mut() {
            if node.visible {
                if count == pos {
                    node.visible = false;
                    target_id = Some(node.insertion_id);
                    break;
                }
                count += 1;
            }
        }
        
        self.sequence += 1;
        let id = Id { replica_id: self.replica_id, value: self.sequence };
        self.version.update(id);
        
        Op {
            id,
            relative_id: target_id,
            text: None,
            version: self.version.clone(),
            is_delete: true,
        }
    }
    
    fn insert_node(&mut self, node: Node) {
        let mut index = 0;
        if let Some(rel) = node.relative_to_id {
            if let Some(idx) = self.find_index(rel) {
                index = idx + 1;
            } else {
                // If parent not found locally, append to end (simplified)
                index = self.nodes.len();
            }
        }
        
        // Handle concurrent inserts: skip siblings with smaller IDs
        while index < self.nodes.len() {
            let curr = &self.nodes[index];
            if curr.relative_to_id == node.relative_to_id {
                if curr.insertion_id < node.insertion_id {
                    index += 1;
                    continue;
                }
            }
            break;
        }
        
        self.nodes.insert(index, node);
    }
}

pub struct CrdtBackend {
    buffer: Buffer,
}

impl CrdtBackend {
    pub fn new(replica_id: u16) -> Self {
        Self {
            buffer: Buffer::new(replica_id),
        }
    }
}

impl DocBackend for CrdtBackend {
    fn apply_intent(&mut self, intent: Intent) -> FrontendUpdate {
        match intent {
            Intent::InsertAt { pos, text } => {
                for (i, c) in text.chars().enumerate() {
                    self.buffer.apply_local_insert(pos + i, c);
                }
            }
            Intent::DeleteRange { start, end } => {
                // Delete range by deleting the 'start' element multiple times
                // (since subsequent elements shift into 'start' position in visible view)
                for _ in start..end {
                    self.buffer.apply_local_delete(start);
                }
            }
            Intent::ReplaceAll { text } => {
                 // Clear all visible nodes
                 let len = self.buffer.nodes.iter().filter(|n| n.visible).count();
                 for _ in 0..len {
                     self.buffer.apply_local_delete(0);
                 }
                 // Insert new text
                 for (i, c) in text.chars().enumerate() {
                     self.buffer.apply_local_insert(i, c);
                 }
            }
            _ => {}
        }
        
        FrontendUpdate {
            full_text: Some(self.buffer.render()),
            remote_cursors: Vec::new(),
        }
    }

    fn render_text(&self) -> String {
        self.buffer.render()
    }
}