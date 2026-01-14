//! Backend API - boundary between editor and CRDT logic.
//! 
//! Defines the core data structures (`Point`, `Stroke`, `Intent`, `FrontendUpdate`)
//! and the `DocBackend` trait which abstracts the document synchronization logic.
use serde::{Deserialize, Serialize};

/// Represents a 2D point with integer coordinates.
/// Used to define the path of a stroke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// X coordinate
    pub x: i32,
    /// Y coordinate
    pub y: i32,
}

/// Represents a drawing stroke consisting of a sequence of points, color, and width.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Points defining the stroke path.
    pub points: Vec<Point>,
    /// Color of the stroke in [R, G, B, A] format.
    pub color: [u8; 4],
    /// Width (thickness) of the stroke.
    pub width: f32,
}

/// Represents a user's intent to modify the document.
/// Passed from the UI to the backend.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    /// Intent to add a new stroke.
    Draw(Stroke),
    /// Intent to clear the document.
    Clear,
}

/// Represents an update to be applied to the frontend/UI.
/// Returned by the backend after processing an intent or receiving a sync message.
#[derive(Debug, Clone, PartialEq)]
pub struct FrontendUpdate {
    /// Current list of strokes to render.
    pub strokes: Vec<Stroke>,
}

impl FrontendUpdate {
    /// Creates an empty update with no strokes.
    pub fn empty() -> Self {
        Self {
            strokes: Vec::new(),
        }
    }
}

/// Trait for document backend management and synchronization.
///
/// Handles CRDT logic, persistence, and network synchronization messages.
/// Implementations must be `Send` to allow usage across threads.
pub trait DocBackend: Send {
    /// Applies a user intent to the document and returns a frontend update.
    ///
    /// # Arguments
    /// * `intent` - The user's intent (e.g., Draw or Clear).
    fn apply_intent(&mut self, intent: Intent) -> FrontendUpdate;

    /// Retrieves the current state of strokes from the backend.
    fn get_strokes(&self) -> Vec<Stroke>;
    
    // Sync methods

    /// Notification that a peer has connected.
    ///
    /// # Arguments
    /// * `peer_id` - Unique identifier of the connected peer.
    fn peer_connected(&mut self, peer_id: &str);

    /// Notification that a peer has disconnected.
    ///
    /// # Arguments
    /// * `peer_id` - Unique identifier of the disconnected peer.
    fn peer_disconnected(&mut self, peer_id: &str);

    /// Processes an incoming synchronization message from a peer.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the sender.
    /// * `message` - The raw byte data of the message.
    ///
    /// # Returns
    /// An update to reflect any changes in the document state.
    fn receive_sync_message(&mut self, peer_id: &str, message: Vec<u8>) -> FrontendUpdate;

    /// Generates a synchronization message to be sent to a specific peer.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the target peer.
    ///
    /// # Returns
    /// `Some(Vec<u8>)` if there is a message to send, or `None` otherwise.
    fn generate_sync_message(&mut self, peer_id: &str) -> Option<Vec<u8>>;

    // Persistence

    /// Serializes the entire document state to bytes for saving.
    fn save(&mut self) -> Vec<u8>;

    /// Loads the document state from serialized bytes.
    ///
    /// # Arguments
    /// * `data` - The byte data to load.
    fn load(&mut self, data: Vec<u8>);

    // Background

    /// Sets the background image data.
    ///
    /// # Arguments
    /// * `data` - Raw bytes of the background image (e.g., PNG/JPEG data).
    fn set_background(&mut self, data: Vec<u8>);

    /// Retrieves the current background image data.
    fn get_background(&self) -> Option<Vec<u8>>;
}