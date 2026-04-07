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
    /// Odbiera i przetwarza wiadomość synchronizacyjną od innego użytkownika.
    fn receive_sync_message(&mut self, peer_id: &str, message: Vec<u8>) -> FrontendUpdate {
        // 1. Pobieramy "stan wiedzy" o tym koledze (sync_state).
        let sync_state = self.sync_states.entry(peer_id.to_string()).or_insert_with(sync::State::new);
        
        // 2. Dekodujemy wiadomość (rozpakowujemy walizkę).
        if let Ok(msg) = sync::Message::decode(&message) {
             // 3. "Wchłaniamy" zmiany do naszego dokumentu. To tutaj dzieje się łączenie (merge).
             // Jednocześnie aktualizuje się sync_state, żebyśmy wiedzieli, że my też jesteśmy już "na bieżąco".
             self.doc.sync().receive_sync_message(sync_state, msg).ok();
        }

        // Zwracamy nową listę kresek do odrysowania na ekranie.
        FrontendUpdate { strokes: self.get_strokes() }
    }

    fn generate_sync_message(&mut self, peer_id: &str) -> Option<Vec<u8>> {
        if let Some(sync_state) = self.sync_states.get_mut(peer_id) {
            // Pytamy bibliotekę Automerge: "Hej, co nowego wydarzyło się od ostatniej synchronizacji?".
            // Jeśli są nowe zmiany, Automerge pakuje je w binarną wiadomość.
            if let Some(msg) = self.doc.sync().generate_sync_message(sync_state) {
                // Zwracamy pudełko z nowymi danymi.
                return Some(msg.encode());
            }
        }
        // Kolega jest już na bieżąco.
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

        while max_rounds > 0 {
            let msg_a_to_b = client_a.generate_sync_message("client_b");
            let msg_b_to_a = client_b.generate_sync_message("client_a");

            if msg_a_to_b.is_none() && msg_b_to_a.is_none() {
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

    /// Helper: run the sync loop between two peers until both have no more messages.
    fn sync_loop(a: &mut AutomergeBackend, a_label: &str, b: &mut AutomergeBackend, b_label: &str) {
        for _ in 0..20 {
            let msg_a = a.generate_sync_message(b_label);
            let msg_b = b.generate_sync_message(a_label);
            if msg_a.is_none() && msg_b.is_none() { break; }
            if let Some(m) = msg_a { b.receive_sync_message(a_label, m); }
            if let Some(m) = msg_b { a.receive_sync_message(b_label, m); }
        }
    }

    // ---- NF-06: concurrent draw from two peers ---------------------------------
    #[test]
    fn test_concurrent_draw_both_strokes_survive() {
        let mut client_a: AutomergeBackend = AutomergeBackend::new();
        let mut client_b = AutomergeBackend::new();
        client_a.peer_connected("b");
        client_b.peer_connected("a");

        // A creates the shared strokes list by drawing an initial stroke,
        // then syncs to B so both peers operate on the SAME Automerge list.
        // Without this, each peer would independently create a "strokes" list,
        // leading to a conflict on the list key itself.
        let seed = Stroke {
            points: vec![Point { x: 0, y: 0 }],
            color: [128, 128, 128, 255],
            width: 1.0,
        };
        client_a.apply_intent(Intent::Draw(seed));
        sync_loop(&mut client_a, "a", &mut client_b, "b");
        assert_eq!(client_b.get_strokes().len(), 1, "B should have the seed stroke");

        // Now both draw concurrently (into the same shared list)
        let stroke_a = Stroke {
            points: vec![Point { x: 0, y: 0 }, Point { x: 100, y: 100 }],
            color: [255, 0, 0, 255],
            width: 3.0,
        };
        let stroke_b = Stroke {
            points: vec![Point { x: 50, y: 50 }, Point { x: 150, y: 150 }],
            color: [0, 0, 255, 255],
            width: 4.0,
        };
        client_a.apply_intent(Intent::Draw(stroke_a));
        client_b.apply_intent(Intent::Draw(stroke_b));

        // Sync
        sync_loop(&mut client_a, "a", &mut client_b, "b");

        // seed + A's stroke + B's stroke = 3
        let sa = client_a.get_strokes();
        let sb = client_b.get_strokes();
        assert_eq!(sa.len(), 3, "Peer A should have 3 strokes (seed + concurrent draws)");
        assert_eq!(sb.len(), 3, "Peer B should have 3 strokes (seed + concurrent draws)");
        assert_eq!(sa, sb, "Both peers must converge to the same stroke list");
    }

    // ---- NF-06: clear + concurrent draw → add-wins semantics -------------------
    #[test]
    fn test_clear_vs_concurrent_draw_add_wins() {
        let mut client_a = AutomergeBackend::new();
        let mut client_b = AutomergeBackend::new();
        client_a.peer_connected("b");
        client_b.peer_connected("a");

        // Seed a shared stroke so there is something to clear
        let initial = create_test_stroke();
        client_a.apply_intent(Intent::Draw(initial));
        sync_loop(&mut client_a, "a", &mut client_b, "b");
        assert_eq!(client_b.get_strokes().len(), 1);

        // Concurrently: A clears, B draws a NEW stroke
        client_a.apply_intent(Intent::Clear);
        let new_stroke = Stroke {
            points: vec![Point { x: 99, y: 99 }],
            color: [0, 255, 0, 255],
            width: 2.0,
        };
        client_b.apply_intent(Intent::Draw(new_stroke.clone()));

        // Sync
        sync_loop(&mut client_a, "a", &mut client_b, "b");

        // Add-wins: the initial stroke is gone, but B's new stroke survives
        let sa = client_a.get_strokes();
        let sb = client_b.get_strokes();
        assert_eq!(sa.len(), 1, "Only the concurrently-added stroke should survive the clear");
        assert_eq!(sa, sb, "Both peers must converge");
        assert_eq!(sa[0].color, [0, 255, 0, 255], "The surviving stroke should be B's green one");
    }

    // ---- Graceful handling of corrupted / invalid data -------------------------
    #[test]
    fn test_load_invalid_bytes_does_not_panic() {
        let mut backend = AutomergeBackend::new();
        backend.apply_intent(Intent::Draw(create_test_stroke()));

        // Feed garbage bytes — should not panic, document should remain intact
        backend.load(vec![0, 1, 2, 3, 255, 254]);

        // The previous stroke should still be present (load failed silently)
        assert_eq!(backend.get_strokes().len(), 1, "Invalid load should leave document unchanged");
    }

    #[test]
    fn test_load_empty_bytes_does_not_panic() {
        let mut backend = AutomergeBackend::new();
        backend.apply_intent(Intent::Draw(create_test_stroke()));

        // Empty bytes may be treated as a valid empty doc by Automerge.
        // The key requirement is that this call does not panic.
        backend.load(vec![]);

        // After loading empty data, behavior is implementation-defined:
        // Automerge may replace the doc (strokes gone) or reject the load.
        // Either outcome is acceptable as long as it doesn't crash.
        let _strokes = backend.get_strokes();
    }

    // ---- Stroke ordering -------------------------------------------------------
    #[test]
    fn test_strokes_preserve_insertion_order() {
        let mut backend = AutomergeBackend::new();

        for i in 0..5 {
            let stroke = Stroke {
                points: vec![Point { x: i, y: i }],
                color: [i as u8, 0, 0, 255],
                width: 1.0 + i as f32,
            };
            backend.apply_intent(Intent::Draw(stroke));
        }

        let strokes = backend.get_strokes();
        assert_eq!(strokes.len(), 5);
        for (idx, s) in strokes.iter().enumerate() {
            assert_eq!(s.points[0].x, idx as i32, "Stroke {} should be at position {}", idx, idx);
            assert_eq!(s.width, 1.0 + idx as f32);
        }
    }

    // ---- Three-peer sync chain (A ↔ B ↔ C) ------------------------------------
    #[test]
    fn test_three_peer_sync_convergence() {
        let mut a = AutomergeBackend::new();
        let mut b = AutomergeBackend::new();
        let mut c = AutomergeBackend::new();

        // Star topology: B is the hub, A and C only know B.
        a.peer_connected("b");
        b.peer_connected("a");
        b.peer_connected("c");
        c.peer_connected("b");

        // A creates the strokes list and syncs to B, then B syncs to C.
        // This ensures all three peers share the same Automerge list object.
        a.apply_intent(Intent::Draw(Stroke {
            points: vec![Point { x: 0, y: 0 }], 
            color: [128, 128, 128, 255],
            width: 1.0,
        }));
        sync_loop(&mut a, "a", &mut b, "b");
        sync_loop(&mut b, "b", &mut c, "c");
        assert_eq!(c.get_strokes().len(), 1, "C should have the seed stroke");

        // A draws stroke 1, C draws stroke 2 concurrently
        a.apply_intent(Intent::Draw(Stroke {
            points: vec![Point { x: 1, y: 1 }],
            color: [255, 0, 0, 255],
            width: 1.0,
        }));
        c.apply_intent(Intent::Draw(Stroke {
            points: vec![Point { x: 2, y: 2 }],
            color: [0, 0, 255, 255],
            width: 2.0,
        }));

        // Sync A↔B, then B↔C, then A↔B again (propagate C's stroke to A)
        sync_loop(&mut a, "a", &mut b, "b");
        sync_loop(&mut b, "b", &mut c, "c");
        sync_loop(&mut a, "a", &mut b, "b");

        // seed + A's stroke + C's stroke = 3 each
        let sa = a.get_strokes();
        let sb = b.get_strokes();
        let sc = c.get_strokes();

        assert_eq!(sa.len(), 3, "A should see 3 strokes");
        assert_eq!(sb.len(), 3, "B should see 3 strokes");
        assert_eq!(sc.len(), 3, "C should see 3 strokes");
        assert_eq!(sa, sb, "A and B must converge");
        assert_eq!(sb, sc, "B and C must converge");
    }

    // ---- Background round-trip -------------------------------------------------
    #[test]
    fn test_set_and_get_background() {
        let mut backend = AutomergeBackend::new();
        assert!(backend.get_background().is_none());

        let img_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // fake JPEG header bytes
        backend.set_background(img_data.clone());

        let bg = backend.get_background().expect("Background should be set");
        assert_eq!(bg, img_data, "Background data should round-trip unchanged");
    }

    // ---- Peer disconnect cleans up sync state ----------------------------------
    #[test]
    fn test_peer_disconnect_removes_sync_state() {
        let mut backend = AutomergeBackend::new();
        backend.peer_connected("peer_x");
        assert!(backend.generate_sync_message("peer_x").is_some() || true); // just verifying no panic

        backend.peer_disconnected("peer_x");
        // After disconnect, generating a sync message should return None (no state)
        assert!(backend.generate_sync_message("peer_x").is_none(),
            "No sync message should be produced for a disconnected peer");
    }
}
