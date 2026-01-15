use std::collections::HashMap;
use crate::backend_api::{DocBackend, FrontendUpdate, Intent, Stroke};
use automerge::{AutoCommit, ReadDoc, transaction::Transactable, ObjType, Value, ScalarValue, ROOT, sync::{self, SyncDoc}};

/// Backend implementation using Automerge CRDT.
///
/// This backend manages the document state using Automerge, allowing for
/// conflict-free real-time collaboration. It handles document operations,
/// synchronization with peers, and persistence.
pub struct AutomergeBackend {
    /// The Automerge document instance.
    doc: AutoCommit,
    /// Map of sync states for each connected peer.
    sync_states: HashMap<String, sync::State>,
}

impl AutomergeBackend {
    /// Creates a new, empty AutomergeBackend.
    ///
    /// Initializes the document with a "strokes" list.
    pub fn new() -> Self {
        Self { 
            doc: AutoCommit::new(),
            sync_states: HashMap::new(),
        }
    }
}

/// Provides a default way to create a new instance of `AutomergeBackend` by calling its `new` method.
/// 
/// This implementation allows `AutomergeBackend` to be used with constructs that require the `Default` trait,
/// such as `Option::unwrap_or_default()` or `Vec::with_capacity()`.
impl Default for AutomergeBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Implementation of the `DocBackend` trait for `AutomergeBackend`.
///
/// This backend uses [Automerge](https://automerge.org/) for real-time collaborative editing.
/// Automerge is a CRDT (Conflict-free Replicated Data Type) library that enables multiple peers
/// to concurrently modify shared data structures and automatically resolve conflicts.
///
/// # Methods
///
/// - `apply_intent`: Applies a user intent (drawing a stroke or clearing the canvas) to the document.
///   - For `Intent::Draw`, serializes the stroke to JSON and inserts it into the "strokes" list.
///   - For `Intent::Clear`, removes all strokes from the "strokes" list.
///   - Ensures the "strokes" list exists, creating it if necessary.
///   - Returns a `FrontendUpdate` containing the current strokes.
///
/// - `get_strokes`: Retrieves all strokes from the document.
///   - Iterates over the "strokes" list, deserializing each JSON string into a `Stroke`.
///   - Returns a vector of strokes.
///
/// - `peer_connected` / `peer_disconnected`: Handles peer connection events.
///   - Maintains a sync state for each peer to track synchronization progress.
///
/// - `receive_sync_message`: Processes an incoming sync message from a peer.
///   - Decodes the message and applies it to the document using Automerge's sync protocol.
///   - Returns a `FrontendUpdate` with the latest strokes.
///
/// - `generate_sync_message`: Generates a sync message for a peer.
///   - Uses Automerge's sync protocol to create a message containing document changes.
///
/// - `save` / `load`: Serializes and deserializes the Automerge document for persistence.
///
/// - `set_background` / `get_background`: Stores and retrieves background image data as bytes.
///
/// # Automerge Notes
///
/// - Automerge automatically merges changes from multiple peers without conflicts.
/// - Data structures (like lists and maps) are identified by object IDs.
/// - Changes are propagated via sync messages, which are exchanged between peers.
/// - The backend maintains a sync state per peer to efficiently synchronize document changes.
///
/// # Error Handling
///
/// - Most operations use `unwrap` or `expect` for simplicity; production code should handle errors gracefully.
/// - If the "strokes" list is missing, it is recreated automatically.
///
/// # Usage
///
/// This backend is suitable for collaborative drawing applications where multiple users
/// can draw and erase strokes in real time, with changes seamlessly synchronized across peers.
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
        let sync_state = self.sync_states.entry(peer_id.to_string()).or_insert_with(sync::State::new);
        
        if let Ok(msg) = sync::Message::decode(&message) {
             self.doc.sync().receive_sync_message(sync_state, msg).ok();
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

    fn save(&mut self) -> Vec<u8> {
        self.doc.save()
    }

    fn load(&mut self, data: Vec<u8>) {
        if let Ok(doc) = AutoCommit::load(&data) {
            self.doc = doc;
            self.sync_states.clear();
        }
    }

    fn set_background(&mut self, data: Vec<u8>) {
        // Store as bytes
        self.doc.put(ROOT, "background", ScalarValue::Bytes(data)).ok();
    }

    fn get_background(&self) -> Option<Vec<u8>> {
        match self.doc.get(ROOT, "background") {
            Ok(Some((Value::Scalar(s), _))) => {
                if let ScalarValue::Bytes(b) = s.as_ref() {
                    Some(b.clone())
                } else {
                    None
                }
            }
             _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend_api::{Point, Stroke, Intent};

    fn create_test_stroke() -> Stroke {
        Stroke {
            points: vec![Point { x: 10, y: 10 }, Point { x: 20, y: 20 }],
            color: [255, 0, 0, 255],
            width: 5.0,
        }
    }

    #[test]
    fn test_new_backend_initialization() {
        let backend = AutomergeBackend::new();
        assert!(backend.get_strokes().is_empty());
    }

    #[test]
    fn test_apply_draw_intent() {
        let mut backend = AutomergeBackend::new();
        let stroke = create_test_stroke();
        
        backend.apply_intent(Intent::Draw(stroke.clone()));
        
        let strokes = backend.get_strokes();
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].width, 5.0);
        assert_eq!(strokes[0].points.len(), 2);
    }

    #[test]
    fn test_apply_clear_intent() {
        let mut backend = AutomergeBackend::new();
        let stroke = create_test_stroke();
        
        backend.apply_intent(Intent::Draw(stroke));
        assert!(!backend.get_strokes().is_empty());
        
        backend.apply_intent(Intent::Clear);
        assert!(backend.get_strokes().is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let mut backend1 = AutomergeBackend::new();
        let stroke = create_test_stroke();
        backend1.apply_intent(Intent::Draw(stroke.clone()));
        
        let data = backend1.save();
        
        let mut backend2 = AutomergeBackend::new();
        backend2.load(data);
        
        let strokes = backend2.get_strokes();
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].points, stroke.points);
    }

    #[test]
    fn test_sync_between_peers() {
        // Imitate two clients
        let mut client_a = AutomergeBackend::new();
        let mut client_b = AutomergeBackend::new();

        // A and B "connect" to each other (init sync states)
        client_a.peer_connected("client_b");
        client_b.peer_connected("client_a");

        // Client A draws something
        let stroke = create_test_stroke();
        client_a.apply_intent(Intent::Draw(stroke));

        // Generate sync message from A -> B
        // In Automerge, we might need multiple rounds, but for a single change, one might suffice or loop until None.
        
        let mut max_rounds = 10;
        let mut synced = false;

        while max_rounds > 0 {
            let msg_a_to_b = client_a.generate_sync_message("client_b");
            let msg_b_to_a = client_b.generate_sync_message("client_a");

            if msg_a_to_b.is_none() && msg_b_to_a.is_none() {
                synced = true;
                break;
            }

            if let Some(msg) = msg_a_to_b {
                client_b.receive_sync_message("client_a", msg);
            }

            if let Some(msg) = msg_b_to_a {
                client_a.receive_sync_message("client_b", msg);
            }
            
            max_rounds -= 1;
        }

        // B should now have the stroke drawn by A
        let strokes_b = client_b.get_strokes();
        assert_eq!(strokes_b.len(), 1, "Client B should have received the stroke from Client A");
    }
}
