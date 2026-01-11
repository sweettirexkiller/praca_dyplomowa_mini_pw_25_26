use std::collections::HashMap;
use crate::backend_api::{DocBackend, FrontendUpdate, Intent, Stroke};
use automerge::{AutoCommit, ReadDoc, transaction::Transactable, ObjType, Value, ScalarValue, ROOT, sync::{self, SyncDoc}};

pub struct AutomergeBackend {
    doc: AutoCommit,
    sync_states: HashMap<String, sync::State>,
}

impl AutomergeBackend {
    pub fn new() -> Self {
        let mut doc = AutoCommit::new();
        // Ensure "strokes" list exists
        if doc.get(ROOT, "strokes").unwrap().is_none() {
             doc.put_object(ROOT, "strokes", ObjType::List).ok();
        }
        Self { 
            doc,
            sync_states: HashMap::new(),
        }
    }
}

impl Default for AutomergeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DocBackend for AutomergeBackend {
    fn apply_intent(&mut self, intent: Intent) -> FrontendUpdate {
        match intent {
            Intent::Draw(stroke) => {
                let json = serde_json::to_string(&stroke).unwrap();
                let list_id = match self.doc.get(ROOT, "strokes") {
                    Ok(Some((Value::Object(ObjType::List), id))) => id,
                     _ => {
                        // Recreate if missing
                        self.doc.put_object(ROOT, "strokes", ObjType::List).unwrap()
                    }
                };
                
                let len = self.doc.length(&list_id);
                // insert expects item: impl Into<ScalarValue>
                // String implements Into<ScalarValue>
                self.doc.insert(&list_id, len, ScalarValue::Str(json.into())).expect("Failed to insert stroke");
            }
            Intent::Clear => {
                 let list_id = match self.doc.get(ROOT, "strokes") {
                    Ok(Some((Value::Object(ObjType::List), id))) => id,
                    _ => return FrontendUpdate::empty(),
                };
                let len = self.doc.length(&list_id);
                if len > 0 {
                    self.doc.splice(&list_id, 0, len as isize, std::iter::empty::<ScalarValue>()).expect("Failed to clear");
                }
            }
        }

        FrontendUpdate {
            strokes: self.get_strokes(),
        }
    }

    fn get_strokes(&self) -> Vec<Stroke> {
         let list_id = match self.doc.get(ROOT, "strokes") {
             Ok(Some((Value::Object(ObjType::List), id))) => id,
             _ => return Vec::new(),
         };
         
         let mut strokes = Vec::new();
         for (val, _) in self.doc.values(&list_id) {
             if let Value::Scalar(scalar) = val {
                 if let ScalarValue::Str(s) = scalar.as_ref() {
                     if let Ok(stroke) = serde_json::from_str(&s) {
                         strokes.push(stroke);
                     }
                 }
             }
         }
         strokes
    }

    fn peer_connected(&mut self, peer_id: &str) {
        println!("Peer connected: {}", peer_id);
        self.sync_states.insert(peer_id.to_string(), sync::State::new());
    }

    fn peer_disconnected(&mut self, peer_id: &str) {
        println!("Peer disconnected: {}", peer_id);
        self.sync_states.remove(peer_id);
    }

    fn receive_sync_message(&mut self, peer_id: &str, message: Vec<u8>) -> FrontendUpdate {
        if let Some(sync_state) = self.sync_states.get_mut(peer_id) {
            if let Ok(msg) = sync::Message::decode(&message) {
                 self.doc.sync().receive_sync_message(sync_state, msg).ok();
            }
        }
        FrontendUpdate { strokes: self.get_strokes() }
    }

    fn generate_sync_message(&mut self, peer_id: &str) -> Option<Vec<u8>> {
        if let Some(sync_state) = self.sync_states.get_mut(peer_id) {
            if let Some(msg) = self.doc.sync().generate_sync_message(sync_state) {
                return Some(msg.encode());
            }
        }
        None
    }
}
